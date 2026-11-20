use crate::{CommitId, GitError, Repository};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommitRange {
    Latest(usize),
    CommittedSince { unix_seconds: i64 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HistoryCommit {
    pub commit: CommitId,
    pub committed_at_unix_seconds: i64,
}

impl Repository {
    pub fn first_parent_history(&self, range: CommitRange) -> Result<Vec<HistoryCommit>, GitError> {
        let _ = range;
        todo!()
    }
}
