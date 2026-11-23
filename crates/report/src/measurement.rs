use std::collections::BTreeMap;

use codeint_config::Boundaries as BoundariesConfig;
use codeint_metrics::OrphanFiles;
use codeint_model::{CodeModel, MetricPlugin, MetricPluginError, SourceSet};
use serde::{Deserialize, Serialize};

use crate::{
    BoundaryFindingRow, CloneGroupFinding, ErodedFunctionFinding, ExternalMetricFinding,
    FlaggedLineRow, ImportCycleFinding, Measured, OrphanFileRow, evaluate_boundaries,
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
        CommitMeasurement::of_with_options(model, metric_plugins, boundaries, None)
    }

    pub fn of_with_options(
        model: CodeModel,
        metric_plugins: &[&dyn MetricPlugin],
        boundaries: Option<&BoundariesConfig>,
        orphan_files: Option<&OrphanFiles>,
    ) -> Result<CommitMeasurement, MetricPluginError> {
        let source_sets = model.into_source_sets();
        Ok(CommitMeasurement {
            production: SourceSetMeasurement::of(
                source_sets.get(SourceSet::Production),
                metric_plugins,
                boundaries.filter(|config| config.applies_to_source_set(SourceSet::Production)),
                orphan_files,
            )?,
            test: SourceSetMeasurement::of(
                source_sets.get(SourceSet::Test),
                metric_plugins,
                boundaries.filter(|config| config.applies_to_source_set(SourceSet::Test)),
                None,
            )?,
            generated: SourceSetMeasurement::of(
                source_sets.get(SourceSet::Generated),
                metric_plugins,
                boundaries.filter(|config| config.applies_to_source_set(SourceSet::Generated)),
                None,
            )?,
            vendored: SourceSetMeasurement::of(
                source_sets.get(SourceSet::Vendored),
                metric_plugins,
                boundaries.filter(|config| config.applies_to_source_set(SourceSet::Vendored)),
                None,
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
        orphan_files: Option<&OrphanFiles>,
    ) -> Result<SourceSetMeasurement, MetricPluginError> {
        let measured = Measured::new(model);
        let boundaries = evaluate_boundaries(model, boundaries)
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
            erosion: erosion(&measured),
            duplication: duplication(&measured),
            verbosity: verbosity(&measured),
            import_cycles: import_cycles(&measured),
            boundaries,
            orphan_files,
            external_metrics,
        })
    }
}

fn finding_measurement<'a, T: 'a, R: From<&'a T>>(
    ratio: f64,
    findings: &'a [T],
) -> MetricMeasurement<R> {
    MetricMeasurement {
        ratio,
        numerator: findings.len() as f64,
        findings_worst_first: findings.iter().map(R::from).collect(),
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
