use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::audit::AuditPopulation;
use crate::ceilings::{Ceilings, ceiling_violations};
use crate::corpus::{CaseStudy, Corpus, CorpusEntry, SizeBucket, load_corpus};
use crate::error::{EvaluationError, io_error};
use crate::lowering::{LoweredFile, lower_language_files};
use crate::machine::{codeint_version, machine_profile};
use crate::measure::{CheckMeasurement, measure_check, median};
use crate::oracle::{PreparedOracle, prepare_oracles, run_oracle};
use crate::recall::{LanguageRecall, RECALL_BAR};
use crate::report::render_markdown;
use crate::review::{load_verdicts, review_samples};
use crate::scratch::{ScratchFolder, clone_at_commit};
use crate::stats::Distribution;
use crate::summary::{
    CaseSnapshot, CaseStudyResult, EvaluationSummary, LanguageSummary, MetricReading, OracleReport,
    OracleStatus, PanelMetric, PanelSummary, PerformanceSummary, RepositoryResult,
};

pub(crate) const SUMMARY_FILE: &str = "evaluation-summary.json";
pub(crate) const MARKDOWN_FILE: &str = "evaluation.md";

#[derive(Clone, Debug)]
pub struct EvaluationConfig {
    pub manifest: PathBuf,
    pub codeint: PathBuf,
    pub output_dir: PathBuf,
    pub protected_repositories: Vec<PathBuf>,
    pub scratch_root: Option<PathBuf>,
    pub languages: Vec<String>,
    pub repositories: Vec<String>,
    pub runs: u32,
    pub audit_sample_size: usize,
    pub oracle_overrides: BTreeMap<String, PathBuf>,
    pub audit_verdicts: Option<PathBuf>,
    pub case_studies: bool,
    pub require_oracles: bool,
    pub ceilings: Ceilings,
}

fn absolute(path: &Path) -> Result<PathBuf, EvaluationError> {
    path.canonicalize().map_err(io_error(path))
}

fn resolve_programs(config: &EvaluationConfig) -> Result<EvaluationConfig, EvaluationError> {
    let mut resolved = config.clone();
    resolved.codeint = absolute(&config.codeint)?;
    for program in resolved.oracle_overrides.values_mut() {
        *program = absolute(program)?;
    }
    Ok(resolved)
}

pub fn run_evaluation(config: &EvaluationConfig) -> Result<EvaluationSummary, EvaluationError> {
    let config = &resolve_programs(config)?;
    let corpus = load_corpus(&config.manifest)?;
    let selected = select(&corpus, config)?;
    let oracle_workspace = ScratchFolder::outside(
        &config.protected_repositories,
        config.scratch_root.as_deref(),
    )?;
    let codeint_version = codeint_version(&config.codeint)?;
    let languages: BTreeSet<&str> = selected
        .iter()
        .map(|entry| entry.language.as_str())
        .collect();
    let oracles = prepare_oracles(languages, oracle_workspace.path(), &config.oracle_overrides);
    let mut population = AuditPopulation::default();
    let repositories: Vec<RepositoryResult> = selected
        .iter()
        .map(|entry| evaluate_repository(entry, config, &oracles, &mut population))
        .collect();
    drop(oracle_workspace);
    let case_studies = if config.case_studies {
        measure_case_studies(&corpus.case_studies, config)
    } else {
        Vec::new()
    };
    let audit = population.draw(config.audit_sample_size);
    let audit_review = match &config.audit_verdicts {
        Some(path) => Some(review_samples(&audit, &load_verdicts(path)?)),
        None => None,
    };
    let summary = EvaluationSummary {
        schema_version: 2,
        measured_on: corpus.measured_on.clone(),
        codeint_version,
        machine: machine_profile(),
        runs_per_repository: config.runs.max(1),
        recall_bar: RECALL_BAR,
        passed: repositories.iter().all(|result| result.passed),
        languages: summarize_languages(&repositories, &oracles),
        performance: summarize_performance(&repositories),
        panels: summarize_panels(&repositories),
        repositories,
        case_studies,
        audit,
        audit_review,
    };
    write_outputs(&config.output_dir, &summary)?;
    Ok(summary)
}

fn select<'corpus>(
    corpus: &'corpus Corpus,
    config: &EvaluationConfig,
) -> Result<Vec<&'corpus CorpusEntry>, EvaluationError> {
    let wanted = |filter: &[String], value: &str| {
        filter.is_empty() || filter.iter().any(|item| item == value)
    };
    let selected: Vec<&CorpusEntry> = corpus
        .repositories
        .iter()
        .filter(|entry| {
            wanted(&config.languages, &entry.language) && wanted(&config.repositories, &entry.name)
        })
        .collect();
    if selected.is_empty() {
        return Err(EvaluationError::EmptySelection {
            path: config.manifest.clone(),
        });
    }
    Ok(selected)
}

fn evaluate_repository(
    entry: &CorpusEntry,
    config: &EvaluationConfig,
    oracles: &BTreeMap<String, PreparedOracle>,
    population: &mut AuditPopulation,
) -> RepositoryResult {
    let mut result = RepositoryResult::pending(entry);
    match measure_repository(&mut result, entry, config, oracles, population) {
        Ok(()) => {
            let violations = ceiling_violations(&result, &config.ceilings, config.require_oracles);
            if !violations.is_empty() {
                result.error = Some(violations.join("; "));
            }
        }
        Err(error) => result.error = Some(error.to_string()),
    }
    result.passed =
        result.error.is_none() && result.exit_code == Some(0) && result.oracle.meets_bar();
    result
}

fn measure_repository(
    result: &mut RepositoryResult,
    entry: &CorpusEntry,
    config: &EvaluationConfig,
    oracles: &BTreeMap<String, PreparedOracle>,
    population: &mut AuditPopulation,
) -> Result<(), EvaluationError> {
    let scratch = ScratchFolder::outside(
        &config.protected_repositories,
        config.scratch_root.as_deref(),
    )?;
    let checkout = clone_at_commit(&scratch, &entry.url, &entry.commit)?;
    let check = measure_check(&config.codeint, &checkout, scratch.path(), config.runs)?;
    record_check(result, &check);
    let lowered = lower_language_files(&checkout, &entry.language)?;
    result.oracle = oracle_report(
        oracles.get(&entry.language),
        &checkout,
        scratch.path(),
        &lowered,
    )?;
    if entry.panel.is_none() {
        population.add_repository(&entry.name, &entry.commit, &check.report, &lowered);
    }
    Ok(())
}

fn language_section<'report>(report: &'report Value, language: &str) -> &'report Value {
    let section = &report["languages"][language];
    if section.is_null() { report } else { section }
}

fn production_lines(report: &Value, language: &str) -> Option<u64> {
    language_section(report, language)["source_sets"]["production"]["source_lines"].as_u64()
}

pub(crate) const RANGE_METRICS: [&str; 4] =
    ["erosion", "verbosity", "duplication", "import_cycles"];

pub(crate) fn metric_readings(production: &Value) -> BTreeMap<String, MetricReading> {
    RANGE_METRICS
        .iter()
        .filter_map(|key| {
            let metric = &production[*key];
            Some((
                (*key).to_owned(),
                MetricReading {
                    ratio: metric["ratio"].as_f64()?,
                    version: u32::try_from(metric["metric_version"].as_u64()?).ok()?,
                },
            ))
        })
        .collect()
}

fn framework_profiles(report: &Value, language: &str) -> Vec<String> {
    language_section(report, language)["framework_profiles"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|name| name.as_str().map(str::to_owned))
        .collect()
}

fn record_check(result: &mut RepositoryResult, check: &CheckMeasurement) {
    result.production_lines = production_lines(&check.report, &result.language).or(Some(0));
    result.measured_bucket = result
        .production_lines
        .and_then(SizeBucket::for_production_lines);
    result.skipped_files = Some(
        check.report["skipped"]
            .as_array()
            .map_or(0, |skipped| skipped.len() as u64),
    );
    result.metrics = metric_readings(
        &language_section(&check.report, &result.language)["source_sets"]["production"],
    );
    result.framework_profiles = framework_profiles(&check.report, &result.language);
    result.exit_code = check.exit_code;
    result.wall_seconds = check.wall_seconds.clone();
    result.median_wall_seconds = median(&check.wall_seconds);
    result.peak_rss_bytes = check.peak_rss_bytes;
    result.production_lines_per_second = result
        .median_wall_seconds
        .filter(|seconds| *seconds > 0.0)
        .zip(result.production_lines)
        .map(|(seconds, lines)| lines as f64 / seconds);
}

fn oracle_report(
    prepared: Option<&PreparedOracle>,
    checkout: &Path,
    scratch: &Path,
    lowered: &[LoweredFile],
) -> Result<OracleReport, EvaluationError> {
    match prepared {
        None => Ok(OracleReport::not_applicable()),
        Some(PreparedOracle::Unavailable(reason)) => Ok(OracleReport {
            status: OracleStatus::Skipped,
            reason: Some(reason.clone()),
            recall: None,
        }),
        Some(PreparedOracle::Ready(command)) => {
            let paths: Vec<&str> = lowered.iter().map(|file| file.path.as_str()).collect();
            let oracle_files = run_oracle(command, checkout, scratch, &paths)?;
            Ok(OracleReport {
                status: OracleStatus::Measured,
                reason: None,
                recall: Some(LanguageRecall::compare(&oracle_files, lowered)),
            })
        }
    }
}

fn language_status(prepared: Option<&PreparedOracle>) -> (OracleStatus, Option<String>) {
    match prepared {
        None => (OracleStatus::NotApplicable, None),
        Some(PreparedOracle::Unavailable(reason)) => (OracleStatus::Skipped, Some(reason.clone())),
        Some(PreparedOracle::Ready(_)) => (OracleStatus::Measured, None),
    }
}

fn summarize_languages(
    repositories: &[RepositoryResult],
    oracles: &BTreeMap<String, PreparedOracle>,
) -> Vec<LanguageSummary> {
    let mut by_language: BTreeMap<&str, Vec<&RepositoryResult>> = BTreeMap::new();
    for result in repositories {
        by_language
            .entry(result.language.as_str())
            .or_default()
            .push(result);
    }
    by_language
        .into_iter()
        .map(|(language, results)| {
            let (oracle_status, oracle_reason) = language_status(oracles.get(language));
            let mut recall: Option<LanguageRecall> = None;
            for result in &results {
                if let Some(measured) = &result.oracle.recall {
                    recall
                        .get_or_insert_default()
                        .absorb(&result.name, measured);
                }
            }
            LanguageSummary {
                language: language.to_owned(),
                repositories: results.len() as u64,
                oracle_status,
                oracle_reason,
                recall,
                passed: results.iter().all(|result| result.passed),
            }
        })
        .collect()
}

fn summarize_performance(repositories: &[RepositoryResult]) -> PerformanceSummary {
    let largest = repositories
        .iter()
        .filter(|result| result.production_lines.is_some())
        .max_by_key(|result| result.production_lines);
    let throughputs: Vec<f64> = repositories
        .iter()
        .filter_map(|result| result.production_lines_per_second)
        .collect();
    PerformanceSummary {
        largest_repository: largest.map(|result| result.name.clone()),
        largest_production_lines: largest.and_then(|result| result.production_lines),
        largest_median_wall_seconds: largest.and_then(|result| result.median_wall_seconds),
        largest_peak_rss_bytes: largest.and_then(|result| result.peak_rss_bytes),
        highest_peak_rss_bytes: repositories
            .iter()
            .filter_map(|result| result.peak_rss_bytes)
            .max(),
        median_production_lines_per_second: median(&throughputs),
    }
}

fn summarize_panels(repositories: &[RepositoryResult]) -> Vec<PanelSummary> {
    let mut groups: BTreeMap<(&str, Option<&str>), Vec<&RepositoryResult>> = BTreeMap::new();
    for result in repositories.iter().filter(|result| result.error.is_none()) {
        groups
            .entry((result.language.as_str(), result.panel.as_deref()))
            .or_default()
            .push(result);
    }
    groups
        .into_iter()
        .map(|((language, panel), results)| {
            let in_bucket = |bucket| {
                results
                    .iter()
                    .filter(|result| result.measured_bucket == Some(bucket))
                    .count() as u64
            };
            let metrics = RANGE_METRICS
                .iter()
                .filter_map(|key| {
                    let readings: Vec<&MetricReading> = results
                        .iter()
                        .filter_map(|result| result.metrics.get(*key))
                        .collect();
                    let ratios: Vec<f64> = readings.iter().map(|reading| reading.ratio).collect();
                    Some((
                        (*key).to_owned(),
                        PanelMetric {
                            version: readings.first()?.version,
                            distribution: Distribution::of(&ratios)?,
                        },
                    ))
                })
                .collect();
            PanelSummary {
                language: language.to_owned(),
                panel: panel.map(str::to_owned),
                repositories: results.len() as u64,
                small: in_bucket(SizeBucket::Small),
                medium: in_bucket(SizeBucket::Medium),
                large: in_bucket(SizeBucket::Large),
                metrics,
            }
        })
        .collect()
}

fn snapshot(
    config: &EvaluationConfig,
    url: &str,
    commit: &str,
) -> Result<CaseSnapshot, EvaluationError> {
    let scratch = ScratchFolder::outside(
        &config.protected_repositories,
        config.scratch_root.as_deref(),
    )?;
    let checkout = clone_at_commit(&scratch, url, commit)?;
    let check = measure_check(&config.codeint, &checkout, scratch.path(), 1)?;
    let production = &check.report["source_sets"]["production"];
    Ok(CaseSnapshot {
        commit: commit.to_owned(),
        production_lines: production["source_lines"].as_u64(),
        metrics: metric_readings(production),
    })
}

fn measure_case_studies(studies: &[CaseStudy], config: &EvaluationConfig) -> Vec<CaseStudyResult> {
    studies
        .iter()
        .map(|study| {
            let before = snapshot(config, &study.url, &study.before);
            let after = snapshot(config, &study.url, &study.after);
            let error = [&before, &after]
                .into_iter()
                .filter_map(|outcome| outcome.as_ref().err())
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("; ");
            CaseStudyResult {
                name: study.name.clone(),
                url: study.url.clone(),
                change: study.change.clone(),
                before: before.ok(),
                after: after.ok(),
                error: (!error.is_empty()).then_some(error),
            }
        })
        .collect()
}

fn write_outputs(output_dir: &Path, summary: &EvaluationSummary) -> Result<(), EvaluationError> {
    std::fs::create_dir_all(output_dir).map_err(io_error(output_dir))?;
    let json_path = output_dir.join(SUMMARY_FILE);
    let mut json = serde_json::to_string_pretty(summary)?;
    json.push('\n');
    std::fs::write(&json_path, json).map_err(io_error(&json_path))?;
    let markdown_path = output_dir.join(MARKDOWN_FILE);
    std::fs::write(&markdown_path, render_markdown(summary)).map_err(io_error(&markdown_path))
}
