use std::collections::BTreeMap;

use codeint_model::Metric;
use serde::{Deserialize, Deserializer, Serialize};

use crate::baseline::{Baselined, BudgetedRatio};
use crate::dependencies::{ImportedDependencyRow, UnusedDependencyRow};
use crate::findings::{
    BoundaryFindingRow, CloneFamilyFinding, CloneGroupFinding, CognitiveComplexityFinding,
    ErodedFunctionFinding, ExternalMetricFinding, FlaggedLineRow, HazardLineRow,
    ImportCycleFinding, OrphanFileRow, TestGapFindingRow, UnresolvedImportRow,
};
use crate::render::{
    Decimal, FindingKind, Findings, PerSourceSet, ReferenceRangeEntry, ReferenceRangeScope,
};

pub(crate) type SourceSetSections = PerSourceSet<SourceSetSection>;

pub(crate) fn default_metric_version() -> u32 {
    1
}

crate_structs! {
    #[derive(Debug, Serialize, Deserialize)]
    struct MeasuredSection {
        files: usize,
        source_lines: u64,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        framework_profiles: Vec<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        reference_range_note: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        import_metrics_note: Option<String>,
        source_sets: SourceSetSections,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        test_gap_note: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        dependencies: Option<DependenciesSection>,
    }
}

impl MeasuredSection {
    pub(crate) fn populate_external_metric_names(&mut self) {
        for slot in self.source_sets.iter_mut() {
            for (key, metric) in &mut slot.external_metrics {
                if metric.name.is_empty() {
                    metric.name = key.clone();
                }
            }
        }
    }
}

crate_structs! {
    #[derive(Debug, Serialize, Deserialize)]
    struct DependenciesSection {
        unused_dependencies: UnusedDependenciesSection,
        unlisted_dependencies: UnlistedDependenciesSection,
        dev_in_production_dependencies: DevInProductionDependenciesSection,
    }

    #[derive(Debug, Serialize, Deserialize)]
    struct MetricRatioHeader {
        ratio: Decimal,
        #[serde(default = "default_metric_version")]
        metric_version: u32,
    }

    #[derive(Debug, Serialize, Deserialize)]
    struct UnusedDependenciesSection {
        #[serde(flatten)]
        header: MetricRatioHeader,
        unused: usize,
        declared: usize,
        #[serde(flatten)]
        findings: Findings<UnusedDependencyRow>,
    }

    #[derive(Debug, Serialize, Deserialize)]
    struct UnlistedDependenciesSection {
        #[serde(flatten)]
        header: MetricRatioHeader,
        unlisted: usize,
        imported: usize,
        #[serde(flatten)]
        findings: Findings<ImportedDependencyRow>,
    }

    #[derive(Debug, Serialize, Deserialize)]
    struct DevInProductionDependenciesSection {
        #[serde(flatten)]
        header: MetricRatioHeader,
        dev_in_production: usize,
        production_imported: usize,
        #[serde(flatten)]
        findings: Findings<ImportedDependencyRow>,
    }

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
        #[serde(default, skip_serializing_if = "Option::is_none")]
        test_gap: Option<TestGapSection>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        hazards: Option<HazardsSection>,
        #[serde(default, flatten)]
        external_metrics: BTreeMap<String, ExternalMetricSection>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        packages: Vec<PackageSection>,
    }

    #[derive(Debug, Serialize, Deserialize)]
    struct HazardsSection {
        #[serde(flatten)]
        header: MetricRatioHeader,
        hazard_lines: u64,
        source_lines: u64,
        #[serde(flatten)]
        findings: Findings<HazardLineRow>,
    }

    #[derive(Debug, Serialize, Deserialize)]
    struct TestGapSection {
        #[serde(flatten)]
        header: MetricRatioHeader,
        untested_function_mass: Decimal,
        total_function_mass: Decimal,
        measured_functions: usize,
        #[serde(flatten)]
        findings: Findings<TestGapFindingRow>,
    }

    #[derive(Debug, Serialize, Deserialize)]
    struct OrphanFilesSection {
        #[serde(flatten)]
        header: MetricRatioHeader,
        orphan_files: usize,
        evaluated_files: usize,
        entry_points: usize,
        unreferenced: usize,
        test_only: usize,
        #[serde(flatten)]
        findings: Findings<OrphanFileRow>,
    }

    #[derive(Debug, Serialize, Deserialize)]
    struct BoundariesSection {
        #[serde(flatten)]
        header: MetricRatioHeader,
        resolved_imports: usize,
        #[serde(flatten)]
        findings: Findings<BoundaryFindingRow>,
    }

    #[derive(Debug, Serialize, Deserialize)]
    struct MetricHeader {
        ratio: Decimal,
        #[serde(default = "default_metric_version")]
        metric_version: u32,
        reference_range: ReferenceRangeEntry,
    }
}

impl MetricRatioHeader {
    pub(crate) fn new(ratio: f64, metric: Metric) -> MetricRatioHeader {
        MetricRatioHeader {
            ratio: Decimal::rounded(ratio),
            metric_version: metric.version(),
        }
    }
}

impl MetricHeader {
    pub(crate) fn new(
        ratio: f64,
        ranges: &ReferenceRangeScope,
        key: &str,
        metric_version: u32,
    ) -> MetricHeader {
        MetricHeader {
            ratio: Decimal::rounded(ratio),
            metric_version,
            reference_range: ranges.entry(key, metric_version),
        }
    }
}

crate_structs! {
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

    #[derive(Debug, Serialize, Deserialize)]
    struct PackageSection {
        path: String,
        files: usize,
        source_lines: u64,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        framework_profiles: Vec<String>,
        erosion: PackageErosion,
        cognitive_complexity: PackageCognitiveComplexity,
        duplication: PackageDuplication,
        verbosity: PackageVerbosity,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        import_cycles: Option<PackageImportCycles>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        test_gap: Option<PackageTestGap>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        hazards: Option<PackageHazards>,
    }

    #[derive(Debug, Serialize, Deserialize)]
    struct PackageHazards {
        ratio: Decimal,
        hazard_lines: u64,
        source_lines: u64,
    }

    #[derive(Debug, Serialize, Deserialize)]
    struct PackageTestGap {
        ratio: Decimal,
        untested_function_mass: Decimal,
        total_function_mass: Decimal,
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
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(bound(deserialize = "Totals: Deserialize<'de>, Finding: FindingKind + Deserialize<'de>"))]
pub(crate) struct MetricSection<Totals, Finding: FindingKind> {
    #[serde(flatten)]
    pub(crate) header: MetricHeader,
    #[serde(flatten)]
    pub(crate) totals: Totals,
    #[serde(flatten)]
    pub(crate) findings: Findings<Finding>,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct NearClonesSummary {
    pub(crate) min_similarity: f64,
}

pub(crate) struct SourceSetMetricSummary<'a> {
    pub(crate) key: &'a str,
    pub(crate) label: &'a str,
    pub(crate) ratio: Decimal,
    pub(crate) metric_version: u32,
    pub(crate) reference_range: Option<&'a ReferenceRangeEntry>,
    pub(crate) budgeted_ratio: BudgetedRatio,
    pub(crate) baselined_hidden: usize,
}

pub(crate) struct DependencyMetricSummary {
    pub(crate) key: &'static str,
    pub(crate) label: &'static str,
    pub(crate) ratio: Decimal,
    pub(crate) metric_version: u32,
    pub(crate) numerator: usize,
    pub(crate) denominator: usize,
    pub(crate) budgeted_ratio: BudgetedRatio,
    pub(crate) baselined_hidden: usize,
}

impl SourceSetSection {
    pub(crate) fn metric_summaries(&self) -> Vec<SourceSetMetricSummary<'_>> {
        let mut summaries = vec![
            summary_with_reference(
                Metric::Erosion.key(),
                "Erosion",
                &self.erosion.header,
                &self.erosion.findings,
            ),
            summary_with_reference(
                Metric::CognitiveComplexity.key(),
                "Cognitive complexity",
                &self.cognitive_complexity.header,
                &self.cognitive_complexity.findings,
            ),
            summary_with_reference(
                Metric::Duplication.key(),
                "Duplication",
                &self.duplication.header,
                &self.duplication.clone_groups,
            ),
            summary_with_reference(
                Metric::Verbosity.key(),
                "Verbosity",
                &self.verbosity.header,
                &self.verbosity.findings,
            ),
        ];
        macro_rules! push_opt {
            ($field:expr, $metric:expr, $label:expr, ref) => {
                if let Some(item) = &$field {
                    summaries.push(summary_with_reference(
                        $metric.key(),
                        $label,
                        &item.header,
                        &item.findings,
                    ));
                }
            };
            ($field:expr, $metric:expr, $label:expr) => {
                if let Some(item) = &$field {
                    summaries.push(summary_without_reference(
                        $metric.key(),
                        $label,
                        &item.header,
                        &item.findings,
                    ));
                }
            };
        }
        push_opt!(
            self.import_cycles,
            Metric::ImportCycles,
            "Import cycles",
            ref
        );
        push_opt!(self.boundaries, Metric::Boundaries, "Boundaries");
        push_opt!(self.orphan_files, Metric::OrphanFiles, "Orphan files");
        push_opt!(self.test_gap, Metric::TestGap, "Test gap");
        push_opt!(self.hazards, Metric::Hazards, "Hazards");
        for (key, item) in &self.external_metrics {
            summaries.push(summary_with_reference(
                key.as_str(),
                item.name.as_str(),
                &item.header,
                &item.findings,
            ));
        }
        summaries
    }
}

fn metric_summary<'a, T>(
    key: &'a str,
    label: &'a str,
    ratio: Decimal,
    metric_version: u32,
    reference_range: Option<&'a ReferenceRangeEntry>,
    findings: &Findings<T>,
) -> SourceSetMetricSummary<'a> {
    SourceSetMetricSummary {
        key,
        label,
        ratio,
        metric_version,
        reference_range,
        budgeted_ratio: findings.budgeted(ratio),
        baselined_hidden: findings.baselined.hidden,
    }
}

fn summary_with_reference<'a, T>(
    key: &'a str,
    label: &'a str,
    header: &'a MetricHeader,
    findings: &Findings<T>,
) -> SourceSetMetricSummary<'a> {
    metric_summary(
        key,
        label,
        header.ratio,
        header.metric_version,
        Some(&header.reference_range),
        findings,
    )
}

fn summary_without_reference<'a, T>(
    key: &'a str,
    label: &'a str,
    header: &MetricRatioHeader,
    findings: &Findings<T>,
) -> SourceSetMetricSummary<'a> {
    metric_summary(
        key,
        label,
        header.ratio,
        header.metric_version,
        None,
        findings,
    )
}

impl DependenciesSection {
    pub(crate) fn metric_summaries(&self) -> [DependencyMetricSummary; 3] {
        [
            DependencyMetricSummary {
                key: Metric::UnusedDependencies.key(),
                label: "Unused dependencies",
                ratio: self.unused_dependencies.header.ratio,
                metric_version: self.unused_dependencies.header.metric_version,
                numerator: self.unused_dependencies.unused,
                denominator: self.unused_dependencies.declared,
                budgeted_ratio: self
                    .unused_dependencies
                    .findings
                    .budgeted(self.unused_dependencies.header.ratio),
                baselined_hidden: self.unused_dependencies.findings.baselined.hidden,
            },
            DependencyMetricSummary {
                key: Metric::UnlistedDependencies.key(),
                label: "Unlisted dependencies",
                ratio: self.unlisted_dependencies.header.ratio,
                metric_version: self.unlisted_dependencies.header.metric_version,
                numerator: self.unlisted_dependencies.unlisted,
                denominator: self.unlisted_dependencies.imported,
                budgeted_ratio: self
                    .unlisted_dependencies
                    .findings
                    .budgeted(self.unlisted_dependencies.header.ratio),
                baselined_hidden: self.unlisted_dependencies.findings.baselined.hidden,
            },
            DependencyMetricSummary {
                key: Metric::DevInProductionDependencies.key(),
                label: "Dev dependencies in production",
                ratio: self.dev_in_production_dependencies.header.ratio,
                metric_version: self.dev_in_production_dependencies.header.metric_version,
                numerator: self.dev_in_production_dependencies.dev_in_production,
                denominator: self.dev_in_production_dependencies.production_imported,
                budgeted_ratio: self
                    .dev_in_production_dependencies
                    .findings
                    .budgeted(self.dev_in_production_dependencies.header.ratio),
                baselined_hidden: self
                    .dev_in_production_dependencies
                    .findings
                    .baselined
                    .hidden,
            },
        ]
    }
}

impl_ratchet_metrics_source!(SourceSetSection => ratio, DependenciesSection => ratio);

impl<'de> Deserialize<'de> for DuplicationSection {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct RawDuplicationSection {
            ratio: Decimal,
            #[serde(default = "default_metric_version")]
            metric_version: u32,
            reference_range: ReferenceRangeEntry,
            #[serde(default)]
            near_clones: Option<NearClonesSummary>,
            clone_lines: u64,
            clone_groups: Vec<CloneGroupFinding>,
            #[serde(default)]
            findings_left_out: usize,
            #[serde(default)]
            baselined_findings: usize,
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
                metric_version: raw.metric_version,
                reference_range: raw.reference_range,
            },
            near_clones: raw.near_clones,
            clone_lines: raw.clone_lines,
            clone_groups: Findings {
                baselined: Baselined {
                    hidden: raw.baselined_findings,
                    retained_share: None,
                },
                ..Findings::listing(raw.clone_groups, raw.findings_left_out)
            },
            clone_families: Findings::listing(raw.clone_families, raw.clone_families_left_out),
            unlisted_clone_family_group_ids: raw.unlisted_clone_family_group_ids,
        })
    }
}
