use std::collections::{BTreeMap, BTreeSet, HashSet};

#[cfg(test)]
use codeint_model::language::Manifest;
use codeint_model::language::{DeclaredDep, DepKind};
use serde::Deserialize;
use serde::de::IgnoredAny;

#[cfg(test)]
use crate::workspaces::PUBSPEC_MANIFEST;

#[cfg(test)]
mod tests;

#[derive(Deserialize)]
pub(crate) struct PubspecManifest {
    name: Option<String>,
    dependencies: Option<BTreeMap<String, IgnoredAny>>,
    dev_dependencies: Option<BTreeMap<String, IgnoredAny>>,
}

pub(crate) fn normalize_pub_name(raw: &str) -> String {
    raw.trim().to_ascii_lowercase()
}

#[cfg(test)]
pub(crate) fn extract_declared(manifests: &[Manifest]) -> Vec<DeclaredDep> {
    let suffix = format!("/{PUBSPEC_MANIFEST}");
    let mut parsed = Vec::new();
    for manifest in manifests {
        if !crate::workspaces::is_pubspec_path(&manifest.path) {
            continue;
        }
        let Ok(doc) = serde_saphyr::from_str::<Option<PubspecManifest>>(&manifest.text) else {
            continue;
        };
        let root = manifest.path.strip_suffix(&suffix).unwrap_or("").to_owned();
        parsed.push((root, manifest.path.clone(), doc));
    }
    extract_declared_from_parsed(&parsed)
}

pub(crate) fn extract_declared_from_parsed(
    parsed: &[(String, String, Option<PubspecManifest>)],
) -> Vec<DeclaredDep> {
    let mut local_packages = HashSet::new();
    for (_, _, doc_opt) in parsed {
        if let Some(doc) = doc_opt
            && let Some(local) = doc.name.as_deref().map(normalize_pub_name)
            && !local.is_empty()
        {
            local_packages.insert(local);
        }
    }

    let mut entries = BTreeSet::new();
    for (root, _, doc_opt) in parsed {
        if let Some(doc) = doc_opt {
            collect_pubspec_deps(root, doc, &local_packages, &mut entries);
        }
    }

    entries
        .into_iter()
        .map(|(package_root, rank, name)| DeclaredDep {
            package_root,
            name,
            kind: if rank == 0 {
                DepKind::Runtime
            } else {
                DepKind::Dev
            },
        })
        .collect()
}

fn collect_pubspec_deps(
    root: &str,
    doc: &PubspecManifest,
    local_packages: &HashSet<String>,
    entries: &mut BTreeSet<(String, u8, String)>,
) {
    for (section, rank) in [(&doc.dependencies, 0u8), (&doc.dev_dependencies, 1u8)] {
        let Some(deps) = section.as_ref() else {
            continue;
        };
        for raw_name in deps.keys() {
            let name = normalize_pub_name(raw_name);
            if !name.is_empty() && !local_packages.contains(&name) {
                entries.insert((root.to_owned(), rank, name));
            }
        }
    }
}
