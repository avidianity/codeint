use std::collections::BTreeSet;

use codeint_model::language::{Manifest, WorkspaceError};
use globset::{GlobBuilder, GlobSet, GlobSetBuilder};
use serde::Deserialize;

#[cfg(test)]
mod tests;

pub(crate) const PUBSPEC_MANIFEST: &str = "pubspec.yaml";

#[derive(Deserialize)]
struct PubspecWorkspace {
    workspace: Option<Vec<String>>,
}

fn pubspec_error(message: String) -> WorkspaceError {
    WorkspaceError {
        path: PUBSPEC_MANIFEST.to_owned(),
        message,
    }
}

pub(crate) fn discover(manifests: &[Manifest]) -> Result<Vec<String>, WorkspaceError> {
    let Some(root_text) = manifests
        .iter()
        .find(|m| m.path == PUBSPEC_MANIFEST)
        .map(|m| m.text.as_str())
    else {
        return Ok(Vec::new());
    };
    let parsed: Option<PubspecWorkspace> = serde_saphyr::from_str(root_text)
        .map_err(|err| pubspec_error(format!("is not valid YAML: {err}")))?;
    let patterns = parsed.and_then(|p| p.workspace).unwrap_or_default();
    let Some(matcher) = compile_workspace_globs(&patterns)? else {
        return Ok(Vec::new());
    };
    Ok(select_member_folders(manifests, &matcher))
}

fn compile_workspace_globs(patterns: &[String]) -> Result<Option<GlobSet>, WorkspaceError> {
    let normalized: Vec<(&str, &str)> = patterns
        .iter()
        .filter_map(|raw| {
            let trimmed = raw.trim();
            let cleaned = trimmed
                .strip_prefix("./")
                .unwrap_or(trimmed)
                .trim_end_matches('/');
            (!cleaned.is_empty() && !cleaned.starts_with('/') && !cleaned.starts_with(".."))
                .then_some((raw.as_str(), cleaned))
        })
        .collect();
    if normalized.is_empty() {
        return Ok(None);
    }
    let mut builder = GlobSetBuilder::new();
    for (raw, cleaned) in normalized {
        let glob = GlobBuilder::new(cleaned)
            .literal_separator(true)
            .build()
            .map_err(|err| {
                pubspec_error(format!(
                    "has workspace pattern {raw:?}, which is not a valid glob: {err}"
                ))
            })?;
        builder.add(glob);
    }
    builder
        .build()
        .map(Some)
        .map_err(|err| pubspec_error(format!("cannot combine workspace patterns: {err}")))
}

fn select_member_folders(manifests: &[Manifest], matcher: &GlobSet) -> Vec<String> {
    let suffix = format!("/{PUBSPEC_MANIFEST}");
    let mut members = BTreeSet::new();
    for manifest in manifests {
        if let Some(folder) = manifest.path.strip_suffix(&suffix)
            && matcher.is_match(folder)
        {
            members.insert(folder.to_owned());
        }
    }
    members.into_iter().collect()
}
