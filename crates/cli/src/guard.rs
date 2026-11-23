use std::path::{Component, Path, PathBuf};

use codeint_config::{ConfigurationFileError, read_configuration};
use codeint_git::{GitError, Repository};
use codeint_model::{InvalidRepoPath, RepoPath};
use codeint_report::GuardReport;

#[derive(Debug, thiserror::Error)]
pub(crate) enum GuardError {
    #[error(transparent)]
    Git(#[from] GitError),
    #[error(transparent)]
    Configuration(#[from] ConfigurationFileError),
    #[error("{} is outside the Git repository at {}", path.display(), root.display())]
    OutsideRepository { path: PathBuf, root: PathBuf },
    #[error("{} is not a valid UTF-8 repository path", path.display())]
    NonUtf8Path { path: PathBuf },
    #[error("{} is not a valid repository path: {source}", path.display())]
    InvalidPath {
        path: PathBuf,
        #[source]
        source: InvalidRepoPath,
    },
}

pub(crate) fn guard(folder: &Path, target: &Path) -> Result<GuardReport, GuardError> {
    let absolute_target = normalize_lexically(&if target.is_absolute() {
        target.to_path_buf()
    } else {
        folder.join(target)
    });
    let discovery_start = nearest_existing_ancestor(&absolute_target).unwrap_or(folder);
    let repository = match Repository::discover(discovery_start) {
        Ok(repo) => repo,
        Err(_) if discovery_start != folder => Repository::discover(folder)?,
        Err(err) => return Err(GuardError::Git(err)),
    };

    let canonical_root = std::fs::canonicalize(repository.root())
        .unwrap_or_else(|_| repository.root().to_path_buf());
    let resolved_target = canonicalize_with_tail(&absolute_target);
    let relative = resolved_target
        .strip_prefix(&canonical_root)
        .or_else(|_| resolved_target.strip_prefix(repository.root()))
        .map_err(|_| GuardError::OutsideRepository {
            path: target.to_path_buf(),
            root: repository.root().to_path_buf(),
        })?;

    let repo_path = to_repo_path(relative, target)?;
    let exists = resolved_target.exists();
    let configuration = read_configuration(repository.root())?;
    Ok(GuardReport::evaluate(
        &repo_path,
        exists,
        configuration.boundaries.as_ref(),
    ))
}

fn nearest_existing_ancestor(path: &Path) -> Option<&Path> {
    let mut cursor = Some(path);
    while let Some(candidate) = cursor {
        if candidate.exists() {
            if candidate.is_dir() {
                return Some(candidate);
            }
            return candidate.parent();
        }
        cursor = candidate.parent();
    }
    None
}

fn canonicalize_with_tail(path: &Path) -> PathBuf {
    let mut missing = Vec::new();
    let mut cursor = path;
    loop {
        if cursor.exists() {
            let mut base = std::fs::canonicalize(cursor).unwrap_or_else(|_| cursor.to_path_buf());
            for segment in missing.into_iter().rev() {
                base.push(segment);
            }
            return base;
        }
        match (cursor.parent(), cursor.file_name()) {
            (Some(parent), Some(name)) => {
                missing.push(name.to_os_string());
                cursor = parent;
            }
            _ => return path.to_path_buf(),
        }
    }
}

fn normalize_lexically(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            other => normalized.push(other.as_os_str()),
        }
    }
    normalized
}

fn to_repo_path(relative: &Path, original: &Path) -> Result<RepoPath, GuardError> {
    let mut segments = Vec::new();
    for component in relative.components() {
        match component {
            Component::Normal(part) => {
                let text = part.to_str().ok_or_else(|| GuardError::NonUtf8Path {
                    path: original.to_path_buf(),
                })?;
                segments.push(text);
            }
            Component::CurDir => {}
            _ => {
                return Err(GuardError::NonUtf8Path {
                    path: original.to_path_buf(),
                });
            }
        }
    }
    let joined = segments.join("/");
    RepoPath::new(&joined).map_err(|source| GuardError::InvalidPath {
        path: original.to_path_buf(),
        source,
    })
}
