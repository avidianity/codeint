use codeint_model::language::{Manifest, WorkspaceError};
use globset::{GlobBuilder, GlobSet, GlobSetBuilder};
use serde_json::Value;

mod pnpm;
#[cfg(test)]
mod tests;

pub(crate) use pnpm::{PNPM_WORKSPACE_MANIFEST, ReadPnpmPackagePatterns, pnpm_package_patterns};

pub(crate) const PACKAGE_MANIFEST: &str = "package.json";

const WRONG_SHAPE: &str = "the workspaces field must be an array of folder patterns, or an object whose packages field is one";

pub(crate) fn workspace_packages(
    manifests: Vec<Manifest>,
    read_pnpm_package_patterns: ReadPnpmPackagePatterns,
) -> Result<Vec<String>, WorkspaceError> {
    let Some(pnpm_workspace_manifest) = manifests
        .iter()
        .find(|manifest| manifest.path == PNPM_WORKSPACE_MANIFEST)
    else {
        return package_json_workspace_packages(manifests);
    };
    let patterns =
        read_pnpm_package_patterns(&pnpm_workspace_manifest.text).map_err(|message| {
            WorkspaceError {
                path: PNPM_WORKSPACE_MANIFEST.to_owned(),
                message,
            }
        })?;
    pnpm::pnpm_workspace_packages(&manifests, &patterns)
}

fn package_json_workspace_packages(
    manifests: Vec<Manifest>,
) -> Result<Vec<String>, WorkspaceError> {
    let Some(root) = manifests
        .iter()
        .find(|manifest| manifest.path == PACKAGE_MANIFEST)
    else {
        return Ok(Vec::new());
    };
    let root_manifest: Value = serde_json::from_str(&root.text)
        .map_err(|error| root_manifest_error(format!("is not valid JSON: {error}")))?;
    let (excluded, included): (Vec<FolderPattern>, Vec<FolderPattern>) =
        workspace_patterns(&root_manifest)?
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
