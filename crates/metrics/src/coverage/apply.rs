use std::collections::{BTreeMap, BTreeSet, HashMap};

use codeint_model::{CodeModel, FileModel, FunctionCoverage, RepoPath};

use super::{ParsedCoverageReport, PathMatcher, merge_line_hit};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NamedCoverageReport {
    pub path: String,
    pub report: ParsedCoverageReport,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CoverageApplyOutcome {
    pub unmatched_paths: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum CoverageApplyError {
    #[error(
        "Coverage report {report:?} lists line {line} for {file}, which is beyond the file's {line_count} lines"
    )]
    LineOutOfRange {
        report: String,
        file: RepoPath,
        line: u32,
        line_count: usize,
    },
}

pub fn apply_coverage_reports(
    reports: &[NamedCoverageReport],
    models: &mut [CodeModel],
    file_line_counts: &BTreeMap<RepoPath, usize>,
) -> Result<CoverageApplyOutcome, CoverageApplyError> {
    if reports.is_empty() {
        return Ok(CoverageApplyOutcome::default());
    }
    let known_paths = collect_known_paths(models, file_line_counts);
    let inferred_counts = collect_inferred_line_counts(models);
    let matcher = PathMatcher::new(&known_paths);
    let mut merged_lines: HashMap<RepoPath, BTreeMap<u32, u64>> = HashMap::new();
    let mut unmatched_paths: BTreeSet<String> = BTreeSet::new();

    for named in reports {
        for record in &named.report.files {
            let Some(matched_path) = matcher.match_path(&record.path, &named.report.source_roots)
            else {
                unmatched_paths.insert(record.path.clone());
                continue;
            };
            let line_count = resolve_line_count(matched_path, file_line_counts, &inferred_counts);
            let target = merged_lines.entry(matched_path.clone()).or_default();
            for (&line, &hits) in &record.lines {
                let line_usize = usize::try_from(line).unwrap_or(usize::MAX);
                if line == 0 || line_usize > line_count {
                    return Err(CoverageApplyError::LineOutOfRange {
                        report: named.path.clone(),
                        file: matched_path.clone(),
                        line,
                        line_count,
                    });
                }
                merge_line_hit(target, line, hits);
            }
        }
    }

    apply_merged_coverage(models, &merged_lines);
    Ok(CoverageApplyOutcome {
        unmatched_paths: unmatched_paths.into_iter().collect(),
    })
}

fn collect_known_paths(
    models: &[CodeModel],
    file_line_counts: &BTreeMap<RepoPath, usize>,
) -> Vec<RepoPath> {
    let mut paths: BTreeSet<RepoPath> = file_line_counts.keys().cloned().collect();
    for model in models {
        for file in &model.files {
            paths.insert(file.path.clone());
        }
        for path in &model.declaration_only {
            paths.insert(path.clone());
        }
        for skipped in &model.skipped {
            paths.insert(skipped.path.clone());
        }
    }
    paths.into_iter().collect()
}

fn collect_inferred_line_counts(models: &[CodeModel]) -> HashMap<RepoPath, usize> {
    let mut counts = HashMap::new();
    for model in models {
        for file in &model.files {
            counts.insert(file.path.clone(), inferred_line_count(&file.model));
        }
    }
    counts
}

fn inferred_line_count(file: &FileModel) -> usize {
    let max_function_line = file
        .functions
        .iter()
        .map(|function| function.end_line)
        .max()
        .unwrap_or(0);
    let max_token_line = file
        .tokens
        .iter()
        .map(|token| token.line)
        .max()
        .unwrap_or(0);
    let max_line = file.source_lines.max(max_function_line).max(max_token_line);
    usize::try_from(max_line).unwrap_or(usize::MAX)
}

fn resolve_line_count(
    path: &RepoPath,
    file_line_counts: &BTreeMap<RepoPath, usize>,
    inferred_counts: &HashMap<RepoPath, usize>,
) -> usize {
    file_line_counts
        .get(path)
        .copied()
        .or_else(|| inferred_counts.get(path).copied())
        .unwrap_or(0)
}

fn apply_merged_coverage(
    models: &mut [CodeModel],
    merged_lines: &HashMap<RepoPath, BTreeMap<u32, u64>>,
) {
    for model in models {
        let mut model_has_coverage = false;
        for file in &mut model.files {
            let Some(file_lines) = merged_lines.get(&file.path) else {
                continue;
            };
            model_has_coverage = true;
            for function in &mut file.model.functions {
                function.coverage = Some(summarize_function_coverage(
                    function.line,
                    function.end_line,
                    file_lines,
                ));
            }
        }
        if model_has_coverage {
            for facts in &mut model.languages {
                facts.has_coverage = true;
            }
        }
    }
}

fn summarize_function_coverage(
    start_line: u32,
    end_line: u32,
    file_lines: &BTreeMap<u32, u64>,
) -> FunctionCoverage {
    if start_line > end_line {
        return FunctionCoverage {
            listed_lines: 0,
            executed_lines: 0,
        };
    }
    let mut listed_lines: u32 = 0;
    let mut executed_lines: u32 = 0;
    for (_, &hits) in file_lines.range(start_line..=end_line) {
        listed_lines = listed_lines.saturating_add(1);
        if hits > 0 {
            executed_lines = executed_lines.saturating_add(1);
        }
    }
    FunctionCoverage {
        listed_lines,
        executed_lines,
    }
}
