#![forbid(unsafe_code)]

use std::collections::{BTreeMap, BTreeSet};

use codeint_config::{
    Boundaries as BoundariesConfig, Budgets, DependenciesConfig, DuplicationConfig, FindingsShown,
    ReportFormat,
};
use codeint_metrics::{DependencyHygiene, OrphanFiles};
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
}

mod agent;
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
mod render;
mod report_schema;
mod sarif;
mod saved;
mod schema;
mod sections;
mod summary;
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
pub use diff::{BaseCommit, DiffReport, DiffReportOptions, Head, HeadCommit, NewFileBudgetFailure};
pub use doctor::{DoctorCheck, DoctorReport, DoctorStatus, DoctorSummary};
pub use findings::{HazardLineRow, RejectedStatementEntry, SqlSchemaSummary};
pub use guard::{GuardAllowedTarget, GuardDeniedTarget, GuardExternalImports, GuardReport};
pub use impact::{ChangedBase, ImpactReport};
pub use measurement::{CommitMeasurement, CommitMeasurementOptions};
pub use ownership::OwnershipOptions;
pub use render::ReportError;
pub use report_schema::report_schema;
pub use saved::{SavedReport, SavedReportError};
pub use schema::{ExitCodeSchema, LanguageSchema, McpToolSchema, MetricSchema, SchemaReport};
pub use trace::{TraceMultiFileImport, TraceReport};
pub use trend::{TrendPoint, TrendReport};

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
    render_text_report, render_toon,
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
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    languages: BTreeMap<String, MeasuredSection>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    skipped: Vec<SkippedEntry>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    rejected_statements: Vec<RejectedStatementEntry>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    sql_schemas: Vec<SqlSchemaSummary>,
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

pub(crate) fn over_budget<'a>(
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

pub(crate) fn dependency_over_budget(
    budgets: &Budgets,
    unused: f64,
    unlisted: f64,
    dev_in_production: f64,
) -> Vec<BudgetFailure> {
    [
        (Metric::UnusedDependencies, unused),
        (Metric::UnlistedDependencies, unlisted),
        (Metric::DevInProductionDependencies, dev_in_production),
    ]
    .into_iter()
    .filter_map(|(metric, ratio)| {
        let budget = budgets.dependency_budget(metric.key())?;
        (!budget.admits(ratio)).then(|| BudgetFailure {
            source_set: None,
            metric: metric.key().to_owned(),
            ratio: Decimal(ratio),
            budget: budget.ratio(),
        })
    })
    .collect()
}

impl DependenciesSection {
    fn over_budget(&self, budgets: &Budgets) -> Vec<BudgetFailure> {
        dependency_over_budget(
            budgets,
            self.unused_dependencies.header.ratio.0,
            self.unlisted_dependencies.header.ratio.0,
            self.dev_in_production_dependencies.header.ratio.0,
        )
    }
}

impl SourceSetSections {
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
                        Metric::CognitiveComplexity.key(),
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
                .chain(section.boundaries.iter().map(move |boundaries| {
                    (
                        source_set,
                        Metric::Boundaries.key(),
                        boundaries.header.ratio.0,
                    )
                }))
                .chain(section.orphan_files.iter().map(move |orphans| {
                    (
                        source_set,
                        Metric::OrphanFiles.key(),
                        orphans.header.ratio.0,
                    )
                }))
                .chain(section.test_gap.iter().map(move |test_gap| {
                    (source_set, Metric::TestGap.key(), test_gap.header.ratio.0)
                }))
                .chain(section.hazards.iter().map(move |hazards| {
                    (source_set, Metric::Hazards.key(), hazards.header.ratio.0)
                }))
                .chain(
                    section.external_metrics.iter().map(move |(key, metric)| {
                        (source_set, key.as_str(), metric.header.ratio.0)
                    }),
                )
            }),
        )
    }
}

pub struct MeasuredLanguage {
    pub plugin: String,
    pub model: CodeModel,
    pub reference_ranges: ReferenceRanges,
    pub has_imports: bool,
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
    pub coverage_reports_named: bool,
    pub unmatched_coverage_paths: &'a [String],
}

impl<'a> ReportOptions<'a> {
    fn basic(
        findings: FindingsShown,
        budgets: &'a Budgets,
        metric_plugins: &'a [&'a dyn MetricPlugin],
        boundaries: Option<&'a BoundariesConfig>,
        dependencies: &'a DependenciesConfig,
    ) -> ReportOptions<'a> {
        ReportOptions {
            findings,
            budgets,
            metric_plugins,
            boundaries,
            dependencies,
            dependency_hygiene: None,
            dependency_hygiene_by_language: None,
            duplication: DuplicationConfig::default(),
            orphan_files: None,
            orphan_files_by_language: None,
            ownership: OwnershipOptions::default(),
            coverage_reports_named: false,
            unmatched_coverage_paths: &[],
        }
    }

    fn with_duplication(mut self, duplication: DuplicationConfig) -> ReportOptions<'a> {
        self.duplication = duplication;
        self
    }

    fn with_ownership(mut self, ownership: OwnershipOptions<'a>) -> ReportOptions<'a> {
        self.ownership = ownership;
        self
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReportExtras {
    pub disabled_hazard_rules: BTreeSet<String>,
    pub rejected_statements: Vec<RejectedStatementEntry>,
    pub sql_schemas: Vec<SqlSchemaSummary>,
}

impl ReportExtras {
    pub fn with_disabled_hazard_rules(mut self, rules: BTreeSet<String>) -> ReportExtras {
        self.disabled_hazard_rules = rules;
        self
    }

    pub fn with_rejected_statements(
        mut self,
        rejected_statements: Vec<RejectedStatementEntry>,
    ) -> ReportExtras {
        self.rejected_statements = rejected_statements;
        self
    }

    pub fn with_sql_schemas(mut self, sql_schemas: Vec<SqlSchemaSummary>) -> ReportExtras {
        self.sql_schemas = sql_schemas;
        self
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
            ReportOptions::basic(findings, budgets, metric_plugins, boundaries, &dependencies),
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
        let options =
            ReportOptions::basic(findings, budgets, metric_plugins, boundaries, &dependencies)
                .with_duplication(duplication);
        Report::summarize_with_options(languages, submodules, options)
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
            ReportOptions::basic(findings, budgets, metric_plugins, boundaries, &dependencies)
                .with_ownership(ownership),
        )
    }

    pub fn summarize_with_options(
        languages: Vec<MeasuredLanguage>,
        submodules: Vec<RepoPath>,
        options: ReportOptions<'_>,
    ) -> Result<Report, MetricPluginError> {
        Report::summarize_with_extras(languages, submodules, options, ReportExtras::default())
    }

    pub fn summarize_with_extras(
        languages: Vec<MeasuredLanguage>,
        submodules: Vec<RepoPath>,
        options: ReportOptions<'_>,
        extras: ReportExtras,
    ) -> Result<Report, MetricPluginError> {
        let ignored = options.dependencies.ignored();
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
        };
        let (total_ranges, sections_per_language) =
            build_per_language_sections(&measured, &options, ignored, section_options)?;
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
        let measured = MeasuredSection::new(
            MeasuredSectionModels {
                model,
                import_model: import_bearing_model,
                coverage_model: coverage_bearing_model,
                hazards_model: hazards_bearing_model,
            },
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
            rejected_statements: extras.rejected_statements,
            sql_schemas: extras.sql_schemas,
            declaration_only,
            submodules: submodules.iter().map(RepoPath::to_string).collect(),
            measured,
            coverage_unmatched_note: format_unmatched_coverage_note(
                options.unmatched_coverage_paths,
            ),
            languages: sections_per_language,
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

fn build_per_language_sections(
    measured: &[MeasuredLanguage],
    options: &ReportOptions<'_>,
    ignored: &BTreeSet<String>,
    section_options: SectionOptions<'_>,
) -> Result<(ReferenceRangeScope, BTreeMap<String, MeasuredSection>), MetricPluginError> {
    match measured {
        [] => Ok((
            ReferenceRangeScope::OneLanguage(ReferenceRanges::default()),
            BTreeMap::new(),
        )),
        [only] => Ok((
            ReferenceRangeScope::OneLanguage(only.reference_ranges.clone()),
            BTreeMap::new(),
        )),
        _ => {
            let mut per_language = BTreeMap::new();
            for language in measured {
                let section =
                    build_single_language_section(language, options, ignored, section_options)?;
                per_language.insert(language.plugin.clone(), section);
            }
            Ok((ReferenceRangeScope::SeveralLanguages, per_language))
        }
    }
}

fn build_single_language_section(
    language: &MeasuredLanguage,
    options: &ReportOptions<'_>,
    ignored: &BTreeSet<String>,
    section_options: SectionOptions<'_>,
) -> Result<MeasuredSection, MetricPluginError> {
    let measured_language_hygiene;
    let language_hygiene = if language.has_imports {
        match options
            .dependency_hygiene_by_language
            .and_then(|map| map.get(&language.plugin))
        {
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
        .then(|| {
            options
                .orphan_files_by_language
                .and_then(|map| map.get(&language.plugin))
        })
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
            ..section_options
        },
    )
}
