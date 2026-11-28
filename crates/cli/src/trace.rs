use std::path::Path;

use codeint_config::PluginHostConfig;
use codeint_model::{CodeModel, RepoPath};
use codeint_report::{TraceError, TraceReport};

use crate::check::{CheckError, WorkspaceContext, WorkspaceRef};

#[derive(Debug, thiserror::Error)]
pub(crate) enum TraceCommandError {
    #[error(transparent)]
    Check(#[from] CheckError),
    #[error("file {path:?} is not in the measured Code model")]
    UnknownModule { path: String },
    #[error(transparent)]
    Trace(#[from] TraceError),
}

pub(crate) fn trace(
    folder: &Path,
    from: &str,
    to: &str,
    all: bool,
    max_paths: usize,
    package: Option<&str>,
    plugin_host: &PluginHostConfig,
) -> Result<TraceReport, TraceCommandError> {
    let context = WorkspaceContext::load(folder, plugin_host)?;
    trace_with_workspace(folder, context.as_ref(), from, to, all, max_paths, package)
}

pub(crate) fn trace_with_workspace(
    folder: &Path,
    workspace: WorkspaceRef<'_>,
    from: &str,
    to: &str,
    all: bool,
    max_paths: usize,
    package: Option<&str>,
) -> Result<TraceReport, TraceCommandError> {
    let model = workspace.measure_working_tree_model_for_package(package)?;
    let from_path = resolve_model_path(from, folder, workspace.repository.root(), &model)?;
    let to_path = resolve_model_path(to, folder, workspace.repository.root(), &model)?;
    Ok(TraceReport::trace(
        &model, &from_path, &to_path, all, max_paths,
    )?)
}

fn resolve_model_path(
    raw: &str,
    folder: &Path,
    repository_root: &Path,
    model: &CodeModel,
) -> Result<RepoPath, TraceCommandError> {
    let normalized = normalize_relative_str(raw);
    if let Ok(candidate) = RepoPath::new(normalized)
        && model.files.iter().any(|file| file.path == candidate)
    {
        return Ok(candidate);
    }
    if let (Ok(canonical_root), Ok(canonical_target)) = (
        std::fs::canonicalize(repository_root),
        std::fs::canonicalize(folder.join(raw)),
    ) && let Ok(relative) = canonical_target.strip_prefix(&canonical_root)
    {
        let joined = relative
            .components()
            .filter_map(|component| component.as_os_str().to_str())
            .collect::<Vec<_>>()
            .join("/");
        if let Ok(candidate) = RepoPath::new(joined)
            && model.files.iter().any(|file| file.path == candidate)
        {
            return Ok(candidate);
        }
    }
    Err(TraceCommandError::UnknownModule {
        path: raw.to_owned(),
    })
}

fn normalize_relative_str(raw: &str) -> String {
    let mut normalized = raw.replace('\\', "/");
    while let Some(stripped) = normalized.strip_prefix("./") {
        normalized = stripped.to_owned();
    }
    normalized
}
