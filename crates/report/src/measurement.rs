use std::collections::BTreeMap;

use codeint_config::Boundaries as BoundariesConfig;
use codeint_metrics::Boundaries;
use codeint_model::{CodeModel, MetricPlugin, MetricPluginError, SourceSet};
use serde::{Deserialize, Serialize};

use crate::{
    BoundaryFindingRow, CloneGroupFinding, CognitiveComplexityFinding, ConfigBoundaryPolicy,
    ErodedFunctionFinding, ExternalMetricFinding, FlaggedLineRow, ImportCycleFinding, Measured,
};

#[derive(Debug, Serialize, Deserialize)]
pub struct CommitMeasurement {
    production: SourceSetMeasurement,
    test: SourceSetMeasurement,
    generated: SourceSetMeasurement,
    vendored: SourceSetMeasurement,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct SourceSetMeasurement {
    pub(crate) files: usize,
    pub(crate) erosion: MetricMeasurement<ErodedFunctionFinding>,
    #[serde(default)]
    pub(crate) cognitive_complexity: MetricMeasurement<CognitiveComplexityFinding>,
    pub(crate) duplication: MetricMeasurement<CloneGroupFinding>,
    pub(crate) verbosity: MetricMeasurement<FlaggedLineRow>,
    pub(crate) import_cycles: MetricMeasurement<ImportCycleFinding>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) boundaries: Option<MetricMeasurement<BoundaryFindingRow>>,
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
        let source_sets = model.into_source_sets();
        Ok(CommitMeasurement {
            production: SourceSetMeasurement::of(
                source_sets.get(SourceSet::Production),
                metric_plugins,
                boundaries.filter(|config| config.applies_to_source_set(SourceSet::Production)),
            )?,
            test: SourceSetMeasurement::of(
                source_sets.get(SourceSet::Test),
                metric_plugins,
                boundaries.filter(|config| config.applies_to_source_set(SourceSet::Test)),
            )?,
            generated: SourceSetMeasurement::of(
                source_sets.get(SourceSet::Generated),
                metric_plugins,
                boundaries.filter(|config| config.applies_to_source_set(SourceSet::Generated)),
            )?,
            vendored: SourceSetMeasurement::of(
                source_sets.get(SourceSet::Vendored),
                metric_plugins,
                boundaries.filter(|config| config.applies_to_source_set(SourceSet::Vendored)),
            )?,
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
}

impl SourceSetMeasurement {
    fn of(
        model: &CodeModel,
        metric_plugins: &[&dyn MetricPlugin],
        boundaries: Option<&BoundariesConfig>,
    ) -> Result<SourceSetMeasurement, MetricPluginError> {
        let measured = Measured::new(model);
        let boundaries = boundaries.map(|config| {
            let evaluated = Boundaries::measure(model, &ConfigBoundaryPolicy(config));
            MetricMeasurement {
                ratio: evaluated.ratio(),
                numerator: evaluated.findings().len() as f64,
                findings_worst_first: evaluated
                    .findings()
                    .iter()
                    .map(BoundaryFindingRow::from)
                    .collect(),
            }
        });
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
