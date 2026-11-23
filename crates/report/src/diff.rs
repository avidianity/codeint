use std::collections::{BTreeMap, BTreeSet, HashSet};

use codeint_config::{
    Budgets, FindingsShown, NewFindingsRule, RegressionLimit, RegressionLimits, ReportFormat,
};
use codeint_model::{Metric, SourceSet};

use serde::ser::SerializeMap;
use serde::{Serialize, Serializer};

use crate::measurement::{CommitMeasurement, MetricMeasurement, SourceSetMeasurement};
use crate::ownership::{OwnedFinding, OwnershipOptions};
use crate::sarif::{ListedExternalMetric, ListedFindings, SarifLog};
use crate::{
    BoundaryFindingRow, BudgetFailure, COGNITIVE_COMPLEXITY_KEY, CloneGroupFinding,
    CognitiveComplexityFinding, Decimal, ErodedFunctionFinding, ExternalMetricFinding, Findings,
    FlaggedLineRow, ImportCycleFinding, ReportError,
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

#[derive(Debug, Serialize)]
pub struct DiffReport {
    base: BaseEntry,
    head: HeadEntry,
    source_sets: SourceSetChangesSections,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    over_budget: Vec<BudgetFailure>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    regressions: Vec<Regression>,
}

#[derive(Debug, Serialize)]
pub(crate) struct Regression {
    source_set: &'static str,
    metric: String,
    rule: RegressionRule,
    base: Decimal,
    head: Decimal,
    new_findings: usize,
    ratio_rise_limit: f64,
    #[serde(skip)]
    numerator_key: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
enum RegressionRule {
    RatioRise,
    NewFindings,
    NumeratorRiseWithNewFindings,
}

#[derive(Debug, Serialize)]
struct BaseEntry {
    #[serde(rename = "ref")]
    git_ref: String,
    merge_base: String,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
enum HeadEntry {
    WorkingTree(&'static str),
    Commit {
        #[serde(rename = "ref")]
        git_ref: String,
        commit: String,
    },
}

#[derive(Debug, Default, Serialize)]
struct SourceSetChangesSections {
    #[serde(skip_serializing_if = "Option::is_none")]
    production: Option<SourceSetChanges>,
    #[serde(skip_serializing_if = "Option::is_none")]
    test: Option<SourceSetChanges>,
    #[serde(skip_serializing_if = "Option::is_none")]
    generated: Option<SourceSetChanges>,
    #[serde(skip_serializing_if = "Option::is_none")]
    vendored: Option<SourceSetChanges>,
}

#[derive(Debug, Serialize)]
struct SourceSetChanges {
    erosion: MetricChanges<ErodedFunctionFinding>,
    cognitive_complexity: MetricChanges<CognitiveComplexityFinding>,
    duplication: MetricChanges<CloneGroupFinding>,
    verbosity: MetricChanges<FlaggedLineRow>,
    import_cycles: MetricChanges<ImportCycleFinding>,
    #[serde(skip_serializing_if = "Option::is_none")]
    boundaries: Option<MetricChanges<BoundaryFindingRow>>,
    #[serde(flatten)]
    external_metrics: BTreeMap<String, ExternalMetricChanges>,
}

#[derive(Debug, Serialize)]
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
        let base_sets = base_measurement.into_source_sets();
        let head_sets = head_measurement.into_source_sets();
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
        DiffReport {
            base: BaseEntry {
                git_ref: base.git_ref,
                merge_base: base.merge_base,
            },
            head: head_entry(head),
            over_budget: over_budget(&source_sets, options.budgets),
            regressions: regressions(&source_sets, options.limits),
            source_sets,
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
}

fn head_entry(head: Head) -> HeadEntry {
    match head {
        Head::WorkingTree => HeadEntry::WorkingTree(WORKING_TREE),
        Head::Commit(HeadCommit { git_ref, commit }) => HeadEntry::Commit { git_ref, commit },
    }
}

impl DiffReport {
    fn new_findings_as_sarif(&self) -> SarifLog {
        SarifLog::listing(self.source_sets.iter().map(|(source_set, changes)| {
            (
                source_set,
                ListedFindings {
                    eroded_functions: &changes.erosion.new.listed,
                    cognitive_complexity: &changes.cognitive_complexity.new.listed,
                    clone_groups: &changes.duplication.new.listed,
                    flagged_lines: &changes.verbosity.new.listed,
                    import_cycles: &changes.import_cycles.new.listed,
                    boundaries: changes
                        .boundaries
                        .as_ref()
                        .map_or(&[], |boundaries| boundaries.new.listed.as_slice()),
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
        }))
    }
}

impl<T: Serialize> Serialize for MetricChanges<T> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(None)?;
        map.serialize_entry("base_ratio", &self.base_ratio)?;
        map.serialize_entry("head_ratio", &self.head_ratio)?;
        map.serialize_entry("new", &self.new.listed)?;
        if self.new.left_out > 0 {
            map.serialize_entry("new_left_out", &self.new.left_out)?;
        }
        map.serialize_entry("resolved", &self.resolved.listed)?;
        if self.resolved.left_out > 0 {
            map.serialize_entry("resolved_left_out", &self.resolved.left_out)?;
        }
        map.serialize_entry("unchanged", &self.unchanged)?;
        map.end()
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
                    changes.duplication.head_ratio.0,
                ),
                (
                    source_set,
                    Metric::Verbosity.key(),
                    changes.verbosity.head_ratio.0,
                ),
                (
                    source_set,
                    Metric::ImportCycles.key(),
                    changes.import_cycles.head_ratio.0,
                ),
            ]
            .into_iter()
            .chain(
                changes
                    .boundaries
                    .iter()
                    .map(move |boundaries| (source_set, "boundaries", boundaries.head_ratio.0)),
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
                .regressions(source_set, Metric::Duplication.key(), limits),
            self.verbosity
                .regressions(source_set, Metric::Verbosity.key(), limits),
            self.import_cycles
                .regressions(source_set, Metric::ImportCycles.key(), limits),
        ]
        .into_iter()
        .flatten()
        .collect();
        if let Some(boundaries) = &self.boundaries {
            found.extend(boundaries.regressions(source_set, "boundaries", limits));
        }
        for (key, external) in &self.external_metrics {
            found.extend(external.changes.regressions(source_set, key, limits));
        }
        found
    }
}

impl<T> MetricChanges<T> {
    fn regressions(
        &self,
        source_set: SourceSet,
        metric_key: &str,
        limits: &RegressionLimits,
    ) -> Vec<Regression> {
        let limit = limits.limit_for_key(source_set, metric_key);
        REGRESSION_RULES
            .into_iter()
            .filter(|rule| self.breaks(*rule, limit))
            .map(|rule| {
                let (base, head) = self.compared_values(rule);
                Regression {
                    source_set: source_set.key(),
                    metric: metric_key.to_owned(),
                    rule,
                    base,
                    head,
                    new_findings: self.new_findings(),
                    ratio_rise_limit: limit.ratio_rise.ratio(),
                    numerator_key: numerator_key(metric_key),
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
        self.new.listed.len() + self.new.left_out
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
                "The {} {} numerator {} rose from {} to {} with {}.",
                crate::markdown::prose(self.source_set),
                crate::markdown::prose(&self.metric),
                crate::markdown::code(self.numerator_key),
                self.base.0,
                self.head.0,
                new_findings_phrase(self.new_findings)
            ),
        }
    }
}

impl std::fmt::Display for Regression {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Regression {
            source_set,
            metric,
            base,
            head,
            ..
        } = self;
        match self.rule {
            RegressionRule::RatioRise => write!(
                formatter,
                "the {source_set} {metric} ratio rose from {} to {}, more than its Regression limit {}",
                base.0, head.0, self.ratio_rise_limit
            ),
            RegressionRule::NewFindings => write!(
                formatter,
                "the {source_set} {metric} Metric has {}, and its Regression limit allows none",
                new_findings_phrase(self.new_findings)
            ),
            RegressionRule::NumeratorRiseWithNewFindings => write!(
                formatter,
                "the {source_set} {metric} numerator {} rose from {} to {} with {}",
                self.numerator_key,
                base.0,
                head.0,
                new_findings_phrase(self.new_findings)
            ),
        }
    }
}

fn numerator_key(metric_key: &str) -> &'static str {
    match metric_key {
        "erosion" => "eroded_function_mass",
        "cognitive_complexity" => "complex_cognitive_complexity",
        "duplication" => "clone_lines",
        "verbosity" => "verbose_lines",
        "import_cycles" => "modules_in_import_cycles",
        "boundaries" => "boundary_findings",
        _ => "numerator",
    }
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
    let boundaries = match (base.boundaries, head.boundaries) {
        (None, None) => None,
        (base_boundaries, head_boundaries) => Some(classify(
            base_boundaries.unwrap_or_default(),
            head_boundaries.unwrap_or_default(),
            shown,
            ownership,
        )),
    };
    SourceSetChanges {
        erosion: classify(base.erosion, head.erosion, shown, ownership),
        cognitive_complexity: classify(
            base.cognitive_complexity,
            head.cognitive_complexity,
            shown,
            ownership,
        ),
        duplication: classify(base.duplication, head.duplication, shown, ownership),
        verbosity: classify(base.verbosity, head.verbosity, shown, ownership),
        import_cycles: classify(base.import_cycles, head.import_cycles, shown, ownership),
        boundaries,
        external_metrics,
    }
}

impl SourceSetChangesSections {
    fn iter(&self) -> impl Iterator<Item = (SourceSet, &SourceSetChanges)> {
        [
            (SourceSet::Production, &self.production),
            (SourceSet::Test, &self.test),
            (SourceSet::Generated, &self.generated),
            (SourceSet::Vendored, &self.vendored),
        ]
        .into_iter()
        .filter_map(|(source_set, changes)| Some((source_set, changes.as_ref()?)))
    }

    fn named(&self) -> Vec<(&'static str, &SourceSetChanges)> {
        self.iter()
            .map(|(source_set, changes)| (source_set_name(source_set), changes))
            .collect()
    }

    fn slot(&mut self, source_set: SourceSet) -> &mut Option<SourceSetChanges> {
        match source_set {
            SourceSet::Production => &mut self.production,
            SourceSet::Test => &mut self.test,
            SourceSet::Generated => &mut self.generated,
            SourceSet::Vendored => &mut self.vendored,
        }
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

fn source_set_name(source_set: SourceSet) -> &'static str {
    match source_set {
        SourceSet::Production => "Production",
        SourceSet::Test => "Test",
        SourceSet::Generated => "Generated",
        SourceSet::Vendored => "Vendored",
    }
}
