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

impl CommitRange {
    fn admits(self, earlier_commits: usize, committed_at_unix_seconds: i64) -> bool {
        match self {
            CommitRange::Latest(count) => earlier_commits < count,
            CommitRange::CommittedSince { unix_seconds } => {
                committed_at_unix_seconds >= unix_seconds
            }
        }
    }
}

impl Repository {
    pub fn first_parent_history(&self, range: CommitRange) -> Result<Vec<HistoryCommit>, GitError> {
        let head = self.repository.head_id().map_err(GitError::NoHistory)?;
        let shallow_boundary = self
            .repository
            .shallow_commits()
            .map_err(GitError::NoHistory)?;
        let mut newest_first = Vec::new();
        let mut next = Some(head.detach());
        while let Some(id) = next {
            let commit = CommitId(id);
            let history_error = |source| GitError::History { commit, source };
            let found = self.repository.find_commit(id).map_err(history_error)?;
            let committed_at_unix_seconds = found.time().map_err(history_error)?.seconds;
            if !range.admits(newest_first.len(), committed_at_unix_seconds) {
                break;
            }
            newest_first.push(HistoryCommit {
                commit,
                committed_at_unix_seconds,
            });
            let beyond_shallow_boundary = shallow_boundary
                .as_ref()
                .is_some_and(|boundary| boundary.contains(&id));
            next = if beyond_shallow_boundary {
                None
            } else {
                found.parent_ids().next().map(|parent| parent.detach())
            };
        }
        newest_first.reverse();
        Ok(newest_first)
    }
}
