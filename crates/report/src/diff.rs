use std::collections::{BTreeMap, BTreeSet, HashSet};

use codeint_config::{
    Budgets, FindingsShown, NewFindingsRule, RegressionLimit, RegressionLimits, ReportFormat,
};
use codeint_metrics::{
    DEV_IN_PRODUCTION_DEPENDENCIES_METRIC_KEY, UNLISTED_DEPENDENCIES_METRIC_KEY,
    UNUSED_DEPENDENCIES_METRIC_KEY,
};
use codeint_model::{Metric, SourceSet};

use serde::ser::SerializeMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::measurement::{
    CommitMeasurement, DependenciesMeasurement, MetricMeasurement, SourceSetMeasurement,
};
use crate::ownership::{OwnedFinding, OwnershipOptions};
use crate::sarif::{ListedDependencies, ListedExternalMetric, ListedFindings, SarifLog};
use crate::{
    BoundaryFindingRow, BudgetFailure, COGNITIVE_COMPLEXITY_KEY, CloneFamilyFinding,
    CloneGroupFinding, CognitiveComplexityFinding, Decimal, ErodedFunctionFinding,
    ExternalMetricFinding, Findings, FlaggedLineRow, ImportCycleFinding, ImportedDependencyRow,
    OrphanFileRow, PerSourceSet, ReportError, UnusedDependencyRow, clustered_clone_group_ids,
    dependency_over_budget, unlisted_clone_family_group_ids,
};

mod markdown;
#[cfg(test)]
mod tests;

const WORKING_TREE: &str = "working tree";

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
    base: BaseEntry,
    head: HeadEntry,
    source_sets: SourceSetChangesSections,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    dependencies: Option<DependenciesChanges>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    over_budget: Vec<BudgetFailure>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    regressions: Vec<Regression>,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct Regression {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    source_set: Option<String>,
    metric: String,
    rule: RegressionRule,
    base: Decimal,
    head: Decimal,
    new_findings: usize,
    ratio_rise_limit: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum RegressionRule {
    RatioRise,
    NewFindings,
    NumeratorRiseWithNewFindings,
}

#[derive(Debug, Serialize, Deserialize)]
struct BaseEntry {
    #[serde(rename = "ref")]
    git_ref: String,
    merge_base: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
enum HeadEntry {
    WorkingTree(String),
    Commit {
        #[serde(rename = "ref")]
        git_ref: String,
        commit: String,
    },
}

type SourceSetChangesSections = PerSourceSet<SourceSetChanges>;

#[derive(Debug, Serialize, Deserialize)]
struct SourceSetChanges {
    erosion: MetricChanges<ErodedFunctionFinding>,
    cognitive_complexity: MetricChanges<CognitiveComplexityFinding>,
    duplication: DuplicationChanges,
    verbosity: MetricChanges<FlaggedLineRow>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    import_cycles: Option<MetricChanges<ImportCycleFinding>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    boundaries: Option<MetricChanges<BoundaryFindingRow>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    orphan_files: Option<MetricChanges<OrphanFileRow>>,
    #[serde(default, flatten)]
    external_metrics: BTreeMap<String, ExternalMetricChanges>,
}

#[derive(Debug)]
struct DuplicationChanges {
    groups: MetricChanges<CloneGroupFinding>,
    clone_families: Option<CloneFamilyChanges>,
}

#[derive(Debug)]
struct CloneFamilyChanges {
    new: Findings<CloneFamilyFinding>,
    resolved: Findings<CloneFamilyFinding>,
    unchanged: usize,
    unlisted_clone_family_group_ids: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ExternalMetricChanges {
    #[serde(skip)]
    name: String,
    #[serde(skip)]
    description: String,
    #[serde(flatten)]
    changes: MetricChanges<ExternalMetricFinding>,
}

#[derive(Debug)]
struct MetricChanges<T> {
    base_ratio: Decimal,
    head_ratio: Decimal,
    base_numerator: Decimal,
    head_numerator: Decimal,
    new: Findings<T>,
    resolved: Findings<T>,
    unchanged: usize,
}

trait Identified {
    fn id(&self) -> &str;
}

pub struct DiffReportOptions<'a> {
    pub findings: FindingsShown,
    pub budgets: &'a Budgets,
    pub limits: &'a RegressionLimits,
    pub ownership: OwnershipOptions<'a>,
}

impl DiffReport {
    pub fn compare(
        base: BaseCommit,
        head: Head,
        base_measurement: CommitMeasurement,
        head_measurement: CommitMeasurement,
        findings: FindingsShown,
        budgets: &Budgets,
        limits: &RegressionLimits,
    ) -> DiffReport {
        DiffReport::compare_with_options(
            base,
            head,
            base_measurement,
            head_measurement,
            DiffReportOptions {
                findings,
                budgets,
                limits,
                ownership: OwnershipOptions::default(),
            },
        )
    }

    pub fn compare_with_options(
        base: BaseCommit,
        head: Head,
        base_measurement: CommitMeasurement,
        head_measurement: CommitMeasurement,
        options: DiffReportOptions<'_>,
    ) -> DiffReport {
        let mut source_sets = SourceSetChangesSections::default();
        let (base_sets, base_dependencies) = base_measurement.into_parts();
        let (head_sets, head_dependencies) = head_measurement.into_parts();
        for ((source_set, base), (_, head)) in base_sets.into_iter().zip(head_sets) {
            if base.files > 0 || head.files > 0 {
                *source_sets.slot(source_set) = Some(source_set_changes(
                    base,
                    head,
                    options.findings,
                    options.ownership,
                ));
            }
        }
        let dependencies = dependencies_changes(
            base_dependencies,
            head_dependencies,
            options.findings,
            options.ownership,
        );
        let mut over_budget_failures = over_budget(&source_sets, options.budgets);
        if let Some(deps) = &dependencies {
            over_budget_failures.extend(deps.over_budget(options.budgets));
        }
        let mut regression_failures = regressions(&source_sets, options.limits);
        if let Some(deps) = &dependencies {
            regression_failures.extend(deps.regressions(options.limits));
        }
        DiffReport {
            base: BaseEntry {
                git_ref: base.git_ref,
                merge_base: base.merge_base,
            },
            head: head_entry(head),
            over_budget: over_budget_failures,
            regressions: regression_failures,
            source_sets,
            dependencies,
        }
    }

    pub fn failure_messages(&self) -> Vec<String> {
        self.over_budget
            .iter()
            .map(ToString::to_string)
            .chain(self.regressions.iter().map(ToString::to_string))
            .collect()
    }

    pub fn render(&self, format: ReportFormat) -> Result<String, ReportError> {
        match format {
            ReportFormat::Toon => crate::render_toon(self),
            ReportFormat::Json => crate::render_json(self),
            ReportFormat::Sarif => serde_json::to_string_pretty(&self.new_findings_as_sarif())
                .map_err(ReportError::Sarif),
            ReportFormat::Markdown => Ok(markdown::render(self)),
        }
    }

    pub(crate) fn populate_external_metric_names(&mut self) {
        for slot in self.source_sets.iter_mut() {
            for (key, external) in &mut slot.external_metrics {
                if external.name.is_empty() {
                    external.name = key.clone();
                }
            }
        }
    }
}

fn head_entry(head: Head) -> HeadEntry {
    match head {
        Head::WorkingTree => HeadEntry::WorkingTree(WORKING_TREE.to_owned()),
        Head::Commit(HeadCommit { git_ref, commit }) => HeadEntry::Commit { git_ref, commit },
    }
}

impl DiffReport {
    fn new_findings_as_sarif(&self) -> SarifLog {
        let dependencies = match &self.dependencies {
            Some(deps) => ListedDependencies {
                unused: &deps.unused_dependencies.new.listed,
                unlisted: &deps.unlisted_dependencies.new.listed,
                dev_in_production: &deps.dev_in_production_dependencies.new.listed,
            },
            None => ListedDependencies::EMPTY,
        };
        SarifLog::listing(
            self.source_sets.iter().map(|(source_set, changes)| {
                (
                    source_set,
                    ListedFindings {
                        eroded_functions: &changes.erosion.new.listed,
                        cognitive_complexity: &changes.cognitive_complexity.new.listed,
                        clone_groups: &changes.duplication.groups.new.listed,
                        clone_families: changes
                            .duplication
                            .clone_families
                            .as_ref()
                            .map_or(&[], |families| families.new.listed.as_slice()),
                        clustered_clone_group_ids: changes
                            .duplication
                            .clone_families
                            .as_ref()
                            .map_or(&[], |families| {
                                families.unlisted_clone_family_group_ids.as_slice()
                            }),
                        flagged_lines: &changes.verbosity.new.listed,
                        import_cycles: changes
                            .import_cycles
                            .as_ref()
                            .map_or(&[], |cycles| cycles.new.listed.as_slice()),
                        boundaries: changes
                            .boundaries
                            .as_ref()
                            .map_or(&[], |boundaries| boundaries.new.listed.as_slice()),
                        orphan_files: changes
                            .orphan_files
                            .as_ref()
                            .map_or(&[], |orphans| orphans.new.listed.as_slice()),
                        external_metrics: changes
                            .external_metrics
                            .iter()
                            .map(|(key, external)| ListedExternalMetric {
                                key: key.as_str(),
                                name: external.name.as_str(),
                                description: external.description.as_str(),
                                findings: &external.changes.new.listed,
                            })
                            .collect(),
                    },
                )
            }),
            dependencies,
        )
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
fn classify<T: Identified + OwnedFinding>(
    base: MetricMeasurement<T>,
    head: MetricMeasurement<T>,
    shown: FindingsShown,
    ownership: OwnershipOptions<'_>,
) -> MetricChanges<T> {
    let base_findings = ownership.apply(base.findings_worst_first);
    let head_findings = ownership.apply(head.findings_worst_first);
    let base_ids = ids(&base_findings);
    let head_ids = ids(&head_findings);
    MetricChanges {
        base_ratio: Decimal::rounded(base.ratio),
        head_ratio: Decimal::rounded(head.ratio),
        base_numerator: Decimal::rounded(base.numerator),
        head_numerator: Decimal::rounded(head.numerator),
        unchanged: head_ids.intersection(&base_ids).count(),
        new: missing_from(head_findings, &base_ids, shown),
        resolved: missing_from(base_findings, &head_ids, shown),
    }
}

const REGRESSION_RULES: [RegressionRule; 3] = [
    RegressionRule::RatioRise,
    RegressionRule::NewFindings,
    RegressionRule::NumeratorRiseWithNewFindings,
];

fn over_budget(source_sets: &SourceSetChangesSections, budgets: &Budgets) -> Vec<BudgetFailure> {
    crate::over_budget(
        budgets,
        source_sets.iter().flat_map(|(source_set, changes)| {
            [
                (
                    source_set,
                    Metric::Erosion.key(),
                    changes.erosion.head_ratio.0,
                ),
                (
                    source_set,
                    COGNITIVE_COMPLEXITY_KEY,
                    changes.cognitive_complexity.head_ratio.0,
                ),
                (
                    source_set,
                    Metric::Duplication.key(),
                    changes.duplication.groups.head_ratio.0,
                ),
                (
                    source_set,
                    Metric::Verbosity.key(),
                    changes.verbosity.head_ratio.0,
                ),
            ]
            .into_iter()
            .chain(
                changes.import_cycles.iter().map(move |cycles| {
                    (source_set, Metric::ImportCycles.key(), cycles.head_ratio.0)
                }),
            )
            .chain(
                changes
                    .boundaries
                    .iter()
                    .map(move |boundaries| (source_set, "boundaries", boundaries.head_ratio.0)),
            )
            .chain(
                changes
                    .orphan_files
                    .iter()
                    .map(move |orphans| (source_set, "orphan_files", orphans.head_ratio.0)),
            )
            .chain(
                changes.external_metrics.iter().map(move |(key, metric)| {
                    (source_set, key.as_str(), metric.changes.head_ratio.0)
                }),
            )
        }),
    )
}

fn regressions(
    source_sets: &SourceSetChangesSections,
    limits: &RegressionLimits,
) -> Vec<Regression> {
    source_sets
        .iter()
        .flat_map(|(source_set, changes)| changes.regressions(source_set, limits))
        .collect()
}

impl SourceSetChanges {
    fn regressions(&self, source_set: SourceSet, limits: &RegressionLimits) -> Vec<Regression> {
        let mut found: Vec<Regression> = [
            self.erosion
                .regressions(source_set, Metric::Erosion.key(), limits),
            self.cognitive_complexity
                .regressions(source_set, COGNITIVE_COMPLEXITY_KEY, limits),
            self.duplication
                .groups
                .regressions(source_set, Metric::Duplication.key(), limits),
            self.verbosity
                .regressions(source_set, Metric::Verbosity.key(), limits),
        ]
        .into_iter()
        .flatten()
        .collect();
        if let Some(import_cycles) = &self.import_cycles {
            found.extend(import_cycles.regressions(source_set, Metric::ImportCycles.key(), limits));
        }
        if let Some(boundaries) = &self.boundaries {
            found.extend(boundaries.regressions(source_set, "boundaries", limits));
        }
        if let Some(orphan_files) = &self.orphan_files {
            found.extend(orphan_files.regressions(source_set, "orphan_files", limits));
        }
        for (key, external) in &self.external_metrics {
            found.extend(external.changes.regressions(source_set, key, limits));
        }
        found
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct DependenciesChanges {
    unused_dependencies: MetricChanges<UnusedDependencyRow>,
    unlisted_dependencies: MetricChanges<ImportedDependencyRow>,
    dev_in_production_dependencies: MetricChanges<ImportedDependencyRow>,
}

impl DependenciesChanges {
    fn over_budget(&self, budgets: &Budgets) -> Vec<BudgetFailure> {
        dependency_over_budget(
            budgets,
            [
                (
                    UNUSED_DEPENDENCIES_METRIC_KEY,
                    self.unused_dependencies.head_ratio.0,
                ),
                (
                    UNLISTED_DEPENDENCIES_METRIC_KEY,
                    self.unlisted_dependencies.head_ratio.0,
                ),
                (
                    DEV_IN_PRODUCTION_DEPENDENCIES_METRIC_KEY,
                    self.dev_in_production_dependencies.head_ratio.0,
                ),
            ],
        )
    }

    fn regressions(&self, limits: &RegressionLimits) -> Vec<Regression> {
        let mut found = Vec::new();
        found.extend(self.unused_dependencies.regressions_with_limit(
            None,
            UNUSED_DEPENDENCIES_METRIC_KEY,
            limits.limit_for_dependency(UNUSED_DEPENDENCIES_METRIC_KEY),
        ));
        found.extend(self.unlisted_dependencies.regressions_with_limit(
            None,
            UNLISTED_DEPENDENCIES_METRIC_KEY,
            limits.limit_for_dependency(UNLISTED_DEPENDENCIES_METRIC_KEY),
        ));
        found.extend(self.dev_in_production_dependencies.regressions_with_limit(
            None,
            DEV_IN_PRODUCTION_DEPENDENCIES_METRIC_KEY,
            limits.limit_for_dependency(DEV_IN_PRODUCTION_DEPENDENCIES_METRIC_KEY),
        ));
        found
    }
}

fn dependencies_changes(
    base: Option<DependenciesMeasurement>,
    head: Option<DependenciesMeasurement>,
    shown: FindingsShown,
    ownership: OwnershipOptions<'_>,
) -> Option<DependenciesChanges> {
    if base.is_none() && head.is_none() {
        return None;
    }
    let base = base.unwrap_or_default();
    let head = head.unwrap_or_default();
    Some(DependenciesChanges {
        unused_dependencies: classify(
            base.unused_dependencies,
            head.unused_dependencies,
            shown,
            ownership,
        ),
        unlisted_dependencies: classify(
            base.unlisted_dependencies,
            head.unlisted_dependencies,
            shown,
            ownership,
        ),
        dev_in_production_dependencies: classify(
            base.dev_in_production_dependencies,
            head.dev_in_production_dependencies,
            shown,
            ownership,
        ),
    })
}

impl<T> MetricChanges<T> {
    fn regressions(
        &self,
        source_set: SourceSet,
        metric_key: &str,
        limits: &RegressionLimits,
    ) -> Vec<Regression> {
        self.regressions_with_limit(
            Some(source_set.key()),
            metric_key,
            limits.limit_for_key(source_set, metric_key),
        )
    }

    fn regressions_with_limit(
        &self,
        source_set: Option<&'static str>,
        metric_key: &str,
        limit: RegressionLimit,
    ) -> Vec<Regression> {
        REGRESSION_RULES
            .into_iter()
            .filter(|rule| self.breaks(*rule, limit))
            .map(|rule| {
                let (base, head) = self.compared_values(rule);
                Regression {
                    source_set: source_set.map(str::to_owned),
                    metric: metric_key.to_owned(),
                    rule,
                    base,
                    head,
                    new_findings: self.new_findings(),
                    ratio_rise_limit: limit.ratio_rise.ratio(),
                }
            })
            .collect()
    }

    fn breaks(&self, rule: RegressionRule, limit: RegressionLimit) -> bool {
        let adds_findings = self.new_findings() > 0;
        match rule {
            RegressionRule::RatioRise => !limit
                .ratio_rise
                .admits(Decimal::rounded(self.head_ratio.0 - self.base_ratio.0).0),
            RegressionRule::NewFindings => {
                adds_findings && limit.new_findings == NewFindingsRule::AnyNewFinding
            }
            RegressionRule::NumeratorRiseWithNewFindings => {
                adds_findings
                    && limit.new_findings == NewFindingsRule::NumeratorRiseWithNewFindings
                    && self.head_numerator.0 > self.base_numerator.0
            }
        }
    }

    fn new_findings(&self) -> usize {
        self.new.listed.len().saturating_add(self.new.left_out)
    }

    fn compared_values(&self, rule: RegressionRule) -> (Decimal, Decimal) {
        match rule {
            RegressionRule::RatioRise | RegressionRule::NewFindings => {
                (Decimal(self.base_ratio.0), Decimal(self.head_ratio.0))
            }
            RegressionRule::NumeratorRiseWithNewFindings => (
                Decimal(self.base_numerator.0),
                Decimal(self.head_numerator.0),
            ),
        }
    }
}

impl Regression {
    pub(super) fn markdown_item(&self) -> String {
        match self.rule {
            RegressionRule::RatioRise | RegressionRule::NewFindings => {
                crate::markdown::sentence_item(self)
            }
            RegressionRule::NumeratorRiseWithNewFindings => format!(
                "The {} numerator {} rose from {} to {} with {}.",
                self.scope_prose(),
                crate::markdown::code(numerator_key(&self.metric)),
                self.base.0,
                self.head.0,
                new_findings_phrase(self.new_findings)
            ),
        }
    }

    fn scope_prose(&self) -> String {
        match self.source_set.as_deref() {
            Some(source_set) => format!(
                "{} {}",
                crate::markdown::prose(source_set),
                crate::markdown::prose(&self.metric)
            ),
            None => crate::markdown::prose(&self.metric),
        }
    }

    fn scope_plain(&self) -> String {
        match self.source_set.as_deref() {
            Some(source_set) => format!("{source_set} {}", self.metric),
            None => self.metric.clone(),
        }
    }
}

impl std::fmt::Display for Regression {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let scope = self.scope_plain();
        match self.rule {
            RegressionRule::RatioRise => write!(
                formatter,
                "the {scope} ratio rose from {} to {}, more than its Regression limit {}",
                self.base.0, self.head.0, self.ratio_rise_limit
            ),
            RegressionRule::NewFindings => write!(
                formatter,
                "the {scope} Metric has {}, and its Regression limit allows none",
                new_findings_phrase(self.new_findings)
            ),
            RegressionRule::NumeratorRiseWithNewFindings => write!(
                formatter,
                "the {scope} numerator {} rose from {} to {} with {}",
                numerator_key(&self.metric),
                self.base.0,
                self.head.0,
                new_findings_phrase(self.new_findings)
            ),
        }
    }
}

const NUMERATOR_KEYS: [(&str, &str); 10] = [
    ("erosion", "eroded_function_mass"),
    ("cognitive_complexity", "complex_cognitive_complexity"),
    ("duplication", "clone_lines"),
    ("verbosity", "verbose_lines"),
    ("import_cycles", "modules_in_import_cycles"),
    ("boundaries", "boundary_findings"),
    ("orphan_files", "orphan_files"),
    ("unused_dependencies", "unused"),
    ("unlisted_dependencies", "unlisted"),
    ("dev_in_production_dependencies", "dev_in_production"),
];

fn numerator_key(metric_key: &str) -> &'static str {
    NUMERATOR_KEYS
        .iter()
        .find(|(key, _)| *key == metric_key)
        .map_or("numerator", |(_, numerator)| *numerator)
}

fn new_findings_phrase(count: usize) -> String {
    if count == 1 {
        "1 new Finding".to_owned()
    } else {
        format!("{count} new Findings")
    }
}

fn ids<T: Identified>(findings: &[T]) -> HashSet<String> {
    findings
        .iter()
        .map(|finding| finding.id().to_owned())
        .collect()
}

fn missing_from<T: Identified>(
    findings_worst_first: Vec<T>,
    other_side: &HashSet<String>,
    shown: FindingsShown,
) -> Findings<T> {
    let missing: Vec<T> = findings_worst_first
        .into_iter()
        .filter(|finding| !other_side.contains(finding.id()))
        .collect();
    Findings::worst_first(missing.into_iter(), shown)
}

fn source_set_changes(
    mut base: SourceSetMeasurement,
    mut head: SourceSetMeasurement,
    shown: FindingsShown,
    ownership: OwnershipOptions<'_>,
) -> SourceSetChanges {
    let keys: BTreeSet<String> = base
        .external_metrics
        .keys()
        .chain(head.external_metrics.keys())
        .cloned()
        .collect();
    let mut external_metrics = BTreeMap::new();
    for key in keys {
        let base_entry = base.external_metrics.remove(&key);
        let head_entry = head.external_metrics.remove(&key);
        let (name, description) = match (&head_entry, &base_entry) {
            (Some(entry), _) | (None, Some(entry)) => {
                (entry.name.clone(), entry.description.clone())
            }
            (None, None) => (key.clone(), String::new()),
        };
        let base_measurement =
            base_entry.map_or_else(MetricMeasurement::default, |e| e.measurement);
        let head_measurement =
            head_entry.map_or_else(MetricMeasurement::default, |e| e.measurement);
        external_metrics.insert(
            key,
            ExternalMetricChanges {
                name,
                description,
                changes: classify(base_measurement, head_measurement, shown, ownership),
            },
        );
    }
    let import_cycles = match (base.import_cycles, head.import_cycles) {
        (None, None) => None,
        (base_cycles, head_cycles) => Some(classify(
            base_cycles.unwrap_or_default(),
            head_cycles.unwrap_or_default(),
            shown,
            ownership,
        )),
    };
    let boundaries = match (base.boundaries, head.boundaries) {
        (None, None) => None,
        (base_boundaries, head_boundaries) => Some(classify(
            base_boundaries.unwrap_or_default(),
            head_boundaries.unwrap_or_default(),
            shown,
            ownership,
        )),
    };
    let orphan_files = match (base.orphan_files, head.orphan_files) {
        (None, None) => None,
        (base_orphans, head_orphans) => Some(classify(
            base_orphans.unwrap_or_default(),
            head_orphans.unwrap_or_default(),
            shown,
            ownership,
        )),
    };
    let duplication_groups = classify(base.duplication, head.duplication, shown, ownership);
    let base_families = ownership.apply(base.clone_families);
    let head_families = ownership.apply(head.clone_families);
    let clone_families = if base_families.is_empty() && head_families.is_empty() {
        None
    } else {
        let base_ids = ids(&base_families);
        let head_ids = ids(&head_families);
        let all_clustered = clustered_clone_group_ids(&head_families);
        let new = missing_from(head_families, &base_ids, shown);
        let resolved = missing_from(base_families, &head_ids, shown);
        let unlisted_clone_family_group_ids = unlisted_clone_family_group_ids(
            &duplication_groups.new.listed,
            &new.listed,
            &all_clustered,
        );
        Some(CloneFamilyChanges {
            unchanged: head_ids.intersection(&base_ids).count(),
            new,
            resolved,
            unlisted_clone_family_group_ids,
        })
    };
    SourceSetChanges {
        erosion: classify(base.erosion, head.erosion, shown, ownership),
        cognitive_complexity: classify(
            base.cognitive_complexity,
            head.cognitive_complexity,
            shown,
            ownership,
        ),
        duplication: DuplicationChanges {
            groups: duplication_groups,
            clone_families,
        },
        verbosity: classify(base.verbosity, head.verbosity, shown, ownership),
        import_cycles,
        boundaries,
        orphan_files,
        external_metrics,
    }
}

impl Identified for ErodedFunctionFinding {
    fn id(&self) -> &str {
        &self.id
    }
}

impl Identified for CognitiveComplexityFinding {
    fn id(&self) -> &str {
        &self.id
    }
}

impl Identified for CloneGroupFinding {
    fn id(&self) -> &str {
        &self.id
    }
}

impl Identified for CloneFamilyFinding {
    fn id(&self) -> &str {
        &self.id
    }
}

impl Identified for FlaggedLineRow {
    fn id(&self) -> &str {
        &self.id
    }
}

impl Identified for ImportCycleFinding {
    fn id(&self) -> &str {
        &self.id
    }
}

impl Identified for ExternalMetricFinding {
    fn id(&self) -> &str {
        &self.id
    }
}

impl Identified for BoundaryFindingRow {
    fn id(&self) -> &str {
        &self.id
    }
}

impl Identified for UnusedDependencyRow {
    fn id(&self) -> &str {
        &self.id
    }
}

impl Identified for ImportedDependencyRow {
    fn id(&self) -> &str {
        &self.id
    }
}

impl Identified for OrphanFileRow {
    fn id(&self) -> &str {
        &self.id
    }
}
