use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use serde_json::Value;

const ARRAY_ITEM: &str = "[]";

pub(crate) fn schema_key_paths() -> BTreeSet<String> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../schema/codeint.schema.json");
    let text = std::fs::read_to_string(&path).expect("read the published JSON schema");
    let schema: Value = serde_json::from_str(&text).expect("the JSON schema is JSON");
    let mut paths = BTreeSet::new();
    collect_schema_paths(&schema, &schema, "", &mut paths);
    paths
}

fn resolved<'a>(root: &'a Value, node: &'a Value) -> &'a Value {
    match node["$ref"]
        .as_str()
        .and_then(|reference| reference.strip_prefix('#'))
    {
        Some(pointer) => resolved(
            root,
            root.pointer(pointer).expect("a $ref names a definition"),
        ),
        None => node,
    }
}

fn collect_schema_paths(root: &Value, node: &Value, prefix: &str, paths: &mut BTreeSet<String>) {
    let node = resolved(root, node);
    if let Some(properties) = node["properties"].as_object() {
        for (name, property) in properties {
            let path = joined(prefix, name);
            paths.insert(path.clone());
            collect_schema_paths(root, property, &path, paths);
        }
    }
    if node.get("items").is_some() {
        collect_schema_paths(
            root,
            &node["items"],
            &format!("{prefix}{ARRAY_ITEM}"),
            paths,
        );
    }
}

fn joined(prefix: &str, name: &str) -> String {
    if prefix.is_empty() {
        name.to_owned()
    } else {
        format!("{prefix}.{name}")
    }
}

pub(crate) fn configuration_key_paths(configuration: &Value) -> BTreeSet<String> {
    let mut paths = BTreeSet::new();
    collect_configuration_paths(configuration, "", &mut paths);
    paths
}

fn collect_configuration_paths(node: &Value, prefix: &str, paths: &mut BTreeSet<String>) {
    match node {
        Value::Object(members) => {
            for (name, member) in members {
                let path = joined(prefix, name);
                paths.insert(path.clone());
                collect_configuration_paths(member, &path, paths);
            }
        }
        Value::Array(items) => {
            for item in items {
                collect_configuration_paths(item, &format!("{prefix}{ARRAY_ITEM}"), paths);
            }
        }
        _ => {}
    }
}

pub(crate) fn expand(pattern: &str, placeholders: &BTreeMap<String, Vec<String>>) -> Vec<String> {
    let Some((placeholder, values)) = placeholders
        .iter()
        .find(|(placeholder, _)| pattern.contains(placeholder.as_str()))
    else {
        return vec![pattern.to_owned()];
    };
    values
        .iter()
        .flat_map(|value| {
            expand(
                &pattern.replacen(placeholder.as_str(), value, 1),
                placeholders,
            )
        })
        .collect()
}
