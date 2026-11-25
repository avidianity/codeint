use std::collections::{BTreeMap, BTreeSet, HashSet};

use codeint_model::language::Manifest;
use codeint_model::scan::normalize_relative_path;
use serde::Deserialize;

use crate::workspaces::PUBSPEC_MANIFEST;

#[derive(Deserialize)]
struct PubspecEntryPoints {
    executables: Option<BTreeMap<String, Option<String>>>,
}

pub(crate) fn discover(repository_paths: &[String], manifests: &[Manifest]) -> Vec<String> {
    let dart_files: HashSet<&str> = repository_paths
        .iter()
        .map(String::as_str)
        .filter(|path| path.ends_with(".dart"))
        .collect();
    let suffix = format!("/{PUBSPEC_MANIFEST}");
    let mut results = BTreeSet::new();
    for manifest in manifests {
        let base_dir = if manifest.path == PUBSPEC_MANIFEST {
            ""
        } else if let Some(dir) = manifest.path.strip_suffix(&suffix) {
            dir
        } else {
            continue;
        };
        if let Ok(Some(spec)) = serde_saphyr::from_str::<Option<PubspecEntryPoints>>(&manifest.text)
        {
            spec.collect_entries(base_dir, repository_paths, &dart_files, &mut results);
        }
    }
    results.into_iter().collect()
}

impl PubspecEntryPoints {
    fn collect_entries(
        &self,
        base_dir: &str,
        repository_paths: &[String],
        dart_files: &HashSet<&str>,
        out: &mut BTreeSet<String>,
    ) {
        collect_public_lib_files(base_dir, repository_paths, out);
        if let Some(executables) = self.executables.as_ref() {
            collect_declared_executables(base_dir, executables, dart_files, out);
        }
        collect_top_level_dart_files(base_dir, "bin", repository_paths, out);
        collect_hook_entries(base_dir, dart_files, out);
    }
}

fn collect_public_lib_files(
    base_dir: &str,
    repository_paths: &[String],
    out: &mut BTreeSet<String>,
) {
    let prefix = if base_dir.is_empty() {
        "lib/".to_owned()
    } else {
        format!("{base_dir}/lib/")
    };
    let is_app = repository_paths.iter().any(|path| {
        path.strip_prefix(&prefix).is_some_and(|rest| {
            rest == "main.dart"
                || (rest.starts_with("main_") && !rest.contains('/') && rest.ends_with(".dart"))
        })
    });
    for path in repository_paths {
        let Some(rest) = path.strip_prefix(&prefix) else {
            continue;
        };
        if !rest.ends_with(".dart") {
            continue;
        }
        if is_app {
            if !rest.contains('/') {
                out.insert(path.clone());
            }
        } else if !rest.starts_with("src/") {
            out.insert(path.clone());
        }
    }
}

fn collect_declared_executables(
    base_dir: &str,
    executables: &BTreeMap<String, Option<String>>,
    dart_files: &HashSet<&str>,
    out: &mut BTreeSet<String>,
) {
    for (command, explicit) in executables {
        let target = explicit
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| command.trim());
        if target.is_empty()
            || target.starts_with('/')
            || target.contains('\\')
            || target.split('/').any(|segment| segment == "..")
        {
            continue;
        }
        let rel = if target.ends_with(".dart") {
            format!("bin/{target}")
        } else {
            format!("bin/{target}.dart")
        };
        if let Some(candidate) = normalize_relative_path(base_dir, &rel)
            && dart_files.contains(candidate.as_str())
        {
            out.insert(candidate);
        }
    }
}

fn collect_top_level_dart_files(
    base_dir: &str,
    subdir: &str,
    repository_paths: &[String],
    out: &mut BTreeSet<String>,
) {
    let prefix = if base_dir.is_empty() {
        format!("{subdir}/")
    } else {
        format!("{base_dir}/{subdir}/")
    };
    for path in repository_paths {
        if let Some(rest) = path.strip_prefix(&prefix)
            && !rest.contains('/')
            && rest.ends_with(".dart")
        {
            out.insert(path.clone());
        }
    }
}

fn collect_hook_entries(base_dir: &str, dart_files: &HashSet<&str>, out: &mut BTreeSet<String>) {
    for rel in ["hook/build.dart", "hook/link.dart"] {
        if let Some(candidate) = normalize_relative_path(base_dir, rel)
            && dart_files.contains(candidate.as_str())
        {
            out.insert(candidate);
        }
    }
}
