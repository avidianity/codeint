#![forbid(unsafe_code)]

use std::collections::{BTreeMap, BTreeSet};

use codeint_config::{
    Baseline, Boundaries as BoundariesConfig, Budgets, DependenciesConfig, DuplicationConfig,
    FindingsShown, MeasuredBaselineMetric, MeasuredRatchetMetric, Ratchet, RatchetVersionMismatch,
    ReportFormat,
};
use codeint_metrics::{DependencyHygiene, Duplication, OrphanFiles};
use codeint_model::{
    CodeModel, Metric, MetricPlugin, MetricPluginError, ReferenceRanges, RepoPath, SourceSet,
};
use serde::{Deserialize, Serialize};

macro_rules! crate_structs {
    (
        $(
            $(#[$meta:meta])*
            struct $name:ident {
                $(
                    $(#[$field_meta:meta])*
                    $field:ident: $ty:ty,
                )*
            }
        )*
    ) => {
        $(
            $(#[$meta])*
            pub(crate) struct $name {
                $(
                    $(#[$field_meta])*
                    pub(crate) $field: $ty,
                )*
            }
        )*
    };
    (
        @finding
        $(
            $(#[$meta:meta])*
            struct $name:ident {
                $(
                    $(#[$field_meta:meta])*
                    $field:ident: $ty:ty,
                )*
            }
        )*
    ) => {
        $(
            crate_structs! {
                $(#[$meta])*
                struct $name {
                    id: String,
                    $(
                        $(#[$field_meta])*
                        $field: $ty,
                    )*
                    #[serde(default, skip_serializing_if = "Vec::is_empty")]
                    owners: Vec<String>,
                    #[serde(skip)]
                    anchor: Option<codeint_config::FindingAnchor>,
                }
            }
        )*
    };
}

macro_rules! with_fields {
    (
        $(
            $method:ident($field:ident: $ty:ty) => $expr:expr
        ),* $(,)?
    ) => {
        $(
            pub fn $method(mut self, $field: $ty) -> Self {
                self.$field = $expr;
                self
            }
        )*
    };
}

macro_rules! impl_ratchet_metrics_source {
    ($($ty:ty => $ratio:ident),* $(,)?) => {
        $(
            impl crate::ratchet::RatchetMetricsSource for $ty {
                fn push_ratchet_metrics(
                    &self,
                    source_set: Option<codeint_model::SourceSet>,
                    out: &mut Vec<codeint_config::MeasuredRatchetMetric>,
                ) {
                    crate::ratchet::push_ratchet_entries(
                        out,
                        source_set,
                        self.metric_summaries()
                            .into_iter()
                            .map(|s| (s.key, s.$ratio.0, s.metric_version)),
                    );
                }
            }
        )*
    };
}

mod agent;
mod baseline;
mod baseline_listing;
mod baseline_rows;
mod churn;
mod config;
mod coupling;
mod dependencies;
mod diff;
mod doctor;
mod findings;
mod guard;
#[cfg(test)]
mod hazards_tests;
mod impact;
mod markdown;
mod measurement;
mod ownership;
mod plugin_test;
mod ratchet;
mod render;
mod report_schema;
mod sarif;
mod saved;
mod schema;
mod sections;
mod summary;
#[cfg(test)]
mod tests;
mod text;
mod trace;
mod trend;

pub use agent::{
    AgentInstallAction, AgentInstallEntry, AgentInstallReport, AgentStatusEntry, AgentStatusReport,
};
pub use baseline_listing::render_baseline_update;
pub use churn::{ChurnHistoryInput, ChurnReport};
pub use codeint_metrics::TraceError;
pub use config::ConfigReport;
pub use coupling::{CouplingHistoryInput, CouplingReport};
pub use diff::{BaseCommit, DiffReport, DiffReportOptions, Head, HeadCommit, NewFileBudgetFailure};
pub use doctor::{DoctorCheck, DoctorReport, DoctorStatus, DoctorSummary};
pub use findings::{FileDiagnosticEntry, HazardLineRow, ScopeSummaryEntry};
pub use guard::{GuardAllowedTarget, GuardDeniedTarget, GuardExternalImports, GuardReport};
pub use impact::{ChangedBase, ImpactReport};
pub use measurement::{CommitMeasurement, CommitMeasurementOptions};
pub use ownership::OwnershipOptions;
pub use plugin_test::{PluginTestCheck, PluginTestReport, PluginTestStatus, PluginTestSummary};
pub use ratchet::{RATCHET_PACKAGE_SKIPPED_NOTE, RatchetFailure, RatchetUpdateReport};
pub use render::{ReportError, render_toon};
pub use report_schema::report_schema;
pub use saved::{SavedReport, SavedReportError};
pub use schema::{ExitCodeSchema, LanguageSchema, McpToolSchema, MetricSchema, SchemaReport};
pub use trace::{TraceMultiFileImport, TraceReport};
pub use trend::{TrendPoint, TrendReport};

pub(crate) use baseline::{BudgetedRatio, ReportBaseline};
pub(crate) use dependencies::{ImportedDependencyRow, UnusedDependencyRow};
pub(crate) use findings::{
    BoundaryFindingRow, CloneFamilyFinding, CloneGroupFinding, CloneLocation,
    CognitiveComplexityFinding, ErodedFunctionFinding, ExternalMetricFinding, FlaggedLineRow,
    ImportCycleFinding, OrphanFileRow, SkippedEntry, TestGapFindingRow, UnresolvedImportRow,
    clustered_clone_group_ids, unlisted_clone_family_group_ids,
};
pub(crate) use render::{
    Decimal, FindingKind, Findings, PerSourceSet, ReferenceRangeEntry, ReferenceRangeScope,
    TEST_GAP_DIFF_NOTE, TEST_GAP_TREND_NOTE, format_unmatched_coverage_note, render_json,
    render_text_report,
};
pub(crate) use sections::{
    DependenciesSection, HazardsSection, ImportCyclesSection, MeasuredSection, MetricHeader,
    PackageSection, SourceSetSection, SourceSetSections,
};
pub(crate) use summary::{Measured, MeasuredSectionModels, SectionModels, SectionOptions};

#[derive(Debug, Serialize, Deserialize)]
pub struct Report {
    #[serde(flatten)]
    measured: MeasuredSection,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    coverage_unmatched_note: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    ratchet_note: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    manifest_warnings: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    baseline: Option<ReportBaseline>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    languages: BTreeMap<String, MeasuredSection>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    skipped: Vec<SkippedEntry>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    file_diagnostics: Vec<FileDiagnosticEntry>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    scope_summaries: Vec<ScopeSummaryEntry>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    declaration_only: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    submodules: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    over_budget: Vec<BudgetFailure>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    over_ratchet: Vec<RatchetFailure>,
    #[serde(skip)]
    show_all: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BudgetFailure {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    source_set: Option<String>,
    metric: String,
    ratio: Decimal,
    budget: f64,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    non_baselined: bool,
}

impl std::fmt::Display for BudgetFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let scope = if self.non_baselined {
            "the non-baselined"
        } else {
            "the"
        };
        let source_set = self
            .source_set
            .as_deref()
            .map(|source_set| format!(" {source_set}"))
            .unwrap_or_default();
        write!(
            formatter,
            "{scope}{source_set} {} ratio {} is over its Budget {}",
            self.metric, self.ratio.0, self.budget
        )
    }
}

impl BudgetFailure {
    fn exceeding(
        source_set: Option<SourceSet>,
        metric: &str,
        measured: BudgetedRatio,
        budget: codeint_config::Budget,
    ) -> Option<BudgetFailure> {
        (!budget.admits(measured.ratio)).then(|| BudgetFailure {
            source_set: source_set.map(|source_set| source_set.key().to_owned()),
            metric: metric.to_owned(),
            ratio: Decimal(measured.ratio),
            budget: budget.ratio(),
            non_baselined: measured.non_baselined,
        })
    }
}

pub(crate) fn over_budget<'a, R: Into<BudgetedRatio>>(
    budgets: &Budgets,
    displayed_ratios: impl Iterator<Item = (SourceSet, &'a str, R)>,
) -> Vec<BudgetFailure> {
    displayed_ratios
        .filter_map(|(source_set, metric, ratio)| {
            let budget = budgets.budget_for_key(source_set, metric)?;
            BudgetFailure::exceeding(Some(source_set), metric, ratio.into(), budget)
        })
        .collect()
}

pub(crate) fn dependency_over_budget(
    budgets: &Budgets,
    unused: impl Into<BudgetedRatio>,
    unlisted: impl Into<BudgetedRatio>,
    dev_in_production: impl Into<BudgetedRatio>,
) -> Vec<BudgetFailure> {
    [
        (Metric::UnusedDependencies, unused.into()),
        (Metric::UnlistedDependencies, unlisted.into()),
        (
            Metric::DevInProductionDependencies,
            dev_in_production.into(),
        ),
    ]
    .into_iter()
    .filter_map(|(metric, ratio)| {
        let budget = budgets.dependency_budget(metric.key())?;
        BudgetFailure::exceeding(None, metric.key(), ratio, budget)
    })
    .collect()
}

impl DependenciesSection {
    fn over_budget(&self, budgets: &Budgets) -> Vec<BudgetFailure> {
        let [unused, unlisted, dev_in_production] = self.metric_summaries();
        dependency_over_budget(
            budgets,
            unused.budgeted_ratio,
            unlisted.budgeted_ratio,
            dev_in_production.budgeted_ratio,
        )
    }
}

impl SourceSetSections {
    fn over_budget(&self, budgets: &Budgets) -> Vec<BudgetFailure> {
        over_budget(
            budgets,
            self.iter().flat_map(|(source_set, section)| {
                section
                    .metric_summaries()
                    .into_iter()
                    .map(move |summary| (source_set, summary.key, summary.budgeted_ratio))
            }),
        )
    }
}

pub struct MeasuredLanguage {
    pub plugin: String,
    pub model: CodeModel,
    pub reference_ranges: ReferenceRanges,
    pub reference_range_note: Option<String>,
    pub has_imports: bool,
    pub framework_profiles: Vec<String>,
    pub framework_profiles_by_package: BTreeMap<String, Vec<String>>,
}

impl MeasuredLanguage {
    pub fn new(
        plugin: impl Into<String>,
        model: CodeModel,
        reference_ranges: ReferenceRanges,
        has_imports: bool,
    ) -> MeasuredLanguage {
        MeasuredLanguage {
            plugin: plugin.into(),
            model,
            reference_ranges,
            reference_range_note: None,
            has_imports,
            framework_profiles: Vec::new(),
            framework_profiles_by_package: BTreeMap::new(),
        }
    }
}

pub fn combine_reference_range_notes<'a>(
    notes: impl IntoIterator<Item = &'a str>,
) -> Option<String> {
    let unique: BTreeSet<&str> = notes.into_iter().filter(|note| !note.is_empty()).collect();
    (!unique.is_empty()).then(|| unique.into_iter().collect::<Vec<_>>().join("; "))
}

pub struct ReportOptions<'a> {
    pub findings: FindingsShown,
    pub budgets: &'a Budgets,
    pub metric_plugins: &'a [&'a dyn MetricPlugin],
    pub boundaries: Option<&'a BoundariesConfig>,
    pub dependencies: Option<&'a DependenciesConfig>,
    pub dependency_hygiene: Option<&'a DependencyHygiene>,
    pub dependency_hygiene_by_language: Option<&'a BTreeMap<String, DependencyHygiene>>,
    pub duplication: DuplicationConfig,
    pub orphan_files: Option<&'a OrphanFiles>,
    pub orphan_files_by_language: Option<&'a BTreeMap<String, OrphanFiles>>,
    pub ownership: OwnershipOptions<'a>,
    pub coverage_reports_named: bool,
    pub unmatched_coverage_paths: &'a [String],
    pub baseline: Option<&'a Baseline>,
    pub extras: ReportExtras,
}

impl<'a> ReportOptions<'a> {
    pub fn new(findings: FindingsShown, budgets: &'a Budgets) -> ReportOptions<'a> {
        ReportOptions {
            findings,
            budgets,
            metric_plugins: &[],
            boundaries: None,
            dependencies: None,
            dependency_hygiene: None,
            dependency_hygiene_by_language: None,
            duplication: DuplicationConfig::default(),
            orphan_files: None,
            orphan_files_by_language: None,
            ownership: OwnershipOptions::default(),
            coverage_reports_named: false,
            unmatched_coverage_paths: &[],
            baseline: None,
            extras: ReportExtras::default(),
        }
    }

    with_fields! {
        with_metric_plugins(metric_plugins: &'a [&'a dyn MetricPlugin]) => metric_plugins,
        with_boundaries(boundaries: Option<&'a BoundariesConfig>) => boundaries,
        with_dependencies(dependencies: &'a DependenciesConfig) => Some(dependencies),
        with_duplication(duplication: DuplicationConfig) => duplication,
        with_ownership(ownership: OwnershipOptions<'a>) => ownership,
        with_baseline(baseline: Option<&'a Baseline>) => baseline,
        with_extras(extras: ReportExtras) => extras,
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReportExtras {
    pub disabled_hazard_rules: BTreeSet<String>,
    pub file_diagnostics: Vec<FileDiagnosticEntry>,
    pub scope_summaries: Vec<ScopeSummaryEntry>,
    pub manifest_warnings: Vec<String>,
}

impl ReportExtras {
    pub fn with_manifest_warnings(mut self, warnings: Vec<String>) -> ReportExtras {
        self.manifest_warnings = warnings;
        self
    }

    pub fn with_disabled_hazard_rules(mut self, rules: BTreeSet<String>) -> ReportExtras {
        self.disabled_hazard_rules = rules;
        self
    }

    pub fn with_file_diagnostics(
        mut self,
        file_diagnostics: Vec<FileDiagnosticEntry>,
    ) -> ReportExtras {
        self.file_diagnostics = file_diagnostics;
        self
    }

    pub fn with_scope_summaries(mut self, scope_summaries: Vec<ScopeSummaryEntry>) -> ReportExtras {
        self.scope_summaries = scope_summaries;
        self
    }
}

impl Report {
    pub fn manifest_warnings(&self) -> &[String] {
        &self.manifest_warnings
    }

    pub fn summarize(
        languages: Vec<MeasuredLanguage>,
        submodules: Vec<RepoPath>,
        findings: FindingsShown,
        budgets: &Budgets,
    ) -> Report {
        Report::summarize_with_options(languages, submodules, ReportOptions::new(findings, budgets))
            .expect("built-in metrics cannot fail")
    }

    pub fn summarize_with_options(
        languages: Vec<MeasuredLanguage>,
        submodules: Vec<RepoPath>,
        options: ReportOptions<'_>,
    ) -> Result<Report, MetricPluginError> {
        let empty_ignored = BTreeSet::new();
        let ignored = options
            .dependencies
            .map_or(&empty_ignored, DependenciesConfig::ignored);
        let extras = options.extras;
        let measured_total_hygiene;
        let total_hygiene = match options.dependency_hygiene {
            Some(precomputed) => precomputed,
            None => {
                measured_total_hygiene = measure_combined_hygiene(&languages, ignored);
                &measured_total_hygiene
            }
        };
        let (measured, unmeasured): (Vec<MeasuredLanguage>, Vec<MeasuredLanguage>) = languages
            .into_iter()
            .partition(|language| !language.model.files.is_empty());
        let top_has_imports =
            measured.is_empty() || measured.iter().any(|language| language.has_imports);
        let top_has_hazard_rules = measured
            .iter()
            .any(|language| model_has_hazard_rules(&language.model));
        let top_framework_profiles = collect_framework_profiles(&measured);
        let top_framework_profiles_by_package = collect_framework_profiles_by_package(&measured);
        let top_reference_range_note = collect_reference_range_note(&measured);
        let section_options = SectionOptions {
            findings: options.findings,
            metric_plugins: options.metric_plugins,
            boundaries: options.boundaries,
            duplication: options.duplication,
            orphan_files: options.orphan_files,
            ownership: options.ownership,
            has_imports: top_has_imports,
            has_hazard_rules: top_has_hazard_rules,
            disabled_hazard_rules: &extras.disabled_hazard_rules,
            coverage_reports_named: options.coverage_reports_named,
            per_language_section: false,
            framework_profiles: &top_framework_profiles,
            framework_profiles_by_package: &top_framework_profiles_by_package,
            reference_range_note: top_reference_range_note.as_deref(),
            baseline: options.baseline,
        };
        let (total_ranges, sections_per_language, precomputed_duplication) =
            build_per_language_sections(
                &measured,
                options.dependency_hygiene_by_language,
                options.orphan_files_by_language,
                ignored,
                section_options,
            )?;
        let import_bearing_model = collect_import_bearing_model(&measured, top_has_imports);
        let coverage_bearing_model = collect_coverage_bearing_model(&measured);
        let hazards_bearing_model = collect_hazards_bearing_model(&measured, top_has_hazard_rules);
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
        let (measured, _) = MeasuredSection::new(
            MeasuredSectionModels {
                model,
                import_model: import_bearing_model,
                coverage_model: coverage_bearing_model,
                hazards_model: hazards_bearing_model,
            },
            total_hygiene,
            &total_ranges,
            section_options,
            precomputed_duplication,
        )?;
        let mut failures = measured.source_sets.over_budget(options.budgets);
        if let Some(dependencies) = &measured.dependencies {
            failures.extend(dependencies.over_budget(options.budgets));
        }
        let baseline = options
            .baseline
            .map(|_| ReportBaseline::new(measured.hidden_by_baseline()));
        Ok(Report {
            baseline,
            over_budget: failures,
            over_ratchet: Vec::new(),
            skipped,
            file_diagnostics: extras.file_diagnostics,
            scope_summaries: extras.scope_summaries,
            declaration_only,
            submodules: submodules.iter().map(RepoPath::to_string).collect(),
            measured,
            coverage_unmatched_note: format_unmatched_coverage_note(
                options.unmatched_coverage_paths,
            ),
            ratchet_note: None,
            manifest_warnings: extras.manifest_warnings,
            languages: sections_per_language,
            show_all: options.findings == FindingsShown::All,
        })
    }

    pub fn has_production_test_gap(&self) -> bool {
        self.measured
            .source_sets
            .production()
            .is_some_and(|section| section.test_gap.is_some())
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

    pub fn over_ratchet(&self) -> &[RatchetFailure] {
        &self.over_ratchet
    }

    pub fn failure_messages(&self) -> Vec<String> {
        self.over_budget
            .iter()
            .map(ToString::to_string)
            .chain(self.over_ratchet.iter().map(ToString::to_string))
            .collect()
    }

    pub fn set_ratchet_note(&mut self, note: impl Into<String>) {
        self.ratchet_note = Some(note.into());
    }

    pub fn baseline_metrics(&self) -> Vec<MeasuredBaselineMetric> {
        self.measured.baseline_metrics()
    }

    pub fn ratchet_metrics(&self) -> Vec<MeasuredRatchetMetric> {
        ratchet::collect_ratchet_metrics(
            &self.measured.source_sets,
            self.measured.dependencies.as_ref(),
        )
    }

    pub fn apply_ratchet(&mut self, ratchet: &Ratchet) -> Result<(), Vec<RatchetVersionMismatch>> {
        self.over_ratchet = ratchet::evaluate_ratchet(ratchet, &self.ratchet_metrics())?;
        Ok(())
    }

    pub(crate) fn set_show_all(&mut self, show_all: bool) {
        self.show_all = show_all;
    }

    pub fn render(&self, format: ReportFormat) -> Result<String, ReportError> {
        match format {
            ReportFormat::Toon => render_toon(self),
            ReportFormat::Text => Ok(text::render_check(self)),
            ReportFormat::Json => render_json(self),
            ReportFormat::Sarif => serde_json::to_string_pretty(&sarif::SarifLog::new(self))
                .map_err(ReportError::Sarif),
            ReportFormat::Markdown => Ok(markdown::render(self)),
        }
    }
}

fn measure_combined_hygiene(
    languages: &[MeasuredLanguage],
    ignored: &BTreeSet<String>,
) -> DependencyHygiene {
    DependencyHygiene::merge(
        languages
            .iter()
            .filter(|language| language.has_imports)
            .map(|language| DependencyHygiene::measure(&language.model, ignored)),
    )
}

fn collect_coverage_bearing_model(measured: &[MeasuredLanguage]) -> Option<CodeModel> {
    let mut covered = Vec::new();
    for language in measured {
        if language
            .model
            .languages
            .iter()
            .any(|facts| facts.has_coverage)
        {
            covered.push(language);
        }
    }
    if covered.is_empty() || covered.len() == measured.len() {
        return None;
    }
    Some(CodeModel::merge(
        covered.into_iter().map(|language| language.model.clone()),
    ))
}

pub(crate) fn model_has_hazard_rules(model: &CodeModel) -> bool {
    model.languages.iter().any(|facts| facts.has_hazard_rules)
}

fn collect_hazards_bearing_model(
    measured: &[MeasuredLanguage],
    any_has_hazard_rules: bool,
) -> Option<CodeModel> {
    let all_measured_have_hazard_rules = measured
        .iter()
        .all(|language| model_has_hazard_rules(&language.model));
    (any_has_hazard_rules && !all_measured_have_hazard_rules).then(|| {
        CodeModel::merge(
            measured
                .iter()
                .filter(|language| model_has_hazard_rules(&language.model))
                .map(|language| language.model.clone()),
        )
    })
}

fn collect_import_bearing_model(
    measured: &[MeasuredLanguage],
    top_has_imports: bool,
) -> Option<CodeModel> {
    let all_measured_have_imports = measured.iter().all(|language| language.has_imports);
    (top_has_imports && !all_measured_have_imports).then(|| {
        CodeModel::merge(
            measured
                .iter()
                .filter(|language| language.has_imports)
                .map(|language| language.model.clone()),
        )
    })
}

fn collect_framework_profiles(measured: &[MeasuredLanguage]) -> Vec<String> {
    let mut profiles = BTreeSet::new();
    for language in measured {
        profiles.extend(language.framework_profiles.iter().cloned());
    }
    profiles.into_iter().collect()
}

fn collect_reference_range_note(measured: &[MeasuredLanguage]) -> Option<String> {
    combine_reference_range_notes(
        measured
            .iter()
            .filter_map(|language| language.reference_range_note.as_deref()),
    )
}

fn collect_framework_profiles_by_package(
    measured: &[MeasuredLanguage],
) -> BTreeMap<String, Vec<String>> {
    let mut by_package: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for language in measured {
        for (package, profiles) in &language.framework_profiles_by_package {
            by_package
                .entry(package.clone())
                .or_default()
                .extend(profiles.iter().cloned());
        }
    }
    by_package
        .into_iter()
        .filter(|(_, profiles)| !profiles.is_empty())
        .map(|(package, profiles)| (package, profiles.into_iter().collect()))
        .collect()
}

type PerLanguageSections = (
    ReferenceRangeScope,
    BTreeMap<String, MeasuredSection>,
    Option<BTreeMap<SourceSet, Duplication>>,
);

fn build_per_language_sections(
    measured: &[MeasuredLanguage],
    dependency_hygiene_by_language: Option<&BTreeMap<String, DependencyHygiene>>,
    orphan_files_by_language: Option<&BTreeMap<String, OrphanFiles>>,
    ignored: &BTreeSet<String>,
    section_options: SectionOptions<'_>,
) -> Result<PerLanguageSections, MetricPluginError> {
    match measured {
        [] => Ok((
            ReferenceRangeScope::OneLanguage(ReferenceRanges::default()),
            BTreeMap::new(),
            None,
        )),
        [only] => Ok((
            ReferenceRangeScope::OneLanguage(only.reference_ranges.clone()),
            BTreeMap::new(),
            None,
        )),
        _ => {
            let mut per_language = BTreeMap::new();
            let mut duplication_parts: BTreeMap<SourceSet, Vec<Duplication>> = BTreeMap::new();
            for language in measured {
                let (section, lang_duplication) = build_single_language_section(
                    language,
                    dependency_hygiene_by_language,
                    orphan_files_by_language,
                    ignored,
                    section_options,
                )?;
                per_language.insert(language.plugin.clone(), section);
                for (source_set, dup) in lang_duplication {
                    duplication_parts.entry(source_set).or_default().push(dup);
                }
            }
            let merged_duplication = duplication_parts
                .into_iter()
                .map(|(source_set, parts)| (source_set, Duplication::merge(parts)))
                .collect();
            Ok((
                ReferenceRangeScope::SeveralLanguages,
                per_language,
                Some(merged_duplication),
            ))
        }
    }
}

fn build_single_language_section(
    language: &MeasuredLanguage,
    dependency_hygiene_by_language: Option<&BTreeMap<String, DependencyHygiene>>,
    orphan_files_by_language: Option<&BTreeMap<String, OrphanFiles>>,
    ignored: &BTreeSet<String>,
    section_options: SectionOptions<'_>,
) -> Result<(MeasuredSection, BTreeMap<SourceSet, Duplication>), MetricPluginError> {
    let measured_language_hygiene;
    let language_hygiene = if language.has_imports {
        match dependency_hygiene_by_language.and_then(|map| map.get(&language.plugin)) {
            Some(precomputed) => precomputed,
            None => {
                measured_language_hygiene = DependencyHygiene::measure(&language.model, ignored);
                &measured_language_hygiene
            }
        }
    } else {
        measured_language_hygiene = DependencyHygiene::default();
        &measured_language_hygiene
    };
    let language_orphans = language
        .has_imports
        .then(|| orphan_files_by_language.and_then(|map| map.get(&language.plugin)))
        .flatten();
    let language_has_hazard_rules = model_has_hazard_rules(&language.model);
    MeasuredSection::new(
        MeasuredSectionModels {
            model: language.model.clone(),
            import_model: None,
            coverage_model: None,
            hazards_model: None,
        },
        language_hygiene,
        &ReferenceRangeScope::OneLanguage(language.reference_ranges.clone()),
        SectionOptions {
            orphan_files: language_orphans,
            has_imports: language.has_imports,
            has_hazard_rules: language_has_hazard_rules,
            per_language_section: true,
            framework_profiles: &language.framework_profiles,
            framework_profiles_by_package: &language.framework_profiles_by_package,
            reference_range_note: language.reference_range_note.as_deref(),
            ..section_options
        },
        None,
    )
}
