use std::collections::{BTreeMap, BTreeSet};

use codeint_config::{
    Boundaries as BoundariesConfig, DependenciesConfig, DuplicationConfig, FindingAnchor,
};
use codeint_metrics::{DependencyHygiene, OrphanFiles};
use codeint_model::{CodeModel, MetricPlugin, MetricPluginError, RepoPath, SourceSet, SourceSets};
use serde::{Deserialize, Serialize};

use crate::baseline::Identified;
use crate::{
    BoundaryFindingRow, CloneFamilyFinding, CloneGroupFinding, CognitiveComplexityFinding,
    ErodedFunctionFinding, ExternalMetricFinding, FlaggedLineRow, HazardLineRow,
    ImportCycleFinding, ImportedDependencyRow, Measured, OrphanFileRow, SectionModels,
    UnusedDependencyRow, model_has_hazard_rules,
};

#[derive(Debug, Serialize, Deserialize)]
pub struct CommitMeasurement {
    production: SourceSetMeasurement,
    test: SourceSetMeasurement,
    generated: SourceSetMeasurement,
    vendored: SourceSetMeasurement,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    dependencies: Option<DependenciesMeasurement>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    manifest_warnings: Vec<String>,
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
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) clone_family_anchors: Vec<FindingAnchor>,
    pub(crate) verbosity: MetricMeasurement<FlaggedLineRow>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) import_cycles: Option<MetricMeasurement<ImportCycleFinding>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) boundaries: Option<MetricMeasurement<BoundaryFindingRow>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) orphan_files: Option<MetricMeasurement<OrphanFileRow>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) hazards: Option<MetricMeasurement<HazardLineRow>>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub(crate) external_metrics: BTreeMap<String, ExternalMetricCommitMeasurement>,
}

#[derive(Clone, Copy)]
pub struct CommitMeasurementOptions<'a> {
    pub metric_plugins: &'a [&'a dyn MetricPlugin],
    pub boundaries: Option<&'a BoundariesConfig>,
    pub dependencies: &'a DependenciesConfig,
    pub duplication: DuplicationConfig,
    pub orphan_files: Option<&'a OrphanFiles>,
    pub dependency_hygiene: Option<&'a DependencyHygiene>,
    pub disabled_hazard_rules: &'a BTreeSet<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct ExternalMetricCommitMeasurement {
    pub(crate) name: String,
    pub(crate) description: String,
    #[serde(default = "crate::sections::default_metric_version")]
    pub(crate) metric_version: u32,
    pub(crate) measurement: MetricMeasurement<ExternalMetricFinding>,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct MetricMeasurement<T> {
    pub(crate) ratio: f64,
    pub(crate) numerator: f64,
    pub(crate) findings_worst_first: Vec<T>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) finding_anchors: Vec<FindingAnchor>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub(crate) file_numerators: BTreeMap<String, f64>,
}

impl<T> Default for MetricMeasurement<T> {
    fn default() -> MetricMeasurement<T> {
        MetricMeasurement {
            ratio: 0.0,
            numerator: 0.0,
            findings_worst_first: Vec::new(),
            finding_anchors: Vec::new(),
            file_numerators: BTreeMap::new(),
        }
    }
}

pub(crate) fn rehydrate_anchors<T: Identified>(
    mut findings: Vec<T>,
    anchors: Vec<FindingAnchor>,
) -> Vec<T> {
    if findings.len() == anchors.len() {
        for (finding, anchor) in findings.iter_mut().zip(anchors) {
            finding.set_anchor(anchor);
        }
    }
    findings
}

impl CommitMeasurement {
    pub fn with_manifest_warnings(mut self, warnings: Vec<String>) -> CommitMeasurement {
        self.manifest_warnings = warnings;
        self
    }

    pub(crate) fn manifest_warnings(&self) -> &[String] {
        &self.manifest_warnings
    }

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
        let disabled_hazard_rules = BTreeSet::new();
        CommitMeasurement::of_languages_with_full_options(
            languages,
            CommitMeasurementOptions {
                metric_plugins,
                boundaries,
                dependencies: dependencies_config,
                duplication,
                orphan_files,
                dependency_hygiene,
                disabled_hazard_rules: &disabled_hazard_rules,
            },
        )
    }

    pub fn of_languages_with_full_options(
        languages: Vec<CodeModel>,
        options: CommitMeasurementOptions<'_>,
    ) -> Result<CommitMeasurement, MetricPluginError> {
        let ignored = options.dependencies.ignored();
        let measured_hygiene;
        let hygiene = match options.dependency_hygiene {
            Some(precomputed) => precomputed,
            None => {
                measured_hygiene = measure_commit_hygiene(&languages, ignored);
                &measured_hygiene
            }
        };
        let measured: Vec<&CodeModel> = languages
            .iter()
            .filter(|model| !model.files.is_empty())
            .collect();
        let any_has_imports =
            measured.is_empty() || measured.iter().copied().any(model_has_imports);
        let all_have_imports = measured.iter().copied().all(model_has_imports);
        let separate_import_source_sets = (any_has_imports && !all_have_imports)
            .then(|| build_filtered_source_sets(&languages, model_has_imports));
        let any_has_hazard_rules = measured.iter().copied().any(model_has_hazard_rules);
        let all_have_hazard_rules =
            !measured.is_empty() && measured.iter().copied().all(model_has_hazard_rules);
        let separate_hazards_source_sets = (any_has_hazard_rules && !all_have_hazard_rules)
            .then(|| build_filtered_source_sets(&languages, model_has_hazard_rules));
        let dependencies = any_has_imports
            .then(|| DependenciesMeasurement::from_hygiene(hygiene))
            .flatten();
        let source_sets = CodeModel::merge(languages).into_source_sets();
        let measure = |source_set: SourceSet, set_orphans: Option<&OrphanFiles>| {
            let import_model = select_subset_model(
                source_set,
                any_has_imports,
                &source_sets,
                separate_import_source_sets.as_ref(),
                false,
            );
            let hazards_model = select_subset_model(
                source_set,
                any_has_hazard_rules,
                &source_sets,
                separate_hazards_source_sets.as_ref(),
                true,
            );
            measure_source_set(
                source_set,
                SectionModels {
                    model: source_sets.get(source_set),
                    import_model,
                    coverage_model: None,
                    hazards_model,
                },
                options,
                set_orphans,
            )
        };
        Ok(CommitMeasurement {
            production: measure(SourceSet::Production, options.orphan_files)?,
            test: measure(SourceSet::Test, None)?,
            generated: measure(SourceSet::Generated, None)?,
            vendored: measure(SourceSet::Vendored, None)?,
            dependencies,
            manifest_warnings: Vec::new(),
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
            )
            .with_file_numerators(count_file_numerators(
                hygiene.unused().iter().map(|finding| &finding.manifest),
            )),
            unlisted_dependencies: MetricMeasurement::from_items(
                hygiene.unlisted_ratio(),
                hygiene.unlisted().len() as f64,
                hygiene.unlisted(),
                ImportedDependencyRow::from,
            )
            .with_file_numerators(count_file_numerators(
                hygiene
                    .unlisted()
                    .iter()
                    .flat_map(|finding| finding.importers.iter()),
            )),
            dev_in_production_dependencies: MetricMeasurement::from_items(
                hygiene.dev_in_production_ratio(),
                hygiene.dev_in_production().len() as f64,
                hygiene.dev_in_production(),
                ImportedDependencyRow::from,
            )
            .with_file_numerators(count_file_numerators(
                hygiene
                    .dev_in_production()
                    .iter()
                    .flat_map(|finding| finding.importers.iter()),
            )),
        })
    }
}

fn model_has_imports(model: &CodeModel) -> bool {
    model.languages.is_empty() || model.languages.iter().any(|facts| facts.has_imports)
}

impl SourceSetMeasurement {
    fn of(
        models: SectionModels<'_>,
        boundaries: Option<&BoundariesConfig>,
        options: CommitMeasurementOptions<'_>,
        orphan_files: Option<&OrphanFiles>,
    ) -> Result<SourceSetMeasurement, MetricPluginError> {
        let model = models.model;
        let measured = Measured::new(
            models,
            boundaries,
            options.duplication,
            options.disabled_hazard_rules,
        );
        let boundaries = measured.boundaries.as_ref().map(|evaluated| {
            finding_measurement(evaluated.ratio(), evaluated.findings()).with_file_numerators(
                count_file_numerators(evaluated.findings().iter().map(|finding| &finding.importer)),
            )
        });
        let orphan_files = orphan_files.map(|evaluated| {
            finding_measurement(evaluated.ratio(), evaluated.findings()).with_file_numerators(
                presence_file_numerators(evaluated.findings().iter().map(|finding| &finding.path)),
            )
        });
        let hazards = measured.hazards.as_ref().map(|evaluated| {
            MetricMeasurement::from_items(
                evaluated.ratio(),
                evaluated.hazard_lines() as f64,
                evaluated.findings(),
                HazardLineRow::from,
            )
            .with_file_numerators(map_file_numerators(
                evaluated
                    .lines_by_file()
                    .iter()
                    .map(|(path, lines)| (path, lines.hazard_lines as f64)),
            ))
        });
        let mut external_metrics = BTreeMap::new();
        for plugin in options.metric_plugins {
            let result = plugin.measure(model)?;
            let file_numerators =
                count_file_numerators(result.findings.iter().map(|finding| &finding.path));
            let findings_worst_first: Vec<ExternalMetricFinding> = result
                .findings
                .iter()
                .map(ExternalMetricFinding::from)
                .collect();
            let finding_anchors = findings_worst_first
                .iter()
                .map(Identified::anchor)
                .collect();
            external_metrics.insert(
                plugin.key().to_owned(),
                ExternalMetricCommitMeasurement {
                    name: plugin.name().to_owned(),
                    description: plugin.description().to_owned(),
                    metric_version: plugin.version(),
                    measurement: MetricMeasurement {
                        ratio: result.ratio,
                        numerator: result.numerator,
                        findings_worst_first,
                        finding_anchors,
                        file_numerators,
                    },
                },
            );
        }
        let clone_families: Vec<CloneFamilyFinding> = measured
            .duplication
            .clone_families
            .iter()
            .map(CloneFamilyFinding::from)
            .collect();
        let clone_family_anchors = clone_families.iter().map(Identified::anchor).collect();
        Ok(SourceSetMeasurement {
            files: model.files.len(),
            erosion: MetricMeasurement::from_items(
                measured.erosion.ratio(),
                measured.erosion.eroded_function_mass(),
                measured.erosion.eroded_functions(),
                ErodedFunctionFinding::from,
            )
            .with_file_numerators(map_file_numerators(
                measured
                    .erosion
                    .masses_by_file()
                    .iter()
                    .map(|(path, masses)| (path, masses.eroded)),
            )),
            cognitive_complexity: MetricMeasurement::from_items(
                measured.cognitive_complexity.ratio(),
                measured.cognitive_complexity.complex_cognitive_complexity() as f64,
                measured.cognitive_complexity.complex_functions(),
                CognitiveComplexityFinding::from,
            )
            .with_file_numerators(map_file_numerators(
                measured
                    .cognitive_complexity
                    .totals_by_file()
                    .iter()
                    .map(|(path, totals)| (path, totals.complex as f64)),
            )),
            duplication: MetricMeasurement::from_items(
                measured.duplication.ratio(),
                measured.duplication.clone_lines.count() as f64,
                &measured.duplication.clone_groups,
                CloneGroupFinding::from,
            )
            .with_file_numerators(map_file_numerators(
                measured
                    .duplication
                    .clone_lines
                    .count_by_file()
                    .map(|(path, lines)| (path, lines as f64)),
            )),
            clone_families,
            clone_family_anchors,
            verbosity: MetricMeasurement::from_items(
                measured.verbosity.ratio(),
                measured.verbosity.verbose_lines() as f64,
                measured.verbosity.flagged_lines(),
                FlaggedLineRow::from,
            )
            .with_file_numerators(map_file_numerators(
                measured
                    .verbosity
                    .verbose_lines_by_file()
                    .iter()
                    .map(|(path, &lines)| (path, lines as f64)),
            )),
            import_cycles: measured.import_cycles.as_ref().map(|cycles| {
                MetricMeasurement::from_items(
                    cycles.ratio(),
                    cycles.modules_in_import_cycles() as f64,
                    cycles.cycles(),
                    ImportCycleFinding::from,
                )
                .with_file_numerators(presence_file_numerators(
                    cycles
                        .cycles()
                        .iter()
                        .flat_map(|cycle| cycle.modules().iter()),
                ))
            }),
            boundaries,
            orphan_files,
            hazards,
            external_metrics,
        })
    }
}

impl<F: Identified> MetricMeasurement<F> {
    fn from_items<'a, T>(
        ratio: f64,
        numerator: f64,
        items: &'a [T],
        convert: impl Fn(&'a T) -> F,
    ) -> MetricMeasurement<F> {
        let findings_worst_first: Vec<F> = items.iter().map(convert).collect();
        let finding_anchors = findings_worst_first
            .iter()
            .map(Identified::anchor)
            .collect();
        MetricMeasurement {
            ratio,
            numerator,
            findings_worst_first,
            finding_anchors,
            file_numerators: BTreeMap::new(),
        }
    }

    fn with_file_numerators(
        mut self,
        file_numerators: BTreeMap<String, f64>,
    ) -> MetricMeasurement<F> {
        self.file_numerators = file_numerators;
        self
    }
}

fn map_file_numerators<'a>(
    entries: impl Iterator<Item = (&'a RepoPath, f64)>,
) -> BTreeMap<String, f64> {
    entries
        .filter(|(_, value)| *value > 0.0)
        .map(|(path, value)| (path.to_string(), value))
        .collect()
}

fn count_file_numerators<'a>(paths: impl Iterator<Item = &'a RepoPath>) -> BTreeMap<String, f64> {
    let mut counts: BTreeMap<String, f64> = BTreeMap::new();
    for path in paths {
        *counts.entry(path.to_string()).or_insert(0.0) += 1.0;
    }
    counts
}

fn presence_file_numerators<'a>(
    paths: impl Iterator<Item = &'a RepoPath>,
) -> BTreeMap<String, f64> {
    paths.map(|path| (path.to_string(), 1.0)).collect()
}

fn finding_measurement<'a, T: 'a, R: From<&'a T> + Identified>(
    ratio: f64,
    findings: &'a [T],
) -> MetricMeasurement<R> {
    MetricMeasurement::from_items(ratio, findings.len() as f64, findings, R::from)
}

fn measure_commit_hygiene(
    languages: &[CodeModel],
    ignored: &BTreeSet<String>,
) -> DependencyHygiene {
    let mut measured = Vec::new();
    for model in languages {
        if model_has_imports(model) {
            measured.push(DependencyHygiene::measure(model, ignored));
        }
    }
    DependencyHygiene::merge(measured)
}

fn build_filtered_source_sets(
    languages: &[CodeModel],
    predicate: fn(&CodeModel) -> bool,
) -> SourceSets {
    let mut models = Vec::new();
    for model in languages {
        if !model.files.is_empty() && predicate(model) {
            models.push(model.clone());
        }
    }
    CodeModel::merge(models).into_source_sets()
}

fn select_subset_model<'a>(
    source_set: SourceSet,
    enabled: bool,
    source_sets: &'a SourceSets,
    separate_source_sets: Option<&'a SourceSets>,
    require_non_empty_shared: bool,
) -> Option<&'a CodeModel> {
    if !enabled {
        return None;
    }
    match separate_source_sets {
        None => {
            let candidate = source_sets.get(source_set);
            (!require_non_empty_shared || !candidate.files.is_empty()).then_some(candidate)
        }
        Some(sets) => {
            let candidate = sets.get(source_set);
            (!candidate.files.is_empty()).then_some(candidate)
        }
    }
}

fn measure_source_set(
    source_set: SourceSet,
    models: SectionModels<'_>,
    options: CommitMeasurementOptions<'_>,
    orphan_files: Option<&OrphanFiles>,
) -> Result<SourceSetMeasurement, MetricPluginError> {
    let applicable_boundaries = options
        .boundaries
        .filter(|config| models.import_model.is_some() && config.applies_to_source_set(source_set));
    let applicable_orphans = orphan_files.filter(|_| models.import_model.is_some());
    SourceSetMeasurement::of(models, applicable_boundaries, options, applicable_orphans)
}
