#![forbid(unsafe_code)]

use std::collections::HashMap;
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
    #[error("the Git ref {name:?} does not name a commit in this repository")]
    UnknownRef {
        name: String,
        #[source]
        source: gix::Error,
    },
    #[error("HEAD does not name a commit yet, so it has no merge-base with {base_ref:?}")]
    NoHeadCommit {
        base_ref: String,
        #[source]
        source: gix::Error,
    },
    #[error("{base_ref:?} and HEAD have no commit in common, so they have no merge-base")]
    NoMergeBase { base_ref: String },
    #[error("cannot find the merge-base of {base_ref:?} and HEAD")]
    MergeBase {
        base_ref: String,
        #[source]
        source: gix::Error,
    },
    #[error("cannot list the files of commit {commit}")]
    CommitTree {
        commit: CommitId,
        #[source]
        source: gix::Error,
    },
    #[error("cannot read {path} from commit {commit}")]
    ReadBlob {
        path: RepoPath,
        commit: CommitId,
        #[source]
        source: gix::Error,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CommitId(gix::ObjectId);

pub struct Repository {
    repository: gix::Repository,
    root: PathBuf,
}

pub struct Snapshot {
    files: Vec<RepoPath>,
    submodules: Vec<RepoPath>,
    contents: Contents,
}

enum Contents {
    WorkingTree {
        root: PathBuf,
    },
    Commit {
        repository: gix::Repository,
        commit: CommitId,
        blobs: HashMap<RepoPath, gix::ObjectId>,
    },
}

impl std::fmt::Display for CommitId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}", self.0)
    }
}

impl Repository {
    pub fn discover(start: &Path) -> Result<Repository, GitError> {
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
        Ok(Repository { repository, root })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn working_tree(&self) -> Result<Snapshot, GitError> {
        let Listing { files, submodules } = list_working_tree(&self.repository)?;
        Ok(Snapshot {
            files,
            submodules,
            contents: Contents::WorkingTree {
                root: self.root.clone(),
            },
        })
    }

    pub fn merge_base_with_head(&self, base_ref: &str) -> Result<CommitId, GitError> {
        let _ = base_ref;
        todo!()
    }

    pub fn commit(&self, commit: CommitId) -> Result<Snapshot, GitError> {
        let (Listing { files, submodules }, blobs) = list_commit(&self.repository, commit)?;
        Ok(Snapshot {
            files,
            submodules,
            contents: Contents::Commit {
                repository: self.repository.clone(),
                commit,
                blobs,
            },
        })
    }
}

impl Snapshot {
    pub fn files(&self) -> &[RepoPath] {
        &self.files
    }

    pub fn submodules(&self) -> &[RepoPath] {
        &self.submodules
    }

    pub fn read(&self, path: &RepoPath) -> Result<Vec<u8>, GitError> {
        match &self.contents {
            Contents::WorkingTree { root } => {
                std::fs::read(root.join(path.as_str())).map_err(|source| GitError::Read {
                    path: path.clone(),
                    source,
                })
            }
            Contents::Commit {
                repository,
                commit,
                blobs,
            } => {
                let _ = (repository, commit, blobs);
                todo!()
            }
        }
    }
}

struct Listing {
    files: Vec<RepoPath>,
    submodules: Vec<RepoPath>,
}

fn list_working_tree(repository: &gix::Repository) -> Result<Listing, GitError> {
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
    let mut listing = Listing {
        files: Vec::new(),
        submodules: Vec::new(),
    };
    for (entry, _) in collect.into_entries_by_path() {
        let is_submodule = entry.index_kind == Some(Kind::Repository);
        let is_measurable_file = matches!(entry.status, Status::Tracked | Status::Untracked)
            && entry.disk_kind == Some(Kind::File);
        if is_submodule {
            listing.submodules.push(repo_path(entry.rela_path)?);
        } else if is_measurable_file {
            listing.files.push(repo_path(entry.rela_path)?);
        }
    }
    Ok(listing)
}

fn list_commit(
    repository: &gix::Repository,
    commit: CommitId,
) -> Result<(Listing, HashMap<RepoPath, gix::ObjectId>), GitError> {
    let _ = (repository, commit, repo_path);
    todo!()
}

fn repo_path(rela_path: gix::bstr::BString) -> Result<RepoPath, GitError> {
    let path = String::from_utf8(rela_path.into()).map_err(|error| GitError::NonUtf8Path {
        path: String::from_utf8_lossy(error.as_bytes()).into_owned(),
    })?;
    Ok(RepoPath::new(path)?)
}
