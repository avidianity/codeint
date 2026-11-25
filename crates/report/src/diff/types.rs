use std::collections::BTreeMap;

use codeint_config::{Budgets, FindingsShown, RegressionLimits};
use serde::ser::SerializeMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::ownership::OwnershipOptions;
use crate::{
    BoundaryFindingRow, BudgetFailure, CloneFamilyFinding, CloneGroupFinding,
    CognitiveComplexityFinding, Decimal, ErodedFunctionFinding, ExternalMetricFinding, Findings,
    FlaggedLineRow, HazardLineRow, ImportCycleFinding, ImportedDependencyRow, OrphanFileRow,
    PerSourceSet, UnusedDependencyRow,
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

#[derive(Debug, Serialize, Deserialize)]
pub struct DiffReport {
    pub(super) base: BaseEntry,
    pub(super) head: HeadEntry,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) test_gap_note: Option<String>,
    pub(super) source_sets: SourceSetChangesSections,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) dependencies: Option<DependenciesChanges>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(super) over_budget: Vec<BudgetFailure>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(super) regressions: Vec<Regression>,
}

#[derive(Debug, Serialize, Deserialize)]
pub(super) struct Regression {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) source_set: Option<String>,
    pub(super) metric: String,
    pub(super) rule: RegressionRule,
    pub(super) base: Decimal,
    pub(super) head: Decimal,
    pub(super) new_findings: usize,
    pub(super) ratio_rise_limit: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum RegressionRule {
    RatioRise,
    NewFindings,
    #[serde(alias = "numerator_rise_with_new_findings")]
    NumeratorRise,
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
    pub(super) base_ratio: Decimal,
    pub(super) head_ratio: Decimal,
    pub(super) base_numerator: Decimal,
    pub(super) head_numerator: Decimal,
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

pub struct DiffReportOptions<'a> {
    pub findings: FindingsShown,
    pub budgets: &'a Budgets,
    pub limits: &'a RegressionLimits,
    pub ownership: OwnershipOptions<'a>,
    pub include_test_gap_note: bool,
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
            Findings {
                listed: self.new,
                left_out: self.new_left_out,
            },
            Findings {
                listed: self.resolved,
                left_out: self.resolved_left_out,
            },
            self.unchanged,
        )
    }
}

impl<'de, T: Deserialize<'de>> Deserialize<'de> for MetricChanges<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct RawMetricChanges<T> {
            base_ratio: Decimal,
            head_ratio: Decimal,
            #[serde(flatten)]
            findings: RawDiffFindings<T>,
        }

        let raw = RawMetricChanges::<T>::deserialize(deserializer)?;
        let (new, resolved, unchanged) = raw.findings.into_parts();
        Ok(MetricChanges {
            base_ratio: raw.base_ratio,
            head_ratio: raw.head_ratio,
            base_numerator: Decimal(0.0),
            head_numerator: Decimal(0.0),
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
                base_ratio: raw.base_ratio,
                head_ratio: raw.head_ratio,
                base_numerator: Decimal(0.0),
                head_numerator: Decimal(0.0),
                new,
                resolved,
                unchanged,
            },
            clone_families: raw.clone_families,
        })
    }
}
