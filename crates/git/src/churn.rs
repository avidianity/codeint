use std::collections::{BTreeMap, BTreeSet, HashMap};

use codeint_model::RepoPath;
use gix::ObjectId;
use gix::objs::tree::EntryMode;

use crate::{CommitId, GitError, Repository};

const SECONDS_PER_DAY: i64 = 86_400;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChurnRange {
    Days(u32),
    Since { unix_seconds: i64 },
    Commits(usize),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileChurnSummary {
    pub commits_analyzed: usize,
    pub shallow: bool,
    pub truncated_by_shallow_boundary: bool,
    pub touches_by_file: BTreeMap<RepoPath, usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoChangeSummary {
    pub commits_analyzed: usize,
    pub commits_excluded_by_bulk_limit: usize,
    pub shallow: bool,
    pub truncated_by_shallow_boundary: bool,
    pub touches_by_file: BTreeMap<RepoPath, usize>,
    pub co_changes: BTreeMap<(RepoPath, RepoPath), usize>,
}

struct WalkOutcome {
    commits_analyzed: usize,
    shallow: bool,
    truncated_by_shallow_boundary: bool,
}

enum EffectiveRange {
    Commits(usize),
    Since(i64),
}

impl EffectiveRange {
    fn from_head(range: ChurnRange, head_unix_seconds: i64) -> EffectiveRange {
        match range {
            ChurnRange::Days(days) => EffectiveRange::Since(
                head_unix_seconds.saturating_sub(i64::from(days) * SECONDS_PER_DAY),
            ),
            ChurnRange::Since { unix_seconds } => EffectiveRange::Since(unix_seconds),
            ChurnRange::Commits(count) => EffectiveRange::Commits(count),
        }
    }

    fn admits(&self, analyzed: usize, committed_at_unix_seconds: i64) -> bool {
        match self {
            EffectiveRange::Commits(limit) => analyzed < *limit,
            EffectiveRange::Since(cutoff) => committed_at_unix_seconds >= *cutoff,
        }
    }
}

impl Repository {
    pub fn file_churn(&self, range: ChurnRange) -> Result<FileChurnSummary, GitError> {
        let mut touches_by_file: BTreeMap<RepoPath, usize> = BTreeMap::new();
        let outcome = self.walk_touched_commits(range, |touched| {
            for path in touched {
                *touches_by_file.entry(path).or_insert(0) += 1;
            }
        })?;
        Ok(FileChurnSummary {
            commits_analyzed: outcome.commits_analyzed,
            shallow: outcome.shallow,
            truncated_by_shallow_boundary: outcome.truncated_by_shallow_boundary,
            touches_by_file,
        })
    }

    pub fn co_change_coupling(
        &self,
        range: ChurnRange,
        max_commit_files: usize,
        allowed_paths: &BTreeSet<RepoPath>,
    ) -> Result<CoChangeSummary, GitError> {
        let mut commits_excluded_by_bulk_limit = 0usize;
        let mut touches_by_file: BTreeMap<RepoPath, usize> = BTreeMap::new();
        let mut co_changes: BTreeMap<(RepoPath, RepoPath), usize> = BTreeMap::new();
        let outcome = self.walk_touched_commits(range, |touched| {
            if touched.len() > max_commit_files {
                commits_excluded_by_bulk_limit += 1;
                return;
            }
            let measured: Vec<RepoPath> = touched
                .into_iter()
                .filter(|path| allowed_paths.contains(path))
                .collect();
            for path in &measured {
                *touches_by_file.entry(path.clone()).or_insert(0) += 1;
            }
            for (index, left) in measured.iter().enumerate() {
                for right in &measured[index + 1..] {
                    *co_changes.entry((left.clone(), right.clone())).or_insert(0) += 1;
                }
            }
        })?;
        Ok(CoChangeSummary {
            commits_analyzed: outcome.commits_analyzed,
            commits_excluded_by_bulk_limit,
            shallow: outcome.shallow,
            truncated_by_shallow_boundary: outcome.truncated_by_shallow_boundary,
            touches_by_file,
            co_changes,
        })
    }

    fn walk_touched_commits(
        &self,
        range: ChurnRange,
        on_commit: impl FnMut(BTreeSet<RepoPath>),
    ) -> Result<WalkOutcome, GitError> {
        let head = self.repository.head_id().map_err(GitError::NoHistory)?;
        let shallow_boundary = self
            .repository
            .shallow_commits()
            .map_err(GitError::NoHistory)?;
        let shallow = shallow_boundary
            .as_ref()
            .is_some_and(|boundary| !boundary.is_empty());
        let head_id = head.detach();
        let head_commit =
            self.repository
                .find_commit(head_id)
                .map_err(|source| GitError::History {
                    commit: CommitId(head_id),
                    source,
                })?;
        let head_time = head_commit
            .time()
            .map_err(|source| GitError::History {
                commit: CommitId(head_id),
                source,
            })?
            .seconds;
        let effective = EffectiveRange::from_head(range, head_time);
        self.walk_commits(
            head_id,
            &effective,
            shallow,
            shallow_boundary.as_ref(),
            on_commit,
        )
    }

    fn walk_commits(
        &self,
        start: ObjectId,
        effective: &EffectiveRange,
        shallow: bool,
        shallow_boundary: Option<&gix::shallow::Commits>,
        mut on_commit: impl FnMut(BTreeSet<RepoPath>),
    ) -> Result<WalkOutcome, GitError> {
        let mut commits_analyzed = 0usize;
        let mut truncated_by_shallow_boundary = false;
        let mut next = Some(start);
        while let Some(id) = next {
            let commit = CommitId(id);
            let found = self
                .repository
                .find_commit(id)
                .map_err(|source| GitError::History { commit, source })?;
            let committed_at = found
                .time()
                .map_err(|source| GitError::History { commit, source })?
                .seconds;
            if !effective.admits(commits_analyzed, committed_at) {
                break;
            }
            commits_analyzed += 1;
            let at_boundary = shallow_boundary.is_some_and(|boundary| boundary.contains(&id));
            let parent_id = if at_boundary {
                truncated_by_shallow_boundary = true;
                None
            } else {
                found.parent_ids().next().map(|parent| parent.detach())
            };
            let touched = self.touched_files_in_commit(commit, &found, parent_id)?;
            on_commit(touched);
            next = parent_id;
        }
        Ok(WalkOutcome {
            commits_analyzed,
            shallow,
            truncated_by_shallow_boundary,
        })
    }

    fn touched_files_in_commit(
        &self,
        commit: CommitId,
        found: &gix::Commit<'_>,
        parent_id: Option<ObjectId>,
    ) -> Result<BTreeSet<RepoPath>, GitError> {
        let new_tree = found
            .tree()
            .map_err(|source| GitError::CommitTree { commit, source })?;
        let mut touched = BTreeSet::new();
        let mut diff = TreeDiffer {
            repository: &self.repository,
            commit,
            touched: &mut touched,
        };
        match parent_id {
            Some(pid) => {
                let parent_commit = CommitId(pid);
                let old_tree = self
                    .repository
                    .find_commit(pid)
                    .and_then(|parent| parent.tree())
                    .map_err(|source| GitError::CommitTree {
                        commit: parent_commit,
                        source,
                    })?;
                diff.trees("", &old_tree, &new_tree)?;
            }
            None => {
                diff.collect_blobs("", &new_tree)?;
            }
        }
        Ok(touched)
    }
}

struct TreeDiffer<'a> {
    repository: &'a gix::Repository,
    commit: CommitId,
    touched: &'a mut BTreeSet<RepoPath>,
}

impl TreeDiffer<'_> {
    fn trees(
        &mut self,
        prefix: &str,
        old_tree: &gix::Tree<'_>,
        new_tree: &gix::Tree<'_>,
    ) -> Result<(), GitError> {
        if old_tree.id == new_tree.id {
            return Ok(());
        }
        let old_entries = tree_entries(self.commit, old_tree)?;
        let new_entries = tree_entries(self.commit, new_tree)?;
        self.visit_present(prefix, &old_entries, &new_entries)?;
        self.visit_removed(prefix, &old_entries, &new_entries)
    }

    fn visit_present(
        &mut self,
        prefix: &str,
        old_entries: &HashMap<String, (EntryMode, ObjectId)>,
        new_entries: &HashMap<String, (EntryMode, ObjectId)>,
    ) -> Result<(), GitError> {
        for (name, &(new_mode, new_oid)) in new_entries {
            let path = join_prefix(prefix, name);
            match old_entries.get(name) {
                None => self.record_entry(&path, new_mode, new_oid)?,
                Some(&(old_mode, old_oid)) if old_oid == new_oid && old_mode == new_mode => {}
                Some(&(old_mode, old_oid)) => {
                    self.visit_changed(&path, (old_mode, old_oid), (new_mode, new_oid))?;
                }
            }
        }
        Ok(())
    }

    fn visit_removed(
        &mut self,
        prefix: &str,
        old_entries: &HashMap<String, (EntryMode, ObjectId)>,
        new_entries: &HashMap<String, (EntryMode, ObjectId)>,
    ) -> Result<(), GitError> {
        for (name, &(old_mode, old_oid)) in old_entries {
            if !new_entries.contains_key(name) {
                let path = join_prefix(prefix, name);
                self.record_entry(&path, old_mode, old_oid)?;
            }
        }
        Ok(())
    }

    fn visit_changed(
        &mut self,
        path: &str,
        (old_mode, old_oid): (EntryMode, ObjectId),
        (new_mode, new_oid): (EntryMode, ObjectId),
    ) -> Result<(), GitError> {
        if old_mode.is_tree() && new_mode.is_tree() {
            let old_sub = find_tree(self.repository, self.commit, old_oid)?;
            let new_sub = find_tree(self.repository, self.commit, new_oid)?;
            return self.trees(path, &old_sub, &new_sub);
        }
        self.record_entry(path, old_mode, old_oid)?;
        self.record_entry(path, new_mode, new_oid)
    }

    fn record_entry(&mut self, path: &str, mode: EntryMode, oid: ObjectId) -> Result<(), GitError> {
        if mode.is_blob() {
            if let Ok(repo_path) = RepoPath::new(path) {
                self.touched.insert(repo_path);
            }
        } else if mode.is_tree() {
            let subtree = find_tree(self.repository, self.commit, oid)?;
            self.collect_blobs(path, &subtree)?;
        }
        Ok(())
    }

    fn collect_blobs(&mut self, prefix: &str, tree: &gix::Tree<'_>) -> Result<(), GitError> {
        for (name, (mode, oid)) in tree_entries(self.commit, tree)? {
            let path = join_prefix(prefix, &name);
            self.record_entry(&path, mode, oid)?;
        }
        Ok(())
    }
}

fn find_tree(
    repository: &gix::Repository,
    commit: CommitId,
    oid: ObjectId,
) -> Result<gix::Tree<'_>, GitError> {
    repository
        .find_tree(oid)
        .map_err(|source| GitError::CommitTree { commit, source })
}

fn tree_entries(
    commit: CommitId,
    tree: &gix::Tree<'_>,
) -> Result<HashMap<String, (EntryMode, ObjectId)>, GitError> {
    let mut map = HashMap::new();
    for entry in tree.iter() {
        let entry = entry.map_err(|source| GitError::CommitTree { commit, source })?;
        let Ok(name) = std::str::from_utf8(entry.inner.filename) else {
            continue;
        };
        map.insert(
            name.to_owned(),
            (entry.inner.mode, entry.inner.oid.to_owned()),
        );
    }
    Ok(map)
}

fn join_prefix(prefix: &str, name: &str) -> String {
    if prefix.is_empty() {
        name.to_owned()
    } else {
        format!("{prefix}/{name}")
    }
}
