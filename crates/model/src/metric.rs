use std::borrow::Cow;
use std::collections::BTreeMap;

use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::{Deserialize, Serialize};

use crate::{CodeModel, RepoPath, SourceSet};

#[cfg(test)]
mod tests;

pub const STRUCTURAL_REPORT_KEYS: [&str; 3] = ["files", "source_lines", "packages"];

pub fn is_reserved_report_key(key: &str) -> bool {
    Metric::from_key(key).is_some() || STRUCTURAL_REPORT_KEYS.contains(&key)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MetricKind {
    SoftwareEntropy,
    Hazards,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MetricScope {
    AllSourceSets,
    ProductionOnly,
    RepositoryWide,
}

impl MetricScope {
    pub const fn applies_to_source_set(self, source_set: SourceSet) -> bool {
        match self {
            MetricScope::AllSourceSets => true,
            MetricScope::ProductionOnly => matches!(source_set, SourceSet::Production),
            MetricScope::RepositoryWide => false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NewFindingsRule {
    AnyNewFinding,
    #[serde(alias = "numerator_rise_with_new_findings")]
    NumeratorRise,
    Never,
}

impl JsonSchema for NewFindingsRule {
    fn schema_name() -> Cow<'static, str> {
        Cow::Borrowed("NewFindingsRule")
    }

    fn schema_id() -> Cow<'static, str> {
        Cow::Borrowed(concat!(module_path!(), "::NewFindingsRule"))
    }

    fn json_schema(_generator: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "type": "string",
            "enum": [
                "any_new_finding",
                "numerator_rise",
                "numerator_rise_with_new_findings",
                "never"
            ]
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MetricCapability {
    None,
    Imports,
    HazardLines,
    CoverageReport,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MetricDescriptor {
    pub key: &'static str,
    pub display_name: &'static str,
    pub version: u32,
    pub kind: MetricKind,
    pub scope: MetricScope,
    pub supports_diff_regression: bool,
    pub default_new_findings_rule: NewFindingsRule,
    pub required_capability: MetricCapability,
    pub numerator_field: &'static str,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(usize)]
pub enum Metric {
    Erosion = 0,
    CognitiveComplexity = 1,
    Duplication = 2,
    Verbosity = 3,
    ImportCycles = 4,
    Boundaries = 5,
    OrphanFiles = 6,
    TestGap = 7,
    Hazards = 8,
    UnusedDependencies = 9,
    UnlistedDependencies = 10,
    DevInProductionDependencies = 11,
}

impl Metric {
    pub const DESCRIPTORS: [(Metric, MetricDescriptor); 12] = [
        (
            Metric::Erosion,
            MetricDescriptor {
                key: "erosion",
                display_name: "Erosion",
                version: 1,
                kind: MetricKind::SoftwareEntropy,
                scope: MetricScope::AllSourceSets,
                supports_diff_regression: true,
                default_new_findings_rule: NewFindingsRule::NumeratorRise,
                required_capability: MetricCapability::None,
                numerator_field: "eroded_function_mass",
            },
        ),
        (
            Metric::CognitiveComplexity,
            MetricDescriptor {
                key: "cognitive_complexity",
                display_name: "Cognitive complexity",
                version: 1,
                kind: MetricKind::SoftwareEntropy,
                scope: MetricScope::AllSourceSets,
                supports_diff_regression: true,
                default_new_findings_rule: NewFindingsRule::NumeratorRise,
                required_capability: MetricCapability::None,
                numerator_field: "complex_cognitive_complexity",
            },
        ),
        (
            Metric::Duplication,
            MetricDescriptor {
                key: "duplication",
                display_name: "Duplication",
                version: 2,
                kind: MetricKind::SoftwareEntropy,
                scope: MetricScope::AllSourceSets,
                supports_diff_regression: true,
                default_new_findings_rule: NewFindingsRule::NumeratorRise,
                required_capability: MetricCapability::None,
                numerator_field: "clone_lines",
            },
        ),
        (
            Metric::Verbosity,
            MetricDescriptor {
                key: "verbosity",
                display_name: "Verbosity",
                version: 2,
                kind: MetricKind::SoftwareEntropy,
                scope: MetricScope::AllSourceSets,
                supports_diff_regression: true,
                default_new_findings_rule: NewFindingsRule::NumeratorRise,
                required_capability: MetricCapability::None,
                numerator_field: "verbose_lines",
            },
        ),
        (
            Metric::ImportCycles,
            MetricDescriptor {
                key: "import_cycles",
                display_name: "Import cycles",
                version: 1,
                kind: MetricKind::SoftwareEntropy,
                scope: MetricScope::AllSourceSets,
                supports_diff_regression: true,
                default_new_findings_rule: NewFindingsRule::AnyNewFinding,
                required_capability: MetricCapability::Imports,
                numerator_field: "modules_in_import_cycles",
            },
        ),
        (
            Metric::Boundaries,
            MetricDescriptor {
                key: "boundaries",
                display_name: "Import boundaries",
                version: 2,
                kind: MetricKind::SoftwareEntropy,
                scope: MetricScope::AllSourceSets,
                supports_diff_regression: true,
                default_new_findings_rule: NewFindingsRule::AnyNewFinding,
                required_capability: MetricCapability::Imports,
                numerator_field: "boundary_findings",
            },
        ),
        (
            Metric::OrphanFiles,
            MetricDescriptor {
                key: "orphan_files",
                display_name: "Orphan files",
                version: 1,
                kind: MetricKind::SoftwareEntropy,
                scope: MetricScope::ProductionOnly,
                supports_diff_regression: true,
                default_new_findings_rule: NewFindingsRule::AnyNewFinding,
                required_capability: MetricCapability::Imports,
                numerator_field: "orphan_files",
            },
        ),
        (
            Metric::TestGap,
            MetricDescriptor {
                key: "test_gap",
                display_name: "Test gap",
                version: 1,
                kind: MetricKind::SoftwareEntropy,
                scope: MetricScope::ProductionOnly,
                supports_diff_regression: false,
                default_new_findings_rule: NewFindingsRule::NumeratorRise,
                required_capability: MetricCapability::CoverageReport,
                numerator_field: "untested_function_mass",
            },
        ),
        (
            Metric::Hazards,
            MetricDescriptor {
                key: "hazards",
                display_name: "Hazards",
                version: 1,
                kind: MetricKind::Hazards,
                scope: MetricScope::AllSourceSets,
                supports_diff_regression: true,
                default_new_findings_rule: NewFindingsRule::AnyNewFinding,
                required_capability: MetricCapability::HazardLines,
                numerator_field: "hazard_lines",
            },
        ),
        (
            Metric::UnusedDependencies,
            MetricDescriptor {
                key: "unused_dependencies",
                display_name: "Unused dependencies",
                version: 1,
                kind: MetricKind::SoftwareEntropy,
                scope: MetricScope::RepositoryWide,
                supports_diff_regression: true,
                default_new_findings_rule: NewFindingsRule::AnyNewFinding,
                required_capability: MetricCapability::Imports,
                numerator_field: "unused",
            },
        ),
        (
            Metric::UnlistedDependencies,
            MetricDescriptor {
                key: "unlisted_dependencies",
                display_name: "Unlisted dependencies",
                version: 1,
                kind: MetricKind::SoftwareEntropy,
                scope: MetricScope::RepositoryWide,
                supports_diff_regression: true,
                default_new_findings_rule: NewFindingsRule::AnyNewFinding,
                required_capability: MetricCapability::Imports,
                numerator_field: "unlisted",
            },
        ),
        (
            Metric::DevInProductionDependencies,
            MetricDescriptor {
                key: "dev_in_production_dependencies",
                display_name: "Dev dependencies in production",
                version: 1,
                kind: MetricKind::SoftwareEntropy,
                scope: MetricScope::RepositoryWide,
                supports_diff_regression: true,
                default_new_findings_rule: NewFindingsRule::AnyNewFinding,
                required_capability: MetricCapability::Imports,
                numerator_field: "dev_in_production",
            },
        ),
    ];

    pub const ALL: [Metric; 12] = [
        Metric::DESCRIPTORS[0].0,
        Metric::DESCRIPTORS[1].0,
        Metric::DESCRIPTORS[2].0,
        Metric::DESCRIPTORS[3].0,
        Metric::DESCRIPTORS[4].0,
        Metric::DESCRIPTORS[5].0,
        Metric::DESCRIPTORS[6].0,
        Metric::DESCRIPTORS[7].0,
        Metric::DESCRIPTORS[8].0,
        Metric::DESCRIPTORS[9].0,
        Metric::DESCRIPTORS[10].0,
        Metric::DESCRIPTORS[11].0,
    ];

    pub const BUILT_IN_KEYS: [&'static str; 12] = [
        Metric::DESCRIPTORS[0].1.key,
        Metric::DESCRIPTORS[1].1.key,
        Metric::DESCRIPTORS[2].1.key,
        Metric::DESCRIPTORS[3].1.key,
        Metric::DESCRIPTORS[4].1.key,
        Metric::DESCRIPTORS[5].1.key,
        Metric::DESCRIPTORS[6].1.key,
        Metric::DESCRIPTORS[7].1.key,
        Metric::DESCRIPTORS[8].1.key,
        Metric::DESCRIPTORS[9].1.key,
        Metric::DESCRIPTORS[10].1.key,
        Metric::DESCRIPTORS[11].1.key,
    ];

    pub const fn descriptor(self) -> &'static MetricDescriptor {
        &Metric::DESCRIPTORS[self.index()].1
    }

    pub const fn key(self) -> &'static str {
        self.descriptor().key
    }

    pub const fn display_name(self) -> &'static str {
        self.descriptor().display_name
    }

    pub const fn version(self) -> u32 {
        self.descriptor().version
    }

    pub const fn kind(self) -> MetricKind {
        self.descriptor().kind
    }

    pub const fn scope(self) -> MetricScope {
        self.descriptor().scope
    }

    pub const fn supports_diff_regression(self) -> bool {
        self.descriptor().supports_diff_regression
    }

    pub const fn default_new_findings_rule(self) -> NewFindingsRule {
        self.descriptor().default_new_findings_rule
    }

    pub const fn required_capability(self) -> MetricCapability {
        self.descriptor().required_capability
    }

    pub const fn numerator_field(self) -> &'static str {
        self.descriptor().numerator_field
    }

    pub fn from_key(key: &str) -> Option<Metric> {
        Metric::DESCRIPTORS
            .iter()
            .find(|(_, descriptor)| descriptor.key == key)
            .map(|(metric, _)| *metric)
    }

    pub fn built_in_versions() -> BTreeMap<&'static str, u32> {
        Metric::DESCRIPTORS
            .iter()
            .map(|(_, descriptor)| (descriptor.key, descriptor.version))
            .collect()
    }

    pub fn versions_with_plugins(metric_plugins: &[&dyn MetricPlugin]) -> BTreeMap<String, u32> {
        let mut versions: BTreeMap<String, u32> = Metric::DESCRIPTORS
            .iter()
            .map(|(_, descriptor)| (descriptor.key.to_owned(), descriptor.version))
            .collect();
        for plugin in metric_plugins {
            versions.insert(plugin.key().to_owned(), plugin.version());
        }
        versions
    }

    pub const fn index(self) -> usize {
        self as usize
    }
}

impl SourceSet {
    pub const ALL: [SourceSet; 4] = [
        SourceSet::Production,
        SourceSet::Test,
        SourceSet::Generated,
        SourceSet::Vendored,
    ];

    pub const fn key(self) -> &'static str {
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
    #[error("Metric plugin {plugin:?} returned a Finding with an empty id")]
    EmptyFindingId { plugin: String },
    #[error(
        "Metric plugin {plugin:?} returned an invalid {field} {value}; it must be finite and non-negative"
    )]
    InvalidNumber {
        plugin: String,
        field: &'static str,
        value: f64,
    },
    #[error("Metric plugin {plugin:?} returned a Finding with line 0; lines start at 1")]
    InvalidFindingLine { plugin: String },
    #[error("Metric plugin {plugin:?} failed during measure: {reason}")]
    Call { plugin: String, reason: String },
    #[error("Metric plugin {plugin:?} returned a Finding with an invalid path {path:?}")]
    InvalidFindingPath { plugin: String, path: String },
    #[error("Metric plugin {plugin:?} exceeded its memory cap of {memory_mb} MiB")]
    MemoryLimitExceeded { plugin: String, memory_mb: u64 },
    #[error("Metric plugin {plugin:?} exceeded its time limit of {timeout_ms} ms")]
    TimeLimitExceeded { plugin: String, timeout_ms: u64 },
    #[error(
        "Metric plugin {plugin:?} returned an invalid ratio {ratio}; it must be finite and between 0 and 1"
    )]
    InvalidRatio { plugin: String, ratio: f64 },
}

pub trait MetricPlugin: Sync {
    fn key(&self) -> &str;

    fn name(&self) -> &str;

    fn description(&self) -> &str;

    fn version(&self) -> u32;

    fn measure(&self, model: &CodeModel) -> Result<ExternalMetricMeasurement, MetricPluginError>;
}
