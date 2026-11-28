use crate::{CommitId, GitError, Repository};

pub(crate) const SECONDS_PER_DAY: i64 = 86_400;
pub(crate) const TIMESTAMP_SKEW_WINDOW_SECONDS: i64 = 14 * SECONDS_PER_DAY;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WalkDecision {
    Include,
    Skip,
    Stop,
}

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
    pub(crate) fn decide(
        self,
        included_commits: usize,
        committed_at_unix_seconds: i64,
    ) -> WalkDecision {
        match self {
            CommitRange::Latest(count) => {
                if included_commits < count {
                    WalkDecision::Include
                } else {
                    WalkDecision::Stop
                }
            }
            CommitRange::CommittedSince { unix_seconds } => {
                if committed_at_unix_seconds >= unix_seconds {
                    WalkDecision::Include
                } else if committed_at_unix_seconds
                    >= unix_seconds.saturating_sub(TIMESTAMP_SKEW_WINDOW_SECONDS)
                {
                    WalkDecision::Skip
                } else {
                    WalkDecision::Stop
                }
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
            match range.decide(newest_first.len(), committed_at_unix_seconds) {
                WalkDecision::Stop => break,
                WalkDecision::Include => {
                    newest_first.push(HistoryCommit {
                        commit,
                        committed_at_unix_seconds,
                    });
                }
                WalkDecision::Skip => {}
            }
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
