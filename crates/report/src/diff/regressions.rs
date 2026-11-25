use codeint_config::{Budgets, NewFindingsRule, RegressionLimit, RegressionLimits};
use codeint_model::{Metric, SourceSet};

use super::types::{
    DependenciesChanges, MetricChanges, Regression, RegressionRule, SourceSetChanges,
    SourceSetChangesSections,
};
use crate::{BudgetFailure, Decimal, dependency_over_budget};

const REGRESSION_RULES: [RegressionRule; 3] = [
    RegressionRule::RatioRise,
    RegressionRule::NewFindings,
    RegressionRule::NumeratorRiseWithNewFindings,
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

pub(super) fn regressions(
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
            self.cognitive_complexity.regressions(
                source_set,
                Metric::CognitiveComplexity.key(),
                limits,
            ),
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
            found.extend(boundaries.regressions(source_set, Metric::Boundaries.key(), limits));
        }
        if let Some(orphan_files) = &self.orphan_files {
            found.extend(orphan_files.regressions(source_set, Metric::OrphanFiles.key(), limits));
        }
        if let Some(hazards) = &self.hazards {
            found.extend(hazards.regressions(source_set, Metric::Hazards.key(), limits));
        }
        for (key, external) in &self.external_metrics {
            found.extend(external.changes.regressions(source_set, key, limits));
        }
        found
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

    pub(super) fn regressions(&self, limits: &RegressionLimits) -> Vec<Regression> {
        let mut found = Vec::new();
        found.extend(self.unused_dependencies.regressions_with_limit(
            None,
            Metric::UnusedDependencies.key(),
            limits.limit_for_dependency(Metric::UnusedDependencies.key()),
        ));
        found.extend(self.unlisted_dependencies.regressions_with_limit(
            None,
            Metric::UnlistedDependencies.key(),
            limits.limit_for_dependency(Metric::UnlistedDependencies.key()),
        ));
        found.extend(self.dev_in_production_dependencies.regressions_with_limit(
            None,
            Metric::DevInProductionDependencies.key(),
            limits.limit_for_dependency(Metric::DevInProductionDependencies.key()),
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

    pub(super) fn breaks(&self, rule: RegressionRule, limit: RegressionLimit) -> bool {
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

    pub(super) fn new_findings(&self) -> usize {
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
