use std::collections::BTreeMap;

use codeint_config::{Boundaries as BoundariesConfig, DependenciesConfig, DuplicationConfig};
use codeint_metrics::{DependencyHygiene, OrphanFiles};
use codeint_model::{CodeModel, MetricPlugin, MetricPluginError, SourceSet};
use serde::{Deserialize, Serialize};

use crate::{
    BoundaryFindingRow, CloneFamilyFinding, CloneGroupFinding, CognitiveComplexityFinding,
    ErodedFunctionFinding, ExternalMetricFinding, FlaggedLineRow, ImportCycleFinding,
    ImportedDependencyRow, Measured, OrphanFileRow, UnusedDependencyRow,
};

#[derive(Debug, Serialize, Deserialize)]
pub struct CommitMeasurement {
    production: SourceSetMeasurement,
    test: SourceSetMeasurement,
    generated: SourceSetMeasurement,
    vendored: SourceSetMeasurement,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    dependencies: Option<DependenciesMeasurement>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub(crate) struct DependenciesMeasurement {
    pub(crate) unused_dependencies: MetricMeasurement<UnusedDependencyRow>,
    pub(crate) unlisted_dependencies: MetricMeasurement<ImportedDependencyRow>,
    pub(crate) dev_in_production_dependencies: MetricMeasurement<ImportedDependencyRow>,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct SourceSetMeasurement {
    pub(crate) files: usize,
    pub(crate) erosion: MetricMeasurement<ErodedFunctionFinding>,
    #[serde(default)]
    pub(crate) cognitive_complexity: MetricMeasurement<CognitiveComplexityFinding>,
    pub(crate) duplication: MetricMeasurement<CloneGroupFinding>,
    pub(crate) clone_families: Vec<CloneFamilyFinding>,
    pub(crate) verbosity: MetricMeasurement<FlaggedLineRow>,
    pub(crate) import_cycles: MetricMeasurement<ImportCycleFinding>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) boundaries: Option<MetricMeasurement<BoundaryFindingRow>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) orphan_files: Option<MetricMeasurement<OrphanFileRow>>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub(crate) external_metrics: BTreeMap<String, ExternalMetricCommitMeasurement>,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct ExternalMetricCommitMeasurement {
    pub(crate) name: String,
    pub(crate) description: String,
    pub(crate) measurement: MetricMeasurement<ExternalMetricFinding>,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct MetricMeasurement<T> {
    pub(crate) ratio: f64,
    pub(crate) numerator: f64,
    pub(crate) findings_worst_first: Vec<T>,
}

impl<T> Default for MetricMeasurement<T> {
    fn default() -> MetricMeasurement<T> {
        MetricMeasurement {
            ratio: 0.0,
            numerator: 0.0,
            findings_worst_first: Vec::new(),
        }
    }
}

impl CommitMeasurement {
    pub fn of(model: CodeModel) -> CommitMeasurement {
        CommitMeasurement::of_with_metrics(model, &[]).expect("built-in metrics cannot fail")
    }

    pub fn of_with_metrics(
        model: CodeModel,
        metric_plugins: &[&dyn MetricPlugin],
    ) -> Result<CommitMeasurement, MetricPluginError> {
        CommitMeasurement::of_with_metrics_and_boundaries(model, metric_plugins, None)
    }

    pub fn of_with_metrics_and_boundaries(
        model: CodeModel,
        metric_plugins: &[&dyn MetricPlugin],
        boundaries: Option<&BoundariesConfig>,
    ) -> Result<CommitMeasurement, MetricPluginError> {
        CommitMeasurement::of_with_config(
            model,
            metric_plugins,
            boundaries,
            DuplicationConfig::default(),
        )
    }

    pub fn of_with_config(
        model: CodeModel,
        metric_plugins: &[&dyn MetricPlugin],
        boundaries: Option<&BoundariesConfig>,
        duplication: DuplicationConfig,
    ) -> Result<CommitMeasurement, MetricPluginError> {
        CommitMeasurement::of_with_options(model, metric_plugins, boundaries, duplication, None)
    }

    pub fn of_with_options(
        model: CodeModel,
        metric_plugins: &[&dyn MetricPlugin],
        boundaries: Option<&BoundariesConfig>,
        duplication: DuplicationConfig,
        orphan_files: Option<&OrphanFiles>,
    ) -> Result<CommitMeasurement, MetricPluginError> {
        let dependencies = DependenciesConfig::default();
        CommitMeasurement::of_languages_with_options(
            vec![model],
            metric_plugins,
            boundaries,
            &dependencies,
            duplication,
            orphan_files,
            None,
        )
    }

    pub fn of_languages_with_options(
        languages: Vec<CodeModel>,
        metric_plugins: &[&dyn MetricPlugin],
        boundaries: Option<&BoundariesConfig>,
        dependencies_config: &DependenciesConfig,
        duplication: DuplicationConfig,
        orphan_files: Option<&OrphanFiles>,
        dependency_hygiene: Option<&DependencyHygiene>,
    ) -> Result<CommitMeasurement, MetricPluginError> {
        let ignored = dependencies_config.ignored();
        let measured_hygiene;
        let hygiene = match dependency_hygiene {
            Some(precomputed) => precomputed,
            None => {
                measured_hygiene = DependencyHygiene::merge(
                    languages
                        .iter()
                        .map(|model| DependencyHygiene::measure(model, ignored)),
                );
                &measured_hygiene
            }
        };
        let dependencies = DependenciesMeasurement::from_hygiene(hygiene);
        let model = CodeModel::merge(languages);
        let source_sets = model.into_source_sets();
        Ok(CommitMeasurement {
            production: SourceSetMeasurement::of(
                source_sets.get(SourceSet::Production),
                metric_plugins,
                boundaries.filter(|config| config.applies_to_source_set(SourceSet::Production)),
                duplication,
                orphan_files,
            )?,
            test: SourceSetMeasurement::of(
                source_sets.get(SourceSet::Test),
                metric_plugins,
                boundaries.filter(|config| config.applies_to_source_set(SourceSet::Test)),
                duplication,
                None,
            )?,
            generated: SourceSetMeasurement::of(
                source_sets.get(SourceSet::Generated),
                metric_plugins,
                boundaries.filter(|config| config.applies_to_source_set(SourceSet::Generated)),
                duplication,
                None,
            )?,
            vendored: SourceSetMeasurement::of(
                source_sets.get(SourceSet::Vendored),
                metric_plugins,
                boundaries.filter(|config| config.applies_to_source_set(SourceSet::Vendored)),
                duplication,
                None,
            )?,
            dependencies,
        })
    }

    pub(crate) fn into_source_sets(self) -> [(SourceSet, SourceSetMeasurement); 4] {
        [
            (SourceSet::Production, self.production),
            (SourceSet::Test, self.test),
            (SourceSet::Generated, self.generated),
            (SourceSet::Vendored, self.vendored),
        ]
    }

    pub(crate) fn into_parts(
        self,
    ) -> (
        [(SourceSet, SourceSetMeasurement); 4],
        Option<DependenciesMeasurement>,
    ) {
        (
            [
                (SourceSet::Production, self.production),
                (SourceSet::Test, self.test),
                (SourceSet::Generated, self.generated),
                (SourceSet::Vendored, self.vendored),
            ],
            self.dependencies,
        )
    }
}

impl DependenciesMeasurement {
    fn from_hygiene(hygiene: &DependencyHygiene) -> Option<DependenciesMeasurement> {
        if !hygiene.has_activity() {
            return None;
        }
        Some(DependenciesMeasurement {
            unused_dependencies: MetricMeasurement::from_items(
                hygiene.unused_ratio(),
                hygiene.unused().len() as f64,
                hygiene.unused(),
                UnusedDependencyRow::from,
            ),
            unlisted_dependencies: MetricMeasurement::from_items(
                hygiene.unlisted_ratio(),
                hygiene.unlisted().len() as f64,
                hygiene.unlisted(),
                ImportedDependencyRow::from,
            ),
            dev_in_production_dependencies: MetricMeasurement::from_items(
                hygiene.dev_in_production_ratio(),
                hygiene.dev_in_production().len() as f64,
                hygiene.dev_in_production(),
                ImportedDependencyRow::from,
            ),
        })
    }
}

impl SourceSetMeasurement {
    fn of(
        model: &CodeModel,
        metric_plugins: &[&dyn MetricPlugin],
        boundaries: Option<&BoundariesConfig>,
        duplication_config: DuplicationConfig,
        orphan_files: Option<&OrphanFiles>,
    ) -> Result<SourceSetMeasurement, MetricPluginError> {
        let measured = Measured::new(model, boundaries, duplication_config);
        let boundaries = measured
            .boundaries
            .as_ref()
            .map(|evaluated| finding_measurement(evaluated.ratio(), evaluated.findings()));
        let orphan_files = orphan_files
            .map(|evaluated| finding_measurement(evaluated.ratio(), evaluated.findings()));
        let mut external_metrics = BTreeMap::new();
        for plugin in metric_plugins {
            let result = plugin.measure(model)?;
            external_metrics.insert(
                plugin.key().to_owned(),
                ExternalMetricCommitMeasurement {
                    name: plugin.name().to_owned(),
                    description: plugin.description().to_owned(),
                    measurement: MetricMeasurement {
                        ratio: result.ratio,
                        numerator: result.numerator,
                        findings_worst_first: result
                            .findings
                            .iter()
                            .map(ExternalMetricFinding::from_model)
                            .collect(),
                    },
                },
            );
        }
        Ok(SourceSetMeasurement {
            files: model.files.len(),
            erosion: MetricMeasurement::from_items(
                measured.erosion.ratio(),
                measured.erosion.eroded_function_mass(),
                measured.erosion.eroded_functions(),
                ErodedFunctionFinding::new,
            ),
            cognitive_complexity: MetricMeasurement::from_items(
                measured.cognitive_complexity.ratio(),
                measured.cognitive_complexity.complex_cognitive_complexity() as f64,
                measured.cognitive_complexity.complex_functions(),
                CognitiveComplexityFinding::new,
            ),
            duplication: MetricMeasurement::from_items(
                measured.duplication.ratio(),
                measured.duplication.clone_lines.count() as f64,
                &measured.duplication.clone_groups,
                CloneGroupFinding::new,
            ),
            clone_families: measured
                .duplication
                .clone_families
                .iter()
                .map(CloneFamilyFinding::new)
                .collect(),
            verbosity: MetricMeasurement::from_items(
                measured.verbosity.ratio(),
                measured.verbosity.verbose_lines() as f64,
                measured.verbosity.flagged_lines(),
                FlaggedLineRow::from,
            ),
            import_cycles: MetricMeasurement::from_items(
                measured.import_cycles.ratio(),
                measured.import_cycles.modules_in_import_cycles() as f64,
                measured.import_cycles.cycles(),
                ImportCycleFinding::new,
            ),
            boundaries,
            orphan_files,
            external_metrics,
        })
    }
}

impl<F> MetricMeasurement<F> {
    fn from_items<'a, T>(
        ratio: f64,
        numerator: f64,
        items: &'a [T],
        convert: impl Fn(&'a T) -> F,
    ) -> MetricMeasurement<F> {
        MetricMeasurement {
            ratio,
            numerator,
            findings_worst_first: items.iter().map(convert).collect(),
        }
    }
}

fn finding_measurement<'a, T: 'a, R: From<&'a T>>(
    ratio: f64,
    findings: &'a [T],
) -> MetricMeasurement<R> {
    MetricMeasurement::from_items(ratio, findings.len() as f64, findings, R::from)
}
