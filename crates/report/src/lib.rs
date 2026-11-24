#![forbid(unsafe_code)]

use std::collections::{BTreeMap, HashSet};

use codeint_config::{
    AgentFormat, Boundaries as BoundariesConfig, BoundaryDirective, BoundaryRuleDecision, Budgets,
    Codeowners, ConfigFormat, DependenciesConfig, DoctorFormat, DuplicationConfig, FindingsShown,
    GuardFormat, ImpactFormat, ReportFormat, TraceFormat,
};
use codeint_metrics::{
    Boundaries, BoundaryEdgeCheck, BoundaryFinding, BoundaryPolicy, CloneFamily, CloneGroup,
    CognitiveComplexity, ComplexFunction, DEV_IN_PRODUCTION_DEPENDENCIES_METRIC_KEY,
    DependencyHygiene, Duplication, ErodedFunction, Erosion, FlaggedLineFinding, Fragment,
    ImportCycle, ImportCycles, MinimumCloneTokens, MinimumDistinctTokens, NearCloneConfig,
    OrphanFileFinding, OrphanFiles, UNLISTED_DEPENDENCIES_METRIC_KEY,
    UNUSED_DEPENDENCIES_METRIC_KEY, UnresolvedImport, Verbosity,
};
use codeint_model::{
    CodeModel, ExternalFinding, Metric, MetricPlugin, MetricPluginError, Package, ReferenceRange,
    ReferenceRanges, RepoPath, SkippedFile, SourceSet, SourceSets,
};
use serde::ser::SerializeMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

mod agent;
mod churn;
mod config;
mod coupling;
mod dependencies;
mod diff;
mod doctor;
mod guard;
mod impact;
mod markdown;
mod measurement;
mod ownership;
mod sarif;
mod saved;
mod schema;
#[cfg(test)]
mod tests;
mod trace;
mod trend;

pub use agent::{
    AgentInstallAction, AgentInstallEntry, AgentInstallReport, AgentStatusEntry, AgentStatusReport,
};
pub use churn::{ChurnHistoryInput, ChurnReport};
pub use codeint_metrics::TraceError;
pub use config::ConfigReport;
pub use coupling::{CouplingHistoryInput, CouplingReport};
pub use diff::{BaseCommit, DiffReport, DiffReportOptions, Head, HeadCommit};
pub use doctor::{DoctorCheck, DoctorReport, DoctorStatus, DoctorSummary};
pub use guard::{GuardAllowedTarget, GuardDeniedTarget, GuardReport};
pub use impact::{ChangedBase, ImpactReport};
pub use measurement::CommitMeasurement;
pub use ownership::OwnershipOptions;
pub use saved::{SavedReport, SavedReportError};
pub use schema::{ExitCodeSchema, LanguageSchema, McpToolSchema, MetricSchema, SchemaReport};
pub use trace::{TraceMultiFileImport, TraceReport};
pub use trend::{TrendPoint, TrendReport};

use dependencies::{ImportedDependencyRow, UnusedDependencyRow};
use ownership::OwnedFinding;

const DECIMAL_SCALE: f64 = 10_000.0;
const COGNITIVE_COMPLEXITY_KEY: &str = "cognitive_complexity";
const NOT_PUBLISHED_FOR_THIS_LANGUAGE: &str = "no published Reference range for this language";
const NOT_SHOWN_FOR_SEVERAL_LANGUAGES: &str =
    "not shown, because the measured files are in more than one language";
const NOT_SHOWN_FOR_NEAR_CLONES: &str = "not shown, because near-clone detection is enabled (Reference ranges are calibrated for exact Clone groups)";
const NO_IMPORTS_METRIC_NOTE: &str = "This language has no imports, so Import cycles, Orphan files, Import boundaries, and Dependency hygiene are not measured.";

#[derive(Debug, thiserror::Error)]
pub enum ReportError {
    #[error("cannot render the report as TOON")]
    Toon(#[source] toon_format::ToonError),
    #[error("cannot render the report as JSON")]
    Json(#[source] serde_json::Error),
    #[error("cannot render the report as SARIF")]
    Sarif(#[source] serde_json::Error),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Report {
    #[serde(flatten)]
    measured: MeasuredSection,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    languages: BTreeMap<String, MeasuredSection>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    skipped: Vec<SkippedEntry>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    declaration_only: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    submodules: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    over_budget: Vec<BudgetFailure>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BudgetFailure {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    source_set: Option<String>,
    metric: String,
    ratio: Decimal,
    budget: f64,
}

pub struct MeasuredLanguage {
    pub plugin: String,
    pub model: CodeModel,
    pub reference_ranges: ReferenceRanges,
    pub has_imports: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct MeasuredSection {
    files: usize,
    source_lines: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    import_metrics_note: Option<String>,
    source_sets: SourceSetSections,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    dependencies: Option<DependenciesSection>,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct DependenciesSection {
    pub(crate) unused_dependencies: UnusedDependenciesSection,
    pub(crate) unlisted_dependencies: UnlistedDependenciesSection,
    pub(crate) dev_in_production_dependencies: DevInProductionDependenciesSection,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct UnusedDependenciesSection {
    pub(crate) ratio: Decimal,
    pub(crate) unused: usize,
    pub(crate) declared: usize,
    #[serde(flatten)]
    pub(crate) findings: Findings<UnusedDependencyRow>,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct UnlistedDependenciesSection {
    pub(crate) ratio: Decimal,
    pub(crate) unlisted: usize,
    pub(crate) imported: usize,
    #[serde(flatten)]
    pub(crate) findings: Findings<ImportedDependencyRow>,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct DevInProductionDependenciesSection {
    pub(crate) ratio: Decimal,
    pub(crate) dev_in_production: usize,
    pub(crate) production_imported: usize,
    #[serde(flatten)]
    pub(crate) findings: Findings<ImportedDependencyRow>,
}

enum ReferenceRangeScope {
    OneLanguage(ReferenceRanges),
    SeveralLanguages,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
enum ReferenceRangeEntry {
    Published {
        low: Decimal,
        high: Decimal,
        source: String,
    },
    NotShown(String),
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(bound(deserialize = "T: Deserialize<'de>"))]
pub(crate) struct PerSourceSet<T> {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    production: Option<T>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    test: Option<T>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    generated: Option<T>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    vendored: Option<T>,
}

impl<T> Default for PerSourceSet<T> {
    fn default() -> PerSourceSet<T> {
        PerSourceSet {
            production: None,
            test: None,
            generated: None,
            vendored: None,
        }
    }
}

impl<T> PerSourceSet<T> {
    pub(crate) fn iter(&self) -> impl Iterator<Item = (SourceSet, &T)> {
        [
            (SourceSet::Production, &self.production),
            (SourceSet::Test, &self.test),
            (SourceSet::Generated, &self.generated),
            (SourceSet::Vendored, &self.vendored),
        ]
        .into_iter()
        .filter_map(|(source_set, section)| Some((source_set, section.as_ref()?)))
    }

    pub(crate) fn iter_mut(&mut self) -> impl Iterator<Item = &mut T> {
        [
            &mut self.production,
            &mut self.test,
            &mut self.generated,
            &mut self.vendored,
        ]
        .into_iter()
        .flatten()
    }

    pub(crate) fn named(&self) -> Vec<(&'static str, &T)> {
        self.iter()
            .map(|(source_set, section)| (source_set_label(source_set), section))
            .collect()
    }

    pub(crate) fn slot(&mut self, source_set: SourceSet) -> &mut Option<T> {
        match source_set {
            SourceSet::Production => &mut self.production,
            SourceSet::Test => &mut self.test,
            SourceSet::Generated => &mut self.generated,
            SourceSet::Vendored => &mut self.vendored,
        }
    }
}

fn source_set_label(source_set: SourceSet) -> &'static str {
    match source_set {
        SourceSet::Production => "Production",
        SourceSet::Test => "Test",
        SourceSet::Generated => "Generated",
        SourceSet::Vendored => "Vendored",
    }
}

type SourceSetSections = PerSourceSet<SourceSetSection>;

#[derive(Debug, Serialize, Deserialize)]
struct SourceSetSection {
    files: usize,
    source_lines: u64,
    erosion: MetricSection<ErosionTotals, ErodedFunctionFinding>,
    cognitive_complexity: MetricSection<CognitiveComplexityTotals, CognitiveComplexityFinding>,
    duplication: DuplicationSection,
    verbosity: MetricSection<VerbosityTotals, FlaggedLineRow>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    import_cycles: Option<ImportCyclesSection>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    boundaries: Option<BoundariesSection>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    orphan_files: Option<OrphanFilesSection>,
    #[serde(default, flatten)]
    external_metrics: BTreeMap<String, ExternalMetricSection>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    packages: Vec<PackageSection>,
}

#[derive(Debug, Serialize, Deserialize)]
struct OrphanFilesSection {
    ratio: Decimal,
    orphan_files: usize,
    evaluated_files: usize,
    entry_points: usize,
    unreferenced: usize,
    test_only: usize,
    #[serde(flatten)]
    findings: Findings<OrphanFileRow>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OrphanFileRow {
    id: String,
    path: String,
    kind: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    test_importers: Vec<String>,
    message: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    owners: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct BoundariesSection {
    ratio: Decimal,
    resolved_imports: usize,
    #[serde(flatten)]
    findings: Findings<BoundaryFindingRow>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BoundaryFindingRow {
    id: String,
    kind: String,
    importer: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    target: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    specifier: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    source_zone: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    target_zone: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<String>,
    message: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    owners: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct MetricHeader {
    ratio: Decimal,
    reference_range: ReferenceRangeEntry,
}

impl MetricHeader {
    fn new(ratio: f64, ranges: &ReferenceRangeScope, key: &str) -> MetricHeader {
        MetricHeader {
            ratio: Decimal::rounded(ratio),
            reference_range: ranges.entry(key),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct ExternalMetricSection {
    #[serde(skip)]
    name: String,
    #[serde(skip)]
    description: String,
    #[serde(flatten)]
    header: MetricHeader,
    numerator: Decimal,
    denominator: Decimal,
    #[serde(flatten)]
    findings: Findings<ExternalMetricFinding>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ExternalMetricFinding {
    id: String,
    path: String,
    line: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    end_line: Option<u32>,
    message: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    owners: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct PackageSection {
    path: String,
    files: usize,
    source_lines: u64,
    erosion: PackageErosion,
    cognitive_complexity: PackageCognitiveComplexity,
    duplication: PackageDuplication,
    verbosity: PackageVerbosity,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    import_cycles: Option<PackageImportCycles>,
}

#[derive(Debug, Serialize, Deserialize)]
struct PackageErosion {
    ratio: Decimal,
    eroded_function_mass: Decimal,
    total_function_mass: Decimal,
}

#[derive(Debug, Serialize, Deserialize)]
struct PackageCognitiveComplexity {
    ratio: Decimal,
    complex_cognitive_complexity: u64,
    total_cognitive_complexity: u64,
}

#[derive(Debug, Serialize, Deserialize)]
struct PackageDuplication {
    ratio: Decimal,
    clone_lines: u64,
}

#[derive(Debug, Serialize, Deserialize)]
struct PackageVerbosity {
    ratio: Decimal,
    verbose_lines: u64,
}

#[derive(Debug, Serialize, Deserialize)]
struct PackageImportCycles {
    ratio: Decimal,
    modules_in_import_cycles: usize,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(bound(deserialize = "Totals: Deserialize<'de>, Finding: FindingKind + Deserialize<'de>"))]
struct MetricSection<Totals, Finding: FindingKind> {
    #[serde(flatten)]
    header: MetricHeader,
    #[serde(flatten)]
    totals: Totals,
    #[serde(flatten)]
    findings: Findings<Finding>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ErosionTotals {
    eroded_function_mass: Decimal,
    total_function_mass: Decimal,
}

#[derive(Debug, Serialize, Deserialize)]
struct CognitiveComplexityTotals {
    complex_cognitive_complexity: u64,
    total_cognitive_complexity: u64,
}

#[derive(Debug, Serialize, Deserialize)]
struct VerbosityTotals {
    verbose_lines: u64,
    flagged_lines: u64,
    clone_lines: u64,
}

trait FindingKind: Serialize {
    const LIST_KEY: &'static str;
    const LEFT_OUT_KEY: &'static str = "findings_left_out";
    const SKIP_WHEN_EMPTY: bool = false;
}

#[derive(Debug)]
struct Findings<T> {
    listed: Vec<T>,
    left_out: usize,
}

#[derive(Debug, Serialize, Deserialize)]
struct ErodedFunctionFinding {
    id: String,
    path: String,
    line: u32,
    end_line: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    cyclomatic_complexity: u32,
    cognitive_complexity: u32,
    source_lines: u32,
    function_mass: Decimal,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    owners: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct CognitiveComplexityFinding {
    id: String,
    path: String,
    line: u32,
    end_line: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    cognitive_complexity: u32,
    cyclomatic_complexity: u32,
    source_lines: u32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    owners: Vec<String>,
}

#[derive(Debug, Serialize)]
struct DuplicationSection {
    #[serde(flatten)]
    header: MetricHeader,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    near_clones: Option<NearClonesSummary>,
    clone_lines: u64,
    #[serde(flatten)]
    clone_groups: Findings<CloneGroupFinding>,
    #[serde(flatten)]
    clone_families: Findings<CloneFamilyFinding>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    unlisted_clone_family_group_ids: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct NearClonesSummary {
    min_similarity: f64,
}

impl<'de> Deserialize<'de> for DuplicationSection {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct RawDuplicationSection {
            ratio: Decimal,
            reference_range: ReferenceRangeEntry,
            #[serde(default)]
            near_clones: Option<NearClonesSummary>,
            clone_lines: u64,
            clone_groups: Vec<CloneGroupFinding>,
            #[serde(default)]
            findings_left_out: usize,
            #[serde(default)]
            clone_families: Vec<CloneFamilyFinding>,
            #[serde(default)]
            clone_families_left_out: usize,
            #[serde(default)]
            unlisted_clone_family_group_ids: Vec<String>,
        }

        let raw = RawDuplicationSection::deserialize(deserializer)?;
        Ok(DuplicationSection {
            header: MetricHeader {
                ratio: raw.ratio,
                reference_range: raw.reference_range,
            },
            near_clones: raw.near_clones,
            clone_lines: raw.clone_lines,
            clone_groups: Findings {
                listed: raw.clone_groups,
                left_out: raw.findings_left_out,
            },
            clone_families: Findings {
                listed: raw.clone_families,
                left_out: raw.clone_families_left_out,
            },
            unlisted_clone_family_group_ids: raw.unlisted_clone_family_group_ids,
        })
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct CloneGroupFinding {
    id: String,
    tokens: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    similarity: Option<Decimal>,
    locations: Vec<CloneLocation>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    owners: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CloneFamilyFinding {
    id: String,
    duplicated_lines: u64,
    clone_group_ids: Vec<String>,
    locations: Vec<CloneLocation>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    owners: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CloneLocation {
    path: String,
    start_line: u32,
    end_line: u32,
}

#[derive(Debug, Serialize, Deserialize)]
struct FlaggedLineRow {
    id: String,
    path: String,
    line: u32,
    rule: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    owners: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ImportCyclesSection {
    #[serde(flatten)]
    header: MetricHeader,
    modules_in_import_cycles: usize,
    modules: usize,
    #[serde(flatten)]
    findings: Findings<ImportCycleFinding>,
    unresolved_imports: UnresolvedImportsSection,
}

#[derive(Debug, Serialize, Deserialize)]
struct UnresolvedImportsSection {
    count: usize,
    #[serde(flatten)]
    imports: Findings<UnresolvedImportRow>,
}

#[derive(Debug, Serialize, Deserialize)]
struct UnresolvedImportRow {
    importer: String,
    specifier: String,
    reason: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    owners: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ImportCycleFinding {
    id: String,
    modules: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    owners: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(transparent)]
struct Decimal(f64);

#[derive(Debug, Serialize, Deserialize)]
struct SkippedEntry {
    path: String,
    reason: String,
}

impl std::fmt::Display for BudgetFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.source_set.as_deref() {
            Some(source_set) => write!(
                formatter,
                "the {source_set} {} ratio {} is over its Budget {}",
                self.metric, self.ratio.0, self.budget
            ),
            None => write!(
                formatter,
                "the {} ratio {} is over its Budget {}",
                self.metric, self.ratio.0, self.budget
            ),
        }
    }
}

fn over_budget<'a>(
    budgets: &Budgets,
    displayed_ratios: impl Iterator<Item = (SourceSet, &'a str, f64)>,
) -> Vec<BudgetFailure> {
    displayed_ratios
        .filter_map(|(source_set, metric, ratio)| {
            let budget = budgets.budget_for_key(source_set, metric)?;
            (!budget.admits(ratio)).then(|| BudgetFailure {
                source_set: Some(source_set.key().to_owned()),
                metric: metric.to_owned(),
                ratio: Decimal(ratio),
                budget: budget.ratio(),
            })
        })
        .collect()
}

impl Decimal {
    fn rounded(value: f64) -> Decimal {
        let rounded = (value * DECIMAL_SCALE).round() / DECIMAL_SCALE;
        let without_negative_zero = if rounded == 0.0 { 0.0 } else { rounded };
        Decimal(without_negative_zero)
    }
}

impl ReferenceRangeScope {
    fn entry(&self, metric: &str) -> ReferenceRangeEntry {
        match self {
            ReferenceRangeScope::OneLanguage(ranges) => ranges.for_metric(metric).map_or_else(
                || ReferenceRangeEntry::NotShown(NOT_PUBLISHED_FOR_THIS_LANGUAGE.to_owned()),
                ReferenceRangeEntry::from,
            ),
            ReferenceRangeScope::SeveralLanguages => {
                ReferenceRangeEntry::NotShown(NOT_SHOWN_FOR_SEVERAL_LANGUAGES.to_owned())
            }
        }
    }
}

impl From<&ReferenceRange> for ReferenceRangeEntry {
    fn from(range: &ReferenceRange) -> ReferenceRangeEntry {
        ReferenceRangeEntry::Published {
            low: Decimal::rounded(range.low),
            high: Decimal::rounded(range.high),
            source: range.source.clone(),
        }
    }
}

impl<T> Findings<T> {
    fn worst_first(
        worst_first: impl ExactSizeIterator<Item = T>,
        shown: FindingsShown,
    ) -> Findings<T> {
        let total = worst_first.len();
        let listed: Vec<T> = match shown {
            FindingsShown::WorstPerMetric(limit) => worst_first.take(limit).collect(),
            FindingsShown::All => worst_first.collect(),
        };
        Findings {
            left_out: total - listed.len(),
            listed,
        }
    }

    fn worst_first_owned(
        items: impl Iterator<Item = T>,
        ownership: OwnershipOptions<'_>,
        shown: FindingsShown,
    ) -> Findings<T>
    where
        T: OwnedFinding,
    {
        let filtered = ownership.apply(items.collect());
        Findings::worst_first(filtered.into_iter(), shown)
    }
}

impl<T: FindingKind> Serialize for Findings<T> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(None)?;
        if !(T::SKIP_WHEN_EMPTY && self.listed.is_empty() && self.left_out == 0) {
            map.serialize_entry(T::LIST_KEY, &self.listed)?;
        }
        if self.left_out > 0 {
            map.serialize_entry(T::LEFT_OUT_KEY, &self.left_out)?;
        }
        map.end()
    }
}

impl<'de, T: FindingKind + Deserialize<'de>> Deserialize<'de> for Findings<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct FindingsVisitor<T>(std::marker::PhantomData<T>);

        impl<'de, T: FindingKind + Deserialize<'de>> serde::de::Visitor<'de> for FindingsVisitor<T> {
            type Value = Findings<T>;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(formatter, "findings map with {}", T::LIST_KEY)
            }

            fn visit_map<A: serde::de::MapAccess<'de>>(
                self,
                mut map: A,
            ) -> Result<Self::Value, A::Error> {
                let mut listed = None;
                let mut left_out = 0;
                while let Some(key) = map.next_key::<String>()? {
                    if key == T::LIST_KEY {
                        listed = Some(map.next_value()?);
                    } else if key == T::LEFT_OUT_KEY {
                        left_out = map.next_value()?;
                    } else {
                        let _ = map.next_value::<serde::de::IgnoredAny>()?;
                    }
                }
                let listed = listed
                    .or_else(|| T::SKIP_WHEN_EMPTY.then(Vec::new))
                    .ok_or_else(|| serde::de::Error::missing_field(T::LIST_KEY))?;
                Ok(Findings { listed, left_out })
            }
        }

        deserializer.deserialize_map(FindingsVisitor(std::marker::PhantomData))
    }
}

impl FindingKind for ErodedFunctionFinding {
    const LIST_KEY: &'static str = "findings";
}

impl FindingKind for CognitiveComplexityFinding {
    const LIST_KEY: &'static str = "findings";
}

impl FindingKind for CloneGroupFinding {
    const LIST_KEY: &'static str = "clone_groups";
}

impl FindingKind for CloneFamilyFinding {
    const LIST_KEY: &'static str = "clone_families";
    const LEFT_OUT_KEY: &'static str = "clone_families_left_out";
    const SKIP_WHEN_EMPTY: bool = true;
}

impl FindingKind for FlaggedLineRow {
    const LIST_KEY: &'static str = "findings";
}

impl FindingKind for ImportCycleFinding {
    const LIST_KEY: &'static str = "findings";
}

impl FindingKind for UnresolvedImportRow {
    const LIST_KEY: &'static str = "imports";
    const LEFT_OUT_KEY: &'static str = "imports_left_out";
}

impl FindingKind for ExternalMetricFinding {
    const LIST_KEY: &'static str = "findings";
}

macro_rules! impl_single_path_owner {
    ($ty:ty, $field:ident) => {
        impl OwnedFinding for $ty {
            fn assign_owners(&mut self, codeowners: &Codeowners) {
                self.owners = codeowners.owners_for(&self.$field);
            }

            fn owners(&self) -> &[String] {
                &self.owners
            }
        }
    };
}

impl_single_path_owner!(ErodedFunctionFinding, path);
impl_single_path_owner!(CognitiveComplexityFinding, path);
impl_single_path_owner!(FlaggedLineRow, path);
impl_single_path_owner!(UnresolvedImportRow, importer);
impl_single_path_owner!(ExternalMetricFinding, path);
impl_single_path_owner!(BoundaryFindingRow, importer);
impl_single_path_owner!(OrphanFileRow, path);

impl OwnedFinding for CloneGroupFinding {
    fn assign_owners(&mut self, codeowners: &Codeowners) {
        self.owners =
            codeowners.owners_for_many(self.locations.iter().map(|loc| loc.path.as_str()));
    }

    fn owners(&self) -> &[String] {
        &self.owners
    }
}

impl OwnedFinding for CloneFamilyFinding {
    fn assign_owners(&mut self, codeowners: &Codeowners) {
        self.owners =
            codeowners.owners_for_many(self.locations.iter().map(|loc| loc.path.as_str()));
    }

    fn owners(&self) -> &[String] {
        &self.owners
    }
}

impl OwnedFinding for ImportCycleFinding {
    fn assign_owners(&mut self, codeowners: &Codeowners) {
        self.owners = codeowners.owners_for_many(self.modules.iter().map(String::as_str));
    }

    fn owners(&self) -> &[String] {
        &self.owners
    }
}

impl FindingKind for BoundaryFindingRow {
    const LIST_KEY: &'static str = "findings";
}

impl FindingKind for UnusedDependencyRow {
    const LIST_KEY: &'static str = "findings";
}

impl FindingKind for ImportedDependencyRow {
    const LIST_KEY: &'static str = "findings";
}

impl FindingKind for OrphanFileRow {
    const LIST_KEY: &'static str = "findings";
}

impl DependenciesSection {
    fn from_hygiene(
        hygiene: &DependencyHygiene,
        view: FindingsView<'_>,
    ) -> Option<DependenciesSection> {
        if !hygiene.has_activity() {
            return None;
        }
        Some(DependenciesSection {
            unused_dependencies: UnusedDependenciesSection {
                ratio: Decimal::rounded(hygiene.unused_ratio()),
                unused: hygiene.unused().len(),
                declared: hygiene.declared_count(),
                findings: view.map(hygiene.unused()),
            },
            unlisted_dependencies: UnlistedDependenciesSection {
                ratio: Decimal::rounded(hygiene.unlisted_ratio()),
                unlisted: hygiene.unlisted().len(),
                imported: hygiene.imported_count(),
                findings: view.map(hygiene.unlisted()),
            },
            dev_in_production_dependencies: DevInProductionDependenciesSection {
                ratio: Decimal::rounded(hygiene.dev_in_production_ratio()),
                dev_in_production: hygiene.dev_in_production().len(),
                production_imported: hygiene.production_imported_count(),
                findings: view.map(hygiene.dev_in_production()),
            },
        })
    }

    fn over_budget(&self, budgets: &Budgets) -> Vec<BudgetFailure> {
        dependency_over_budget(
            budgets,
            [
                (
                    UNUSED_DEPENDENCIES_METRIC_KEY,
                    self.unused_dependencies.ratio.0,
                ),
                (
                    UNLISTED_DEPENDENCIES_METRIC_KEY,
                    self.unlisted_dependencies.ratio.0,
                ),
                (
                    DEV_IN_PRODUCTION_DEPENDENCIES_METRIC_KEY,
                    self.dev_in_production_dependencies.ratio.0,
                ),
            ],
        )
    }
}

pub(crate) fn dependency_over_budget(
    budgets: &Budgets,
    displayed_ratios: [(&'static str, f64); 3],
) -> Vec<BudgetFailure> {
    displayed_ratios
        .into_iter()
        .filter_map(|(metric, ratio)| {
            let budget = budgets.dependency_budget(metric)?;
            (!budget.admits(ratio)).then(|| BudgetFailure {
                source_set: None,
                metric: metric.to_owned(),
                ratio: Decimal(ratio),
                budget: budget.ratio(),
            })
        })
        .collect()
}
struct ConfigBoundaryPolicy<'config>(&'config BoundariesConfig);

impl BoundaryPolicy for ConfigBoundaryPolicy<'_> {
    fn zone_of(&self, path: &RepoPath) -> Option<&str> {
        self.0.zone_of(path)
    }

    fn has_public_files(&self, zone: &str) -> bool {
        self.0.has_public_files(zone)
    }

    fn is_public_file(&self, zone: &str, path: &RepoPath) -> bool {
        self.0.is_public_file(zone, path)
    }

    fn check_import(&self, source_zone: &str, target_zone: &str) -> BoundaryEdgeCheck {
        match self.0.check_import(source_zone, target_zone) {
            BoundaryRuleDecision::Allowed => BoundaryEdgeCheck::Allowed,
            BoundaryRuleDecision::Denied {
                directive: BoundaryDirective::Allow,
                reason,
            } => BoundaryEdgeCheck::DeniedByAllowRule { reason },
            BoundaryRuleDecision::Denied {
                directive: BoundaryDirective::Deny,
                reason,
            } => BoundaryEdgeCheck::DeniedByDenyRule { reason },
        }
    }

    fn strict_zones(&self) -> bool {
        self.0.strict_zones()
    }
}

#[derive(Clone, Copy)]
struct FindingsView<'a> {
    shown: FindingsShown,
    ownership: OwnershipOptions<'a>,
}

impl FindingsView<'_> {
    fn select<T: OwnedFinding>(self, items: impl Iterator<Item = T>) -> Findings<T> {
        Findings::worst_first_owned(items, self.ownership, self.shown)
    }

    fn map<'a, T: 'a, R: From<&'a T> + OwnedFinding>(self, items: &'a [T]) -> Findings<R> {
        self.select(items.iter().map(R::from))
    }
}

impl BoundariesSection {
    fn new(boundaries: &Boundaries, view: FindingsView<'_>) -> BoundariesSection {
        BoundariesSection {
            ratio: Decimal::rounded(boundaries.ratio()),
            resolved_imports: boundaries.resolved_imports(),
            findings: view.map(boundaries.findings()),
        }
    }
}

impl From<&BoundaryFinding> for BoundaryFindingRow {
    fn from(finding: &BoundaryFinding) -> BoundaryFindingRow {
        BoundaryFindingRow {
            id: finding.id.to_string(),
            kind: finding.kind.key().to_owned(),
            importer: finding.importer.to_string(),
            target: finding.target.as_ref().map(RepoPath::to_string),
            specifier: finding.specifier.clone(),
            source_zone: finding.source_zone.clone(),
            target_zone: finding.target_zone.clone(),
            reason: finding.reason.clone(),
            message: finding.message.clone(),
            owners: Vec::new(),
        }
    }
}

impl ExternalMetricFinding {
    fn from_model(finding: &ExternalFinding) -> ExternalMetricFinding {
        ExternalMetricFinding {
            id: finding.id.clone(),
            path: finding.path.to_string(),
            line: finding.line,
            end_line: finding.end_line,
            message: finding.message.clone(),
            owners: Vec::new(),
        }
    }
}

impl OrphanFilesSection {
    fn new(orphan_files: &OrphanFiles, view: FindingsView<'_>) -> OrphanFilesSection {
        let all_findings = orphan_files.findings();
        let findings = view.map(all_findings);
        OrphanFilesSection {
            ratio: Decimal::rounded(orphan_files.ratio()),
            orphan_files: all_findings.len(),
            evaluated_files: orphan_files.evaluated_files(),
            entry_points: orphan_files.entry_points().len(),
            unreferenced: orphan_files.unreferenced_count(),
            test_only: orphan_files.test_only_count(),
            findings,
        }
    }
}

impl From<&OrphanFileFinding> for OrphanFileRow {
    fn from(finding: &OrphanFileFinding) -> OrphanFileRow {
        let test_importers = finding
            .test_importers
            .iter()
            .map(RepoPath::to_string)
            .collect();
        OrphanFileRow {
            id: finding.id.to_string(),
            path: finding.path.to_string(),
            kind: finding.kind.key().to_owned(),
            test_importers,
            message: finding.message.clone(),
            owners: Vec::new(),
        }
    }
}

#[derive(Clone, Copy)]
struct SectionOptions<'a> {
    findings: FindingsShown,
    metric_plugins: &'a [&'a dyn MetricPlugin],
    boundaries: Option<&'a BoundariesConfig>,
    duplication: DuplicationConfig,
    orphan_files: Option<&'a OrphanFiles>,
    ownership: OwnershipOptions<'a>,
    has_imports: bool,
}

impl SourceSetSections {
    fn new(
        source_sets: &SourceSets,
        import_source_sets: Option<&SourceSets>,
        ranges: &ReferenceRangeScope,
        options: SectionOptions<'_>,
    ) -> Result<SourceSetSections, MetricPluginError> {
        let mut sections = SourceSetSections::default();
        for (source_set, model) in source_sets.iter() {
            if !model.files.is_empty() {
                let import_model = import_source_sets
                    .map(|sets| sets.get(source_set))
                    .filter(|imports| !imports.files.is_empty());
                let source_set_options = SectionOptions {
                    boundaries: options.boundaries.filter(|config| {
                        import_model.is_some() && config.applies_to_source_set(source_set)
                    }),
                    orphan_files: options
                        .orphan_files
                        .filter(|_| import_model.is_some() && source_set == SourceSet::Production),
                    ..options
                };
                *sections.slot(source_set) = Some(SourceSetSection::new(
                    model,
                    import_model,
                    ranges,
                    source_set_options,
                )?);
            }
        }
        Ok(sections)
    }

    fn over_budget(&self, budgets: &Budgets) -> Vec<BudgetFailure> {
        over_budget(
            budgets,
            self.iter().flat_map(|(source_set, section)| {
                [
                    (
                        source_set,
                        Metric::Erosion.key(),
                        section.erosion.header.ratio.0,
                    ),
                    (
                        source_set,
                        COGNITIVE_COMPLEXITY_KEY,
                        section.cognitive_complexity.header.ratio.0,
                    ),
                    (
                        source_set,
                        Metric::Duplication.key(),
                        section.duplication.header.ratio.0,
                    ),
                    (
                        source_set,
                        Metric::Verbosity.key(),
                        section.verbosity.header.ratio.0,
                    ),
                ]
                .into_iter()
                .chain(section.import_cycles.iter().map(move |import_cycles| {
                    (
                        source_set,
                        Metric::ImportCycles.key(),
                        import_cycles.header.ratio.0,
                    )
                }))
                .chain(
                    section
                        .boundaries
                        .iter()
                        .map(move |boundaries| (source_set, "boundaries", boundaries.ratio.0)),
                )
                .chain(
                    section
                        .orphan_files
                        .iter()
                        .map(move |orphans| (source_set, "orphan_files", orphans.ratio.0)),
                )
                .chain(
                    section.external_metrics.iter().map(move |(key, metric)| {
                        (source_set, key.as_str(), metric.header.ratio.0)
                    }),
                )
            }),
        )
    }
}

impl SourceSetSection {
    fn new<'a>(
        model: &'a CodeModel,
        import_model: Option<&'a CodeModel>,
        ranges: &ReferenceRangeScope,
        options: SectionOptions<'_>,
    ) -> Result<SourceSetSection, MetricPluginError> {
        let view = FindingsView {
            shown: options.findings,
            ownership: options.ownership,
        };
        let effective_import_model = import_model.filter(|_| options.has_imports);
        let boundaries_config = effective_import_model
            .is_some()
            .then_some(options.boundaries)
            .flatten();
        let measured = Measured::new(
            model,
            effective_import_model,
            boundaries_config,
            options.duplication,
        );
        let boundaries = measured
            .boundaries
            .as_ref()
            .map(|evaluated| BoundariesSection::new(evaluated, view));
        let orphan_files = effective_import_model
            .is_some()
            .then_some(options.orphan_files)
            .flatten()
            .map(|measured| OrphanFilesSection::new(measured, view));
        let mut external_metrics = BTreeMap::new();
        for plugin in options.metric_plugins {
            let result = plugin.measure(model)?;
            external_metrics.insert(
                plugin.key().to_owned(),
                ExternalMetricSection {
                    name: plugin.name().to_owned(),
                    description: plugin.description().to_owned(),
                    header: MetricHeader::new(result.ratio, ranges, plugin.key()),
                    numerator: Decimal::rounded(result.numerator),
                    denominator: Decimal::rounded(result.denominator),
                    findings: view.select(
                        result
                            .findings
                            .iter()
                            .map(ExternalMetricFinding::from_model),
                    ),
                },
            );
        }
        Ok(SourceSetSection {
            files: model.files.len(),
            source_lines: source_lines(model),
            erosion: MetricSection {
                header: MetricHeader::new(measured.erosion.ratio(), ranges, Metric::Erosion.key()),
                totals: ErosionTotals {
                    eroded_function_mass: Decimal::rounded(measured.erosion.eroded_function_mass()),
                    total_function_mass: Decimal::rounded(measured.erosion.total_function_mass()),
                },
                findings: view.select(
                    measured
                        .erosion
                        .eroded_functions()
                        .iter()
                        .map(ErodedFunctionFinding::new),
                ),
            },
            cognitive_complexity: MetricSection {
                header: MetricHeader::new(
                    measured.cognitive_complexity.ratio(),
                    ranges,
                    COGNITIVE_COMPLEXITY_KEY,
                ),
                totals: CognitiveComplexityTotals {
                    complex_cognitive_complexity: measured
                        .cognitive_complexity
                        .complex_cognitive_complexity(),
                    total_cognitive_complexity: measured
                        .cognitive_complexity
                        .total_cognitive_complexity(),
                },
                findings: view.select(
                    measured
                        .cognitive_complexity
                        .complex_functions()
                        .iter()
                        .map(CognitiveComplexityFinding::new),
                ),
            },
            duplication: DuplicationSection::new(
                &measured.duplication,
                ranges,
                view,
                options.duplication,
            ),
            verbosity: MetricSection {
                header: MetricHeader::new(
                    measured.verbosity.ratio(),
                    ranges,
                    Metric::Verbosity.key(),
                ),
                totals: VerbosityTotals {
                    verbose_lines: measured.verbosity.verbose_lines(),
                    flagged_lines: measured.verbosity.flagged_line_count(),
                    clone_lines: measured.verbosity.clone_line_count(),
                },
                findings: view.map(measured.verbosity.flagged_lines()),
            },
            import_cycles: measured
                .import_cycles
                .as_ref()
                .map(|cycles| ImportCyclesSection {
                    header: MetricHeader::new(cycles.ratio(), ranges, Metric::ImportCycles.key()),
                    modules_in_import_cycles: cycles.modules_in_import_cycles(),
                    modules: cycles.modules(),
                    findings: view.select(cycles.cycles().iter().map(ImportCycleFinding::new)),
                    unresolved_imports: UnresolvedImportsSection::new(cycles, view),
                }),
            boundaries,
            orphan_files,
            external_metrics,
            packages: package_sections(model, effective_import_model, &measured),
        })
    }
}

fn source_lines(model: &CodeModel) -> u64 {
    model
        .files
        .iter()
        .map(|file| u64::from(file.model.source_lines))
        .sum()
}

impl ErodedFunctionFinding {
    fn new(function: &ErodedFunction) -> ErodedFunctionFinding {
        ErodedFunctionFinding {
            id: function.id.to_string(),
            path: function.path.to_string(),
            line: function.line,
            end_line: function.end_line,
            name: function.name.clone(),
            cyclomatic_complexity: function.cyclomatic_complexity,
            cognitive_complexity: function.cognitive_complexity,
            source_lines: function.source_lines,
            function_mass: Decimal::rounded(function.function_mass),
            owners: Vec::new(),
        }
    }
}

impl CognitiveComplexityFinding {
    fn new(function: &ComplexFunction) -> CognitiveComplexityFinding {
        CognitiveComplexityFinding {
            id: function.id.to_string(),
            path: function.path.to_string(),
            line: function.line,
            end_line: function.end_line,
            name: function.name.clone(),
            cognitive_complexity: function.cognitive_complexity,
            cyclomatic_complexity: function.cyclomatic_complexity,
            source_lines: function.source_lines,
            owners: Vec::new(),
        }
    }
}

impl DuplicationSection {
    fn new(
        duplication: &Duplication,
        ranges: &ReferenceRangeScope,
        view: FindingsView<'_>,
        config: DuplicationConfig,
    ) -> DuplicationSection {
        let (reference_range, near_clones) = if config.near_clones {
            (
                ReferenceRangeEntry::NotShown(NOT_SHOWN_FOR_NEAR_CLONES.to_owned()),
                Some(NearClonesSummary {
                    min_similarity: config.min_similarity.ratio(),
                }),
            )
        } else {
            (ranges.entry(Metric::Duplication.key()), None)
        };
        let all_clone_families = view.ownership.apply(
            duplication
                .clone_families
                .iter()
                .map(CloneFamilyFinding::new)
                .collect(),
        );
        let all_clustered = clustered_clone_group_ids(&all_clone_families);
        let clone_groups = view.select(duplication.clone_groups.iter().map(CloneGroupFinding::new));
        let clone_families = Findings::worst_first(all_clone_families.into_iter(), view.shown);
        let unlisted_clone_family_group_ids = unlisted_clone_family_group_ids(
            &clone_groups.listed,
            &clone_families.listed,
            &all_clustered,
        );
        DuplicationSection {
            header: MetricHeader {
                ratio: Decimal::rounded(duplication.ratio()),
                reference_range,
            },
            near_clones,
            clone_lines: duplication.clone_lines.count(),
            clone_groups,
            clone_families,
            unlisted_clone_family_group_ids,
        }
    }
}

pub(crate) fn clustered_clone_group_ids(families: &[CloneFamilyFinding]) -> HashSet<String> {
    families
        .iter()
        .flat_map(|family| family.clone_group_ids.iter().cloned())
        .collect()
}

pub(crate) fn unlisted_clone_family_group_ids(
    listed_groups: &[CloneGroupFinding],
    listed_families: &[CloneFamilyFinding],
    all_clustered: &HashSet<String>,
) -> Vec<String> {
    if all_clustered.is_empty() {
        return Vec::new();
    }
    let listed_clustered: HashSet<&str> = listed_families
        .iter()
        .flat_map(|family| family.clone_group_ids.iter().map(String::as_str))
        .collect();
    listed_groups
        .iter()
        .filter(|group| {
            let id = group.id.as_str();
            all_clustered.contains(id) && !listed_clustered.contains(id)
        })
        .map(|group| group.id.clone())
        .collect()
}
impl CloneGroupFinding {
    fn new(group: &CloneGroup) -> CloneGroupFinding {
        CloneGroupFinding {
            id: group.id.to_string(),
            tokens: group.tokens,
            similarity: group.similarity.map(Decimal::rounded),
            locations: group.fragments.iter().map(CloneLocation::new).collect(),
            owners: Vec::new(),
        }
    }
}

impl CloneFamilyFinding {
    fn new(family: &CloneFamily) -> CloneFamilyFinding {
        CloneFamilyFinding {
            id: family.id.to_string(),
            duplicated_lines: family.duplicated_lines,
            clone_group_ids: family
                .clone_group_ids
                .iter()
                .map(ToString::to_string)
                .collect(),
            locations: family.fragments.iter().map(CloneLocation::new).collect(),
            owners: Vec::new(),
        }
    }
}

impl CloneLocation {
    fn new(fragment: &Fragment) -> CloneLocation {
        CloneLocation {
            path: fragment.path.to_string(),
            start_line: fragment.start_line,
            end_line: fragment.end_line,
        }
    }
}

impl From<&FlaggedLineFinding> for FlaggedLineRow {
    fn from(finding: &FlaggedLineFinding) -> FlaggedLineRow {
        FlaggedLineRow {
            id: finding.id.to_string(),
            path: finding.path.to_string(),
            line: finding.line,
            rule: finding.rule.clone(),
            owners: Vec::new(),
        }
    }
}

impl ImportCycleFinding {
    fn new(cycle: &ImportCycle) -> ImportCycleFinding {
        ImportCycleFinding {
            id: cycle.id().to_string(),
            modules: cycle.modules().iter().map(RepoPath::to_string).collect(),
            owners: Vec::new(),
        }
    }
}

impl UnresolvedImportsSection {
    fn new(import_cycles: &ImportCycles, view: FindingsView<'_>) -> UnresolvedImportsSection {
        let imports = view.map(import_cycles.unresolved_imports());
        UnresolvedImportsSection {
            count: imports.listed.len() + imports.left_out,
            imports,
        }
    }
}

impl From<&UnresolvedImport> for UnresolvedImportRow {
    fn from(import: &UnresolvedImport) -> UnresolvedImportRow {
        UnresolvedImportRow {
            importer: import.importer.to_string(),
            specifier: import.specifier.clone(),
            reason: import.reason.clone(),
            owners: Vec::new(),
        }
    }
}

impl From<&SkippedFile> for SkippedEntry {
    fn from(file: &SkippedFile) -> SkippedEntry {
        SkippedEntry {
            path: file.path.to_string(),
            reason: file.reason.clone(),
        }
    }
}

struct Measured {
    erosion: Erosion,
    cognitive_complexity: CognitiveComplexity,
    duplication: Duplication,
    verbosity: Verbosity,
    import_cycles: Option<ImportCycles>,
    boundaries: Option<Boundaries>,
}

impl Measured {
    fn new(
        model: &CodeModel,
        import_model: Option<&CodeModel>,
        boundaries: Option<&BoundariesConfig>,
        duplication_config: DuplicationConfig,
    ) -> Measured {
        let near_clones = duplication_config.near_clones.then(|| {
            NearCloneConfig::new(duplication_config.min_similarity.ratio())
                .expect("validated min_similarity is in (0, 1]")
        });
        let duplication = Duplication::measure_with_near_clones(
            model,
            MinimumCloneTokens::DEFAULT,
            MinimumDistinctTokens::DEFAULT,
            near_clones,
        );
        let import_cycles = import_model.map(ImportCycles::measure);
        let boundaries = import_model.and_then(|imports| {
            boundaries.map(|config| Boundaries::measure(imports, &ConfigBoundaryPolicy(config)))
        });
        Measured {
            erosion: Erosion::measure(model),
            cognitive_complexity: CognitiveComplexity::measure(model),
            verbosity: Verbosity::measure(model, &duplication.exact_clone_lines),
            duplication,
            import_cycles,
            boundaries,
        }
    }
}

fn package_sections<'a>(
    model: &'a CodeModel,
    import_model: Option<&'a CodeModel>,
    measured: &Measured,
) -> Vec<PackageSection> {
    if !model.workspace.has_members() {
        return Vec::new();
    }
    let mut files: BTreeMap<Package<'a>, usize> = BTreeMap::new();
    for file in &model.files {
        *files
            .entry(model.workspace.package_of(&file.path))
            .or_default() += 1;
    }
    let mut cognitive = measured.cognitive_complexity.by_package(model);
    let mut duplication = measured.duplication.by_package(model);
    let mut verbosity = measured.verbosity.by_package(model);
    let mut import_cycles = import_model
        .zip(measured.import_cycles.as_ref())
        .map(|(imports, cycles)| cycles.by_package(imports));
    measured
        .erosion
        .by_package(model)
        .into_iter()
        .map(|(package, masses)| {
            let package_files = files.get(&package).copied().unwrap_or_default();
            let cognitive_totals = cognitive.remove(&package).unwrap_or_default();
            let clone_lines = duplication.remove(&package).unwrap_or_default();
            let verbose_lines = verbosity.remove(&package).unwrap_or_default();
            let package_import_cycles = import_cycles.as_mut().and_then(|by_package| {
                let modules = by_package.remove(&package).unwrap_or_default();
                (modules.modules > 0 || package_files == 0).then(|| PackageImportCycles {
                    ratio: Decimal::rounded(modules.ratio()),
                    modules_in_import_cycles: modules.modules_in_import_cycles,
                })
            });
            PackageSection {
                path: package.to_string(),
                files: package_files,
                source_lines: clone_lines.source_lines,
                erosion: PackageErosion {
                    ratio: Decimal::rounded(masses.ratio()),
                    eroded_function_mass: Decimal::rounded(masses.eroded),
                    total_function_mass: Decimal::rounded(masses.total),
                },
                cognitive_complexity: PackageCognitiveComplexity {
                    ratio: Decimal::rounded(cognitive_totals.ratio()),
                    complex_cognitive_complexity: cognitive_totals.complex,
                    total_cognitive_complexity: cognitive_totals.total,
                },
                duplication: PackageDuplication {
                    ratio: Decimal::rounded(clone_lines.ratio()),
                    clone_lines: clone_lines.lines,
                },
                verbosity: PackageVerbosity {
                    ratio: Decimal::rounded(verbose_lines.ratio()),
                    verbose_lines: verbose_lines.lines,
                },
                import_cycles: package_import_cycles,
            }
        })
        .collect()
}

pub struct ReportOptions<'a> {
    pub findings: FindingsShown,
    pub budgets: &'a Budgets,
    pub metric_plugins: &'a [&'a dyn MetricPlugin],
    pub boundaries: Option<&'a BoundariesConfig>,
    pub dependencies: &'a DependenciesConfig,
    pub dependency_hygiene: Option<&'a DependencyHygiene>,
    pub dependency_hygiene_by_language: Option<&'a BTreeMap<String, DependencyHygiene>>,
    pub duplication: DuplicationConfig,
    pub orphan_files: Option<&'a OrphanFiles>,
    pub orphan_files_by_language: Option<&'a BTreeMap<String, OrphanFiles>>,
    pub ownership: OwnershipOptions<'a>,
}

impl MeasuredSection {
    fn new(
        model: CodeModel,
        import_model: Option<CodeModel>,
        hygiene: &DependencyHygiene,
        ranges: &ReferenceRangeScope,
        options: SectionOptions<'_>,
    ) -> Result<MeasuredSection, MetricPluginError> {
        let files = model.files.len();
        let lines = source_lines(&model);
        let view = FindingsView {
            shown: options.findings,
            ownership: options.ownership,
        };
        let dependencies = options
            .has_imports
            .then(|| DependenciesSection::from_hygiene(hygiene, view))
            .flatten();
        let import_metrics_note = (!options.has_imports).then(|| NO_IMPORTS_METRIC_NOTE.to_owned());
        let section_options = if options.has_imports {
            options
        } else {
            SectionOptions {
                boundaries: None,
                orphan_files: None,
                ..options
            }
        };
        let source_sets = model.into_source_sets();
        let separate_import_source_sets = import_model.map(CodeModel::into_source_sets);
        let import_source_sets = if options.has_imports {
            Some(separate_import_source_sets.as_ref().unwrap_or(&source_sets))
        } else {
            None
        };
        let source_sets =
            SourceSetSections::new(&source_sets, import_source_sets, ranges, section_options)?;
        Ok(MeasuredSection {
            files,
            source_lines: lines,
            import_metrics_note,
            source_sets,
            dependencies,
        })
    }

    fn populate_external_metric_names(&mut self) {
        for slot in self.source_sets.iter_mut() {
            for (key, metric) in &mut slot.external_metrics {
                if metric.name.is_empty() {
                    metric.name = key.clone();
                }
            }
        }
    }
}

impl Report {
    pub fn summarize(
        languages: Vec<MeasuredLanguage>,
        submodules: Vec<RepoPath>,
        findings: FindingsShown,
        budgets: &Budgets,
    ) -> Report {
        Report::summarize_with_metrics(languages, submodules, findings, budgets, &[])
            .expect("built-in metrics cannot fail")
    }

    pub fn summarize_with_metrics(
        languages: Vec<MeasuredLanguage>,
        submodules: Vec<RepoPath>,
        findings: FindingsShown,
        budgets: &Budgets,
        metric_plugins: &[&dyn MetricPlugin],
    ) -> Result<Report, MetricPluginError> {
        Report::summarize_with_metrics_and_boundaries(
            languages,
            submodules,
            findings,
            budgets,
            metric_plugins,
            None,
        )
    }

    pub fn summarize_with_metrics_and_boundaries(
        languages: Vec<MeasuredLanguage>,
        submodules: Vec<RepoPath>,
        findings: FindingsShown,
        budgets: &Budgets,
        metric_plugins: &[&dyn MetricPlugin],
        boundaries: Option<&BoundariesConfig>,
    ) -> Result<Report, MetricPluginError> {
        let dependencies = DependenciesConfig::default();
        Report::summarize_with_options(
            languages,
            submodules,
            ReportOptions {
                findings,
                budgets,
                metric_plugins,
                boundaries,
                dependencies: &dependencies,
                dependency_hygiene: None,
                dependency_hygiene_by_language: None,
                duplication: DuplicationConfig::default(),
                orphan_files: None,
                orphan_files_by_language: None,
                ownership: OwnershipOptions::default(),
            },
        )
    }

    pub fn summarize_with_config(
        languages: Vec<MeasuredLanguage>,
        submodules: Vec<RepoPath>,
        findings: FindingsShown,
        budgets: &Budgets,
        metric_plugins: &[&dyn MetricPlugin],
        boundaries: Option<&BoundariesConfig>,
        duplication: DuplicationConfig,
    ) -> Result<Report, MetricPluginError> {
        let dependencies = DependenciesConfig::default();
        Report::summarize_with_options(
            languages,
            submodules,
            ReportOptions {
                findings,
                budgets,
                metric_plugins,
                boundaries,
                dependencies: &dependencies,
                dependency_hygiene: None,
                dependency_hygiene_by_language: None,
                duplication,
                orphan_files: None,
                orphan_files_by_language: None,
                ownership: OwnershipOptions::default(),
            },
        )
    }

    pub fn summarize_with_ownership(
        languages: Vec<MeasuredLanguage>,
        submodules: Vec<RepoPath>,
        findings: FindingsShown,
        budgets: &Budgets,
        metric_plugins: &[&dyn MetricPlugin],
        boundaries: Option<&BoundariesConfig>,
        ownership: OwnershipOptions<'_>,
    ) -> Result<Report, MetricPluginError> {
        let dependencies = DependenciesConfig::default();
        Report::summarize_with_options(
            languages,
            submodules,
            ReportOptions {
                findings,
                budgets,
                metric_plugins,
                boundaries,
                dependencies: &dependencies,
                dependency_hygiene: None,
                dependency_hygiene_by_language: None,
                duplication: DuplicationConfig::default(),
                orphan_files: None,
                orphan_files_by_language: None,
                ownership,
            },
        )
    }

    pub fn summarize_with_options(
        languages: Vec<MeasuredLanguage>,
        submodules: Vec<RepoPath>,
        options: ReportOptions<'_>,
    ) -> Result<Report, MetricPluginError> {
        let ignored = options.dependencies.ignored();
        let measured_total_hygiene;
        let total_hygiene = match options.dependency_hygiene {
            Some(precomputed) => precomputed,
            None => {
                measured_total_hygiene = DependencyHygiene::merge(
                    languages
                        .iter()
                        .filter(|language| language.has_imports)
                        .map(|language| DependencyHygiene::measure(&language.model, ignored)),
                );
                &measured_total_hygiene
            }
        };
        let (measured, unmeasured): (Vec<MeasuredLanguage>, Vec<MeasuredLanguage>) = languages
            .into_iter()
            .partition(|language| !language.model.files.is_empty());
        let top_has_imports =
            measured.is_empty() || measured.iter().any(|language| language.has_imports);
        let section_options = SectionOptions {
            findings: options.findings,
            metric_plugins: options.metric_plugins,
            boundaries: options.boundaries,
            duplication: options.duplication,
            orphan_files: options.orphan_files,
            ownership: options.ownership,
            has_imports: top_has_imports,
        };
        let (total_ranges, sections_per_language) = match measured.as_slice() {
            [] => (
                ReferenceRangeScope::OneLanguage(ReferenceRanges::default()),
                BTreeMap::new(),
            ),
            [only] => (
                ReferenceRangeScope::OneLanguage(only.reference_ranges.clone()),
                BTreeMap::new(),
            ),
            _ => {
                let mut per_language = BTreeMap::new();
                for language in &measured {
                    let measured_language_hygiene;
                    let language_hygiene = if language.has_imports {
                        match options
                            .dependency_hygiene_by_language
                            .and_then(|map| map.get(&language.plugin))
                        {
                            Some(precomputed) => precomputed,
                            None => {
                                measured_language_hygiene =
                                    DependencyHygiene::measure(&language.model, ignored);
                                &measured_language_hygiene
                            }
                        }
                    } else {
                        measured_language_hygiene = DependencyHygiene::default();
                        &measured_language_hygiene
                    };
                    let language_orphans = language
                        .has_imports
                        .then(|| {
                            options
                                .orphan_files_by_language
                                .and_then(|map| map.get(&language.plugin))
                        })
                        .flatten();
                    per_language.insert(
                        language.plugin.clone(),
                        MeasuredSection::new(
                            language.model.clone(),
                            None,
                            language_hygiene,
                            &ReferenceRangeScope::OneLanguage(language.reference_ranges.clone()),
                            SectionOptions {
                                orphan_files: language_orphans,
                                has_imports: language.has_imports,
                                ..section_options
                            },
                        )?,
                    );
                }
                (ReferenceRangeScope::SeveralLanguages, per_language)
            }
        };
        let all_measured_have_imports = measured.iter().all(|language| language.has_imports);
        let import_bearing_model = (top_has_imports && !all_measured_have_imports).then(|| {
            CodeModel::merge(
                measured
                    .iter()
                    .filter(|language| language.has_imports)
                    .map(|language| language.model.clone()),
            )
        });
        let model = CodeModel::merge(
            measured
                .into_iter()
                .chain(unmeasured)
                .map(|language| language.model),
        );
        let skipped = model.skipped.iter().map(SkippedEntry::from).collect();
        let declaration_only = model
            .declaration_only
            .iter()
            .map(RepoPath::to_string)
            .collect();
        let measured = MeasuredSection::new(
            model,
            import_bearing_model,
            total_hygiene,
            &total_ranges,
            section_options,
        )?;
        let mut failures = measured.source_sets.over_budget(options.budgets);
        if let Some(dependencies) = &measured.dependencies {
            failures.extend(dependencies.over_budget(options.budgets));
        }
        Ok(Report {
            over_budget: failures,
            skipped,
            declaration_only,
            submodules: submodules.iter().map(RepoPath::to_string).collect(),
            measured,
            languages: sections_per_language,
        })
    }

    pub(crate) fn populate_external_metric_names(&mut self) {
        self.measured.populate_external_metric_names();
        for section in self.languages.values_mut() {
            section.populate_external_metric_names();
        }
    }

    pub fn over_budget(&self) -> &[BudgetFailure] {
        &self.over_budget
    }

    pub fn render(&self, format: ReportFormat) -> Result<String, ReportError> {
        match format {
            ReportFormat::Toon => render_toon(self),
            ReportFormat::Json => render_json(self),
            ReportFormat::Sarif => serde_json::to_string_pretty(&sarif::SarifLog::new(self))
                .map_err(ReportError::Sarif),
            ReportFormat::Markdown => Ok(markdown::render(self)),
        }
    }
}

pub(crate) enum TextReportFormat {
    Toon,
    Json,
    Text,
}

macro_rules! impl_text_report_format {
    ($($ty:ty),+ $(,)?) => {
        $(
            impl From<$ty> for TextReportFormat {
                fn from(format: $ty) -> TextReportFormat {
                    match format {
                        <$ty>::Toon => TextReportFormat::Toon,
                        <$ty>::Json => TextReportFormat::Json,
                        <$ty>::Text => TextReportFormat::Text,
                    }
                }
            }
        )+
    };
}

impl_text_report_format!(
    AgentFormat,
    ConfigFormat,
    DoctorFormat,
    GuardFormat,
    ImpactFormat,
    TraceFormat,
);

pub(crate) fn render_text_report<T: Serialize>(
    value: &T,
    format: TextReportFormat,
    render_text: impl FnOnce(&T) -> String,
) -> Result<String, ReportError> {
    match format {
        TextReportFormat::Toon => render_toon(value),
        TextReportFormat::Json => render_json(value),
        TextReportFormat::Text => Ok(render_text(value)),
    }
}

pub(crate) fn render_toon<T: Serialize>(value: &T) -> Result<String, ReportError> {
    toon_format::encode_default(value).map_err(ReportError::Toon)
}

pub(crate) fn render_json<T: Serialize>(value: &T) -> Result<String, ReportError> {
    serde_json::to_string_pretty(value).map_err(ReportError::Json)
}
