use std::collections::BTreeSet;

use codeint_model::language::{Manifest, ManifestAnalysis, WorkspaceError};
use codeint_model::scan::{analyze_parsed_manifests, manifest_root_excluding};
use globset::{GlobBuilder, GlobSet, GlobSetBuilder};
use serde_json::Value;

#[cfg(test)]
mod tests;

pub(crate) const COMPOSER_MANIFEST: &str = "composer.json";

pub(crate) fn analyze_manifests(manifests: &[Manifest]) -> ManifestAnalysis {
    analyze_parsed_manifests(
        manifests,
        COMPOSER_MANIFEST,
        |manifest| {
            let root = manifest_root_excluding(&manifest.path, COMPOSER_MANIFEST, &["vendor"])?;
            Some(
                serde_json::from_str::<Value>(&manifest.text)
                    .map(|doc| (root.to_owned(), doc))
                    .map_err(|err| WorkspaceError {
                        path: manifest.path.clone(),
                        message: format!("is not valid JSON: {err}"),
                    }),
            )
        },
        |all, parsed| {
            let Some((_, _, root_doc)) = parsed.iter().find(|(_, p, _)| p == COMPOSER_MANIFEST)
            else {
                return Ok(Vec::new());
            };
            Ok(build_path_glob_set(root_doc)?
                .map(|set| matching_package_folders(all, &set))
                .unwrap_or_default())
        },
        |_| true,
        crate::dependencies::extract_declared_from_parsed,
    )
}

fn composer_error(message: String) -> WorkspaceError {
    WorkspaceError {
        path: COMPOSER_MANIFEST.to_owned(),
        message,
    }
}

#[cfg(test)]
fn parse_root_composer(manifests: &[Manifest]) -> Result<Option<Value>, WorkspaceError> {
    for manifest in manifests {
        if manifest.path == COMPOSER_MANIFEST {
            return match serde_json::from_str::<Value>(&manifest.text) {
                Ok(parsed) => Ok(Some(parsed)),
                Err(err) => Err(composer_error(format!("is not valid JSON: {err}"))),
            };
        }
    }
    Ok(None)
}

#[cfg(test)]
pub(crate) fn workspace_packages(manifests: &[Manifest]) -> Result<Vec<String>, WorkspaceError> {
    let Some(parsed) = parse_root_composer(manifests)? else {
        return Ok(Vec::new());
    };
    let Some(set) = build_path_glob_set(&parsed)? else {
        return Ok(Vec::new());
    };
    Ok(matching_package_folders(manifests, &set))
}

fn path_repository_urls(parsed: &Value) -> Vec<&str> {
    let entries: Vec<&Value> = match parsed.get("repositories") {
        Some(Value::Array(items)) => items.iter().collect(),
        Some(Value::Object(map)) => map.values().collect(),
        _ => return Vec::new(),
    };
    entries
        .into_iter()
        .filter_map(Value::as_object)
        .filter(|obj| obj.get("type").and_then(Value::as_str) == Some("path"))
        .filter_map(|obj| obj.get("url").and_then(Value::as_str))
        .collect()
}

fn build_path_glob_set(parsed: &Value) -> Result<Option<GlobSet>, WorkspaceError> {
    let mut builder = GlobSetBuilder::new();
    let mut count = 0usize;
    for url in path_repository_urls(parsed) {
        let normalized = url.strip_prefix("./").unwrap_or(url).trim_end_matches('/');
        if normalized.is_empty() || normalized.starts_with('/') || normalized.starts_with("..") {
            continue;
        }
        let glob = GlobBuilder::new(normalized)
            .literal_separator(true)
            .build()
            .map_err(|err| {
                composer_error(format!(
                    "has repository path pattern {url:?}, which is not a valid glob: {err}"
                ))
            })?;
        builder.add(glob);
        count += 1;
    }
    if count == 0 {
        return Ok(None);
    }
    let set = builder.build().map_err(|err| {
        composer_error(format!(
            "has repository path patterns that cannot be combined: {err}"
        ))
    })?;
    Ok(Some(set))
}

fn matching_package_folders(manifests: &[Manifest], set: &GlobSet) -> Vec<String> {
    let suffix = format!("/{COMPOSER_MANIFEST}");
    let mut found = BTreeSet::new();
    for manifest in manifests {
        if let Some(folder) = manifest.path.strip_suffix(&suffix)
            && !folder.split('/').any(|part| part == "vendor")
            && set.is_match(folder)
        {
            found.insert(folder.to_owned());
        }
    }
    found.into_iter().collect()
}
