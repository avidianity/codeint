use std::collections::BTreeMap;

use serde::{Deserialize, Deserializer, Serialize};

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

crate_structs! {
    #[derive(Debug, Serialize, Deserialize)]
    struct MeasuredSection {
        files: usize,
        source_lines: u64,
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
    struct UnusedDependenciesSection {
        ratio: Decimal,
        unused: usize,
        declared: usize,
        #[serde(flatten)]
        findings: Findings<UnusedDependencyRow>,
    }

    #[derive(Debug, Serialize, Deserialize)]
    struct UnlistedDependenciesSection {
        ratio: Decimal,
        unlisted: usize,
        imported: usize,
        #[serde(flatten)]
        findings: Findings<ImportedDependencyRow>,
    }

    #[derive(Debug, Serialize, Deserialize)]
    struct DevInProductionDependenciesSection {
        ratio: Decimal,
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
        ratio: Decimal,
        hazard_lines: u64,
        source_lines: u64,
        #[serde(flatten)]
        findings: Findings<HazardLineRow>,
    }

    #[derive(Debug, Serialize, Deserialize)]
    struct TestGapSection {
        ratio: Decimal,
        untested_function_mass: Decimal,
        total_function_mass: Decimal,
        measured_functions: usize,
        #[serde(flatten)]
        findings: Findings<TestGapFindingRow>,
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

    #[derive(Debug, Serialize, Deserialize)]
    struct BoundariesSection {
        ratio: Decimal,
        resolved_imports: usize,
        #[serde(flatten)]
        findings: Findings<BoundaryFindingRow>,
    }

    #[derive(Debug, Serialize, Deserialize)]
    struct MetricHeader {
        ratio: Decimal,
        reference_range: ReferenceRangeEntry,
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
        clone_lines: u64,
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

impl<'de> Deserialize<'de> for DuplicationSection {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct RawDuplicationSection {
            ratio: Decimal,
            reference_range: ReferenceRangeEntry,
            #[serde(default)]
            near_clones: Option<NearClonesSummary>,
            clone_lines: u64,
            clone_groups: Vec<CloneGroupFinding>,
            #[serde(default)]
            findings_left_out: usize,
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
                reference_range: raw.reference_range,
            },
            near_clones: raw.near_clones,
            clone_lines: raw.clone_lines,
            clone_groups: Findings {
                listed: raw.clone_groups,
                left_out: raw.findings_left_out,
            },
            clone_families: Findings {
                listed: raw.clone_families,
                left_out: raw.clone_families_left_out,
            },
            unlisted_clone_family_group_ids: raw.unlisted_clone_family_group_ids,
        })
    }
}
