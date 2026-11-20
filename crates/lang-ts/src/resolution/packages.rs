use std::collections::HashMap;

use codeint_model::language::Manifest;
use serde_json::Value;

pub(super) const PACKAGE_MANIFEST: &str = "package.json";

pub(super) struct PackageLink {
    pub(super) name: String,
    pub(super) folder: String,
}

pub(super) fn package_links(manifests: &[Manifest]) -> Vec<PackageLink> {
    let mut folders_by_name: HashMap<String, Vec<&str>> = HashMap::new();
    for manifest in manifests {
        let Some(folder) = manifest
            .path
            .strip_suffix(PACKAGE_MANIFEST)
            .and_then(|folder| folder.strip_suffix('/'))
        else {
            continue;
        };
        if folder.split('/').any(|segment| segment == "node_modules") {
            continue;
        }
        if let Some(name) = package_name(&manifest.text) {
            folders_by_name.entry(name).or_default().push(folder);
        }
    }
    let mut links: Vec<PackageLink> = folders_by_name
        .into_iter()
        .filter_map(|(name, folders)| match folders.as_slice() {
            [folder] => Some(PackageLink {
                name,
                folder: (*folder).to_owned(),
            }),
            _ => None,
        })
        .collect();
    links.sort_by(|left, right| left.name.cmp(&right.name));
    links
}

fn package_name(text: &str) -> Option<String> {
    let manifest: Value = serde_json::from_str(text).ok()?;
    let name = manifest.get("name")?.as_str()?;
    is_package_name(name).then(|| name.to_owned())
}

fn is_package_name(name: &str) -> bool {
    let segments: Vec<&str> = name.split('/').collect();
    let valid = |segment: &&str| !segment.is_empty() && *segment != "." && *segment != "..";
    match segments.as_slice() {
        [scope, package] => scope.starts_with('@') && scope.len() > 1 && valid(package),
        [package] => !package.starts_with('@') && valid(package),
        _ => false,
    }
}
