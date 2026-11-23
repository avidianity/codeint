#![forbid(unsafe_code)]

use std::collections::BTreeMap;

use codeint_config::{
    Boundaries as BoundariesConfig, BoundaryDirective, BoundaryRuleDecision, Budgets, Codeowners,
    DependenciesConfig, FindingsShown, ImpactFormat, ReportFormat, TraceFormat,
};
use codeint_metrics::{
    Boundaries, BoundaryEdgeCheck, BoundaryFinding, BoundaryPolicy, CloneGroup,
    CognitiveComplexity, ComplexFunction, DEV_IN_PRODUCTION_DEPENDENCIES_METRIC_KEY,
    DependencyHygiene, Duplication, ErodedFunction, Erosion, FlaggedLineFinding, Fragment,
    ImportCycle, ImportCycles, MinimumCloneTokens, MinimumDistinctTokens,
    UNLISTED_DEPENDENCIES_METRIC_KEY, UNUSED_DEPENDENCIES_METRIC_KEY, UnresolvedImport, Verbosity,
};
use codeint_model::{
    CodeModel, ExternalFinding, Metric, MetricPlugin, MetricPluginError, Package, ReferenceRange,
    ReferenceRanges, RepoPath, SkippedFile, SourceSet, SourceSets,
};
use serde::ser::SerializeMap;
use serde::{Deserialize, Serialize, Serializer};

mod churn;
mod coupling;
mod dependencies;
mod diff;
mod guard;
mod impact;
mod markdown;
mod measurement;
mod ownership;
mod sarif;
#[cfg(test)]
mod tests;
mod trace;
mod trend;

pub use churn::{ChurnHistoryInput, ChurnReport};
pub use codeint_metrics::TraceError;
pub use coupling::{CouplingHistoryInput, CouplingReport};
pub use diff::{BaseCommit, DiffReport, DiffReportOptions, Head, HeadCommit};
pub use guard::{GuardAllowedTarget, GuardDeniedTarget, GuardReport};
pub use impact::{ChangedBase, ImpactReport};
pub use measurement::CommitMeasurement;
pub use ownership::OwnershipOptions;
pub use trace::TraceReport;
pub use trend::{TrendPoint, TrendReport};

use dependencies::{ImportedDependencyRow, UnusedDependencyRow};
use ownership::OwnedFinding;

const DECIMAL_SCALE: f64 = 10_000.0;
const COGNITIVE_COMPLEXITY_KEY: &str = "cognitive_complexity";
const NOT_PUBLISHED_FOR_THIS_LANGUAGE: &str = "no published Reference range for this language";
const NOT_SHOWN_FOR_SEVERAL_LANGUAGES: &str =
    "not shown, because the measured files are in more than one language";

#[derive(Debug, thiserror::Error)]
pub enum ReportError {
    #[error("cannot render the report as TOON")]
    Toon(#[source] toon_format::ToonError),
    #[error("cannot render the report as JSON")]
    Json(#[source] serde_json::Error),
    #[error("cannot render the report as SARIF")]
    Sarif(#[source] serde_json::Error),
}

#[derive(Debug, Serialize)]
pub struct Report {
    #[serde(flatten)]
    measured: MeasuredSection,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    languages: BTreeMap<String, MeasuredSection>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    skipped: Vec<SkippedEntry>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    declaration_only: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    submodules: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    over_budget: Vec<BudgetFailure>,
}

#[derive(Debug, Serialize)]
pub struct BudgetFailure {
    #[serde(skip_serializing_if = "Option::is_none")]
    source_set: Option<&'static str>,
    metric: String,
    ratio: Decimal,
    budget: f64,
}

pub struct MeasuredLanguage {
    pub plugin: String,
    pub model: CodeModel,
    pub reference_ranges: ReferenceRanges,
}

#[derive(Debug, Serialize)]
struct MeasuredSection {
    files: usize,
    source_lines: u64,
    source_sets: SourceSetSections,
    #[serde(skip_serializing_if = "Option::is_none")]
    dependencies: Option<DependenciesSection>,
}

#[derive(Debug, Serialize)]
pub(crate) struct DependenciesSection {
    pub(crate) unused_dependencies: UnusedDependenciesSection,
    pub(crate) unlisted_dependencies: UnlistedDependenciesSection,
    pub(crate) dev_in_production_dependencies: DevInProductionDependenciesSection,
}

#[derive(Debug, Serialize)]
pub(crate) struct UnusedDependenciesSection {
    pub(crate) ratio: Decimal,
    pub(crate) unused: usize,
    pub(crate) declared: usize,
    #[serde(flatten)]
    pub(crate) findings: Findings<UnusedDependencyRow>,
}

#[derive(Debug, Serialize)]
pub(crate) struct UnlistedDependenciesSection {
    pub(crate) ratio: Decimal,
    pub(crate) unlisted: usize,
    pub(crate) imported: usize,
    #[serde(flatten)]
    pub(crate) findings: Findings<ImportedDependencyRow>,
}

#[derive(Debug, Serialize)]
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

#[derive(Debug, Serialize)]
#[serde(untagged)]
enum ReferenceRangeEntry {
    Published {
        low: Decimal,
        high: Decimal,
        source: String,
    },
    NotShown(&'static str),
}

#[derive(Debug, Default, Serialize)]
struct SourceSetSections {
    #[serde(skip_serializing_if = "Option::is_none")]
    production: Option<SourceSetSection>,
    #[serde(skip_serializing_if = "Option::is_none")]
    test: Option<SourceSetSection>,
    #[serde(skip_serializing_if = "Option::is_none")]
    generated: Option<SourceSetSection>,
    #[serde(skip_serializing_if = "Option::is_none")]
    vendored: Option<SourceSetSection>,
}

#[derive(Debug, Serialize)]
struct SourceSetSection {
    files: usize,
    source_lines: u64,
    erosion: MetricSection<ErosionTotals, ErodedFunctionFinding>,
    cognitive_complexity: MetricSection<CognitiveComplexityTotals, CognitiveComplexityFinding>,
    duplication: MetricSection<DuplicationTotals, CloneGroupFinding>,
    verbosity: MetricSection<VerbosityTotals, FlaggedLineRow>,
    import_cycles: ImportCyclesSection,
    #[serde(skip_serializing_if = "Option::is_none")]
    boundaries: Option<BoundariesSection>,
    #[serde(flatten)]
    external_metrics: BTreeMap<String, ExternalMetricSection>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    packages: Vec<PackageSection>,
}

#[derive(Debug, Serialize)]
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

#[derive(Debug, Serialize)]
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

#[derive(Debug, Serialize)]
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

#[derive(Debug, Serialize)]
struct PackageSection {
    path: String,
    files: usize,
    source_lines: u64,
    erosion: PackageErosion,
    cognitive_complexity: PackageCognitiveComplexity,
    duplication: PackageDuplication,
    verbosity: PackageVerbosity,
    import_cycles: PackageImportCycles,
}

#[derive(Debug, Serialize)]
struct PackageErosion {
    ratio: Decimal,
    eroded_function_mass: Decimal,
    total_function_mass: Decimal,
}

#[derive(Debug, Serialize)]
struct PackageCognitiveComplexity {
    ratio: Decimal,
    complex_cognitive_complexity: u64,
    total_cognitive_complexity: u64,
}

#[derive(Debug, Serialize)]
struct PackageDuplication {
    ratio: Decimal,
    clone_lines: u64,
}

#[derive(Debug, Serialize)]
struct PackageVerbosity {
    ratio: Decimal,
    verbose_lines: u64,
}

#[derive(Debug, Serialize)]
struct PackageImportCycles {
    ratio: Decimal,
    modules_in_import_cycles: usize,
}

#[derive(Debug, Serialize)]
struct MetricSection<Totals, Finding: FindingKind> {
    #[serde(flatten)]
    header: MetricHeader,
    #[serde(flatten)]
    totals: Totals,
    #[serde(flatten)]
    findings: Findings<Finding>,
}

#[derive(Debug, Serialize)]
struct ErosionTotals {
    eroded_function_mass: Decimal,
    total_function_mass: Decimal,
}

#[derive(Debug, Serialize)]
struct CognitiveComplexityTotals {
    complex_cognitive_complexity: u64,
    total_cognitive_complexity: u64,
}

#[derive(Debug, Serialize)]
struct DuplicationTotals {
    clone_lines: u64,
}

#[derive(Debug, Serialize)]
struct VerbosityTotals {
    verbose_lines: u64,
    flagged_lines: u64,
    clone_lines: u64,
}

trait FindingKind: Serialize {
    const LIST_KEY: &'static str;
    const LEFT_OUT_KEY: &'static str = "findings_left_out";
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

#[derive(Debug, Serialize, Deserialize)]
struct CloneGroupFinding {
    id: String,
    tokens: usize,
    locations: Vec<CloneLocation>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    owners: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
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

#[derive(Debug, Serialize)]
struct ImportCyclesSection {
    #[serde(flatten)]
    header: MetricHeader,
    modules_in_import_cycles: usize,
    modules: usize,
    #[serde(flatten)]
    findings: Findings<ImportCycleFinding>,
    unresolved_imports: UnresolvedImportsSection,
}

#[derive(Debug, Serialize)]
struct UnresolvedImportsSection {
    count: usize,
    #[serde(flatten)]
    imports: Findings<UnresolvedImportRow>,
}

#[derive(Debug, Serialize)]
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

#[derive(Debug, Serialize)]
struct SkippedEntry {
    path: String,
    reason: String,
}

impl std::fmt::Display for BudgetFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.source_set {
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
                source_set: Some(source_set.key()),
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
            ReferenceRangeScope::OneLanguage(ranges) => ranges.for_metric(metric).map_or(
                ReferenceRangeEntry::NotShown(NOT_PUBLISHED_FOR_THIS_LANGUAGE),
                ReferenceRangeEntry::from,
            ),
            ReferenceRangeScope::SeveralLanguages => {
                ReferenceRangeEntry::NotShown(NOT_SHOWN_FOR_SEVERAL_LANGUAGES)
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
        map.serialize_entry(T::LIST_KEY, &self.listed)?;
        if self.left_out > 0 {
            map.serialize_entry(T::LEFT_OUT_KEY, &self.left_out)?;
        }
        map.end()
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

impl OwnedFinding for CloneGroupFinding {
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

impl SourceSetSections {
    fn new(
        source_sets: &SourceSets,
        ranges: &ReferenceRangeScope,
        findings: FindingsShown,
        metric_plugins: &[&dyn MetricPlugin],
        boundaries: Option<&BoundariesConfig>,
        ownership: OwnershipOptions<'_>,
    ) -> Result<SourceSetSections, MetricPluginError> {
        let mut sections = SourceSetSections::default();
        for (source_set, model) in source_sets.iter() {
            if !model.files.is_empty() {
                let source_set_boundaries =
                    boundaries.filter(|config| config.applies_to_source_set(source_set));
                *sections.slot(source_set) = Some(SourceSetSection::new(
                    model,
                    ranges,
                    findings,
                    metric_plugins,
                    source_set_boundaries,
                    ownership,
                )?);
            }
        }
        Ok(sections)
    }

    fn iter(&self) -> impl Iterator<Item = (SourceSet, &SourceSetSection)> {
        [
            (SourceSet::Production, &self.production),
            (SourceSet::Test, &self.test),
            (SourceSet::Generated, &self.generated),
            (SourceSet::Vendored, &self.vendored),
        ]
        .into_iter()
        .filter_map(|(source_set, section)| Some((source_set, section.as_ref()?)))
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
                    (
                        source_set,
                        Metric::ImportCycles.key(),
                        section.import_cycles.header.ratio.0,
                    ),
                ]
                .into_iter()
                .chain(
                    section
                        .boundaries
                        .iter()
                        .map(move |boundaries| (source_set, "boundaries", boundaries.ratio.0)),
                )
                .chain(
                    section.external_metrics.iter().map(move |(key, metric)| {
                        (source_set, key.as_str(), metric.header.ratio.0)
                    }),
                )
            }),
        )
    }

    fn slot(&mut self, source_set: SourceSet) -> &mut Option<SourceSetSection> {
        match source_set {
            SourceSet::Production => &mut self.production,
            SourceSet::Test => &mut self.test,
            SourceSet::Generated => &mut self.generated,
            SourceSet::Vendored => &mut self.vendored,
        }
    }
}

impl SourceSetSection {
    fn new(
        model: &CodeModel,
        ranges: &ReferenceRangeScope,
        findings: FindingsShown,
        metric_plugins: &[&dyn MetricPlugin],
        boundaries: Option<&BoundariesConfig>,
        ownership: OwnershipOptions<'_>,
    ) -> Result<SourceSetSection, MetricPluginError> {
        let view = FindingsView {
            shown: findings,
            ownership,
        };
        let measured = Measured::new(model);
        let boundaries = boundaries.map(|config| {
            let evaluated = Boundaries::measure(model, &ConfigBoundaryPolicy(config));
            BoundariesSection::new(&evaluated, view)
        });
        let mut external_metrics = BTreeMap::new();
        for plugin in metric_plugins {
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
            duplication: MetricSection {
                header: MetricHeader::new(
                    measured.duplication.ratio(),
                    ranges,
                    Metric::Duplication.key(),
                ),
                totals: DuplicationTotals {
                    clone_lines: measured.duplication.clone_lines.count(),
                },
                findings: view.select(
                    measured
                        .duplication
                        .clone_groups
                        .iter()
                        .map(CloneGroupFinding::new),
                ),
            },
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
            import_cycles: ImportCyclesSection {
                header: MetricHeader::new(
                    measured.import_cycles.ratio(),
                    ranges,
                    Metric::ImportCycles.key(),
                ),
                modules_in_import_cycles: measured.import_cycles.modules_in_import_cycles(),
                modules: measured.import_cycles.modules(),
                findings: view.select(
                    measured
                        .import_cycles
                        .cycles()
                        .iter()
                        .map(ImportCycleFinding::new),
                ),
                unresolved_imports: UnresolvedImportsSection::new(&measured.import_cycles, view),
            },
            boundaries,
            external_metrics,
            packages: package_sections(model, &measured),
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

impl CloneGroupFinding {
    fn new(group: &CloneGroup) -> CloneGroupFinding {
        CloneGroupFinding {
            id: group.id.to_string(),
            tokens: group.tokens,
            locations: group.fragments.iter().map(CloneLocation::new).collect(),
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
    import_cycles: ImportCycles,
}

impl Measured {
    fn new(model: &CodeModel) -> Measured {
        let duplication = Duplication::measure(
            model,
            MinimumCloneTokens::DEFAULT,
            MinimumDistinctTokens::DEFAULT,
        );
        Measured {
            erosion: Erosion::measure(model),
            cognitive_complexity: CognitiveComplexity::measure(model),
            verbosity: Verbosity::measure(model, &duplication.clone_lines),
            duplication,
            import_cycles: ImportCycles::measure(model),
        }
    }
}

fn package_sections(model: &CodeModel, measured: &Measured) -> Vec<PackageSection> {
    if !model.workspace.has_members() {
        return Vec::new();
    }
    let mut files: BTreeMap<Package<'_>, usize> = BTreeMap::new();
    for file in &model.files {
        *files
            .entry(model.workspace.package_of(&file.path))
            .or_default() += 1;
    }
    let mut cognitive = measured.cognitive_complexity.by_package(model);
    let mut duplication = measured.duplication.by_package(model);
    let mut verbosity = measured.verbosity.by_package(model);
    let mut import_cycles = measured.import_cycles.by_package(model);
    measured
        .erosion
        .by_package(model)
        .into_iter()
        .map(|(package, masses)| {
            let cognitive_totals = cognitive.remove(&package).unwrap_or_default();
            let clone_lines = duplication.remove(&package).unwrap_or_default();
            let verbose_lines = verbosity.remove(&package).unwrap_or_default();
            let modules = import_cycles.remove(&package).unwrap_or_default();
            PackageSection {
                path: package.to_string(),
                files: files.get(&package).copied().unwrap_or_default(),
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
                import_cycles: PackageImportCycles {
                    ratio: Decimal::rounded(modules.ratio()),
                    modules_in_import_cycles: modules.modules_in_import_cycles,
                },
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
    pub ownership: OwnershipOptions<'a>,
}

impl MeasuredSection {
    fn new(
        model: CodeModel,
        hygiene: &DependencyHygiene,
        ranges: &ReferenceRangeScope,
        findings: FindingsShown,
        metric_plugins: &[&dyn MetricPlugin],
        boundaries: Option<&BoundariesConfig>,
        ownership: OwnershipOptions<'_>,
    ) -> Result<MeasuredSection, MetricPluginError> {
        let files = model.files.len();
        let lines = source_lines(&model);
        let view = FindingsView {
            shown: findings,
            ownership,
        };
        let dependencies = DependenciesSection::from_hygiene(hygiene, view);
        let source_sets = SourceSetSections::new(
            &model.into_source_sets(),
            ranges,
            findings,
            metric_plugins,
            boundaries,
            ownership,
        )?;
        Ok(MeasuredSection {
            files,
            source_lines: lines,
            source_sets,
            dependencies,
        })
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
        Report::summarize_with_ownership(
            languages,
            submodules,
            findings,
            budgets,
            metric_plugins,
            boundaries,
            OwnershipOptions::default(),
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
        let total_hygiene = DependencyHygiene::merge(
            languages
                .iter()
                .map(|language| DependencyHygiene::measure(&language.model, ignored)),
        );
        let (measured, unmeasured): (Vec<MeasuredLanguage>, Vec<MeasuredLanguage>) = languages
            .into_iter()
            .partition(|language| !language.model.files.is_empty());
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
                    let language_hygiene = DependencyHygiene::measure(&language.model, ignored);
                    per_language.insert(
                        language.plugin.clone(),
                        MeasuredSection::new(
                            language.model.clone(),
                            &language_hygiene,
                            &ReferenceRangeScope::OneLanguage(language.reference_ranges.clone()),
                            options.findings,
                            options.metric_plugins,
                            options.boundaries,
                            options.ownership,
                        )?,
                    );
                }
                (ReferenceRangeScope::SeveralLanguages, per_language)
            }
        };
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
            &total_hygiene,
            &total_ranges,
            options.findings,
            options.metric_plugins,
            options.boundaries,
            options.ownership,
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

impl From<ImpactFormat> for TextReportFormat {
    fn from(format: ImpactFormat) -> TextReportFormat {
        match format {
            ImpactFormat::Toon => TextReportFormat::Toon,
            ImpactFormat::Json => TextReportFormat::Json,
            ImpactFormat::Text => TextReportFormat::Text,
        }
    }
}

impl From<TraceFormat> for TextReportFormat {
    fn from(format: TraceFormat) -> TextReportFormat {
        match format {
            TraceFormat::Toon => TextReportFormat::Toon,
            TraceFormat::Json => TextReportFormat::Json,
            TraceFormat::Text => TextReportFormat::Text,
        }
    }
}

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
