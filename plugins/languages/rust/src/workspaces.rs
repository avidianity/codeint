use std::collections::BTreeSet;

use codeint_model::language::{Manifest, ManifestAnalysis, WorkspaceError};
use codeint_model::scan::{analyze_parsed_manifests, manifest_root_excluding};
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

pub(crate) fn analyze_manifests(manifests: &[Manifest]) -> ManifestAnalysis {
    analyze_parsed_manifests(
        manifests,
        CARGO_MANIFEST,
        |manifest| {
            let root = cargo_manifest_root(&manifest.path)?;
            Some(
                toml::from_str::<toml::Value>(&manifest.text)
                    .map(|doc| (root.to_owned(), doc))
                    .map_err(|error| WorkspaceError {
                        path: manifest.path.clone(),
                        message: format!("is not valid TOML: {error}"),
                    }),
            )
        },
        |all, _| workspace_packages(all),
        |doc| !is_virtual_cargo_workspace(doc),
        crate::dependencies::extract_declared_from_docs,
    )
}

pub(crate) fn cargo_manifest_root(path: &str) -> Option<&str> {
    manifest_root_excluding(path, CARGO_MANIFEST, &["target"])
}

fn is_virtual_cargo_workspace(doc: &toml::Value) -> bool {
    doc.get("workspace").is_some() && doc.get("package").is_none()
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

pub(crate) fn workspace_members_under(
    ws_root: &str,
    ws_text: &str,
    manifest_roots: &[String],
) -> Vec<String> {
    let Some((included, excluded)) = compile_workspace_globs(ws_text) else {
        return Vec::new();
    };
    let mut members = BTreeSet::new();
    for root in manifest_roots {
        if let Some(rel) = relative_member_root(ws_root, root)
            && included.is_match(rel)
            && !excluded.is_match(rel)
        {
            members.insert(root.clone());
        }
    }
    members.into_iter().collect()
}

fn compile_workspace_globs(ws_text: &str) -> Option<(GlobSet, GlobSet)> {
    let parsed: RootCargoManifest = toml::from_str(ws_text).ok()?;
    let workspace = parsed.workspace?;
    if workspace.members.is_empty() {
        return None;
    }
    let included = build_glob_set(&workspace.members, "members").ok()?;
    let excluded = build_glob_set(&workspace.exclude, "exclude").ok()?;
    Some((included, excluded))
}

fn relative_member_root<'a>(ws_root: &str, root: &'a str) -> Option<&'a str> {
    if ws_root.is_empty() {
        return (!root.is_empty()).then_some(root);
    }
    let rest = root.strip_prefix(ws_root)?.strip_prefix('/')?;
    (!rest.is_empty()).then_some(rest)
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
