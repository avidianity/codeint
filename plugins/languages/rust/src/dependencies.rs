use std::collections::{BTreeSet, HashSet};

use codeint_model::language::{DeclaredDep, DepKind, Manifest};
use toml::Value;

use crate::workspaces::{CARGO_MANIFEST, workspace_packages};

#[cfg(test)]
mod tests;

pub(crate) fn normalize_crate_name(raw: &str) -> String {
    let trimmed = raw.trim();
    let stripped = trimmed.strip_prefix("r#").unwrap_or(trimmed);
    stripped.replace('-', "_")
}

pub(crate) fn extract_declared(manifests: &[Manifest]) -> Vec<DeclaredDep> {
    let docs = load_active_manifests(manifests);
    let local_crates = collect_local_crates(&docs);

    let mut collected: BTreeSet<(String, u8, String)> = BTreeSet::new();
    for (root, doc) in &docs {
        collect_manifest_tables(root, doc, &local_crates, &mut collected);
    }

    collected
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

pub(crate) fn load_active_manifests(manifests: &[Manifest]) -> Vec<(String, Value)> {
    let members: HashSet<String> = workspace_packages(manifests)
        .unwrap_or_default()
        .into_iter()
        .collect();
    let has_root = manifests.iter().any(|m| m.path == CARGO_MANIFEST);

    let mut docs = Vec::new();
    for manifest in manifests {
        let root = match manifest.path.as_str() {
            CARGO_MANIFEST => "",
            path => match path.strip_suffix("/Cargo.toml") {
                Some(dir) if !dir.split('/').any(|seg| seg == "target") => dir,
                _ => continue,
            },
        };
        let selected = match (!members.is_empty(), has_root) {
            (true, _) => root.is_empty() || members.contains(root),
            (false, true) => root.is_empty(),
            (false, false) => true,
        };
        if selected && let Ok(doc) = toml::from_str::<Value>(&manifest.text) {
            docs.push((root.to_owned(), doc));
        }
    }
    docs
}

fn collect_local_crates(docs: &[(String, Value)]) -> HashSet<String> {
    let mut local = HashSet::new();
    for (root, doc) in docs {
        for (section, key) in [("package", "name"), ("lib", "name")] {
            if let Some(name) = table_str(doc, section, key) {
                insert_normalized(&mut local, name);
            }
        }
        if root.is_empty() {
            collect_workspace_path_crates(doc, &mut local);
        }
    }
    local
}

fn collect_workspace_path_crates(doc: &Value, local: &mut HashSet<String>) {
    let Some(ws_deps) = doc
        .get("workspace")
        .and_then(|ws| ws.get("dependencies"))
        .and_then(Value::as_table)
    else {
        return;
    };
    for (dep_key, spec) in ws_deps {
        let Some(table) = spec.as_table().filter(|t| t.contains_key("path")) else {
            continue;
        };
        insert_normalized(local, dep_key);
        if let Some(pkg) = table.get("package").and_then(Value::as_str) {
            insert_normalized(local, pkg);
        }
    }
}

fn table_str<'a>(doc: &'a Value, section: &str, key: &str) -> Option<&'a str> {
    doc.get(section)
        .and_then(|s| s.get(key))
        .and_then(Value::as_str)
}

fn insert_normalized(set: &mut HashSet<String>, raw: &str) {
    let normalized = normalize_crate_name(raw);
    if !normalized.is_empty() {
        set.insert(normalized);
    }
}

fn collect_manifest_tables(
    root: &str,
    doc: &Value,
    local_crates: &HashSet<String>,
    out: &mut BTreeSet<(String, u8, String)>,
) {
    collect_dep_table(root, doc.get("dependencies"), false, local_crates, out);
    collect_dep_table(root, doc.get("dev-dependencies"), true, local_crates, out);
    collect_dep_table(root, doc.get("build-dependencies"), true, local_crates, out);

    if let Some(targets) = doc.get("target").and_then(Value::as_table) {
        for cfg_table in targets.values() {
            collect_dep_table(
                root,
                cfg_table.get("dependencies"),
                false,
                local_crates,
                out,
            );
            collect_dep_table(
                root,
                cfg_table.get("dev-dependencies"),
                true,
                local_crates,
                out,
            );
            collect_dep_table(
                root,
                cfg_table.get("build-dependencies"),
                true,
                local_crates,
                out,
            );
        }
    }
}

fn collect_dep_table(
    root: &str,
    section: Option<&Value>,
    is_dev_section: bool,
    local_crates: &HashSet<String>,
    out: &mut BTreeSet<(String, u8, String)>,
) {
    let Some(table) = section.and_then(Value::as_table) else {
        return;
    };
    for (raw_key, spec) in table {
        if is_local_dependency(raw_key, spec, local_crates) {
            continue;
        }
        let name = normalize_crate_name(raw_key);
        if name.is_empty() {
            continue;
        }
        let rank = if is_dev_section {
            1
        } else if spec
            .as_table()
            .and_then(|t| t.get("optional"))
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            2
        } else {
            0
        };
        out.insert((root.to_owned(), rank, name));
    }
}

fn is_local_dependency(raw_key: &str, spec: &Value, local_crates: &HashSet<String>) -> bool {
    if spec.as_table().is_some_and(|t| t.contains_key("path")) {
        return true;
    }
    let normalized_key = normalize_crate_name(raw_key);
    if local_crates.contains(&normalized_key) {
        return true;
    }
    spec.as_table()
        .and_then(|t| t.get("package"))
        .and_then(Value::as_str)
        .is_some_and(|pkg| local_crates.contains(&normalize_crate_name(pkg)))
}
