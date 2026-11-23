#![forbid(unsafe_code)]

use std::collections::BTreeMap;

use codeint_config::{
    AgentFormat, Boundaries as BoundariesConfig, BoundaryDirective, BoundaryRuleDecision, Budgets,
    Codeowners, ConfigFormat, DoctorFormat, FindingsShown, GuardFormat, ImpactFormat, ReportFormat,
    TraceFormat,
};
use codeint_metrics::{
    Boundaries, BoundaryEdgeCheck, BoundaryFinding, BoundaryPolicy, CloneGroup, Duplication,
    ErodedFunction, Erosion, FlaggedLineFinding, Fragment, ImportCycle, ImportCycles,
    MinimumCloneTokens, MinimumDistinctTokens, OrphanFileFinding, OrphanFiles, UnresolvedImport,
    Verbosity,
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
pub use trace::TraceReport;
pub use trend::{TrendPoint, TrendReport};

use ownership::OwnedFinding;

const DECIMAL_SCALE: f64 = 10_000.0;
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
    source_set: String,
    metric: String,
    ratio: Decimal,
    budget: f64,
}

pub struct MeasuredLanguage {
    pub plugin: String,
    pub model: CodeModel,
    pub reference_ranges: ReferenceRanges,
}

#[derive(Default)]
pub struct ReportOptions<'a> {
    pub metric_plugins: &'a [&'a dyn MetricPlugin],
    pub boundaries: Option<&'a BoundariesConfig>,
    pub orphan_files: Option<&'a OrphanFiles>,
    pub orphan_files_by_language: Option<&'a BTreeMap<String, OrphanFiles>>,
    pub ownership: OwnershipOptions<'a>,
}

#[derive(Debug, Serialize, Deserialize)]
struct MeasuredSection {
    files: usize,
    source_lines: u64,
    source_sets: SourceSetSections,
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
    erosion: ErosionSection,
    duplication: DuplicationSection,
    verbosity: VerbositySection,
    import_cycles: ImportCyclesSection,
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
struct ExternalMetricSection {
    #[serde(skip)]
    name: String,
    #[serde(skip)]
    description: String,
    ratio: Decimal,
    reference_range: ReferenceRangeEntry,
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
    duplication: PackageDuplication,
    verbosity: PackageVerbosity,
    import_cycles: PackageImportCycles,
}

#[derive(Debug, Serialize, Deserialize)]
struct PackageErosion {
    ratio: Decimal,
    eroded_function_mass: Decimal,
    total_function_mass: Decimal,
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
struct ErosionSection {
    ratio: Decimal,
    reference_range: ReferenceRangeEntry,
    eroded_function_mass: Decimal,
    total_function_mass: Decimal,
    #[serde(flatten)]
    findings: Findings<ErodedFunctionFinding>,
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
    cyclomatic_complexity: u32,
    source_lines: u32,
    function_mass: Decimal,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    owners: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct DuplicationSection {
    ratio: Decimal,
    reference_range: ReferenceRangeEntry,
    clone_lines: u64,
    #[serde(flatten)]
    clone_groups: Findings<CloneGroupFinding>,
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
struct VerbositySection {
    ratio: Decimal,
    reference_range: ReferenceRangeEntry,
    verbose_lines: u64,
    flagged_lines: u64,
    clone_lines: u64,
    #[serde(flatten)]
    findings: Findings<FlaggedLineRow>,
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
    ratio: Decimal,
    reference_range: ReferenceRangeEntry,
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
        write!(
            formatter,
            "the {} {} ratio {} is over its Budget {}",
            self.source_set, self.metric, self.ratio.0, self.budget
        )
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
                source_set: source_set.key().to_owned(),
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
        map.serialize_entry(T::LIST_KEY, &self.listed)?;
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
                let listed = listed.ok_or_else(|| serde::de::Error::missing_field(T::LIST_KEY))?;
                Ok(Findings { listed, left_out })
            }
        }

        deserializer.deserialize_map(FindingsVisitor(std::marker::PhantomData))
    }
}

impl FindingKind for ErodedFunctionFinding {
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

impl FindingKind for OrphanFileRow {
    const LIST_KEY: &'static str = "findings";
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

pub(crate) fn evaluate_boundaries(
    model: &CodeModel,
    config: Option<&BoundariesConfig>,
) -> Option<Boundaries> {
    config.map(|policy| Boundaries::measure(model, &ConfigBoundaryPolicy(policy)))
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

impl SourceSetSections {
    fn new(
        source_sets: &SourceSets,
        ranges: &ReferenceRangeScope,
        findings: FindingsShown,
        metric_plugins: &[&dyn MetricPlugin],
        boundaries: Option<&BoundariesConfig>,
        orphan_files: Option<&OrphanFiles>,
        ownership: OwnershipOptions<'_>,
    ) -> Result<SourceSetSections, MetricPluginError> {
        let mut sections = SourceSetSections::default();
        for (source_set, model) in source_sets.iter() {
            if !model.files.is_empty() {
                let source_set_boundaries =
                    boundaries.filter(|config| config.applies_to_source_set(source_set));
                let source_set_orphans =
                    orphan_files.filter(|_| source_set == SourceSet::Production);
                *sections.slot(source_set) = Some(SourceSetSection::new(
                    model,
                    ranges,
                    findings,
                    metric_plugins,
                    source_set_boundaries,
                    source_set_orphans,
                    ownership,
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
                    (source_set, Metric::Erosion.key(), section.erosion.ratio.0),
                    (
                        source_set,
                        Metric::Duplication.key(),
                        section.duplication.ratio.0,
                    ),
                    (
                        source_set,
                        Metric::Verbosity.key(),
                        section.verbosity.ratio.0,
                    ),
                    (
                        source_set,
                        Metric::ImportCycles.key(),
                        section.import_cycles.ratio.0,
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
                    section
                        .orphan_files
                        .iter()
                        .map(move |orphans| (source_set, "orphan_files", orphans.ratio.0)),
                )
                .chain(
                    section
                        .external_metrics
                        .iter()
                        .map(move |(key, metric)| (source_set, key.as_str(), metric.ratio.0)),
                )
            }),
        )
    }
}

impl SourceSetSection {
    fn new(
        model: &CodeModel,
        ranges: &ReferenceRangeScope,
        findings: FindingsShown,
        metric_plugins: &[&dyn MetricPlugin],
        boundaries: Option<&BoundariesConfig>,
        orphan_files: Option<&OrphanFiles>,
        ownership: OwnershipOptions<'_>,
    ) -> Result<SourceSetSection, MetricPluginError> {
        let view = FindingsView {
            shown: findings,
            ownership,
        };
        let measured = Measured::new(model);
        let boundaries = evaluate_boundaries(model, boundaries)
            .map(|evaluated| BoundariesSection::new(&evaluated, view));
        let orphan_files = orphan_files.map(|measured| OrphanFilesSection::new(measured, view));
        let mut external_metrics = BTreeMap::new();
        for plugin in metric_plugins {
            let result = plugin.measure(model)?;
            external_metrics.insert(
                plugin.key().to_owned(),
                ExternalMetricSection {
                    name: plugin.name().to_owned(),
                    description: plugin.description().to_owned(),
                    ratio: Decimal::rounded(result.ratio),
                    reference_range: ranges.entry(plugin.key()),
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
            erosion: ErosionSection {
                ratio: Decimal::rounded(measured.erosion.ratio()),
                reference_range: ranges.entry(Metric::Erosion.key()),
                eroded_function_mass: Decimal::rounded(measured.erosion.eroded_function_mass()),
                total_function_mass: Decimal::rounded(measured.erosion.total_function_mass()),
                findings: view.select(
                    measured
                        .erosion
                        .eroded_functions()
                        .iter()
                        .map(ErodedFunctionFinding::new),
                ),
            },
            duplication: DuplicationSection {
                ratio: Decimal::rounded(measured.duplication.ratio()),
                reference_range: ranges.entry(Metric::Duplication.key()),
                clone_lines: measured.duplication.clone_lines.count(),
                clone_groups: view.select(
                    measured
                        .duplication
                        .clone_groups
                        .iter()
                        .map(CloneGroupFinding::new),
                ),
            },
            verbosity: VerbositySection {
                ratio: Decimal::rounded(measured.verbosity.ratio()),
                reference_range: ranges.entry(Metric::Verbosity.key()),
                verbose_lines: measured.verbosity.verbose_lines(),
                flagged_lines: measured.verbosity.flagged_line_count(),
                clone_lines: measured.verbosity.clone_line_count(),
                findings: view.map(measured.verbosity.flagged_lines()),
            },
            import_cycles: ImportCyclesSection {
                ratio: Decimal::rounded(measured.import_cycles.ratio()),
                reference_range: ranges.entry(Metric::ImportCycles.key()),
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
            orphan_files,
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
            cyclomatic_complexity: function.cyclomatic_complexity,
            source_lines: function.source_lines,
            function_mass: Decimal::rounded(function.function_mass),
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
    let mut duplication = measured.duplication.by_package(model);
    let mut verbosity = measured.verbosity.by_package(model);
    let mut import_cycles = measured.import_cycles.by_package(model);
    measured
        .erosion
        .by_package(model)
        .into_iter()
        .map(|(package, masses)| {
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

impl MeasuredSection {
    fn new(
        model: CodeModel,
        ranges: &ReferenceRangeScope,
        findings: FindingsShown,
        metric_plugins: &[&dyn MetricPlugin],
        boundaries: Option<&BoundariesConfig>,
        orphan_files: Option<&OrphanFiles>,
        ownership: OwnershipOptions<'_>,
    ) -> Result<MeasuredSection, MetricPluginError> {
        let files = model.files.len();
        let lines = source_lines(&model);
        let source_sets = SourceSetSections::new(
            &model.into_source_sets(),
            ranges,
            findings,
            metric_plugins,
            boundaries,
            orphan_files,
            ownership,
        )?;
        Ok(MeasuredSection {
            files,
            source_lines: lines,
            source_sets,
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
        Report::summarize_with_options(
            languages,
            submodules,
            findings,
            budgets,
            ReportOptions {
                metric_plugins,
                boundaries,
                orphan_files: None,
                orphan_files_by_language: None,
                ownership,
            },
        )
    }

    pub fn summarize_with_options(
        languages: Vec<MeasuredLanguage>,
        submodules: Vec<RepoPath>,
        findings: FindingsShown,
        budgets: &Budgets,
        options: ReportOptions<'_>,
    ) -> Result<Report, MetricPluginError> {
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
                    let language_orphans = options
                        .orphan_files_by_language
                        .and_then(|map| map.get(&language.plugin));
                    per_language.insert(
                        language.plugin.clone(),
                        MeasuredSection::new(
                            language.model.clone(),
                            &ReferenceRangeScope::OneLanguage(language.reference_ranges.clone()),
                            findings,
                            options.metric_plugins,
                            options.boundaries,
                            language_orphans,
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
            &total_ranges,
            findings,
            options.metric_plugins,
            options.boundaries,
            options.orphan_files,
            options.ownership,
        )?;
        Ok(Report {
            over_budget: measured.source_sets.over_budget(budgets),
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
