use std::collections::HashSet;
use std::path::{Component, Path, PathBuf};

use codeint_config::{PluginHostConfig, read_configuration};
use codeint_git::Repository;
use codeint_metrics::{ImpactAnalysis, ImpactError};
use codeint_model::{CodeModel, RepoPath};
use codeint_report::{ChangedBase, ImpactReport};

use crate::check::{CheckError, LoadedPlugins, measure_with};

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
    pub(crate) plugin_host: &'a PluginHostConfig,
}

pub(crate) fn impact(
    folder: &Path,
    request: &ImpactRequest<'_>,
) -> Result<ImpactReport, ImpactCommandError> {
    if request.paths.is_empty() && request.changed.is_none() {
        return Err(ImpactCommandError::MissingTargets);
    }
    let repository = Repository::discover(folder).map_err(CheckError::Git)?;
    let configuration = read_configuration(repository.root()).map_err(CheckError::Configuration)?;
    let loaded = LoadedPlugins::load(repository.root(), &configuration, request.plugin_host)?;
    let plugins = loaded.as_named();
    let tree = repository.working_tree().map_err(CheckError::Git)?;
    let languages = measure_with(&tree, &plugins, &configuration)?;
    let model = CodeModel::merge(languages.into_iter().map(|language| language.model));

    let mut target_paths = Vec::new();
    for raw in request.paths {
        target_paths.push(resolve_target_path(folder, repository.root(), raw)?);
    }

    let changed_base = if let Some(base_ref) = request.changed {
        let changed = repository
            .changed_files_since(base_ref)
            .map_err(CheckError::Git)?;
        let measured_paths: HashSet<&RepoPath> =
            model.files.iter().map(|file| &file.path).collect();
        for path in changed.files {
            if measured_paths.contains(&path) {
                target_paths.push(path);
            }
        }
        Some(ChangedBase {
            git_ref: base_ref.to_owned(),
            merge_base: changed.merge_base.to_string(),
        })
    } else {
        None
    };

    let analysis = ImpactAnalysis::compute(&model, &target_paths, request.depth)?;
    Ok(ImpactReport::from_analysis(analysis, changed_base))
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
