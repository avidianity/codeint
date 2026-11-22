use std::collections::BTreeSet;

use codeint_model::language::{Manifest, WorkspaceError};
use globset::{GlobBuilder, GlobSetBuilder};
use serde_json::Value;

#[cfg(test)]
mod tests;

pub(crate) const COMPOSER_MANIFEST: &str = "composer.json";

pub(crate) fn workspace_packages(manifests: &[Manifest]) -> Result<Vec<String>, WorkspaceError> {
    let Some(root) = manifests
        .iter()
        .find(|manifest| manifest.path == COMPOSER_MANIFEST)
    else {
        return Ok(Vec::new());
    };
    let value: Value = serde_json::from_str(&root.text).map_err(|error| WorkspaceError {
        path: COMPOSER_MANIFEST.to_owned(),
        message: format!("is not valid JSON: {error}"),
    })?;
    let Some(repositories) = value.get("repositories") else {
        return Ok(Vec::new());
    };
    let entries: Vec<&Value> = match repositories {
        Value::Array(items) => items.iter().collect(),
        Value::Object(map) => map.values().collect(),
        _ => return Ok(Vec::new()),
    };
    let mut builder = GlobSetBuilder::new();
    let mut has_patterns = false;
    for entry in entries {
        let Some(obj) = entry.as_object() else {
            continue;
        };
        if obj.get("type").and_then(Value::as_str) != Some("path") {
            continue;
        }
        let Some(url) = obj.get("url").and_then(Value::as_str) else {
            continue;
        };
        let normalized = url.strip_prefix("./").unwrap_or(url).trim_end_matches('/');
        if normalized.is_empty() || normalized.starts_with('/') || normalized.starts_with("..") {
            continue;
        }
        let glob = GlobBuilder::new(normalized)
            .literal_separator(true)
            .build()
            .map_err(|error| WorkspaceError {
                path: COMPOSER_MANIFEST.to_owned(),
                message: format!(
                    "has repository path pattern {url:?}, which is not a valid glob: {error}"
                ),
            })?;
        builder.add(glob);
        has_patterns = true;
    }
    if !has_patterns {
        return Ok(Vec::new());
    }
    let set = builder.build().map_err(|error| WorkspaceError {
        path: COMPOSER_MANIFEST.to_owned(),
        message: format!("has repository path patterns that cannot be combined: {error}"),
    })?;
    let packages: BTreeSet<String> = manifests
        .iter()
        .filter_map(|manifest| manifest.path.strip_suffix(&format!("/{COMPOSER_MANIFEST}")))
        .filter(|folder| !folder.split('/').any(|segment| segment == "vendor"))
        .filter(|folder| set.is_match(folder))
        .map(str::to_owned)
        .collect();
    Ok(packages.into_iter().collect())
}
