use std::collections::{BTreeSet, HashMap};

use codeint_model::RepoPath;

pub struct PathMatcher<'a> {
    by_file_name: HashMap<&'a str, Vec<IndexedRepoPath<'a>>>,
}

struct IndexedRepoPath<'a> {
    path: &'a RepoPath,
    segments: Vec<&'a str>,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct MatchScore {
    exact: bool,
    common_segments: usize,
}

enum MatchCandidateResult<'a> {
    None,
    Matched {
        score: MatchScore,
        paths: Vec<&'a RepoPath>,
    },
}

impl<'a> PathMatcher<'a> {
    pub fn new(repository_paths: &'a [RepoPath]) -> PathMatcher<'a> {
        let mut by_file_name: HashMap<&'a str, Vec<IndexedRepoPath<'a>>> = HashMap::new();
        for path in repository_paths {
            let segments: Vec<&'a str> = path.as_str().split('/').collect();
            by_file_name
                .entry(path.file_name())
                .or_default()
                .push(IndexedRepoPath { path, segments });
        }
        PathMatcher { by_file_name }
    }

    pub fn match_path(&self, report_path: &str, source_roots: &[String]) -> Option<&'a RepoPath> {
        if !source_roots.is_empty()
            && !is_absolute_path(report_path)
            && let Some(resolved) = self.match_with_source_roots(report_path, source_roots)
        {
            return resolved;
        }
        let segments = normalize_segments(report_path)?;
        self.unique_match_for_segments(&segments)
    }

    fn match_with_source_roots(
        &self,
        report_path: &str,
        source_roots: &[String],
    ) -> Option<Option<&'a RepoPath>> {
        let mut best_score: Option<MatchScore> = None;
        let mut winners: BTreeSet<&'a RepoPath> = BTreeSet::new();
        for root in source_roots {
            let trimmed_root = root.trim();
            if trimmed_root.is_empty() {
                continue;
            }
            let combined = format!("{trimmed_root}/{report_path}");
            let Some(segments) = normalize_segments(&combined) else {
                continue;
            };
            let MatchCandidateResult::Matched { score, paths } =
                self.best_matches_for_segments(&segments)
            else {
                continue;
            };
            match best_score {
                Some(current) if score < current => {}
                Some(current) if score == current => {
                    winners.extend(paths);
                }
                _ => {
                    best_score = Some(score);
                    winners.clear();
                    winners.extend(paths);
                }
            }
        }
        if winners.is_empty() {
            return None;
        }
        if winners.len() == 1 {
            return Some(winners.into_iter().next());
        }
        Some(None)
    }

    fn unique_match_for_segments(&self, segments: &[String]) -> Option<&'a RepoPath> {
        match self.best_matches_for_segments(segments) {
            MatchCandidateResult::Matched { paths, .. } if paths.len() == 1 => Some(paths[0]),
            _ => None,
        }
    }

    fn best_matches_for_segments(&self, segments: &[String]) -> MatchCandidateResult<'a> {
        let Some(file_name) = segments.last().map(String::as_str) else {
            return MatchCandidateResult::None;
        };
        if file_name == ".." {
            return MatchCandidateResult::None;
        }
        let Some(candidates) = self.by_file_name.get(file_name) else {
            return MatchCandidateResult::None;
        };
        let mut best_score: Option<MatchScore> = None;
        let mut winners: Vec<&'a RepoPath> = Vec::new();
        for candidate in candidates {
            let Some(score) = score_match(segments, &candidate.segments) else {
                continue;
            };
            match best_score {
                Some(current) if score < current => {}
                Some(current) if score == current => {
                    winners.push(candidate.path);
                }
                _ => {
                    best_score = Some(score);
                    winners.clear();
                    winners.push(candidate.path);
                }
            }
        }
        match best_score {
            Some(score) => MatchCandidateResult::Matched {
                score,
                paths: winners,
            },
            None => MatchCandidateResult::None,
        }
    }
}

pub fn match_coverage_path<'a>(
    report_path: &str,
    source_roots: &[String],
    repository_paths: &'a [RepoPath],
) -> Option<&'a RepoPath> {
    PathMatcher::new(repository_paths).match_path(report_path, source_roots)
}

fn score_match(report_segments: &[String], repo_segments: &[&str]) -> Option<MatchScore> {
    let common = common_trailing_segments(report_segments, repo_segments);
    let required = report_segments.len().min(repo_segments.len());
    if common == 0 || common < required {
        return None;
    }
    Some(MatchScore {
        exact: report_segments.len() == repo_segments.len() && common == repo_segments.len(),
        common_segments: common,
    })
}

fn common_trailing_segments(report_segments: &[String], repo_segments: &[&str]) -> usize {
    report_segments
        .iter()
        .rev()
        .zip(repo_segments.iter().rev())
        .take_while(|(left, right)| left.as_str() == **right)
        .count()
}

fn is_absolute_path(raw: &str) -> bool {
    let trimmed = raw.trim();
    trimmed.starts_with('/') || trimmed.starts_with('\\') || has_windows_drive_prefix(trimmed)
}

fn has_windows_drive_prefix(path: &str) -> bool {
    let bytes = path.as_bytes();
    bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':'
}

fn normalize_segments(raw: &str) -> Option<Vec<String>> {
    let replaced = raw.trim().replace('\\', "/");
    let without_drive = if has_windows_drive_prefix(&replaced) {
        &replaced[2..]
    } else {
        replaced.as_str()
    };
    let mut stack: Vec<String> = Vec::new();
    for segment in without_drive.split('/') {
        match segment {
            "" | "." => {}
            ".." => {
                stack.pop()?;
            }
            other => stack.push(other.to_owned()),
        }
    }
    Some(stack)
}
