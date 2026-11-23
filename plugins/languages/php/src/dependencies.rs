use std::collections::{BTreeSet, HashSet};

use codeint_model::language::{DeclaredDep, DepKind, Manifest};
use serde_json::Value;

use crate::workspaces::COMPOSER_MANIFEST;

#[cfg(test)]
mod tests;

pub(crate) fn normalize_composer_name(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    let (vendor, package) = trimmed.split_once('/')?;
    if vendor.is_empty() || package.is_empty() || package.contains('/') {
        return None;
    }
    Some(format!(
        "{}/{}",
        vendor.to_ascii_lowercase(),
        package.to_ascii_lowercase()
    ))
}

pub(crate) fn declared_package_names(manifests: &[Manifest]) -> HashSet<String> {
    extract_declared(manifests)
        .into_iter()
        .map(|dep| dep.name)
        .collect()
}

pub(crate) fn extract_declared(manifests: &[Manifest]) -> Vec<DeclaredDep> {
    let suffix = format!("/{COMPOSER_MANIFEST}");
    let parsed: Vec<(String, Value)> = manifests
        .iter()
        .filter(|m| is_project_composer(&m.path, &suffix))
        .filter_map(|m| {
            let doc = serde_json::from_str::<Value>(&m.text).ok()?;
            let root = m.path.strip_suffix(&suffix).unwrap_or("").to_owned();
            Some((root, doc))
        })
        .collect();

    let local_packages: HashSet<String> = parsed
        .iter()
        .filter_map(|(_, doc)| doc.get("name").and_then(Value::as_str))
        .filter_map(normalize_composer_name)
        .collect();

    let mut entries = BTreeSet::new();
    for (root, doc) in &parsed {
        collect_section(root, doc, "require", 0, &local_packages, &mut entries);
        collect_section(root, doc, "require-dev", 1, &local_packages, &mut entries);
        collect_section(root, doc, "suggest", 2, &local_packages, &mut entries);
    }

    entries
        .into_iter()
        .map(|(package_root, rank, name)| DeclaredDep {
            package_root,
            name,
            kind: match rank {
                0 => DepKind::Runtime,
                1 => DepKind::Dev,
                _ => DepKind::Optional,
            },
        })
        .collect()
}

fn is_project_composer(path: &str, suffix: &str) -> bool {
    let matches_name = path == COMPOSER_MANIFEST || path.ends_with(suffix);
    let in_vendor = path.split('/').any(|seg| seg == "vendor");
    matches_name && !in_vendor
}

fn collect_section(
    root: &str,
    doc: &Value,
    key: &str,
    rank: u8,
    local_packages: &HashSet<String>,
    out: &mut BTreeSet<(String, u8, String)>,
) {
    let Some(map) = doc.get(key).and_then(Value::as_object) else {
        return;
    };
    for raw_name in map.keys() {
        if let Some(name) = normalize_composer_name(raw_name)
            && !local_packages.contains(&name)
        {
            out.insert((root.to_owned(), rank, name));
        }
    }
}
