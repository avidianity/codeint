use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};

use codeint_config::{ChurnFormat, ChurnWindow, FindingsShown};
use codeint_model::{CodeModel, ImportResolution, RepoPath};
use serde::Serialize;

use crate::churn::render_churn_format;
use crate::{Decimal, FindingKind, Findings, ReportError};

mod markdown;
mod text;

const SHALLOW_WARNING: &str = "Repository is a shallow clone; co-change coupling counts stop at the shallow boundary and may be incomplete. Fetch full history with `git fetch --unshallow`.";

pub struct CouplingHistoryInput {
    pub window: ChurnWindow,
    pub commits_analyzed: usize,
    pub commits_excluded_by_bulk_limit: usize,
    pub max_commit_files: usize,
    pub min_co_changes: usize,
    pub min_ratio: f64,
    pub include_tests: bool,
    pub shallow: bool,
    pub touches_by_file: BTreeMap<RepoPath, usize>,
    pub co_changes: BTreeMap<(RepoPath, RepoPath), usize>,
}

#[derive(Debug, Serialize)]
pub struct CouplingReport {
    window: String,
    commits_analyzed: usize,
    commits_excluded_by_bulk_limit: usize,
    max_commit_files: usize,
    min_co_changes: usize,
    min_ratio: Decimal,
    source_sets: Vec<&'static str>,
    shallow: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    warning: Option<String>,
    #[serde(flatten)]
    pairs: Findings<CoupledPairRow>,
}

#[derive(Debug, Serialize)]
struct CoupledPairRow {
    file_a: String,
    file_b: String,
    co_change_count: usize,
    file_a_commits: usize,
    file_b_commits: usize,
    coupling_ratio: Decimal,
    has_direct_import: bool,
}

impl FindingKind for CoupledPairRow {
    const LIST_KEY: &'static str = "pairs";
    const LEFT_OUT_KEY: &'static str = "pairs_left_out";
}

impl CouplingReport {
    pub fn summarize(
        model: &CodeModel,
        history: &CouplingHistoryInput,
        findings: FindingsShown,
    ) -> CouplingReport {
        let direct_imports = direct_import_pairs(model);
        let min_ratio = Decimal::rounded(history.min_ratio);
        let mut rows: Vec<CoupledPairRow> = history
            .co_changes
            .iter()
            .filter_map(|((file_a, file_b), &co_change_count)| {
                build_pair_row(
                    file_a,
                    file_b,
                    co_change_count,
                    history,
                    min_ratio.0,
                    &direct_imports,
                )
            })
            .collect();
        rows.sort_by(compare_pairs);
        let source_sets = if history.include_tests {
            vec!["production", "test"]
        } else {
            vec!["production"]
        };
        let warning = history.shallow.then(|| SHALLOW_WARNING.to_owned());
        CouplingReport {
            window: history.window.to_string(),
            commits_analyzed: history.commits_analyzed,
            commits_excluded_by_bulk_limit: history.commits_excluded_by_bulk_limit,
            max_commit_files: history.max_commit_files,
            min_co_changes: history.min_co_changes,
            min_ratio,
            source_sets,
            shallow: history.shallow,
            warning,
            pairs: Findings::worst_first(rows.into_iter(), findings),
        }
    }

    pub fn render(&self, format: ChurnFormat) -> Result<String, ReportError> {
        render_churn_format(self, format, text::render, markdown::render)
    }
}

fn direct_import_pairs(model: &CodeModel) -> BTreeSet<(RepoPath, RepoPath)> {
    let mut edges = BTreeSet::new();
    for import in &model.runtime_imports {
        if let ImportResolution::Resolved(target) = &import.resolution {
            edges.insert(ordered_pair(&import.importer, target));
        }
    }
    edges
}

fn ordered_pair(left: &RepoPath, right: &RepoPath) -> (RepoPath, RepoPath) {
    if left <= right {
        (left.clone(), right.clone())
    } else {
        (right.clone(), left.clone())
    }
}

fn build_pair_row(
    file_a: &RepoPath,
    file_b: &RepoPath,
    co_change_count: usize,
    history: &CouplingHistoryInput,
    min_ratio: f64,
    direct_imports: &BTreeSet<(RepoPath, RepoPath)>,
) -> Option<CoupledPairRow> {
    if co_change_count < history.min_co_changes {
        return None;
    }
    let file_a_commits = history.touches_by_file.get(file_a).copied().unwrap_or(0);
    let file_b_commits = history.touches_by_file.get(file_b).copied().unwrap_or(0);
    let union = file_a_commits + file_b_commits - co_change_count;
    if union == 0 {
        return None;
    }
    let coupling_ratio = Decimal::rounded(co_change_count as f64 / union as f64);
    if coupling_ratio.0 < min_ratio {
        return None;
    }
    let has_direct_import = direct_imports.contains(&ordered_pair(file_a, file_b));
    Some(CoupledPairRow {
        file_a: file_a.to_string(),
        file_b: file_b.to_string(),
        co_change_count,
        file_a_commits,
        file_b_commits,
        coupling_ratio,
        has_direct_import,
    })
}

fn compare_pairs(left: &CoupledPairRow, right: &CoupledPairRow) -> Ordering {
    right
        .co_change_count
        .cmp(&left.co_change_count)
        .then_with(|| right.coupling_ratio.0.total_cmp(&left.coupling_ratio.0))
        .then_with(|| left.has_direct_import.cmp(&right.has_direct_import))
        .then_with(|| left.file_a.cmp(&right.file_a))
        .then_with(|| left.file_b.cmp(&right.file_b))
}
