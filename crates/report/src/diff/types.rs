use std::collections::{BTreeMap, BTreeSet};

use codeint_config::{Baseline, Budgets, FindingsShown, RegressionLimits};
use codeint_model::Metric;
use serde::ser::SerializeMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::baseline::BudgetedRatio;
use crate::measurement::CommitMeasurement;
use crate::ownership::OwnershipOptions;
use crate::{
    BoundaryFindingRow, BudgetFailure, CloneFamilyFinding, CloneGroupFinding,
    CognitiveComplexityFinding, Decimal, ErodedFunctionFinding, ExternalMetricFinding, Findings,
    FlaggedLineRow, HazardLineRow, ImportCycleFinding, ImportedDependencyRow, OrphanFileRow,
    PerSourceSet, RatchetFailure, UnusedDependencyRow,
};

pub struct BaseCommit {
    pub git_ref: String,
    pub merge_base: String,
}

pub enum Head {
    WorkingTree,
    Commit(HeadCommit),
}

pub struct HeadCommit {
    pub git_ref: String,
    pub commit: String,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub(super) struct ManifestWarnings {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(super) base: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(super) head: Vec<String>,
}

impl ManifestWarnings {
    fn is_empty(&self) -> bool {
        self.base.is_empty() && self.head.is_empty()
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DiffReport {
    pub(super) base: BaseEntry,
    pub(super) head: HeadEntry,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) test_gap_note: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) ratchet_note: Option<String>,
    #[serde(default, skip_serializing_if = "ManifestWarnings::is_empty")]
    pub(super) manifest_warnings: ManifestWarnings,
    pub(super) source_sets: SourceSetChangesSections,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) dependencies: Option<DependenciesChanges>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(super) over_budget: Vec<BudgetFailure>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(super) over_new_file_budget: Vec<NewFileBudgetFailure>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(super) over_ratchet: Vec<RatchetFailure>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(super) regressions: Vec<Regression>,
    #[serde(skip)]
    pub(super) show_all: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct NewFileBudgetFailure {
    pub(crate) source_set: String,
    pub(crate) metric: String,
    pub(crate) ratio: Decimal,
    pub(crate) budget: f64,
    pub(crate) files: usize,
}

impl std::fmt::Display for NewFileBudgetFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "the new-file {} {} ratio {} is over its new-file Budget {}",
            self.source_set, self.metric, self.ratio.0, self.budget
        )
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct Regression {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) source_set: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) file: Option<String>,
    pub(crate) metric: String,
    pub(crate) rule: RegressionRule,
    pub(crate) base: Decimal,
    pub(crate) head: Decimal,
    pub(crate) new_findings: usize,
    pub(crate) ratio_rise_limit: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum RegressionRule {
    RatioRise,
    NewFindings,
    #[serde(alias = "numerator_rise_with_new_findings")]
    NumeratorRise,
    PerFileNumeratorRise,
    PerFileNewFindings,
}

impl RegressionRule {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            RegressionRule::RatioRise => "ratio_rise",
            RegressionRule::NewFindings => "new_findings",
            RegressionRule::NumeratorRise => "numerator_rise",
            RegressionRule::PerFileNumeratorRise => "per_file_numerator_rise",
            RegressionRule::PerFileNewFindings => "per_file_new_findings",
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub(super) struct BaseEntry {
    #[serde(rename = "ref")]
    pub(super) git_ref: String,
    pub(super) merge_base: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub(super) enum HeadEntry {
    WorkingTree(String),
    Commit {
        #[serde(rename = "ref")]
        git_ref: String,
        commit: String,
    },
}

pub(super) type SourceSetChangesSections = PerSourceSet<SourceSetChanges>;

#[derive(Debug, Serialize, Deserialize)]
pub(super) struct SourceSetChanges {
    pub(super) erosion: MetricChanges<ErodedFunctionFinding>,
    pub(super) cognitive_complexity: MetricChanges<CognitiveComplexityFinding>,
    pub(super) duplication: DuplicationChanges,
    pub(super) verbosity: MetricChanges<FlaggedLineRow>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) import_cycles: Option<MetricChanges<ImportCycleFinding>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) boundaries: Option<MetricChanges<BoundaryFindingRow>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) orphan_files: Option<MetricChanges<OrphanFileRow>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) hazards: Option<MetricChanges<HazardLineRow>>,
    #[serde(default, flatten)]
    pub(super) external_metrics: BTreeMap<String, ExternalMetricChanges>,
}

#[derive(Debug)]
pub(super) struct DuplicationChanges {
    pub(super) groups: MetricChanges<CloneGroupFinding>,
    pub(super) clone_families: Option<CloneFamilyChanges>,
}

#[derive(Debug)]
pub(super) struct CloneFamilyChanges {
    pub(super) new: Findings<CloneFamilyFinding>,
    pub(super) resolved: Findings<CloneFamilyFinding>,
    pub(super) unchanged: usize,
    pub(super) unlisted_clone_family_group_ids: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub(super) struct ExternalMetricChanges {
    #[serde(skip)]
    pub(super) name: String,
    #[serde(skip)]
    pub(super) description: String,
    #[serde(flatten)]
    pub(super) changes: MetricChanges<ExternalMetricFinding>,
}

#[derive(Debug)]
pub(super) struct MetricChanges<T> {
    pub(super) metric_version: u32,
    pub(super) base_ratio: Decimal,
    pub(super) head_ratio: Decimal,
    pub(super) head_budgeted_ratio: BudgetedRatio,
    pub(super) base_numerator: Decimal,
    pub(super) head_numerator: Decimal,
    pub(super) base_file_numerators: BTreeMap<String, Decimal>,
    pub(super) head_file_numerators: BTreeMap<String, Decimal>,
    pub(super) new_findings_by_file: BTreeMap<String, usize>,
    pub(super) new: Findings<T>,
    pub(super) resolved: Findings<T>,
    pub(super) unchanged: usize,
}

#[derive(Debug, Serialize, Deserialize)]
pub(super) struct DependenciesChanges {
    pub(super) unused_dependencies: MetricChanges<UnusedDependencyRow>,
    pub(super) unlisted_dependencies: MetricChanges<ImportedDependencyRow>,
    pub(super) dev_in_production_dependencies: MetricChanges<ImportedDependencyRow>,
}

pub(super) struct DiffMetricSummary<'a> {
    pub(super) key: &'a str,
    pub(super) label: &'a str,
    pub(super) base_ratio: Decimal,
    pub(super) head_ratio: Decimal,
    pub(super) metric_version: u32,
    pub(super) head_budgeted_ratio: BudgetedRatio,
    pub(super) new_count: usize,
    pub(super) resolved_count: usize,
    pub(super) unchanged: usize,
}

impl<T> MetricChanges<T> {
    fn summary<'a>(&'a self, key: &'a str, label: &'a str) -> DiffMetricSummary<'a> {
        DiffMetricSummary {
            key,
            label,
            base_ratio: self.base_ratio,
            head_ratio: self.head_ratio,
            metric_version: self.metric_version,
            head_budgeted_ratio: self.head_budgeted_ratio,
            new_count: self.new.listed.len().saturating_add(self.new.left_out),
            resolved_count: self
                .resolved
                .listed
                .len()
                .saturating_add(self.resolved.left_out),
            unchanged: self.unchanged,
        }
    }
}

impl SourceSetChanges {
    pub(super) fn metric_summaries(&self) -> Vec<DiffMetricSummary<'_>> {
        let mut summaries = vec![
            self.erosion.summary(Metric::Erosion.key(), "Erosion"),
            self.cognitive_complexity
                .summary(Metric::CognitiveComplexity.key(), "Cognitive complexity"),
            self.duplication
                .groups
                .summary(Metric::Duplication.key(), "Duplication"),
            self.verbosity.summary(Metric::Verbosity.key(), "Verbosity"),
        ];
        if let Some(item) = &self.import_cycles {
            summaries.push(item.summary(Metric::ImportCycles.key(), "Import cycles"));
        }
        if let Some(item) = &self.boundaries {
            summaries.push(item.summary(Metric::Boundaries.key(), "Boundaries"));
        }
        if let Some(item) = &self.orphan_files {
            summaries.push(item.summary(Metric::OrphanFiles.key(), "Orphan files"));
        }
        if let Some(item) = &self.hazards {
            summaries.push(item.summary(Metric::Hazards.key(), "Hazards"));
        }
        for (key, external) in &self.external_metrics {
            summaries.push(
                external
                    .changes
                    .summary(key.as_str(), external.name.as_str()),
            );
        }
        summaries
    }
}

impl DependenciesChanges {
    pub(super) fn metric_summaries(&self) -> [DiffMetricSummary<'_>; 3] {
        [
            self.unused_dependencies
                .summary(Metric::UnusedDependencies.key(), "Unused dependencies"),
            self.unlisted_dependencies
                .summary(Metric::UnlistedDependencies.key(), "Unlisted dependencies"),
            self.dev_in_production_dependencies.summary(
                Metric::DevInProductionDependencies.key(),
                "Dev dependencies in production",
            ),
        ]
    }
}

impl_ratchet_metrics_source!(SourceSetChanges => head_ratio, DependenciesChanges => head_ratio);

pub struct DiffReportOptions<'a> {
    pub findings: FindingsShown,
    pub budgets: &'a Budgets,
    pub new_file_budgets: Option<&'a Budgets>,
    pub new_files: Option<CommitMeasurement>,
    pub changed_files: BTreeSet<String>,
    pub limits: &'a RegressionLimits,
    pub ownership: OwnershipOptions<'a>,
    pub baseline: Option<&'a Baseline>,
    pub include_test_gap_note: bool,
}

impl<'a> DiffReportOptions<'a> {
    pub fn new(
        findings: FindingsShown,
        budgets: &'a Budgets,
        limits: &'a RegressionLimits,
    ) -> DiffReportOptions<'a> {
        DiffReportOptions {
            findings,
            budgets,
            new_file_budgets: None,
            new_files: None,
            changed_files: BTreeSet::new(),
            limits,
            ownership: OwnershipOptions::default(),
            baseline: None,
            include_test_gap_note: false,
        }
    }

    with_fields! {
        with_new_file_budgets(new_file_budgets: &'a Budgets) => Some(new_file_budgets),
        with_new_files(new_files: Option<CommitMeasurement>) => new_files,
        with_changed_files(changed_files: BTreeSet<String>) => changed_files,
        with_ownership(ownership: OwnershipOptions<'a>) => ownership,
        with_baseline(baseline: Option<&'a Baseline>) => baseline,
        with_test_gap_note(include_test_gap_note: bool) => include_test_gap_note,
    }
}

impl<T: Serialize> Serialize for MetricChanges<T> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(None)?;
        serialize_metric_changes_fields(&mut map, self)?;
        map.end()
    }
}

impl Serialize for DuplicationChanges {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(None)?;
        serialize_metric_changes_fields(&mut map, &self.groups)?;
        if let Some(clone_families) = &self.clone_families {
            map.serialize_entry("clone_families", clone_families)?;
        }
        map.end()
    }
}

impl Serialize for CloneFamilyChanges {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(None)?;
        serialize_diff_findings(&mut map, &self.new, &self.resolved, self.unchanged)?;
        if !self.unlisted_clone_family_group_ids.is_empty() {
            map.serialize_entry(
                "unlisted_clone_family_group_ids",
                &self.unlisted_clone_family_group_ids,
            )?;
        }
        map.end()
    }
}

fn serialize_metric_changes_fields<M: SerializeMap, T: Serialize>(
    map: &mut M,
    changes: &MetricChanges<T>,
) -> Result<(), M::Error> {
    map.serialize_entry("base_ratio", &changes.base_ratio)?;
    map.serialize_entry("head_ratio", &changes.head_ratio)?;
    map.serialize_entry("metric_version", &changes.metric_version)?;
    serialize_diff_findings(map, &changes.new, &changes.resolved, changes.unchanged)
}

fn serialize_diff_findings<M: SerializeMap, T: Serialize>(
    map: &mut M,
    new: &Findings<T>,
    resolved: &Findings<T>,
    unchanged: usize,
) -> Result<(), M::Error> {
    map.serialize_entry("new", &new.listed)?;
    if new.left_out > 0 {
        map.serialize_entry("new_left_out", &new.left_out)?;
    }
    map.serialize_entry("resolved", &resolved.listed)?;
    if resolved.left_out > 0 {
        map.serialize_entry("resolved_left_out", &resolved.left_out)?;
    }
    map.serialize_entry("unchanged", &unchanged)
}

#[derive(Deserialize)]
struct RawDiffFindings<T> {
    new: Vec<T>,
    #[serde(default)]
    new_left_out: usize,
    resolved: Vec<T>,
    #[serde(default)]
    resolved_left_out: usize,
    unchanged: usize,
}

impl<T> RawDiffFindings<T> {
    fn into_parts(self) -> (Findings<T>, Findings<T>, usize) {
        (
            Findings::listing(self.new, self.new_left_out),
            Findings::listing(self.resolved, self.resolved_left_out),
            self.unchanged,
        )
    }
}

impl<'de, T: Deserialize<'de>> Deserialize<'de> for MetricChanges<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct RawMetricChanges<T> {
            #[serde(default = "crate::sections::default_metric_version")]
            metric_version: u32,
            base_ratio: Decimal,
            head_ratio: Decimal,
            #[serde(flatten)]
            findings: RawDiffFindings<T>,
        }

        let raw = RawMetricChanges::<T>::deserialize(deserializer)?;
        let (new, resolved, unchanged) = raw.findings.into_parts();
        Ok(MetricChanges {
            metric_version: raw.metric_version,
            base_ratio: raw.base_ratio,
            head_ratio: raw.head_ratio,
            head_budgeted_ratio: BudgetedRatio::from(raw.head_ratio.0),
            base_numerator: Decimal(0.0),
            head_numerator: Decimal(0.0),
            base_file_numerators: BTreeMap::new(),
            head_file_numerators: BTreeMap::new(),
            new_findings_by_file: BTreeMap::new(),
            new,
            resolved,
            unchanged,
        })
    }
}

impl<'de> Deserialize<'de> for CloneFamilyChanges {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct RawCloneFamilyChanges {
            #[serde(flatten)]
            findings: RawDiffFindings<CloneFamilyFinding>,
            #[serde(default)]
            unlisted_clone_family_group_ids: Vec<String>,
        }

        let raw = RawCloneFamilyChanges::deserialize(deserializer)?;
        let (new, resolved, unchanged) = raw.findings.into_parts();
        Ok(CloneFamilyChanges {
            new,
            resolved,
            unchanged,
            unlisted_clone_family_group_ids: raw.unlisted_clone_family_group_ids,
        })
    }
}

impl<'de> Deserialize<'de> for DuplicationChanges {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct RawDuplicationChanges {
            #[serde(default = "crate::sections::default_metric_version")]
            metric_version: u32,
            base_ratio: Decimal,
            head_ratio: Decimal,
            #[serde(default)]
            clone_families: Option<CloneFamilyChanges>,
            #[serde(flatten)]
            findings: RawDiffFindings<CloneGroupFinding>,
        }

        let raw = RawDuplicationChanges::deserialize(deserializer)?;
        let (new, resolved, unchanged) = raw.findings.into_parts();
        Ok(DuplicationChanges {
            groups: MetricChanges {
                metric_version: raw.metric_version,
                base_ratio: raw.base_ratio,
                head_ratio: raw.head_ratio,
                head_budgeted_ratio: BudgetedRatio::from(raw.head_ratio.0),
                base_numerator: Decimal(0.0),
                head_numerator: Decimal(0.0),
                base_file_numerators: BTreeMap::new(),
                head_file_numerators: BTreeMap::new(),
                new_findings_by_file: BTreeMap::new(),
                new,
                resolved,
                unchanged,
            },
            clone_families: raw.clone_families,
        })
    }
}
