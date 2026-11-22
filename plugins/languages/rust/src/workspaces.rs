use std::collections::BTreeSet;

use codeint_model::language::{Manifest, WorkspaceError};
use globset::{GlobBuilder, GlobSet, GlobSetBuilder};
use serde::Deserialize;

#[cfg(test)]
mod tests;

pub(crate) const CARGO_MANIFEST: &str = "Cargo.toml";

#[derive(Deserialize)]
struct RootCargoManifest {
    workspace: Option<WorkspaceSection>,
}

#[derive(Deserialize)]
struct WorkspaceSection {
    #[serde(default)]
    members: Vec<String>,
    #[serde(default)]
    exclude: Vec<String>,
}

pub(crate) fn workspace_packages(manifests: &[Manifest]) -> Result<Vec<String>, WorkspaceError> {
    let Some(root) = manifests
        .iter()
        .find(|manifest| manifest.path == CARGO_MANIFEST)
    else {
        return Ok(Vec::new());
    };
    let parsed: RootCargoManifest = toml::from_str(&root.text).map_err(|error| WorkspaceError {
        path: CARGO_MANIFEST.to_owned(),
        message: format!("is not valid TOML: {error}"),
    })?;
    let Some(workspace) = parsed.workspace else {
        return Ok(Vec::new());
    };
    if workspace.members.is_empty() {
        return Ok(Vec::new());
    }
    let included = build_glob_set(&workspace.members, "members")?;
    let excluded = build_glob_set(&workspace.exclude, "exclude")?;
    let packages: BTreeSet<String> = manifests
        .iter()
        .filter_map(|manifest| manifest.path.strip_suffix(&format!("/{CARGO_MANIFEST}")))
        .filter(|folder| !folder.split('/').any(|segment| segment == "target"))
        .filter(|folder| included.is_match(folder) && !excluded.is_match(folder))
        .map(str::to_owned)
        .collect();
    Ok(packages.into_iter().collect())
}

fn build_glob_set(patterns: &[String], field: &str) -> Result<GlobSet, WorkspaceError> {
    let mut builder = GlobSetBuilder::new();
    for written in patterns {
        let normalized = written
            .strip_prefix("./")
            .unwrap_or(written)
            .trim_end_matches('/');
        if normalized.is_empty() {
            continue;
        }
        let glob = GlobBuilder::new(normalized)
            .literal_separator(true)
            .build()
            .map_err(|error| WorkspaceError {
                path: CARGO_MANIFEST.to_owned(),
                message: format!(
                    "has workspace.{field} pattern {written:?}, which is not a valid glob: {error}"
                ),
            })?;
        builder.add(glob);
    }
    builder.build().map_err(|error| WorkspaceError {
        path: CARGO_MANIFEST.to_owned(),
        message: format!("has workspace.{field} patterns that cannot be combined: {error}"),
    })
}
