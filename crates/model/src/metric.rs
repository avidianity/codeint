use crate::{CodeModel, RepoPath, SourceSet};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Metric {
    Erosion,
    Duplication,
    Verbosity,
    ImportCycles,
}

impl Metric {
    pub const BUILT_IN_KEYS: [&'static str; 4] =
        ["erosion", "duplication", "verbosity", "import_cycles"];

    pub fn key(self) -> &'static str {
        match self {
            Metric::Erosion => "erosion",
            Metric::Duplication => "duplication",
            Metric::Verbosity => "verbosity",
            Metric::ImportCycles => "import_cycles",
        }
    }
}

impl SourceSet {
    pub fn key(self) -> &'static str {
        match self {
            SourceSet::Production => "production",
            SourceSet::Test => "test",
            SourceSet::Generated => "generated",
            SourceSet::Vendored => "vendored",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExternalFinding {
    pub id: String,
    pub path: RepoPath,
    pub line: u32,
    pub end_line: Option<u32>,
    pub message: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ExternalMetricMeasurement {
    pub ratio: f64,
    pub numerator: f64,
    pub denominator: f64,
    pub findings: Vec<ExternalFinding>,
}

#[derive(Debug, thiserror::Error)]
pub enum MetricPluginError {
    #[error("Metric plugin {plugin:?} failed during measure: {reason}")]
    Call { plugin: String, reason: String },
    #[error(
        "Metric plugin {plugin:?} returned an invalid ratio {ratio}; it must be finite and between 0 and 1"
    )]
    InvalidRatio { plugin: String, ratio: f64 },
    #[error(
        "Metric plugin {plugin:?} returned an invalid {field} {value}; it must be finite and non-negative"
    )]
    InvalidNumber {
        plugin: String,
        field: &'static str,
        value: f64,
    },
    #[error("Metric plugin {plugin:?} returned a Finding with an empty id")]
    EmptyFindingId { plugin: String },
    #[error("Metric plugin {plugin:?} returned a Finding with an invalid path {path:?}")]
    InvalidFindingPath { plugin: String, path: String },
    #[error("Metric plugin {plugin:?} returned a Finding with line 0; lines start at 1")]
    InvalidFindingLine { plugin: String },
}

pub trait MetricPlugin: Sync {
    fn key(&self) -> &str;

    fn name(&self) -> &str;

    fn description(&self) -> &str;

    fn measure(&self, model: &CodeModel) -> Result<ExternalMetricMeasurement, MetricPluginError>;
}
