#![forbid(unsafe_code)]

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

use codeint_model::{InvalidRepoPath, RepoPath};
use gix::dir::entry::{Kind, Status};
use gix::dir::walk::EmissionMode;
use gix::dir::walk::delegate::Collect;
use gix::index::entry::{Flags, Mode, Stage, Stat, stat::Options as StatOptions};
use gix::index::fs::Metadata as IndexMetadata;

mod churn;
mod history;
mod result_cache;
#[cfg(test)]
mod tests;

pub use churn::{ChurnRange, CoChangeSummary, FileChurnSummary};
pub use history::{CommitRange, HistoryCommit};
pub use result_cache::{ResultCache, ResultCacheError};

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
    NonUtf8Path {
        path: String,
        #[source]
        source: std::string::FromUtf8Error,
    },
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
    #[error("{base_ref:?} and {head_ref:?} have no commit in common, so they have no merge-base")]
    NoMergeBase { base_ref: String, head_ref: String },
    #[error("cannot find the merge-base of {base_ref:?} and {head_ref:?}")]
    MergeBase {
        base_ref: String,
        head_ref: String,
        #[source]
        source: gix::Error,
    },
    #[error("HEAD does not name a commit yet, so the current branch has no history to measure")]
    NoHistory(#[source] gix::Error),
    #[error("cannot read commit {commit} of the history")]
    History {
        commit: CommitId,
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

const HEAD: &str = "HEAD";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CommitId(gix::ObjectId);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BlobId(gix::ObjectId);

pub struct Repository {
    repository: gix::Repository,
    root: PathBuf,
}

pub struct ChangedFiles {
    pub merge_base: CommitId,
    pub files: Vec<RepoPath>,
}

pub struct Snapshot {
    files: Vec<RepoPath>,
    submodules: Vec<RepoPath>,
    contents: Contents,
}

enum Contents {
    WorkingTree {
        root: PathBuf,
        index: gix::worktree::Index,
        stat_options: StatOptions,
    },
    Commit {
        repository: Box<gix::ThreadSafeRepository>,
        commit: CommitId,
        blobs: HashMap<RepoPath, gix::ObjectId>,
    },
}

#[derive(Clone, Copy)]
pub struct ParallelSnapshotReader<'snapshot> {
    source: ParallelSource<'snapshot>,
}

#[derive(Clone, Copy)]
enum ParallelSource<'snapshot> {
    WorkingTree {
        root: &'snapshot Path,
    },
    Commit {
        repository: &'snapshot gix::ThreadSafeRepository,
        commit: CommitId,
        blobs: &'snapshot HashMap<RepoPath, gix::ObjectId>,
    },
}

pub struct ThreadLocalSnapshotReader<'snapshot> {
    source: ThreadLocalSource<'snapshot>,
}

enum ThreadLocalSource<'snapshot> {
    WorkingTree {
        root: &'snapshot Path,
    },
    Commit {
        repository: Box<gix::Repository>,
        commit: CommitId,
        blobs: &'snapshot HashMap<RepoPath, gix::ObjectId>,
    },
}

impl std::fmt::Display for CommitId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}", self.0)
    }
}

impl Repository {
    pub fn init(folder: &Path) -> Result<Repository, GitError> {
        gix::init(folder).map_err(|source| GitError::Open {
            start: folder.to_path_buf(),
            source,
        })?;
        Repository::discover(folder)
    }

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
        let (Listing { files, submodules }, index) = list_working_tree(&self.repository)?;
        let stat_options = self.repository.stat_options().map_err(GitError::Walk)?;
        Ok(Snapshot {
            files,
            submodules,
            contents: Contents::WorkingTree {
                root: self.root.clone(),
                index,
                stat_options,
            },
        })
    }

    pub fn resolve(&self, git_ref: &str) -> Result<CommitId, GitError> {
        self.repository
            .rev_parse_single(git_ref)
            .and_then(|id| id.object()?.peel_to_commit())
            .map(|commit| CommitId(commit.id))
            .map_err(|source| GitError::UnknownRef {
                name: git_ref.to_owned(),
                source,
            })
    }

    pub fn merge_base_with_head(&self, base_ref: &str) -> Result<CommitId, GitError> {
        let base = self.resolve(base_ref)?;
        let head = self
            .repository
            .head_id()
            .map_err(|source| GitError::NoHeadCommit {
                base_ref: base_ref.to_owned(),
                source,
            })?;
        self.merge_base_of((base_ref, base), (HEAD, CommitId(head.detach())))
    }

    pub fn merge_base(&self, base_ref: &str, head_ref: &str) -> Result<CommitId, GitError> {
        let base = self.resolve(base_ref)?;
        let head = self.resolve(head_ref)?;
        self.merge_base_of((base_ref, base), (head_ref, head))
    }

    fn merge_base_of(
        &self,
        (base_ref, base): (&str, CommitId),
        (head_ref, head): (&str, CommitId),
    ) -> Result<CommitId, GitError> {
        let merge_base = self
            .repository
            .merge_base(base.0, head.0)
            .map_err(|source| GitError::MergeBase {
                base_ref: base_ref.to_owned(),
                head_ref: head_ref.to_owned(),
                source,
            })?
            .ok_or_else(|| GitError::NoMergeBase {
                base_ref: base_ref.to_owned(),
                head_ref: head_ref.to_owned(),
            })?;
        Ok(CommitId(merge_base.detach()))
    }

    pub fn commit(&self, commit: CommitId) -> Result<Snapshot, GitError> {
        let (Listing { files, submodules }, blobs) = list_commit(&self.repository, commit)?;
        Ok(Snapshot {
            files,
            submodules,
            contents: Contents::Commit {
                repository: Box::new(self.repository.clone().into_sync()),
                commit,
                blobs,
            },
        })
    }

    pub fn changed_files_since(&self, base_ref: &str) -> Result<ChangedFiles, GitError> {
        let merge_base = self.merge_base_with_head(base_ref)?;
        let base = self.commit(merge_base)?;
        let working = self.working_tree()?;
        let files = working.changed_files_from(&base)?;
        Ok(ChangedFiles { merge_base, files })
    }
}

impl Snapshot {
    pub fn files(&self) -> &[RepoPath] {
        &self.files
    }

    pub fn submodules(&self) -> &[RepoPath] {
        &self.submodules
    }

    pub fn changed_files_from(&self, base: &Snapshot) -> Result<Vec<RepoPath>, GitError> {
        let mut changed = Vec::new();
        for path in self.files() {
            if self.file_differs_from(base, path)? {
                changed.push(path.clone());
            }
        }
        changed.sort();
        Ok(changed)
    }

    fn file_differs_from(&self, base: &Snapshot, path: &RepoPath) -> Result<bool, GitError> {
        if !base.contains_file(path) {
            return Ok(true);
        }
        match (&self.contents, &base.contents) {
            (
                Contents::Commit {
                    blobs: head_blobs, ..
                },
                Contents::Commit {
                    blobs: base_blobs, ..
                },
            ) => Ok(head_blobs.get(path) != base_blobs.get(path)),
            (
                Contents::WorkingTree {
                    root,
                    index,
                    stat_options,
                },
                Contents::Commit {
                    blobs: base_blobs, ..
                },
            ) if working_tree_matches_base_blob(
                root,
                index,
                *stat_options,
                path,
                base_blobs.get(path),
            ) =>
            {
                Ok(false)
            }
            _ => Ok(self.read(path)? != base.read(path)?),
        }
    }

    fn contains_file(&self, path: &RepoPath) -> bool {
        match &self.contents {
            Contents::Commit { blobs, .. } => blobs.contains_key(path),
            Contents::WorkingTree { .. } => self.files.binary_search(path).is_ok(),
        }
    }

    pub fn blob_id(&self, path: &RepoPath) -> Option<BlobId> {
        match &self.contents {
            Contents::Commit { blobs, .. } => blobs.get(path).copied().map(BlobId),
            Contents::WorkingTree { .. } => None,
        }
    }

    pub fn parallel_reader(&self) -> ParallelSnapshotReader<'_> {
        let source = match &self.contents {
            Contents::WorkingTree { root, .. } => ParallelSource::WorkingTree { root },
            Contents::Commit {
                repository,
                commit,
                blobs,
            } => ParallelSource::Commit {
                repository,
                commit: *commit,
                blobs,
            },
        };
        ParallelSnapshotReader { source }
    }

    pub fn read(&self, path: &RepoPath) -> Result<Vec<u8>, GitError> {
        self.parallel_reader().thread_local().read(path)
    }
}

impl<'snapshot> ParallelSnapshotReader<'snapshot> {
    pub fn thread_local(self) -> ThreadLocalSnapshotReader<'snapshot> {
        let source = match self.source {
            ParallelSource::WorkingTree { root } => ThreadLocalSource::WorkingTree { root },
            ParallelSource::Commit {
                repository,
                commit,
                blobs,
            } => ThreadLocalSource::Commit {
                repository: Box::new(repository.to_thread_local()),
                commit,
                blobs,
            },
        };
        ThreadLocalSnapshotReader { source }
    }
}

impl ThreadLocalSnapshotReader<'_> {
    pub fn blob_id(&self, path: &RepoPath) -> Option<BlobId> {
        match &self.source {
            ThreadLocalSource::Commit { blobs, .. } => blobs.get(path).copied().map(BlobId),
            ThreadLocalSource::WorkingTree { .. } => None,
        }
    }

    pub fn read(&self, path: &RepoPath) -> Result<Vec<u8>, GitError> {
        match &self.source {
            ThreadLocalSource::WorkingTree { root } => std::fs::read(root.join(path.as_str()))
                .map_err(|source| GitError::Read {
                    path: path.clone(),
                    source,
                }),
            ThreadLocalSource::Commit {
                repository,
                commit,
                blobs,
            } => {
                let blob = blobs.get(path).ok_or_else(|| GitError::Read {
                    path: path.clone(),
                    source: std::io::ErrorKind::NotFound.into(),
                })?;
                repository
                    .find_blob(*blob)
                    .map(|mut blob| blob.take_data())
                    .map_err(|source| GitError::ReadBlob {
                        path: path.clone(),
                        commit: *commit,
                        source,
                    })
            }
        }
    }
}

struct Listing {
    files: Vec<RepoPath>,
    submodules: Vec<RepoPath>,
}

fn list_working_tree(
    repository: &gix::Repository,
) -> Result<(Listing, gix::worktree::Index), GitError> {
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
    Ok((listing, index))
}

fn working_tree_matches_base_blob(
    root: &Path,
    index: &gix::worktree::Index,
    stat_options: StatOptions,
    path: &RepoPath,
    base_blob: Option<&gix::ObjectId>,
) -> bool {
    let Some(&base_oid) = base_blob else {
        return false;
    };
    let Some(entry) = index.entry_by_path_and_stage(path.as_str().into(), Stage::Unconflicted)
    else {
        return false;
    };
    if entry.id != base_oid || !is_trustworthy_regular_file_entry(entry) {
        return false;
    }
    let racy_options = StatOptions {
        use_nsec: false,
        ..stat_options
    };
    if entry.stat.is_racy(index.timestamp(), racy_options) {
        return false;
    }
    let Ok(metadata) = IndexMetadata::from_path_no_follow(&root.join(path.as_str())) else {
        return false;
    };
    if !metadata.is_file()
        || u32::try_from(metadata.len()).ok() != Some(entry.stat.size)
        || mode_changed(entry.mode, &metadata)
    {
        return false;
    }
    let Ok(fs_stat) = Stat::from_fs(&metadata) else {
        return false;
    };
    entry.stat.matches(&fs_stat, stat_options)
}

fn is_trustworthy_regular_file_entry(entry: &gix::index::Entry) -> bool {
    let untrusted_flags = Flags::INTENT_TO_ADD
        | Flags::SKIP_WORKTREE
        | Flags::ASSUME_VALID
        | Flags::UPDATE
        | Flags::REMOVE
        | Flags::CONFLICTED
        | Flags::WORKTREE_REMOVE;
    matches!(entry.mode, Mode::FILE | Mode::FILE_EXECUTABLE)
        && entry.stat.mtime.secs != 0
        && !entry.flags.intersects(untrusted_flags)
}

fn mode_changed(mode: Mode, metadata: &IndexMetadata) -> bool {
    #[cfg(unix)]
    {
        (mode == Mode::FILE_EXECUTABLE) != metadata.is_executable()
    }
    #[cfg(not(unix))]
    {
        let _ = (mode, metadata);
        false
    }
}

fn list_commit(
    repository: &gix::Repository,
    commit: CommitId,
) -> Result<(Listing, HashMap<RepoPath, gix::ObjectId>), GitError> {
    let entries = repository
        .find_commit(commit.0)
        .and_then(|found| found.tree())
        .and_then(|tree| tree.traverse().breadthfirst.files())
        .map_err(|source| GitError::CommitTree { commit, source })?;
    let mut listing = Listing {
        files: Vec::new(),
        submodules: Vec::new(),
    };
    let mut blobs = HashMap::new();
    for entry in entries {
        if entry.mode.is_commit() {
            listing.submodules.push(repo_path(entry.filepath)?);
        } else if entry.mode.is_blob() {
            let path = repo_path(entry.filepath)?;
            blobs.insert(path.clone(), entry.oid);
            listing.files.push(path);
        }
    }
    listing.files.sort();
    listing.submodules.sort();
    Ok((listing, blobs))
}

fn repo_path(rela_path: gix::bstr::BString) -> Result<RepoPath, GitError> {
    let path = String::from_utf8(rela_path.into()).map_err(|source| GitError::NonUtf8Path {
        path: String::from_utf8_lossy(source.as_bytes()).into_owned(),
        source,
    })?;
    Ok(RepoPath::new(path)?)
}
