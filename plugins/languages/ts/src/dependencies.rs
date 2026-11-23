#[cfg(test)]
mod tests;

use std::collections::{BTreeSet, HashSet};

use codeint_model::language::{DeclaredDep, DepKind, Manifest};
use serde_json::Value;

use crate::workspaces::PACKAGE_MANIFEST;

const RUNTIME_ORDER: u8 = 0;
const DEV_ORDER: u8 = 1;
const OPTIONAL_ORDER: u8 = 2;

const DEP_SECTIONS: [(&str, u8); 4] = [
    ("dependencies", RUNTIME_ORDER),
    ("devDependencies", DEV_ORDER),
    ("optionalDependencies", OPTIONAL_ORDER),
    ("peerDependencies", OPTIONAL_ORDER),
];

pub(crate) fn extract_declared(manifests: &[Manifest]) -> Vec<DeclaredDep> {
    let active: Vec<(&str, Value)> = manifests.iter().filter_map(parse_active_package).collect();
    let local_names = collect_local_names(&active);

    let mut collected = BTreeSet::new();
    for (root, doc) in &active {
        collect_package_deps(root, doc, &local_names, &mut collected);
    }

    collected
        .into_iter()
        .map(|(package_root, order, name)| DeclaredDep {
            package_root,
            name,
            kind: dep_kind_from_order(order),
        })
        .collect()
}

fn collect_local_names(active: &[(&str, Value)]) -> HashSet<String> {
    active
        .iter()
        .filter_map(|(_, doc)| doc.get("name").and_then(Value::as_str))
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(str::to_owned)
        .collect()
}

fn is_ambient_types_package(name: &str) -> bool {
    name.starts_with("@types/")
}

fn parse_active_package(manifest: &Manifest) -> Option<(&str, Value)> {
    let root = if manifest.path == PACKAGE_MANIFEST {
        ""
    } else {
        let dir = manifest.path.strip_suffix("/package.json")?;
        if dir.split('/').any(|segment| segment == "node_modules") {
            return None;
        }
        dir
    };
    let doc = serde_json::from_str::<Value>(&manifest.text).ok()?;
    Some((root, doc))
}

fn collect_package_deps(
    root: &str,
    doc: &Value,
    local_names: &HashSet<String>,
    out: &mut BTreeSet<(String, u8, String)>,
) {
    for &(field, order) in &DEP_SECTIONS {
        let Some(entries) = doc.get(field).and_then(Value::as_object) else {
            continue;
        };
        for (raw_name, spec) in entries {
            if is_workspace_spec(spec) {
                continue;
            }
            let name = raw_name.trim();
            if name.is_empty() || local_names.contains(name) {
                continue;
            }
            let effective_order = if is_ambient_types_package(name) {
                OPTIONAL_ORDER
            } else {
                order
            };
            out.insert((root.to_owned(), effective_order, name.to_owned()));
        }
    }
}

fn is_workspace_spec(spec: &Value) -> bool {
    spec.as_str()
        .is_some_and(|version| version.trim().starts_with("workspace:"))
}

fn dep_kind_from_order(order: u8) -> DepKind {
    match order {
        RUNTIME_ORDER => DepKind::Runtime,
        DEV_ORDER => DepKind::Dev,
        _ => DepKind::Optional,
    }
}
