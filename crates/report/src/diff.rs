use std::collections::BTreeSet;

use codeint_config::{
    Budgets, FindingsShown, MeasuredBaselineMetric, MeasuredRatchetMetric, Ratchet,
    RatchetVersionMismatch, RegressionLimits, ReportFormat,
};
use codeint_model::{Metric, SourceSet};

use crate::measurement::CommitMeasurement;
use crate::ownership::OwnershipOptions;
use crate::sarif::{ListedDependencies, ListedExternalMetric, ListedFindings, SarifLog};
use crate::{Decimal, RatchetFailure, ReportError, TEST_GAP_DIFF_NOTE};

mod classify;
mod markdown;
mod regressions;
#[cfg(test)]
mod tests;
mod text;
mod types;

pub(crate) use types::Regression;
pub use types::{
    BaseCommit, DiffReport, DiffReportOptions, Head, HeadCommit, NewFileBudgetFailure,
};

use types::{BaseEntry, HeadEntry, ManifestWarnings, SourceSetChangesSections};
#[cfg(test)]
use types::{MetricChanges, RegressionRule};

const WORKING_TREE: &str = "working tree";

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
        let empty_new_file_budgets = Budgets::default();
        DiffReport::compare_with_options(
            base,
            head,
            base_measurement,
            head_measurement,
            DiffReportOptions {
                findings,
                budgets,
                new_file_budgets: &empty_new_file_budgets,
                new_files: None,
                changed_files: BTreeSet::new(),
                limits,
                ownership: OwnershipOptions::default(),
                baseline: None,
                include_test_gap_note: false,
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
        let manifest_warnings = ManifestWarnings {
            base: base_measurement.manifest_warnings().to_vec(),
            head: head_measurement.manifest_warnings().to_vec(),
        };
        let (base_sets, base_dependencies) = base_measurement.into_parts();
        let (head_sets, head_dependencies) = head_measurement.into_parts();
        for ((source_set, base), (_, head)) in base_sets.into_iter().zip(head_sets) {
            if base.files > 0 || head.files > 0 {
                *source_sets.slot(source_set) = Some(classify::source_set_changes(
                    source_set,
                    base,
                    head,
                    options.findings,
                    options.ownership,
                    options.baseline,
                ));
            }
        }
        let dependencies = classify::dependencies_changes(
            base_dependencies,
            head_dependencies,
            options.findings,
            options.ownership,
            options.baseline,
        );
        let mut over_budget_failures = regressions::over_budget(&source_sets, options.budgets);
        if let Some(deps) = &dependencies {
            over_budget_failures.extend(deps.over_budget(options.budgets));
        }
        let over_new_file_budget_failures =
            regressions::over_new_file_budget(options.new_files, options.new_file_budgets);
        let empty_changed_files = BTreeSet::new();
        let per_file_targets = if options.limits.per_file() {
            &options.changed_files
        } else {
            &empty_changed_files
        };
        let mut regression_failures =
            regressions::regressions(&source_sets, options.limits, per_file_targets);
        if let Some(deps) = &dependencies {
            regression_failures.extend(deps.regressions(options.limits, per_file_targets));
        }
        DiffReport {
            base: BaseEntry {
                git_ref: base.git_ref,
                merge_base: base.merge_base,
            },
            head: head_entry(head),
            over_budget: over_budget_failures,
            over_new_file_budget: over_new_file_budget_failures,
            over_ratchet: Vec::new(),
            regressions: regression_failures,
            source_sets,
            test_gap_note: options
                .include_test_gap_note
                .then(|| TEST_GAP_DIFF_NOTE.to_owned()),
            ratchet_note: None,
            manifest_warnings,
            dependencies,
            show_all: options.findings == FindingsShown::All,
        }
    }

    pub fn over_new_file_budget(&self) -> &[NewFileBudgetFailure] {
        &self.over_new_file_budget
    }

    pub fn over_ratchet(&self) -> &[RatchetFailure] {
        &self.over_ratchet
    }

    pub fn set_ratchet_note(&mut self, note: impl Into<String>) {
        self.ratchet_note = Some(note.into());
    }

    pub fn head_ratchet_metrics(&self) -> Vec<MeasuredRatchetMetric> {
        let mut metrics = Vec::new();
        for (source_set, changes) in self.source_sets.iter() {
            push_diff_source_set_ratchet_metrics(&mut metrics, source_set, changes);
        }
        if let Some(deps) = &self.dependencies {
            crate::ratchet::push_dependency_ratchet_metrics(
                &mut metrics,
                (
                    deps.unused_dependencies.head_ratio.0,
                    deps.unused_dependencies.metric_version,
                ),
                (
                    deps.unlisted_dependencies.head_ratio.0,
                    deps.unlisted_dependencies.metric_version,
                ),
                (
                    deps.dev_in_production_dependencies.head_ratio.0,
                    deps.dev_in_production_dependencies.metric_version,
                ),
            );
        }
        metrics
    }

    pub fn head_baseline_metrics(&self) -> Vec<MeasuredBaselineMetric> {
        self.head_ratchet_metrics()
            .into_iter()
            .map(|item| MeasuredBaselineMetric {
                source_set: item.source_set,
                metric: item.metric,
                metric_version: item.metric_version,
                findings: Vec::new(),
            })
            .collect()
    }

    pub fn apply_ratchet(&mut self, ratchet: &Ratchet) -> Result<(), Vec<RatchetVersionMismatch>> {
        self.over_ratchet =
            crate::ratchet::evaluate_ratchet(ratchet, &self.head_ratchet_metrics())?;
        Ok(())
    }

    pub fn failure_messages(&self) -> Vec<String> {
        self.over_budget
            .iter()
            .map(ToString::to_string)
            .chain(self.over_new_file_budget.iter().map(ToString::to_string))
            .chain(self.over_ratchet.iter().map(ToString::to_string))
            .chain(self.regressions.iter().map(ToString::to_string))
            .collect()
    }

    pub(crate) fn set_show_all(&mut self, show_all: bool) {
        self.show_all = show_all;
    }

    pub fn render(&self, format: ReportFormat) -> Result<String, ReportError> {
        match format {
            ReportFormat::Toon => crate::render_toon(self),
            ReportFormat::Text => Ok(text::render(self)),
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
                        test_gap: &[],
                        hazards: changes
                            .hazards
                            .as_ref()
                            .map_or(&[], |hazards| hazards.new.listed.as_slice()),
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
        .with_new_file_budget_failures(&self.over_new_file_budget)
        .with_ratchet_failures(&self.over_ratchet)
        .with_per_file_regressions(&self.regressions)
    }
}

fn push_diff_source_set_ratchet_metrics(
    metrics: &mut Vec<MeasuredRatchetMetric>,
    source_set: SourceSet,
    changes: &types::SourceSetChanges,
) {
    let mut push = |key: &str, ratio: Decimal, metric_version: u32| {
        metrics.push(MeasuredRatchetMetric {
            source_set: Some(source_set),
            metric: key.to_owned(),
            ratio: ratio.0,
            metric_version,
        });
    };
    push(
        Metric::Erosion.key(),
        changes.erosion.head_ratio,
        changes.erosion.metric_version,
    );
    push(
        Metric::CognitiveComplexity.key(),
        changes.cognitive_complexity.head_ratio,
        changes.cognitive_complexity.metric_version,
    );
    push(
        Metric::Duplication.key(),
        changes.duplication.groups.head_ratio,
        changes.duplication.groups.metric_version,
    );
    push(
        Metric::Verbosity.key(),
        changes.verbosity.head_ratio,
        changes.verbosity.metric_version,
    );
    if let Some(import_cycles) = &changes.import_cycles {
        push(
            Metric::ImportCycles.key(),
            import_cycles.head_ratio,
            import_cycles.metric_version,
        );
    }
    if let Some(boundaries) = &changes.boundaries {
        push(
            Metric::Boundaries.key(),
            boundaries.head_ratio,
            boundaries.metric_version,
        );
    }
    if let Some(orphan_files) = &changes.orphan_files {
        push(
            Metric::OrphanFiles.key(),
            orphan_files.head_ratio,
            orphan_files.metric_version,
        );
    }
    if let Some(hazards) = &changes.hazards {
        push(
            Metric::Hazards.key(),
            hazards.head_ratio,
            hazards.metric_version,
        );
    }
    for (key, external) in &changes.external_metrics {
        push(
            key.as_str(),
            external.changes.head_ratio,
            external.changes.metric_version,
        );
    }
}

fn head_entry(head: Head) -> HeadEntry {
    match head {
        Head::WorkingTree => HeadEntry::WorkingTree(WORKING_TREE.to_owned()),
        Head::Commit(HeadCommit { git_ref, commit }) => HeadEntry::Commit { git_ref, commit },
    }
}
