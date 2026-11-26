use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::audit::AuditSample;
use crate::corpus::{CorpusEntry, SizeBucket};
use crate::recall::LanguageRecall;
use crate::review::AuditReview;
use crate::stats::Distribution;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EvaluationSummary {
    pub schema_version: u32,
    pub measured_on: String,
    pub codeint_version: String,
    pub machine: MachineProfile,
    pub runs_per_repository: u32,
    pub recall_bar: f64,
    pub passed: bool,
    pub languages: Vec<LanguageSummary>,
    pub performance: PerformanceSummary,
    pub panels: Vec<PanelSummary>,
    pub repositories: Vec<RepositoryResult>,
    pub case_studies: Vec<CaseStudyResult>,
    pub audit: Vec<AuditSample>,
    pub audit_review: Option<AuditReview>,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct MetricReading {
    pub ratio: f64,
    pub version: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PanelSummary {
    pub language: String,
    pub panel: Option<String>,
    pub repositories: u64,
    pub small: u64,
    pub medium: u64,
    pub large: u64,
    pub metrics: BTreeMap<String, PanelMetric>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PanelMetric {
    pub version: u32,
    pub distribution: Distribution,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CaseSnapshot {
    pub commit: String,
    pub production_lines: Option<u64>,
    pub metrics: BTreeMap<String, MetricReading>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CaseStudyResult {
    pub name: String,
    pub url: String,
    pub change: String,
    pub before: Option<CaseSnapshot>,
    pub after: Option<CaseSnapshot>,
    pub error: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MachineProfile {
    pub os: String,
    pub arch: String,
    pub logical_cpus: u64,
    pub cpu_model: Option<String>,
    pub memory_bytes: Option<u64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OracleStatus {
    Measured,
    Skipped,
    NotApplicable,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OracleReport {
    pub status: OracleStatus,
    pub reason: Option<String>,
    pub recall: Option<LanguageRecall>,
}

impl OracleReport {
    pub(crate) fn not_applicable() -> OracleReport {
        OracleReport {
            status: OracleStatus::NotApplicable,
            reason: None,
            recall: None,
        }
    }

    pub fn meets_bar(&self) -> bool {
        self.recall.as_ref().is_none_or(LanguageRecall::meets_bar)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RepositoryResult {
    pub name: String,
    pub language: String,
    pub panel: Option<String>,
    pub url: String,
    pub commit: String,
    pub declared_bucket: SizeBucket,
    pub measured_bucket: Option<SizeBucket>,
    pub production_lines: Option<u64>,
    pub skipped_files: Option<u64>,
    pub exit_code: Option<i32>,
    pub wall_seconds: Vec<f64>,
    pub median_wall_seconds: Option<f64>,
    pub peak_rss_bytes: Option<u64>,
    pub production_lines_per_second: Option<f64>,
    pub framework_profiles: Vec<String>,
    pub metrics: BTreeMap<String, MetricReading>,
    pub oracle: OracleReport,
    pub error: Option<String>,
    pub passed: bool,
}

impl RepositoryResult {
    pub(crate) fn pending(entry: &CorpusEntry) -> RepositoryResult {
        RepositoryResult {
            name: entry.name.clone(),
            language: entry.language.clone(),
            panel: entry.panel.clone(),
            url: entry.url.clone(),
            commit: entry.commit.clone(),
            declared_bucket: entry.bucket,
            measured_bucket: None,
            production_lines: None,
            skipped_files: None,
            exit_code: None,
            wall_seconds: Vec::new(),
            median_wall_seconds: None,
            peak_rss_bytes: None,
            production_lines_per_second: None,
            framework_profiles: Vec::new(),
            metrics: BTreeMap::new(),
            oracle: OracleReport::not_applicable(),
            error: None,
            passed: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LanguageSummary {
    pub language: String,
    pub repositories: u64,
    pub oracle_status: OracleStatus,
    pub oracle_reason: Option<String>,
    pub recall: Option<LanguageRecall>,
    pub passed: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PerformanceSummary {
    pub largest_repository: Option<String>,
    pub largest_production_lines: Option<u64>,
    pub largest_median_wall_seconds: Option<f64>,
    pub largest_peak_rss_bytes: Option<u64>,
    pub highest_peak_rss_bytes: Option<u64>,
    pub median_production_lines_per_second: Option<f64>,
}
