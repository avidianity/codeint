use std::collections::{BTreeSet, HashSet};

use codeint_model::language::Manifest;
use codeint_model::scan::normalize_relative_path;
use serde_json::Value;

use crate::workspaces::COMPOSER_MANIFEST;

pub(crate) fn extract(repository_paths: &[String], manifests: &[Manifest]) -> Vec<String> {
    let available: HashSet<&str> = repository_paths.iter().map(String::as_str).collect();
    let available_ref = &available;
    let nested_suffix = format!("/{COMPOSER_MANIFEST}");
    let collected: BTreeSet<String> = manifests
        .iter()
        .filter_map(|manifest| {
            let dir = if manifest.path == COMPOSER_MANIFEST {
                ""
            } else {
                manifest.path.strip_suffix(&nested_suffix)?
            };
            let parsed = serde_json::from_str::<Value>(&manifest.text).ok()?;
            Some(
                bin_strings(parsed.get("bin")?)
                    .into_iter()
                    .filter_map(move |raw| resolve_composer_bin(dir, &raw, available_ref)),
            )
        })
        .flatten()
        .collect();
    collected.into_iter().collect()
}

fn bin_strings(value: &Value) -> Vec<String> {
    match value {
        Value::String(single) => vec![single.clone()],
        Value::Array(items) => items
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_owned)
            .collect(),
        _ => Vec::new(),
    }
}

fn resolve_composer_bin(base_dir: &str, raw: &str, available: &HashSet<&str>) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed.starts_with('/') || trimmed.contains('\\') {
        return None;
    }
    let normalized = normalize_relative_path(base_dir, trimmed)?;
    (!normalized.is_empty() && available.contains(normalized.as_str())).then_some(normalized)
}
