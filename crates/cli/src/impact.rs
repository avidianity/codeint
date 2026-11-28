use std::collections::HashSet;
use std::path::{Component, Path, PathBuf};

use codeint_config::PluginHostConfig;
use codeint_git::Repository;
use codeint_metrics::{ImpactAnalysis, ImpactError};
use codeint_model::{CodeModel, RepoPath};
use codeint_report::{ChangedBase, ImpactReport};

use crate::check::{CheckError, WorkspaceContext, WorkspaceRef, select_workspace_package};

#[derive(Debug, thiserror::Error)]
pub(crate) enum ImpactCommandError {
    #[error(transparent)]
    Check(#[from] CheckError),
    #[error(transparent)]
    Analysis(#[from] ImpactError),
    #[error("provide at least one target file path or pass --changed <base>")]
    MissingTargets,
    #[error("{path:?} is outside the Git repository")]
    PathOutsideRepository { path: String },
    #[error("{path:?} is not a valid repository file path")]
    InvalidTargetPath { path: String },
}

pub(crate) struct ImpactRequest<'a> {
    pub(crate) paths: &'a [String],
    pub(crate) changed: Option<&'a str>,
    pub(crate) depth: Option<usize>,
    pub(crate) package: Option<&'a str>,
    pub(crate) plugin_host: &'a PluginHostConfig,
}

pub(crate) fn impact(
    folder: &Path,
    request: &ImpactRequest<'_>,
) -> Result<ImpactReport, ImpactCommandError> {
    if request.paths.is_empty() && request.changed.is_none() {
        return Err(ImpactCommandError::MissingTargets);
    }
    let context = WorkspaceContext::load(folder, request.plugin_host)?;
    impact_with_workspace(folder, context.as_ref(), request)
}

pub(crate) fn impact_with_workspace(
    folder: &Path,
    workspace: WorkspaceRef<'_>,
    request: &ImpactRequest<'_>,
) -> Result<ImpactReport, ImpactCommandError> {
    if request.paths.is_empty() && request.changed.is_none() {
        return Err(ImpactCommandError::MissingTargets);
    }
    let model = workspace.measure_working_tree_model()?;
    let selected_package = request
        .package
        .map(|requested| select_workspace_package(&model.workspace, requested))
        .transpose()?;
    let (target_paths, changed_base) = collect_targets(
        folder,
        workspace.repository,
        &model,
        request.paths,
        request.changed,
    )?;
    let mut analysis = ImpactAnalysis::compute(&model, &target_paths, request.depth)?;
    if let Some(selected) = selected_package {
        analysis = analysis.scoped_to_package(selected.as_str());
    }
    Ok(ImpactReport::from_analysis(analysis, changed_base))
}

fn collect_targets(
    folder: &Path,
    repository: &Repository,
    model: &CodeModel,
    raw_paths: &[String],
    changed_ref: Option<&str>,
) -> Result<(Vec<RepoPath>, Option<ChangedBase>), ImpactCommandError> {
    let mut target_paths = Vec::with_capacity(raw_paths.len());
    for raw in raw_paths {
        target_paths.push(resolve_target_path(folder, repository.root(), raw)?);
    }
    let changed_base = match changed_ref {
        Some(base_ref) => Some(collect_changed_targets(
            repository,
            model,
            base_ref,
            &mut target_paths,
        )?),
        None => None,
    };
    Ok((target_paths, changed_base))
}

fn collect_changed_targets(
    repository: &Repository,
    model: &CodeModel,
    base_ref: &str,
    target_paths: &mut Vec<RepoPath>,
) -> Result<ChangedBase, ImpactCommandError> {
    let changed = repository
        .changed_files_since(base_ref)
        .map_err(CheckError::Git)?;
    let measured_paths: HashSet<&RepoPath> = model.files.iter().map(|file| &file.path).collect();
    for path in changed.files {
        if measured_paths.contains(&path) {
            target_paths.push(path);
        }
    }
    Ok(ChangedBase {
        git_ref: base_ref.to_owned(),
        merge_base: changed.merge_base.to_string(),
    })
}

fn resolve_target_path(
    folder: &Path,
    repo_root: &Path,
    raw: &str,
) -> Result<RepoPath, ImpactCommandError> {
    if raw.is_empty() {
        return Err(ImpactCommandError::InvalidTargetPath {
            path: raw.to_owned(),
        });
    }
    let base_folder = std::fs::canonicalize(folder).unwrap_or_else(|_| folder.to_path_buf());
    let root = std::fs::canonicalize(repo_root).unwrap_or_else(|_| repo_root.to_path_buf());
    let candidate = Path::new(raw);
    let combined = if candidate.is_absolute() {
        candidate.to_path_buf()
    } else {
        base_folder.join(candidate)
    };
    let normalized = normalize_lexical(&combined);
    let relative =
        normalized
            .strip_prefix(&root)
            .map_err(|_| ImpactCommandError::PathOutsideRepository {
                path: raw.to_owned(),
            })?;
    let mut segments = Vec::new();
    for component in relative.components() {
        match component {
            Component::Normal(part) => {
                let text = part
                    .to_str()
                    .ok_or_else(|| ImpactCommandError::InvalidTargetPath {
                        path: raw.to_owned(),
                    })?;
                segments.push(text);
            }
            _ => {
                return Err(ImpactCommandError::InvalidTargetPath {
                    path: raw.to_owned(),
                });
            }
        }
    }
    if segments.is_empty() {
        return Err(ImpactCommandError::InvalidTargetPath {
            path: raw.to_owned(),
        });
    }
    RepoPath::new(segments.join("/")).map_err(|_| ImpactCommandError::InvalidTargetPath {
        path: raw.to_owned(),
    })
}

fn normalize_lexical(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                if !normalized.pop() {
                    normalized.push(component.as_os_str());
                }
            }
            other => normalized.push(other.as_os_str()),
        }
    }
    normalized
}
