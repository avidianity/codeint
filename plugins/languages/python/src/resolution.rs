use std::collections::{HashMap, HashSet};

use codeint_model::language::{ImportRequest, Manifest, Resolution};
use codeint_model::scan::find_manifests;

pub(crate) fn find_pyprojects(repository_paths: &[String]) -> Vec<String> {
    find_manifests(repository_paths, "pyproject.toml")
}

pub(crate) fn resolve_all(
    repository_paths: &[String],
    manifests: &[Manifest],
    requests: &[ImportRequest],
) -> Vec<Resolution> {
    let py_files: HashSet<String> = repository_paths
        .iter()
        .filter(|path| path.ends_with(".py"))
        .cloned()
        .collect();
    let module_map = build_module_map(&py_files, manifests);
    requests
        .iter()
        .map(|req| resolve_one(&py_files, &module_map, &req.importer, &req.specifier))
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
    let mut sorted_files: Vec<&String> = py_files.iter().collect();
    sorted_files.sort();
    let mut map = HashMap::new();
    for root in &roots {
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
    importer: &str,
    specifier: &str,
) -> Resolution {
    if specifier.starts_with('.') {
        return resolve_relative(py_files, importer, specifier);
    }
    if let Some(target) = module_map.get(specifier) {
        if target == importer {
            return Resolution::External(String::new());
        }
        return Resolution::Resolved(target.clone());
    }
    Resolution::External(String::new())
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
    if !remainder.is_empty() {
        segments.extend(remainder.split('.').filter(|s| !s.is_empty()));
    }
    let base = segments.join("/");
    let candidates = if base.is_empty() {
        vec!["__init__.py".to_owned()]
    } else {
        vec![format!("{base}.py"), format!("{base}/__init__.py")]
    };
    for candidate in candidates {
        if py_files.contains(&candidate) {
            if candidate == importer {
                return Resolution::External(String::new());
            }
            return Resolution::Resolved(candidate);
        }
    }
    Resolution::Unresolved(format!(
        "no Python module in the repository matches {specifier:?}"
    ))
}
