use std::collections::{HashMap, HashSet};

use codeint_model::language::Manifest;
use serde_json::Value;

use crate::workspaces::{PACKAGE_MANIFEST, workspace_packages};

pub(super) struct PackageLink {
    pub(super) name: String,
    pub(super) folder: String,
}

pub(super) fn package_links(manifests: &[Manifest]) -> Vec<PackageLink> {
    let members: HashSet<String> = workspace_packages(manifests)
        .unwrap_or_default()
        .into_iter()
        .collect();
    let mut folders_by_name: HashMap<String, Vec<&str>> = HashMap::new();
    for manifest in manifests {
        let Some(folder) = manifest
            .path
            .strip_suffix(PACKAGE_MANIFEST)
            .and_then(|folder| folder.strip_suffix('/'))
        else {
            continue;
        };
        if !members.contains(folder) {
            continue;
        }
        if let Some(name) = package_name(&manifest.text) {
            folders_by_name.entry(name).or_default().push(folder);
        }
    }
    let mut links: Vec<PackageLink> = folders_by_name
        .iter()
        .filter_map(|(name, folders)| match folders.as_slice() {
            [folder] => Some(PackageLink {
                name: name.clone(),
                folder: (*folder).to_owned(),
            }),
            _ => None,
        })
        .collect();
    if let Some(root) = manifests.iter().find(|m| m.path == PACKAGE_MANIFEST)
        && let Some(name) = package_name(&root.text)
        && !folders_by_name.contains_key(&name)
    {
        links.push(PackageLink {
            name,
            folder: String::new(),
        });
    }
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
