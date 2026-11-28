mod django;
mod fastapi;
mod flask;

use std::collections::BTreeSet;

use codeint_model::RepoPath;
use codeint_model::language::{FrameworkProfileResult, FrameworkProfileSelection, Manifest};
use toml::Value;

use crate::dependencies::{
    extract_pipfile_runtime_deps, extract_pyproject_runtime_deps, extract_requirements_deps,
    extract_setup_cfg_runtime_deps, is_dev_requirements_file, is_requirements_file,
};
use crate::entry_points::module_spec_to_rel_path;

pub(crate) const KNOWN_PROFILES: [&str; 3] = ["django", "fastapi", "flask"];

const PROFILES: [&PythonProfile; 3] = [&django::PROFILE, &fastapi::PROFILE, &flask::PROFILE];

const IGNORED_DIRS: [&str; 6] = [
    ".venv",
    "venv",
    "vendor",
    "third_party",
    "node_modules",
    "__pycache__",
];

pub(crate) struct PythonProfile {
    pub(crate) name: &'static str,
    pub(crate) detect: fn(&PythonPackage<'_>) -> bool,
    pub(crate) entry_points: fn(&PythonPackage<'_>) -> Vec<String>,
    pub(crate) generated_paths: fn(&PythonPackage<'_>) -> Vec<String>,
    pub(crate) fallback_roots: fn(&[RepoPath], &[Manifest]) -> Vec<String>,
}

pub(crate) struct PythonPackage<'a> {
    root: &'a str,
    files: &'a [RepoPath],
    runtime_deps: BTreeSet<String>,
    pyproject: Option<Value>,
}

impl<'a> PythonPackage<'a> {
    fn new(root: &'a str, files: &'a [RepoPath], manifests: &[Manifest]) -> Self {
        let mut runtime_deps = BTreeSet::new();
        let mut pyproject = None;
        for manifest in manifests {
            let Some(rel) = strip_package_root(root, &manifest.path) else {
                continue;
            };
            if rel.contains('/') || has_ignored_dir(&manifest.path) {
                continue;
            }
            if rel == "pyproject.toml" {
                if let Ok(doc) = toml::from_str::<Value>(&manifest.text) {
                    runtime_deps.extend(extract_pyproject_runtime_deps(&doc));
                    pyproject = Some(doc);
                }
            } else if is_requirements_file(rel) && !is_dev_requirements_file(rel) {
                runtime_deps.extend(extract_requirements_deps(&manifest.text));
            } else if rel == "setup.cfg" {
                runtime_deps.extend(extract_setup_cfg_runtime_deps(&manifest.text));
            } else if rel == "Pipfile" {
                runtime_deps.extend(extract_pipfile_runtime_deps(&manifest.text));
            }
        }
        Self {
            root,
            files,
            runtime_deps,
            pyproject,
        }
    }

    pub(crate) fn rel_path<'p>(&self, full: &'p str) -> Option<&'p str> {
        let rel = strip_package_root(self.root, full)?;
        (!has_ignored_dir(rel)).then_some(rel)
    }

    pub(crate) fn has_dep(&self, normalized: &str) -> bool {
        self.runtime_deps.contains(normalized)
    }

    pub(crate) fn has_any_rel(&self, targets: &[&str]) -> bool {
        self.files
            .iter()
            .filter_map(|full| self.rel_path(full.as_str()))
            .any(|rel| targets.contains(&rel))
    }

    pub(crate) fn matching_py_files(&self, predicate: impl Fn(&str) -> bool) -> Vec<String> {
        self.files
            .iter()
            .map(RepoPath::as_str)
            .filter(|full| {
                self.rel_path(full)
                    .is_some_and(|rel| rel.ends_with(".py") && predicate(rel))
            })
            .map(str::to_owned)
            .collect()
    }

    pub(crate) fn has_non_test_py(&self, predicate: impl Fn(&str) -> bool) -> bool {
        self.files.iter().any(|full| {
            self.rel_path(full.as_str())
                .is_some_and(|rel| rel.ends_with(".py") && !is_test_dir_rel(rel) && predicate(rel))
        })
    }

    pub(crate) fn pyproject(&self) -> Option<&Value> {
        self.pyproject.as_ref()
    }

    pub(crate) fn resolve_module_file(&self, module: &str) -> Option<String> {
        let base = module_spec_to_rel_path(module)?;
        let candidates = [
            format!("{base}.py"),
            format!("src/{base}.py"),
            format!("{base}/__init__.py"),
            format!("src/{base}/__init__.py"),
        ];
        candidates.iter().find_map(|candidate| {
            self.files
                .iter()
                .map(RepoPath::as_str)
                .find(|full| self.rel_path(full) == Some(candidate.as_str()))
                .map(str::to_owned)
        })
    }
}

pub(crate) fn has_ignored_dir(path: &str) -> bool {
    path.split('/').any(|seg| IGNORED_DIRS.contains(&seg))
}

pub(crate) fn is_test_dir_rel(rel: &str) -> bool {
    let Some((dir, _)) = rel.rsplit_once('/') else {
        return false;
    };
    dir.split('/').any(|seg| matches!(seg, "test" | "tests"))
}

pub(crate) fn split_parent_and_name(rel: &str) -> (&str, &str) {
    rel.rsplit_once('/').unwrap_or(("", rel))
}

pub(crate) fn direct_child_of_folder<'a>(rel: &'a str, folder: &str) -> Option<&'a str> {
    let (parent, name) = rel.rsplit_once('/')?;
    let immediate = parent.rsplit_once('/').map_or(parent, |(_, last)| last);
    (immediate == folder).then_some(name)
}

pub(crate) fn direct_child_of_nested_folder<'a>(
    rel: &'a str,
    outer: &str,
    inner: &str,
) -> Option<&'a str> {
    let (parent, name) = rel.rsplit_once('/')?;
    let (grandparent, immediate) = parent.rsplit_once('/')?;
    if immediate != inner {
        return None;
    }
    let outer_seg = grandparent
        .rsplit_once('/')
        .map_or(grandparent, |(_, last)| last);
    (outer_seg == outer).then_some(name)
}

pub(crate) fn empty_generated(_pkg: &PythonPackage<'_>) -> Vec<String> {
    Vec::new()
}

pub(crate) fn fallback_roots_for_filenames(
    repository_paths: &[RepoPath],
    file_names: &[&str],
) -> Vec<String> {
    repository_paths
        .iter()
        .map(RepoPath::as_str)
        .filter(|path| !has_ignored_dir(path) && !is_test_dir_rel(path))
        .filter_map(|path| {
            let (parent, name) = split_parent_and_name(path);
            (!parent.is_empty() && file_names.contains(&name)).then(|| parent.to_owned())
        })
        .collect()
}

pub(crate) fn known_profiles() -> Vec<String> {
    KNOWN_PROFILES
        .iter()
        .map(|name| (*name).to_owned())
        .collect()
}

pub(crate) fn resolve_framework_profiles(
    package_root: &str,
    files: &[RepoPath],
    manifests: &[Manifest],
    selection: FrameworkProfileSelection,
) -> Result<FrameworkProfileResult, String> {
    if let FrameworkProfileSelection::Only(ref only) = selection
        && let Some(unknown) = only.iter().find(|n| !KNOWN_PROFILES.contains(&n.as_str()))
    {
        return Err(format!("unknown Framework profile {unknown:?}"));
    }
    let pkg = PythonPackage::new(package_root, files, manifests);
    let mut active_profiles = BTreeSet::new();
    let mut entry_points = BTreeSet::new();
    let mut generated_paths = BTreeSet::new();
    for profile in PROFILES {
        if profile_matches(&selection, profile, &pkg) {
            active_profiles.insert(profile.name.to_owned());
            entry_points.extend((profile.entry_points)(&pkg));
            generated_paths.extend((profile.generated_paths)(&pkg));
        }
    }
    Ok(FrameworkProfileResult {
        active_profiles: active_profiles.into_iter().collect(),
        entry_points: entry_points.into_iter().collect(),
        generated_paths: generated_paths.into_iter().collect(),
        reference_ranges: Vec::new(),
        reference_range_note: None,
    })
}

fn profile_matches(
    selection: &FrameworkProfileSelection,
    profile: &PythonProfile,
    pkg: &PythonPackage<'_>,
) -> bool {
    match selection {
        FrameworkProfileSelection::Disabled => false,
        FrameworkProfileSelection::Auto => (profile.detect)(pkg),
        FrameworkProfileSelection::Exclude(excluded) => {
            !excluded.iter().any(|item| item == profile.name) && (profile.detect)(pkg)
        }
        FrameworkProfileSelection::Only(only) => only.iter().any(|item| item == profile.name),
    }
}

pub(crate) fn fallback_import_roots(
    active_profiles: &[String],
    repository_paths: &[RepoPath],
    manifests: &[Manifest],
) -> Vec<String> {
    let mut roots = Vec::new();
    for profile in PROFILES {
        if active_profiles.iter().any(|name| name == profile.name) {
            for root in (profile.fallback_roots)(repository_paths, manifests) {
                if !roots.contains(&root) {
                    roots.push(root);
                }
            }
        }
    }
    roots
}

fn strip_package_root<'p>(root: &str, full: &'p str) -> Option<&'p str> {
    if root.is_empty() {
        Some(full)
    } else {
        full.strip_prefix(root)?.strip_prefix('/')
    }
}
