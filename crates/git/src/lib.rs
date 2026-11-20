#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

use codeint_model::{InvalidRepoPath, RepoPath};
use gix::dir::entry::{Kind, Status};
use gix::dir::walk::EmissionMode;
use gix::dir::walk::delegate::Collect;

#[derive(Debug, thiserror::Error)]
pub enum GitError {
    #[error("{} is not inside a Git repository", start.display())]
    NotInRepository { start: PathBuf },
    #[error("{} is inside a bare Git repository, which has no working tree", start.display())]
    BareRepository { start: PathBuf },
    #[error("cannot open the Git repository that contains {}", start.display())]
    Open {
        start: PathBuf,
        #[source]
        source: gix::Error,
    },
    #[error("cannot list the files of the working tree")]
    Walk(#[source] gix::Error),
    #[error("a working tree path is not valid UTF-8: {path}")]
    NonUtf8Path { path: String },
    #[error(transparent)]
    InvalidPath(#[from] InvalidRepoPath),
    #[error("cannot read {path}")]
    Read {
        path: RepoPath,
        #[source]
        source: std::io::Error,
    },
}

pub struct WorkingTree {
    root: PathBuf,
    files: Vec<RepoPath>,
}

impl WorkingTree {
    pub fn discover(start: &Path) -> Result<WorkingTree, GitError> {
        let repository = gix::discover(start).map_err(|source| {
            if source.is_not_found() {
                GitError::NotInRepository {
                    start: start.to_path_buf(),
                }
            } else {
                GitError::Open {
                    start: start.to_path_buf(),
                    source,
                }
            }
        })?;
        let root = repository
            .workdir()
            .ok_or_else(|| GitError::BareRepository {
                start: start.to_path_buf(),
            })?
            .to_path_buf();
        let files = list_measurable_files(&repository)?;
        Ok(WorkingTree { root, files })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn files(&self) -> &[RepoPath] {
        &self.files
    }

    pub fn read(&self, path: &RepoPath) -> Result<Vec<u8>, GitError> {
        std::fs::read(self.root.join(path.as_str())).map_err(|source| GitError::Read {
            path: path.clone(),
            source,
        })
    }
}

fn list_measurable_files(repository: &gix::Repository) -> Result<Vec<RepoPath>, GitError> {
    let index = repository.index_or_empty().map_err(GitError::Walk)?;
    let options = repository
        .dirwalk_options()
        .map_err(GitError::Walk)?
        .emit_tracked(true)
        .emit_untracked(EmissionMode::Matching)
        .emit_ignored(None)
        .recurse_repositories(false);
    let mut collect = Collect::default();
    repository
        .dirwalk(
            &index,
            None::<&str>,
            &AtomicBool::new(false),
            options,
            &mut collect,
        )
        .map_err(GitError::Walk)?;
    collect
        .into_entries_by_path()
        .into_iter()
        .map(|(entry, _)| entry)
        .filter(|entry| {
            matches!(entry.status, Status::Tracked | Status::Untracked)
                && entry.disk_kind == Some(Kind::File)
        })
        .map(|entry| {
            let path = String::from_utf8(entry.rela_path.into()).map_err(|error| {
                GitError::NonUtf8Path {
                    path: String::from_utf8_lossy(error.as_bytes()).into_owned(),
                }
            })?;
            Ok(RepoPath::new(path)?)
        })
        .collect()
}
