use std::collections::{HashMap, HashSet};

use codeint_model::language::{ImportRequest, Manifest, Resolution};
use codeint_model::scan::{find_manifests, resolve_relative_file};
use serde::Deserialize;

use crate::dependencies::normalize_pub_name;
use crate::frameworks::{fallback_package_import, is_virtual_generated_import};
use crate::workspaces::PUBSPEC_MANIFEST;

#[cfg(test)]
mod tests;

#[derive(Deserialize)]
struct PackageHeader {
    name: Option<String>,
}

pub(crate) fn find_pubspecs(repository_paths: &[String]) -> Vec<String> {
    find_manifests(repository_paths, PUBSPEC_MANIFEST)
}

pub(crate) fn resolve_dart_imports(
    paths: &[String],
    manifests: &[Manifest],
    requests: &[ImportRequest],
    profiles: &[String],
) -> Vec<Resolution> {
    let files: HashSet<String> = paths.iter().cloned().collect();
    let packages = collect_package_roots(manifests);
    requests
        .iter()
        .map(|req| resolve_one(&files, &packages, req, profiles))
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
        let Some(name) = header.name.map(|n| normalize_pub_name(&n)) else {
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
    active_profiles: &[String],
) -> Resolution {
    let spec = request.specifier.as_str();
    if spec.starts_with("dart:") {
        return Resolution::Builtin;
    }
    if let Some(pkg_rest) = spec.strip_prefix("package:") {
        let resolved = resolve_package_uri(files, packages, &request.importer, pkg_rest);
        if matches!(resolved, Resolution::External(_))
            && let Some(fallback) = fallback_package_import(
                active_profiles,
                files,
                packages,
                &request.importer,
                pkg_rest,
            )
        {
            return fallback;
        }
        return resolved;
    }
    match resolve_relative_file(files, &request.importer, spec, false) {
        Resolution::External(pkg) if pkg.is_empty() => Resolution::Builtin,
        Resolution::Unresolved(_) if is_virtual_generated_import(active_profiles, spec) => {
            Resolution::Builtin
        }
        other => other,
    }
}

fn resolve_package_uri(
    files: &HashSet<String>,
    packages: &HashMap<String, String>,
    importer: &str,
    pkg_rest: &str,
) -> Resolution {
    let (raw_pkg, rel_opt) = match pkg_rest.split_once('/') {
        Some((pkg, rel)) => (pkg, Some(rel)),
        None => (pkg_rest, None),
    };
    let pkg_name = normalize_pub_name(raw_pkg);
    if pkg_name.is_empty() {
        return Resolution::Unresolved(format!("malformed Dart package URI package:{pkg_rest:?}"));
    }
    let Some(pkg_dir) = packages.get(&pkg_name) else {
        return Resolution::External(pkg_name);
    };
    let Some(rel_path) = rel_opt.filter(|rel| !rel.is_empty()) else {
        return Resolution::Unresolved(format!(
            "no file of the repository matches package:{pkg_rest:?}"
        ));
    };
    let candidate = if pkg_dir.is_empty() {
        format!("lib/{rel_path}")
    } else {
        format!("{pkg_dir}/lib/{rel_path}")
    };
    if files.contains(&candidate) {
        if candidate == importer {
            Resolution::Builtin
        } else {
            Resolution::Resolved(vec![candidate])
        }
    } else {
        Resolution::Unresolved(format!(
            "no file of the repository matches package:{pkg_rest:?}"
        ))
    }
}

#[cfg(test)]
pub(crate) fn resolve_all(
    paths: &[String],
    manifests: &[Manifest],
    requests: &[ImportRequest],
) -> Vec<Resolution> {
    resolve_dart_imports(paths, manifests, requests, &[])
}
