#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};

use codeint_model::{InvalidRepoPath, RepoPath};

#[derive(Debug, thiserror::Error)]
pub enum GitError {
    #[error("{} is not inside a Git repository with a working tree", start.display())]
    NoWorkingTree {
        start: PathBuf,
        #[source]
        source: Option<gix::Error>,
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
    pub fn discover(_start: &Path) -> Result<WorkingTree, GitError> {
        todo!()
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
