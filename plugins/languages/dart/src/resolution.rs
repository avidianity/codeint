use std::collections::{HashMap, HashSet};

use codeint_model::language::{ImportRequest, Manifest, Resolution};
use codeint_model::scan::{find_manifests, resolve_relative_file};
use serde::Deserialize;

use crate::workspaces::PUBSPEC_MANIFEST;

#[derive(Deserialize)]
struct PackageHeader {
    name: Option<String>,
}

pub(crate) fn find_pubspecs(repository_paths: &[String]) -> Vec<String> {
    find_manifests(repository_paths, PUBSPEC_MANIFEST)
}

pub(crate) fn resolve_all(
    repository_paths: &[String],
    manifests: &[Manifest],
    requests: &[ImportRequest],
) -> Vec<Resolution> {
    let files: HashSet<String> = repository_paths.iter().cloned().collect();
    let packages = collect_package_roots(manifests);
    requests
        .iter()
        .map(|req| resolve_one(&files, &packages, req))
        .collect()
}

fn collect_package_roots(manifests: &[Manifest]) -> HashMap<String, String> {
    let suffix = format!("/{PUBSPEC_MANIFEST}");
    let mut roots = HashMap::new();
    for manifest in manifests {
        let Ok(Some(header)) = serde_saphyr::from_str::<Option<PackageHeader>>(&manifest.text)
        else {
            continue;
        };
        let Some(name) = header.name.map(|n| n.trim().to_owned()) else {
            continue;
        };
        if name.is_empty() {
            continue;
        }
        let dir = manifest.path.strip_suffix(&suffix).unwrap_or("").to_owned();
        roots.insert(name, dir);
    }
    roots
}

fn resolve_one(
    files: &HashSet<String>,
    packages: &HashMap<String, String>,
    request: &ImportRequest,
) -> Resolution {
    let spec = request.specifier.as_str();
    if spec.starts_with("dart:") {
        return Resolution::External;
    }
    if let Some(pkg_rest) = spec.strip_prefix("package:") {
        return resolve_package_uri(files, packages, &request.importer, pkg_rest);
    }
    resolve_relative_file(files, &request.importer, spec, false)
}

fn resolve_package_uri(
    files: &HashSet<String>,
    packages: &HashMap<String, String>,
    importer: &str,
    pkg_rest: &str,
) -> Resolution {
    let Some((pkg_name, rel_path)) = pkg_rest.split_once('/') else {
        return Resolution::External;
    };
    let Some(pkg_dir) = packages.get(pkg_name) else {
        return Resolution::External;
    };
    let candidate = if pkg_dir.is_empty() {
        format!("lib/{rel_path}")
    } else {
        format!("{pkg_dir}/lib/{rel_path}")
    };
    if files.contains(&candidate) {
        if candidate == importer {
            Resolution::External
        } else {
            Resolution::Resolved(candidate)
        }
    } else {
        Resolution::Unresolved(format!(
            "no file of the repository matches package:{pkg_rest:?}"
        ))
    }
}
