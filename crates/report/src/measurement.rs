use std::collections::BTreeMap;

use codeint_config::{Boundaries as BoundariesConfig, DuplicationConfig};
use codeint_model::{CodeModel, MetricPlugin, MetricPluginError, SourceSet};
use serde::{Deserialize, Serialize};

use crate::{
    BoundaryFindingRow, CloneFamilyFinding, CloneGroupFinding, ErodedFunctionFinding,
    ExternalMetricFinding, FlaggedLineRow, ImportCycleFinding, Measured,
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
    pub(crate) duplication: MetricMeasurement<CloneGroupFinding>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) clone_families: Vec<CloneFamilyFinding>,
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
        let source_sets = model.into_source_sets();
        Ok(CommitMeasurement {
            production: SourceSetMeasurement::of(
                source_sets.get(SourceSet::Production),
                metric_plugins,
                boundaries.filter(|config| config.applies_to_source_set(SourceSet::Production)),
                duplication,
            )?,
            test: SourceSetMeasurement::of(
                source_sets.get(SourceSet::Test),
                metric_plugins,
                boundaries.filter(|config| config.applies_to_source_set(SourceSet::Test)),
                duplication,
            )?,
            generated: SourceSetMeasurement::of(
                source_sets.get(SourceSet::Generated),
                metric_plugins,
                boundaries.filter(|config| config.applies_to_source_set(SourceSet::Generated)),
                duplication,
            )?,
            vendored: SourceSetMeasurement::of(
                source_sets.get(SourceSet::Vendored),
                metric_plugins,
                boundaries.filter(|config| config.applies_to_source_set(SourceSet::Vendored)),
                duplication,
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
        duplication_config: DuplicationConfig,
    ) -> Result<SourceSetMeasurement, MetricPluginError> {
        let measured = Measured::new(model, boundaries, duplication_config);
        let boundaries = measured
            .boundaries
            .as_ref()
            .map(|evaluated| MetricMeasurement {
                ratio: evaluated.ratio(),
                numerator: evaluated.findings().len() as f64,
                findings_worst_first: evaluated
                    .findings()
                    .iter()
                    .map(BoundaryFindingRow::from)
                    .collect(),
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
            erosion: erosion(&measured),
            duplication: duplication(&measured),
            clone_families: clone_families(&measured),
            verbosity: verbosity(&measured),
            import_cycles: import_cycles(&measured),
            boundaries,
            external_metrics,
        })
    }
}

fn erosion(measured: &Measured) -> MetricMeasurement<ErodedFunctionFinding> {
    MetricMeasurement {
        ratio: measured.erosion.ratio(),
        numerator: measured.erosion.eroded_function_mass(),
        findings_worst_first: measured
            .erosion
            .eroded_functions()
            .iter()
            .map(ErodedFunctionFinding::new)
            .collect(),
    }
}

fn duplication(measured: &Measured) -> MetricMeasurement<CloneGroupFinding> {
    MetricMeasurement {
        ratio: measured.duplication.ratio(),
        numerator: measured.duplication.clone_lines.count() as f64,
        findings_worst_first: measured
            .duplication
            .clone_groups
            .iter()
            .map(CloneGroupFinding::new)
            .collect(),
    }
}

fn clone_families(measured: &Measured) -> Vec<CloneFamilyFinding> {
    measured
        .duplication
        .clone_families
        .iter()
        .map(CloneFamilyFinding::new)
        .collect()
}

fn verbosity(measured: &Measured) -> MetricMeasurement<FlaggedLineRow> {
    MetricMeasurement {
        ratio: measured.verbosity.ratio(),
        numerator: measured.verbosity.verbose_lines() as f64,
        findings_worst_first: measured
            .verbosity
            .flagged_lines()
            .iter()
            .map(FlaggedLineRow::from)
            .collect(),
    }
}

fn import_cycles(measured: &Measured) -> MetricMeasurement<ImportCycleFinding> {
    MetricMeasurement {
        ratio: measured.import_cycles.ratio(),
        numerator: measured.import_cycles.modules_in_import_cycles() as f64,
        findings_worst_first: measured
            .import_cycles
            .cycles()
            .iter()
            .map(ImportCycleFinding::new)
            .collect(),
    }
}
