use super::{DiffReport, HeadEntry, MetricChanges, Regression, SourceSetChanges};
use crate::Findings;
use crate::markdown::{
    BOUNDARY_FINDING_COLUMNS, CLONE_FAMILY, CLONE_GROUP, ERODED_FUNCTION, ERODED_FUNCTION_COLUMNS,
    EXTERNAL_FINDING_COLUMNS, FINDING, FLAGGED_LINE, FLAGGED_LINE_COLUMNS, IMPORT_CYCLE, Noun,
    boundary_finding_row, clone_family_item, clone_group_item, code, eroded_function_row,
    external_finding_row, flagged_line_row, import_cycle_item, list, push_findings, sentence_item,
    table,
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

pub(super) fn render(report: &DiffReport) -> String {
    let sections = report.source_sets.named();
    let mut blocks = vec![TITLE.to_string(), comparison_sentence(report)];
    if !sections.is_empty() {
        blocks.push(table(
            &METRIC_COLUMNS,
            sections
                .iter()
                .flat_map(|(name, changes)| metric_rows(name, changes))
                .collect(),
        ));
    }
    if !report.over_budget.is_empty() {
        blocks.push("## Over Budget".to_string());
        blocks.push(list(report.over_budget.iter().map(sentence_item)));
    }
    if !report.regressions.is_empty() {
        blocks.push("## Regressions".to_string());
        blocks.push(list(
            report.regressions.iter().map(Regression::markdown_item),
        ));
    }
    for (name, changes) in &sections {
        let findings = findings_blocks(changes);
        if !findings.is_empty() {
            blocks.push(format!("## {name} changes"));
            blocks.extend(findings);
        }
    }
    blocks.join("\n\n")
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
    let mut rows = vec![
        metric_row(source_set, "Erosion", &changes.erosion),
        metric_row(source_set, "Duplication", &changes.duplication.groups),
        metric_row(source_set, "Verbosity", &changes.verbosity),
        metric_row(source_set, "Import cycles", &changes.import_cycles),
    ];
    if let Some(boundaries) = &changes.boundaries {
        rows.push(metric_row(source_set, "Boundaries", boundaries));
    }
    for external in changes.external_metrics.values() {
        rows.push(metric_row(source_set, &external.name, &external.changes));
    }
    rows
}

fn metric_row<T>(source_set: &str, metric: &str, changes: &MetricChanges<T>) -> Vec<String> {
    vec![
        source_set.to_string(),
        metric.to_string(),
        changes.base_ratio.0.to_string(),
        changes.head_ratio.0.to_string(),
        count(&changes.new).to_string(),
        count(&changes.resolved).to_string(),
        changes.unchanged.to_string(),
    ]
}

fn count<T>(findings: &Findings<T>) -> usize {
    findings.listed.len() + findings.left_out
}

fn findings_blocks(changes: &SourceSetChanges) -> Vec<String> {
    let mut blocks = Vec::new();
    push_changes(
        &mut blocks,
        "Erosion",
        &changes.erosion,
        &ERODED_FUNCTION,
        |listed| {
            table(
                &ERODED_FUNCTION_COLUMNS,
                listed.iter().map(eroded_function_row).collect(),
            )
        },
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
        |listed| {
            table(
                &FLAGGED_LINE_COLUMNS,
                listed.iter().map(flagged_line_row).collect(),
            )
        },
    );
    push_changes(
        &mut blocks,
        "Import cycles",
        &changes.import_cycles,
        &IMPORT_CYCLE,
        |listed| list(listed.iter().map(import_cycle_item)),
    );
    if let Some(boundaries) = &changes.boundaries {
        push_changes(&mut blocks, "Boundaries", boundaries, &FINDING, |listed| {
            table(
                &BOUNDARY_FINDING_COLUMNS,
                listed.iter().map(boundary_finding_row).collect(),
            )
        });
    }
    for external in changes.external_metrics.values() {
        push_changes(
            &mut blocks,
            &external.name,
            &external.changes,
            &FINDING,
            |listed| {
                table(
                    &EXTERNAL_FINDING_COLUMNS,
                    listed.iter().map(external_finding_row).collect(),
                )
            },
        );
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
