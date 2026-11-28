use super::types::{
    DependenciesChanges, DiffReport, HeadEntry, MetricChanges, Regression, SourceSetChanges,
};
use crate::markdown::{
    BOUNDARY_FINDING_COLUMNS, CLONE_FAMILY, CLONE_GROUP, COMPLEX_FUNCTION,
    COMPLEX_FUNCTION_COLUMNS, ERODED_FUNCTION, ERODED_FUNCTION_COLUMNS, EXTERNAL_FINDING_COLUMNS,
    FINDING, FLAGGED_LINE, FLAGGED_LINE_COLUMNS, HAZARD_LINE, HAZARD_LINE_COLUMNS, IMPORT_CYCLE,
    IMPORTED_DEPENDENCY_COLUMNS, Noun, ORPHAN_FILE, ORPHAN_FILE_COLUMNS, UNUSED_DEPENDENCY_COLUMNS,
    boundary_finding_row, clone_family_item, clone_group_item, code, complex_function_row,
    eroded_function_row, external_finding_row, findings_table, flagged_line_row, hazard_line_row,
    import_cycle_item, imported_dependency_row, list, orphan_file_row, prose, push_findings,
    sentence_item, table, unused_dependency_row,
};

const TITLE: &str = "# codeint diff";
const METRIC_COLUMNS: [&str; 7] = [
    "Source set",
    "Metric",
    "Base",
    "Head",
    "New",
    "Resolved",
    "Unchanged",
];
const DEPENDENCY_COLUMNS: [&str; 6] = ["Metric", "Base", "Head", "New", "Resolved", "Unchanged"];

fn push_manifest_warnings(blocks: &mut Vec<String>, report: &DiffReport) {
    let warnings = &report.manifest_warnings;
    for (side, side_warnings) in [
        ("Base commit", &warnings.base),
        ("Head commit", &warnings.head),
    ] {
        blocks.extend(
            side_warnings
                .iter()
                .map(|warning| prose(&format!("{side}: {warning}"))),
        );
    }
}

pub(super) fn render(report: &DiffReport) -> String {
    let sections = report.source_sets.named();
    let mut blocks = vec![TITLE.to_string(), comparison_sentence(report)];
    if let Some(note) = &report.test_gap_note {
        blocks.push(prose(note));
    }
    if let Some(note) = &report.ratchet_note {
        blocks.push(prose(note));
    }
    push_manifest_warnings(&mut blocks, report);
    if !sections.is_empty() {
        blocks.push(table(
            &METRIC_COLUMNS,
            sections
                .iter()
                .flat_map(|(name, changes)| metric_rows(name, changes))
                .collect(),
        ));
    }
    if let Some(dependencies) = &report.dependencies {
        blocks.push("## Dependencies".to_string());
        blocks.push(dependency_table(dependencies));
    }
    push_failure_blocks(&mut blocks, report);
    for (name, changes) in &sections {
        let findings = findings_blocks(changes);
        if !findings.is_empty() {
            blocks.push(format!("## {name} changes"));
            blocks.extend(findings);
        }
    }
    if let Some(dependencies) = &report.dependencies {
        let findings = dependency_findings_blocks(dependencies);
        if !findings.is_empty() {
            blocks.push("## Dependency changes".to_string());
            blocks.extend(findings);
        }
    }
    blocks.join("\n\n")
}

fn push_failure_blocks(blocks: &mut Vec<String>, report: &DiffReport) {
    if !report.over_budget.is_empty() {
        blocks.push("## Over Budget".to_string());
        blocks.push(list(report.over_budget.iter().map(sentence_item)));
    }
    if !report.over_new_file_budget.is_empty() {
        blocks.push("## Over New-file Budget".to_string());
        blocks.push(list(report.over_new_file_budget.iter().map(sentence_item)));
    }
    if !report.over_ratchet.is_empty() {
        blocks.push("## Over Ratchet".to_string());
        blocks.push(list(report.over_ratchet.iter().map(sentence_item)));
    }
    if !report.regressions.is_empty() {
        blocks.push("## Regressions".to_string());
        blocks.push(list(
            report.regressions.iter().map(Regression::markdown_item),
        ));
    }
}

fn comparison_sentence(report: &DiffReport) -> String {
    let (head, head_ref) = match &report.head {
        HeadEntry::WorkingTree(label) => (format!("the {label}"), "HEAD".to_owned()),
        HeadEntry::Commit { git_ref, commit } => (
            format!("{} at {}", code(git_ref), code(commit)),
            code(git_ref),
        ),
    };
    format!(
        "codeint compared {head} with {}, the merge-base of {} and {head_ref}.",
        code(&report.base.merge_base),
        code(&report.base.git_ref)
    )
}

fn metric_rows(source_set: &str, changes: &SourceSetChanges) -> Vec<Vec<String>> {
    changes
        .metric_summaries()
        .into_iter()
        .map(|summary| {
            vec![
                source_set.to_string(),
                summary.label.to_owned(),
                summary.base_ratio.0.to_string(),
                summary.head_ratio.0.to_string(),
                summary.new_count.to_string(),
                summary.resolved_count.to_string(),
                summary.unchanged.to_string(),
            ]
        })
        .collect()
}

fn findings_blocks(changes: &SourceSetChanges) -> Vec<String> {
    let mut blocks = Vec::new();
    push_changes(
        &mut blocks,
        "Erosion",
        &changes.erosion,
        &ERODED_FUNCTION,
        |listed| findings_table(&ERODED_FUNCTION_COLUMNS, listed, eroded_function_row),
    );
    push_changes(
        &mut blocks,
        "Cognitive complexity",
        &changes.cognitive_complexity,
        &COMPLEX_FUNCTION,
        |listed| findings_table(&COMPLEX_FUNCTION_COLUMNS, listed, complex_function_row),
    );
    push_changes(
        &mut blocks,
        "Duplication",
        &changes.duplication.groups,
        &CLONE_GROUP,
        |listed| list(listed.iter().map(clone_group_item)),
    );
    if let Some(clone_families) = &changes.duplication.clone_families {
        push_findings(
            &mut blocks,
            "New Clone families",
            &clone_families.new,
            &CLONE_FAMILY,
            |listed| list(listed.iter().map(clone_family_item)),
        );
        push_findings(
            &mut blocks,
            "Resolved Clone families",
            &clone_families.resolved,
            &CLONE_FAMILY,
            |listed| list(listed.iter().map(clone_family_item)),
        );
    }
    push_changes(
        &mut blocks,
        "Verbosity",
        &changes.verbosity,
        &FLAGGED_LINE,
        |listed| findings_table(&FLAGGED_LINE_COLUMNS, listed, flagged_line_row),
    );
    if let Some(import_cycles) = &changes.import_cycles {
        push_changes(
            &mut blocks,
            "Import cycles",
            import_cycles,
            &IMPORT_CYCLE,
            |listed| list(listed.iter().map(import_cycle_item)),
        );
    }
    if let Some(boundaries) = &changes.boundaries {
        push_changes(&mut blocks, "Boundaries", boundaries, &FINDING, |listed| {
            findings_table(&BOUNDARY_FINDING_COLUMNS, listed, boundary_finding_row)
        });
    }
    if let Some(orphan_files) = &changes.orphan_files {
        push_changes(
            &mut blocks,
            "Orphan files",
            orphan_files,
            &ORPHAN_FILE,
            |listed| findings_table(&ORPHAN_FILE_COLUMNS, listed, orphan_file_row),
        );
    }
    if let Some(hazards) = &changes.hazards {
        push_changes(&mut blocks, "Hazards", hazards, &HAZARD_LINE, |listed| {
            findings_table(&HAZARD_LINE_COLUMNS, listed, hazard_line_row)
        });
    }
    for external in changes.external_metrics.values() {
        push_changes(
            &mut blocks,
            &external.name,
            &external.changes,
            &FINDING,
            |listed| findings_table(&EXTERNAL_FINDING_COLUMNS, listed, external_finding_row),
        );
    }
    blocks
}

fn dependency_table(changes: &DependenciesChanges) -> String {
    table(
        &DEPENDENCY_COLUMNS,
        changes
            .metric_summaries()
            .into_iter()
            .map(|summary| {
                vec![
                    summary.label.to_owned(),
                    summary.base_ratio.0.to_string(),
                    summary.head_ratio.0.to_string(),
                    summary.new_count.to_string(),
                    summary.resolved_count.to_string(),
                    summary.unchanged.to_string(),
                ]
            })
            .collect(),
    )
}

fn dependency_findings_blocks(changes: &DependenciesChanges) -> Vec<String> {
    let mut blocks = Vec::new();
    push_changes(
        &mut blocks,
        "Unused dependencies",
        &changes.unused_dependencies,
        &FINDING,
        |listed| findings_table(&UNUSED_DEPENDENCY_COLUMNS, listed, unused_dependency_row),
    );
    for (heading, metric_changes) in [
        ("Unlisted dependencies", &changes.unlisted_dependencies),
        (
            "Dev dependencies in production",
            &changes.dev_in_production_dependencies,
        ),
    ] {
        push_changes(&mut blocks, heading, metric_changes, &FINDING, |listed| {
            findings_table(
                &IMPORTED_DEPENDENCY_COLUMNS,
                listed,
                imported_dependency_row,
            )
        });
    }
    blocks
}

fn push_changes<T>(
    blocks: &mut Vec<String>,
    metric: &str,
    changes: &MetricChanges<T>,
    noun: &Noun,
    listed_block: impl Fn(&[T]) -> String,
) {
    push_findings(
        blocks,
        &format!("New {metric} Findings"),
        &changes.new,
        noun,
        &listed_block,
    );
    push_findings(
        blocks,
        &format!("Resolved {metric} Findings"),
        &changes.resolved,
        noun,
        &listed_block,
    );
}
