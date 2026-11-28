use std::collections::HashSet;

use codeint_model::language::{
    DependencyManifestOwner, Manifest, ManifestAnalysis, WorkspaceError,
};
use globset::{GlobBuilder, GlobSet, GlobSetBuilder};
use serde_json::Value;

mod pnpm;
#[cfg(test)]
mod tests;

pub(crate) use pnpm::PNPM_WORKSPACE_MANIFEST;

pub(crate) const PACKAGE_MANIFEST: &str = "package.json";

const WRONG_SHAPE: &str = "the workspaces field must be an array of folder patterns, or an object whose packages field is one";

pub(crate) fn analyze_manifests(manifests: &[Manifest]) -> ManifestAnalysis {
    let mut invalid_manifests = Vec::new();
    let mut active_parsed = parse_package_manifests(manifests, &mut invalid_manifests);
    let workspace_packages = discover_workspace_packages_single_pass(
        manifests,
        &mut active_parsed,
        &mut invalid_manifests,
    );
    let dependency_manifests: Vec<DependencyManifestOwner> = active_parsed
        .iter()
        .map(|(root, path, _)| DependencyManifestOwner {
            package_root: (*root).to_owned(),
            path: (*path).to_owned(),
        })
        .collect();
    let declared_dependencies = crate::dependencies::extract_declared_from_active(&active_parsed);
    let manifest_used_dependencies = crate::package_scripts::extract_manifest_used_dependencies(
        &active_parsed,
        &workspace_packages,
        &declared_dependencies,
    );
    let root_dependency_inheritance =
        collect_root_inheritance(&workspace_packages, &dependency_manifests);
    ManifestAnalysis {
        workspace_packages,
        invalid_manifests,
        dependency_manifests,
        declared_dependencies,
        manifest_used_dependencies,
        root_dependency_inheritance,
    }
}

fn parse_package_manifests<'a>(
    manifests: &'a [Manifest],
    invalid_manifests: &mut Vec<WorkspaceError>,
) -> Vec<(&'a str, &'a str, Value)> {
    let mut parsed = Vec::new();
    for manifest in manifests {
        let Some(root) = package_manifest_root(&manifest.path) else {
            continue;
        };
        match serde_json::from_str::<Value>(&manifest.text) {
            Ok(doc) => parsed.push((root, manifest.path.as_str(), doc)),
            Err(error) => {
                invalid_manifests.push(WorkspaceError {
                    path: manifest.path.clone(),
                    message: format!("is not valid JSON: {error}"),
                });
            }
        }
    }
    parsed
}

fn discover_workspace_packages_single_pass<'a>(
    manifests: &'a [Manifest],
    active_parsed: &mut Vec<(&'a str, &'a str, Value)>,
    invalid_manifests: &mut Vec<WorkspaceError>,
) -> Vec<String> {
    if let Some(pnpm_manifest) = manifests
        .iter()
        .find(|manifest| manifest.path == PNPM_WORKSPACE_MANIFEST)
    {
        match pnpm::pnpm_package_patterns(&pnpm_manifest.text)
            .map_err(|message| WorkspaceError {
                path: PNPM_WORKSPACE_MANIFEST.to_owned(),
                message,
            })
            .and_then(|patterns| pnpm::pnpm_workspace_packages(manifests, &patterns))
        {
            Ok(packages) => return packages,
            Err(error) => invalid_manifests.push(error),
        }
    }
    let Some((_, _, root_doc)) = active_parsed
        .iter()
        .find(|(_, path, _)| *path == PACKAGE_MANIFEST)
    else {
        return Vec::new();
    };
    match package_json_workspace_packages_from_doc(root_doc, manifests) {
        Ok(packages) => packages,
        Err(error) => {
            invalid_manifests.push(error);
            active_parsed.retain(|(_, path, _)| *path != PACKAGE_MANIFEST);
            Vec::new()
        }
    }
}

fn collect_root_inheritance(
    workspace_packages: &[String],
    dependency_manifests: &[DependencyManifestOwner],
) -> Vec<String> {
    let manifest_roots: HashSet<&str> = dependency_manifests
        .iter()
        .map(|owner| owner.package_root.as_str())
        .collect();
    if !manifest_roots.contains("") {
        return Vec::new();
    }
    workspace_packages
        .iter()
        .filter(|root| !root.is_empty() && manifest_roots.contains(root.as_str()))
        .cloned()
        .collect()
}

pub(crate) fn package_manifest_root(path: &str) -> Option<&str> {
    codeint_model::scan::manifest_root_excluding(
        path,
        PACKAGE_MANIFEST,
        &["node_modules", "vendor"],
    )
}

pub(crate) fn workspace_packages(manifests: &[Manifest]) -> Result<Vec<String>, WorkspaceError> {
    let Some(pnpm_workspace_manifest) = manifests
        .iter()
        .find(|manifest| manifest.path == PNPM_WORKSPACE_MANIFEST)
    else {
        return package_json_workspace_packages(manifests);
    };
    let patterns =
        pnpm::pnpm_package_patterns(&pnpm_workspace_manifest.text).map_err(|message| {
            WorkspaceError {
                path: PNPM_WORKSPACE_MANIFEST.to_owned(),
                message,
            }
        })?;
    pnpm::pnpm_workspace_packages(manifests, &patterns)
}

fn package_json_workspace_packages(manifests: &[Manifest]) -> Result<Vec<String>, WorkspaceError> {
    let Some(root) = manifests
        .iter()
        .find(|manifest| manifest.path == PACKAGE_MANIFEST)
    else {
        return Ok(Vec::new());
    };
    let root_manifest: Value = serde_json::from_str(&root.text)
        .map_err(|error| root_manifest_error(format!("is not valid JSON: {error}")))?;
    package_json_workspace_packages_from_doc(&root_manifest, manifests)
}

fn package_json_workspace_packages_from_doc(
    root_manifest: &Value,
    manifests: &[Manifest],
) -> Result<Vec<String>, WorkspaceError> {
    let (excluded, included): (Vec<FolderPattern>, Vec<FolderPattern>) =
        workspace_patterns(root_manifest)?
            .into_iter()
            .map(FolderPattern::parse)
            .partition(|pattern| pattern.excludes);
    let included = glob_set(&included)?;
    let excluded = glob_set(&excluded)?;
    Ok(manifests
        .iter()
        .filter_map(|manifest| manifest.path.strip_suffix(&format!("/{PACKAGE_MANIFEST}")))
        .filter(|folder| !folder.split('/').any(|segment| segment == "node_modules"))
        .filter(|folder| included.is_match(folder) && !excluded.is_match(folder))
        .map(str::to_owned)
        .collect())
}

fn workspace_patterns(root_manifest: &Value) -> Result<Vec<&str>, WorkspaceError> {
    let patterns = match root_manifest.get("workspaces") {
        None => return Ok(Vec::new()),
        Some(Value::Object(fields)) => fields.get("packages"),
        workspaces => workspaces,
    };
    match patterns {
        None => Ok(Vec::new()),
        Some(Value::Array(patterns)) => patterns
            .iter()
            .map(|pattern| {
                pattern
                    .as_str()
                    .ok_or_else(|| root_manifest_error(WRONG_SHAPE.to_owned()))
            })
            .collect(),
        Some(_) => Err(root_manifest_error(WRONG_SHAPE.to_owned())),
    }
}

struct FolderPattern<'manifest> {
    written: &'manifest str,
    folders: &'manifest str,
    excludes: bool,
}

impl<'manifest> FolderPattern<'manifest> {
    fn parse(written: &'manifest str) -> FolderPattern<'manifest> {
        let (excludes, folders) = match written.strip_prefix('!') {
            Some(folders) => (true, folders),
            None => (false, written),
        };
        let folders = folders.strip_prefix("./").unwrap_or(folders);
        FolderPattern {
            written,
            folders: folders.trim_end_matches('/'),
            excludes,
        }
    }
}

fn glob_set(patterns: &[FolderPattern<'_>]) -> Result<GlobSet, WorkspaceError> {
    let mut builder = GlobSetBuilder::new();
    for pattern in patterns {
        builder.add(
            GlobBuilder::new(pattern.folders)
                .literal_separator(true)
                .build()
                .map_err(|error| {
                    root_manifest_error(format!(
                        "has the workspace pattern {:?}, which is not a valid glob: {error}",
                        pattern.written
                    ))
                })?,
        );
    }
    builder.build().map_err(|error| {
        root_manifest_error(format!(
            "has workspace patterns that cannot be combined: {error}"
        ))
    })
}

fn root_manifest_error(message: String) -> WorkspaceError {
    WorkspaceError {
        path: PACKAGE_MANIFEST.to_owned(),
        message,
    }
}
