use std::collections::BTreeSet;

use codeint_config::{Budgets, NewFindingsRule, RegressionLimit, RegressionLimits};
use codeint_model::{Metric, SourceSet};

use super::types::{
    DependenciesChanges, MetricChanges, NewFileBudgetFailure, Regression, RegressionRule,
    SourceSetChanges, SourceSetChangesSections,
};
use crate::measurement::{CommitMeasurement, SourceSetMeasurement};
use crate::{BudgetFailure, Decimal, dependency_over_budget};

const REGRESSION_RULES: [RegressionRule; 3] = [
    RegressionRule::RatioRise,
    RegressionRule::NewFindings,
    RegressionRule::NumeratorRise,
];

pub(super) fn over_budget(
    source_sets: &SourceSetChangesSections,
    budgets: &Budgets,
) -> Vec<BudgetFailure> {
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
                    Metric::CognitiveComplexity.key(),
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
            .chain(changes.boundaries.iter().map(move |boundaries| {
                (
                    source_set,
                    Metric::Boundaries.key(),
                    boundaries.head_ratio.0,
                )
            }))
            .chain(
                changes.orphan_files.iter().map(move |orphans| {
                    (source_set, Metric::OrphanFiles.key(), orphans.head_ratio.0)
                }),
            )
            .chain(
                changes
                    .hazards
                    .iter()
                    .map(move |hazards| (source_set, Metric::Hazards.key(), hazards.head_ratio.0)),
            )
            .chain(
                changes.external_metrics.iter().map(move |(key, metric)| {
                    (source_set, key.as_str(), metric.changes.head_ratio.0)
                }),
            )
        }),
    )
}

pub(super) fn over_new_file_budget(
    new_files: Option<CommitMeasurement>,
    budgets: &Budgets,
) -> Vec<NewFileBudgetFailure> {
    let Some(measurement) = new_files else {
        return Vec::new();
    };
    if budgets.is_empty() {
        return Vec::new();
    }
    let mut failures = Vec::new();
    for (source_set, set_measurement) in measurement.into_source_sets() {
        if set_measurement.files == 0 {
            continue;
        }
        for (metric, ratio) in source_set_measurement_ratios(&set_measurement) {
            let rounded = Decimal::rounded(ratio);
            if let Some(budget) = budgets.budget_for_key(source_set, metric)
                && !budget.admits(rounded.0)
            {
                failures.push(NewFileBudgetFailure {
                    source_set: source_set.key().to_owned(),
                    metric: metric.to_owned(),
                    ratio: rounded,
                    budget: budget.ratio(),
                    files: set_measurement.files,
                });
            }
        }
    }
    failures
}

fn source_set_measurement_ratios(measurement: &SourceSetMeasurement) -> Vec<(&str, f64)> {
    let mut ratios = vec![
        (Metric::Erosion.key(), measurement.erosion.ratio),
        (
            Metric::CognitiveComplexity.key(),
            measurement.cognitive_complexity.ratio,
        ),
        (Metric::Duplication.key(), measurement.duplication.ratio),
        (Metric::Verbosity.key(), measurement.verbosity.ratio),
    ];
    if let Some(cycles) = &measurement.import_cycles {
        ratios.push((Metric::ImportCycles.key(), cycles.ratio));
    }
    if let Some(boundaries) = &measurement.boundaries {
        ratios.push((Metric::Boundaries.key(), boundaries.ratio));
    }
    if let Some(orphans) = &measurement.orphan_files {
        ratios.push((Metric::OrphanFiles.key(), orphans.ratio));
    }
    if let Some(hazards) = &measurement.hazards {
        ratios.push((Metric::Hazards.key(), hazards.ratio));
    }
    for (key, external) in &measurement.external_metrics {
        ratios.push((key.as_str(), external.measurement.ratio));
    }
    ratios
}

pub(super) fn regressions(
    source_sets: &SourceSetChangesSections,
    limits: &RegressionLimits,
    changed_files: &BTreeSet<String>,
) -> Vec<Regression> {
    source_sets
        .iter()
        .flat_map(|(source_set, changes)| changes.regressions(source_set, limits, changed_files))
        .collect()
}

impl SourceSetChanges {
    fn regressions(
        &self,
        source_set: SourceSet,
        limits: &RegressionLimits,
        changed_files: &BTreeSet<String>,
    ) -> Vec<Regression> {
        let mut found = Vec::new();
        let ctx = (source_set, limits, changed_files);
        push_metric(&mut found, &self.erosion, Metric::Erosion.key(), ctx);
        push_metric(
            &mut found,
            &self.cognitive_complexity,
            Metric::CognitiveComplexity.key(),
            ctx,
        );
        push_metric(
            &mut found,
            &self.duplication.groups,
            Metric::Duplication.key(),
            ctx,
        );
        push_metric(&mut found, &self.verbosity, Metric::Verbosity.key(), ctx);
        push_opt_metric(
            &mut found,
            self.import_cycles.as_ref(),
            Metric::ImportCycles.key(),
            ctx,
        );
        push_opt_metric(
            &mut found,
            self.boundaries.as_ref(),
            Metric::Boundaries.key(),
            ctx,
        );
        push_opt_metric(
            &mut found,
            self.orphan_files.as_ref(),
            Metric::OrphanFiles.key(),
            ctx,
        );
        push_opt_metric(
            &mut found,
            self.hazards.as_ref(),
            Metric::Hazards.key(),
            ctx,
        );
        for (key, external) in &self.external_metrics {
            push_metric(&mut found, &external.changes, key, ctx);
        }
        found
    }
}

fn push_metric<T>(
    found: &mut Vec<Regression>,
    changes: &MetricChanges<T>,
    metric_key: &str,
    (source_set, limits, changed_files): (SourceSet, &RegressionLimits, &BTreeSet<String>),
) {
    found.extend(changes.regressions(source_set, metric_key, limits, changed_files));
}

fn push_opt_metric<T>(
    found: &mut Vec<Regression>,
    changes: Option<&MetricChanges<T>>,
    metric_key: &str,
    ctx: (SourceSet, &RegressionLimits, &BTreeSet<String>),
) {
    if let Some(changes) = changes {
        push_metric(found, changes, metric_key, ctx);
    }
}

impl DependenciesChanges {
    pub(super) fn over_budget(&self, budgets: &Budgets) -> Vec<BudgetFailure> {
        dependency_over_budget(
            budgets,
            self.unused_dependencies.head_ratio.0,
            self.unlisted_dependencies.head_ratio.0,
            self.dev_in_production_dependencies.head_ratio.0,
        )
    }

    pub(super) fn regressions(
        &self,
        limits: &RegressionLimits,
        changed_files: &BTreeSet<String>,
    ) -> Vec<Regression> {
        let mut found = Vec::new();
        found.extend(self.unused_dependencies.regressions_with_limit(
            None,
            Metric::UnusedDependencies.key(),
            limits.limit_for_dependency(Metric::UnusedDependencies.key()),
            changed_files,
        ));
        found.extend(self.unlisted_dependencies.regressions_with_limit(
            None,
            Metric::UnlistedDependencies.key(),
            limits.limit_for_dependency(Metric::UnlistedDependencies.key()),
            changed_files,
        ));
        found.extend(self.dev_in_production_dependencies.regressions_with_limit(
            None,
            Metric::DevInProductionDependencies.key(),
            limits.limit_for_dependency(Metric::DevInProductionDependencies.key()),
            changed_files,
        ));
        found
    }
}

impl<T> MetricChanges<T> {
    fn regressions(
        &self,
        source_set: SourceSet,
        metric_key: &str,
        limits: &RegressionLimits,
        changed_files: &BTreeSet<String>,
    ) -> Vec<Regression> {
        self.regressions_with_limit(
            Some(source_set.key()),
            metric_key,
            limits.limit_for_key(source_set, metric_key),
            changed_files,
        )
    }

    fn regressions_with_limit(
        &self,
        source_set: Option<&'static str>,
        metric_key: &str,
        limit: RegressionLimit,
        changed_files: &BTreeSet<String>,
    ) -> Vec<Regression> {
        let mut found: Vec<Regression> = REGRESSION_RULES
            .into_iter()
            .filter(|rule| self.breaks(*rule, limit))
            .map(|rule| {
                let (base, head) = self.compared_values(rule);
                Regression {
                    source_set: source_set.map(str::to_owned),
                    file: None,
                    metric: metric_key.to_owned(),
                    rule,
                    base,
                    head,
                    new_findings: self.new_findings(),
                    ratio_rise_limit: limit.ratio_rise.ratio(),
                }
            })
            .collect();
        found.extend(self.per_file_regressions_with_limit(
            source_set,
            metric_key,
            limit,
            changed_files,
        ));
        found
    }

    fn per_file_regressions_with_limit(
        &self,
        source_set: Option<&'static str>,
        metric_key: &str,
        limit: RegressionLimit,
        changed_files: &BTreeSet<String>,
    ) -> Vec<Regression> {
        if limit.new_findings == NewFindingsRule::Never || changed_files.is_empty() {
            return Vec::new();
        }
        let mut found = Vec::new();
        for file in changed_files {
            let base = self
                .base_file_numerators
                .get(file)
                .copied()
                .unwrap_or(Decimal(0.0));
            let head = self
                .head_file_numerators
                .get(file)
                .copied()
                .unwrap_or(Decimal(0.0));
            let new_findings = self.new_findings_by_file.get(file).copied().unwrap_or(0);
            let rule = per_file_regression_rule(base, head, new_findings, limit.new_findings);
            if let Some(rule) = rule {
                found.push(Regression {
                    source_set: source_set.map(str::to_owned),
                    file: Some(file.clone()),
                    metric: metric_key.to_owned(),
                    rule,
                    base,
                    head,
                    new_findings,
                    ratio_rise_limit: limit.ratio_rise.ratio(),
                });
            }
        }
        found
    }

    pub(super) fn breaks(&self, rule: RegressionRule, limit: RegressionLimit) -> bool {
        let adds_findings = self.new_findings() > 0;
        let numerator_rises =
            Decimal::rounded(self.head_numerator.0).0 > Decimal::rounded(self.base_numerator.0).0;
        match rule {
            RegressionRule::RatioRise => !limit
                .ratio_rise
                .admits(Decimal::rounded(self.head_ratio.0 - self.base_ratio.0).0),
            RegressionRule::NewFindings => {
                adds_findings && limit.new_findings == NewFindingsRule::AnyNewFinding
            }
            RegressionRule::NumeratorRise => {
                numerator_rises
                    && matches!(
                        limit.new_findings,
                        NewFindingsRule::NumeratorRise | NewFindingsRule::AnyNewFinding
                    )
            }
            RegressionRule::PerFileNumeratorRise | RegressionRule::PerFileNewFindings => false,
        }
    }

    pub(super) fn new_findings(&self) -> usize {
        self.new.listed.len().saturating_add(self.new.left_out)
    }

    fn compared_values(&self, rule: RegressionRule) -> (Decimal, Decimal) {
        match rule {
            RegressionRule::RatioRise | RegressionRule::NewFindings => {
                (Decimal(self.base_ratio.0), Decimal(self.head_ratio.0))
            }
            RegressionRule::NumeratorRise
            | RegressionRule::PerFileNumeratorRise
            | RegressionRule::PerFileNewFindings => (
                Decimal::rounded(self.base_numerator.0),
                Decimal::rounded(self.head_numerator.0),
            ),
        }
    }
}

fn per_file_regression_rule(
    base: Decimal,
    head: Decimal,
    new_findings: usize,
    rule: NewFindingsRule,
) -> Option<RegressionRule> {
    if head.0 > base.0 {
        Some(RegressionRule::PerFileNumeratorRise)
    } else if rule == NewFindingsRule::AnyNewFinding && new_findings > 0 {
        Some(RegressionRule::PerFileNewFindings)
    } else {
        None
    }
}

impl Regression {
    pub(super) fn markdown_item(&self) -> String {
        match self.rule {
            RegressionRule::RatioRise | RegressionRule::NewFindings => {
                crate::markdown::sentence_item(self)
            }
            RegressionRule::NumeratorRise => format!(
                "The {} numerator {} rose from {} to {}.",
                self.scope_prose(),
                crate::markdown::code(numerator_key(&self.metric)),
                self.base.0,
                self.head.0
            ),
            RegressionRule::PerFileNumeratorRise => format!(
                "File {} regressed in {}: {} rose from {} to {}.",
                crate::markdown::code(self.file.as_deref().unwrap_or("")),
                self.scope_prose(),
                crate::markdown::code(numerator_key(&self.metric)),
                self.base.0,
                self.head.0
            ),
            RegressionRule::PerFileNewFindings => format!(
                "File {} regressed in {}: {} ({} from {} to {}).",
                crate::markdown::code(self.file.as_deref().unwrap_or("")),
                self.scope_prose(),
                new_findings_phrase(self.new_findings),
                crate::markdown::code(numerator_key(&self.metric)),
                self.base.0,
                self.head.0
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
            RegressionRule::NumeratorRise => write!(
                formatter,
                "the {scope} numerator {} rose from {} to {}",
                numerator_key(&self.metric),
                self.base.0,
                self.head.0
            ),
            RegressionRule::PerFileNumeratorRise => write!(
                formatter,
                "file {} regressed in {scope}: {} rose from {} to {}",
                self.file.as_deref().unwrap_or(""),
                numerator_key(&self.metric),
                self.base.0,
                self.head.0
            ),
            RegressionRule::PerFileNewFindings => write!(
                formatter,
                "file {} regressed in {scope}: {} with {} from {} to {}",
                self.file.as_deref().unwrap_or(""),
                new_findings_phrase(self.new_findings),
                numerator_key(&self.metric),
                self.base.0,
                self.head.0
            ),
        }
    }
}

fn numerator_key(metric_key: &str) -> &'static str {
    Metric::from_key(metric_key).map_or("numerator", Metric::numerator_field)
}

fn new_findings_phrase(count: usize) -> String {
    if count == 1 {
        "1 new Finding".to_owned()
    } else {
        format!("{count} new Findings")
    }
}
