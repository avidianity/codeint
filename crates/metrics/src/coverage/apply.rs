use std::collections::{BTreeMap, BTreeSet, HashMap};

use codeint_model::language::Function;
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
            let coverages = summarize_file_coverage(&file.model.functions, file_lines);
            for (function, coverage) in file.model.functions.iter_mut().zip(coverages) {
                function.coverage = Some(coverage);
            }
        }
        if model_has_coverage {
            for facts in &mut model.languages {
                facts.has_coverage = true;
            }
        }
    }
}

#[derive(Clone, Copy)]
struct FunctionSpan {
    start: u32,
    end: u32,
    index: usize,
}

fn summarize_file_coverage(
    functions: &[Function],
    file_lines: &BTreeMap<u32, u64>,
) -> Vec<FunctionCoverage> {
    let mut results = vec![
        FunctionCoverage {
            listed_lines: 0,
            executed_lines: 0,
        };
        functions.len()
    ];
    let spans = collect_valid_spans(functions);
    if spans.is_empty() || file_lines.is_empty() {
        return results;
    }

    let mut active = Vec::new();
    let mut next_span = 0;

    for (&line, &hits) in file_lines {
        advance_active_spans(&spans, &mut next_span, &mut active, line);
        attribute_line_to_innermost(&active, hits, &mut results);
    }

    results
}

fn collect_valid_spans(functions: &[Function]) -> Vec<FunctionSpan> {
    let mut spans: Vec<FunctionSpan> = functions
        .iter()
        .enumerate()
        .filter(|(_, function)| function.line <= function.end_line)
        .map(|(index, function)| FunctionSpan {
            start: function.line,
            end: function.end_line,
            index,
        })
        .collect();
    spans.sort_unstable_by(|left, right| {
        left.start
            .cmp(&right.start)
            .then_with(|| right.end.cmp(&left.end))
            .then_with(|| left.index.cmp(&right.index))
    });
    spans
}

fn advance_active_spans(
    spans: &[FunctionSpan],
    next_span: &mut usize,
    active: &mut Vec<FunctionSpan>,
    line: u32,
) {
    while let Some(&span) = spans.get(*next_span).filter(|span| span.start <= line) {
        if span.end >= line {
            active.push(span);
        }
        *next_span += 1;
    }
    active.retain(|span| span.end >= line);
}

fn attribute_line_to_innermost(
    active: &[FunctionSpan],
    hits: u64,
    results: &mut [FunctionCoverage],
) {
    for &span in active {
        if has_strictly_nested_child(active, span) {
            continue;
        }
        record_line_hit(&mut results[span.index], hits);
    }
}

fn has_strictly_nested_child(active: &[FunctionSpan], outer: FunctionSpan) -> bool {
    active
        .iter()
        .any(|&other| is_strictly_nested_in((other.start, other.end), (outer.start, outer.end)))
}

fn record_line_hit(coverage: &mut FunctionCoverage, hits: u64) {
    coverage.listed_lines = coverage.listed_lines.saturating_add(1);
    if hits > 0 {
        coverage.executed_lines = coverage.executed_lines.saturating_add(1);
    }
}

fn is_strictly_nested_in(inner: (u32, u32), outer: (u32, u32)) -> bool {
    outer.0 <= inner.0 && inner.1 <= outer.1 && inner != outer
}
