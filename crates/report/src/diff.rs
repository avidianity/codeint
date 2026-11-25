use codeint_config::{Budgets, FindingsShown, RegressionLimits, ReportFormat};

use crate::measurement::CommitMeasurement;
use crate::ownership::OwnershipOptions;
use crate::sarif::{ListedDependencies, ListedExternalMetric, ListedFindings, SarifLog};
use crate::{ReportError, TEST_GAP_DIFF_NOTE};

mod classify;
mod markdown;
mod regressions;
#[cfg(test)]
mod tests;
mod types;

pub use types::{BaseCommit, DiffReport, DiffReportOptions, Head, HeadCommit};

use types::{BaseEntry, HeadEntry, SourceSetChangesSections};
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
        let (base_sets, base_dependencies) = base_measurement.into_parts();
        let (head_sets, head_dependencies) = head_measurement.into_parts();
        for ((source_set, base), (_, head)) in base_sets.into_iter().zip(head_sets) {
            if base.files > 0 || head.files > 0 {
                *source_sets.slot(source_set) = Some(classify::source_set_changes(
                    base,
                    head,
                    options.findings,
                    options.ownership,
                ));
            }
        }
        let dependencies = classify::dependencies_changes(
            base_dependencies,
            head_dependencies,
            options.findings,
            options.ownership,
        );
        let mut over_budget_failures = regressions::over_budget(&source_sets, options.budgets);
        if let Some(deps) = &dependencies {
            over_budget_failures.extend(deps.over_budget(options.budgets));
        }
        let mut regression_failures = regressions::regressions(&source_sets, options.limits);
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
            test_gap_note: options
                .include_test_gap_note
                .then(|| TEST_GAP_DIFF_NOTE.to_owned()),
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
    }
}

fn head_entry(head: Head) -> HeadEntry {
    match head {
        Head::WorkingTree => HeadEntry::WorkingTree(WORKING_TREE.to_owned()),
        Head::Commit(HeadCommit { git_ref, commit }) => HeadEntry::Commit { git_ref, commit },
    }
}
