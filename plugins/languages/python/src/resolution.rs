use std::collections::{HashMap, HashSet};

use codeint_model::RepoPath;
use codeint_model::language::{ImportRequest, Manifest, Resolution};

use crate::dependencies::{is_requirements_file, normalize_pep503};
use crate::frameworks::{fallback_import_roots, has_ignored_dir};
use crate::stdlib::is_stdlib_module;

#[cfg(test)]
mod tests;

pub(crate) fn find_python_manifests<P: AsRef<str>>(repository_paths: &[P]) -> Vec<String> {
    repository_paths
        .iter()
        .map(AsRef::as_ref)
        .filter(|path| {
            if has_ignored_dir(path) {
                return false;
            }
            let file_name = path.rsplit_once('/').map_or(*path, |(_, name)| name);
            file_name == "pyproject.toml"
                || file_name == "setup.cfg"
                || file_name == "Pipfile"
                || is_requirements_file(file_name)
        })
        .map(str::to_owned)
        .collect()
}

#[cfg(test)]
pub(crate) fn resolve_all(
    repository_paths: &[String],
    manifests: &[Manifest],
    requests: &[ImportRequest],
) -> Vec<Resolution> {
    let paths: Vec<RepoPath> = repository_paths
        .iter()
        .map(|path| RepoPath::new(path).expect("valid test repo path"))
        .collect();
    resolve_with_profiles(&paths, manifests, requests, &[])
}

pub(crate) fn resolve_with_profiles(
    repository_paths: &[RepoPath],
    manifests: &[Manifest],
    requests: &[ImportRequest],
    active_profiles: &[String],
) -> Vec<Resolution> {
    let py_files: HashSet<String> = repository_paths
        .iter()
        .map(RepoPath::as_str)
        .filter(|path| path.ends_with(".py"))
        .map(str::to_owned)
        .collect();
    let module_map = build_module_map(&py_files, manifests);
    let fallback_map = if active_profiles.is_empty() {
        HashMap::new()
    } else {
        let extra_roots = fallback_import_roots(active_profiles, repository_paths, manifests);
        build_module_map_for_roots(&py_files, &extra_roots)
    };
    requests
        .iter()
        .map(|req| {
            resolve_one(
                &py_files,
                &module_map,
                &fallback_map,
                &req.importer,
                &req.specifier,
            )
        })
        .collect()
}

fn build_module_map(py_files: &HashSet<String>, manifests: &[Manifest]) -> HashMap<String, String> {
    let mut roots = vec![String::new(), "src".to_owned()];
    for manifest in manifests {
        if let Some(dir) = manifest.path.strip_suffix("/pyproject.toml") {
            roots.push(dir.to_owned());
            roots.push(format!("{dir}/src"));
        }
    }
    build_module_map_for_roots(py_files, &roots)
}

fn build_module_map_for_roots(
    py_files: &HashSet<String>,
    roots: &[String],
) -> HashMap<String, String> {
    let mut sorted_files: Vec<&String> = py_files.iter().collect();
    sorted_files.sort();
    let mut map = HashMap::new();
    for root in roots {
        let prefix = if root.is_empty() {
            String::new()
        } else {
            format!("{root}/")
        };
        for file in &sorted_files {
            let Some(rel) = file.strip_prefix(&prefix) else {
                continue;
            };
            if let Some(module) = path_to_module(rel) {
                map.entry(module).or_insert_with(|| (*file).clone());
            }
        }
    }
    map
}

fn path_to_module(rel: &str) -> Option<String> {
    if rel == "__init__.py" {
        return None;
    }
    if let Some(pkg) = rel.strip_suffix("/__init__.py") {
        return Some(pkg.replace('/', "."));
    }
    rel.strip_suffix(".py").map(|stem| stem.replace('/', "."))
}

fn resolve_one(
    py_files: &HashSet<String>,
    module_map: &HashMap<String, String>,
    fallback_map: &HashMap<String, String>,
    importer: &str,
    specifier: &str,
) -> Resolution {
    if specifier.starts_with('.') {
        return resolve_relative(py_files, importer, specifier);
    }
    if let Some(target) = module_map
        .get(specifier)
        .or_else(|| fallback_map.get(specifier))
    {
        if target == importer {
            return Resolution::Builtin;
        }
        return Resolution::Resolved(vec![target.clone()]);
    }
    let top = specifier.split('.').next().unwrap_or("");
    if top.is_empty() {
        return Resolution::Unresolved(format!("empty Python import specifier {specifier:?}"));
    }
    if is_stdlib_module(top) {
        return Resolution::Builtin;
    }
    Resolution::External(normalize_pep503(top))
}

fn resolve_relative(py_files: &HashSet<String>, importer: &str, specifier: &str) -> Resolution {
    let dots = specifier.bytes().take_while(|&b| b == b'.').count();
    let remainder = &specifier[dots..];
    let parent = importer.rsplit_once('/').map_or("", |(dir, _)| dir);
    let mut segments: Vec<&str> = if parent.is_empty() {
        Vec::new()
    } else {
        parent.split('/').collect()
    };
    let climbs = dots.saturating_sub(1);
    if climbs > segments.len() {
        return Resolution::Unresolved(format!("{specifier:?} climbs outside the repository"));
    }
    segments.truncate(segments.len() - climbs);
    let base_dir = segments.join("/");
    let remainder_segments: Vec<&str> = remainder.split('.').filter(|s| !s.is_empty()).collect();
    segments.extend(remainder_segments.iter().copied());
    find_relative_target(
        py_files,
        importer,
        specifier,
        &base_dir,
        &segments.join("/"),
        remainder_segments.len() == 1,
    )
}

fn find_relative_target(
    py_files: &HashSet<String>,
    importer: &str,
    specifier: &str,
    base_dir: &str,
    target_base: &str,
    allow_init_fallback: bool,
) -> Resolution {
    let candidates = if target_base.is_empty() {
        vec!["__init__.py".to_owned()]
    } else {
        vec![
            format!("{target_base}.py"),
            format!("{target_base}/__init__.py"),
        ]
    };
    for candidate in candidates {
        if py_files.contains(&candidate) {
            return if candidate == importer {
                Resolution::Builtin
            } else {
                Resolution::Resolved(vec![candidate])
            };
        }
    }
    if allow_init_fallback {
        let init_candidate = init_path_for(base_dir);
        if init_candidate != importer && py_files.contains(&init_candidate) {
            return Resolution::Resolved(vec![init_candidate]);
        }
    }
    Resolution::Unresolved(format!(
        "no Python module in the repository matches {specifier:?}"
    ))
}

fn init_path_for(dir: &str) -> String {
    if dir.is_empty() {
        "__init__.py".to_owned()
    } else {
        format!("{dir}/__init__.py")
    }
}
