use codeint_model::{Metric, SourceSet};

use super::types::{
    DependenciesChanges, DiffReport, HeadEntry, MetricChanges, RegressionRule, SourceSetChanges,
};
use crate::Findings;
use crate::text::{
    align_table, format_boundary_finding, format_clone_family, format_clone_group,
    format_complex_function, format_eroded_function, format_external_finding, format_flagged_line,
    format_hazard_line, format_import_cycle, format_imported_dependency, format_orphan_file,
    format_unused_dependency, plural, push_bullet_block, push_findings_lines, push_notes_section,
    short_hash, source_set_heading,
};

pub(super) fn render(report: &DiffReport) -> String {
    let mut blocks = vec![diff_header(report)];
    push_diff_failure_blocks(&mut blocks, report);
    for (source_set, changes) in report.source_sets.iter() {
        blocks.push(source_set_diff_table(source_set, changes, report));
    }
    if let Some(dependencies) = &report.dependencies {
        blocks.push(dependencies_diff_table(dependencies, report));
    }
    push_diff_findings(&mut blocks, report);
    push_notes_section(&mut blocks, collect_diff_notes(report));
    blocks.join("\n\n")
}

fn diff_header(report: &DiffReport) -> String {
    let passed = report.over_budget.is_empty()
        && report.over_new_file_budget.is_empty()
        && report.over_ratchet.is_empty()
        && report.regressions.is_empty();
    let status = if passed { "PASS" } else { "FAIL" };
    let target = match &report.head {
        HeadEntry::WorkingTree(_) => format!(
            "against {} ({})",
            report.base.git_ref,
            short_hash(&report.base.merge_base)
        ),
        HeadEntry::Commit { git_ref, commit } => format!(
            "{git_ref} ({}) against {} ({})",
            short_hash(commit),
            report.base.git_ref,
            short_hash(&report.base.merge_base)
        ),
    };
    let counts = diff_failure_summary(report);
    if counts.is_empty() {
        return format!("codeint diff: {status} {target}");
    }
    format!("codeint diff: {status} {target} ({})", counts.join(", "))
}

fn diff_failure_summary(report: &DiffReport) -> Vec<String> {
    let mut parts = Vec::new();
    if !report.regressions.is_empty() {
        let per_file = report
            .regressions
            .iter()
            .filter(|item| is_per_file_rule(item.rule))
            .count();
        let total = report.regressions.len();
        let label = plural(total, "Regression", "Regressions");
        if per_file > 0 {
            parts.push(format!("{total} {label} ({per_file} per-file)"));
        } else {
            parts.push(format!("{total} {label}"));
        }
    }
    if !report.over_budget.is_empty() {
        parts.push(format!("{} over Budget", report.over_budget.len()));
    }
    if !report.over_new_file_budget.is_empty() {
        parts.push(format!(
            "{} over new-file Budget",
            report.over_new_file_budget.len()
        ));
    }
    if !report.over_ratchet.is_empty() {
        parts.push(format!("{} over Ratchet", report.over_ratchet.len()));
    }
    parts
}

fn is_per_file_rule(rule: RegressionRule) -> bool {
    matches!(
        rule,
        RegressionRule::PerFileNumeratorRise | RegressionRule::PerFileNewFindings
    )
}

fn push_diff_failure_blocks(blocks: &mut Vec<String>, report: &DiffReport) {
    push_bullet_block(blocks, "Over Budget", &report.over_budget);
    push_bullet_block(blocks, "Over New-file Budget", &report.over_new_file_budget);
    push_bullet_block(blocks, "Over Ratchet", &report.over_ratchet);
    push_bullet_block(blocks, "Regressions", &report.regressions);
}

fn push_regression_reasons(
    reasons: &mut Vec<&'static str>,
    source_set: Option<&str>,
    metric_key: &str,
    report: &DiffReport,
) {
    let matching = report
        .regressions
        .iter()
        .filter(|item| item.source_set.as_deref() == source_set && item.metric == metric_key);
    let (mut repo, mut per_file) = (false, false);
    for item in matching {
        if is_per_file_rule(item.rule) {
            per_file = true;
        } else {
            repo = true;
        }
    }
    if repo {
        reasons.push("regression");
    }
    if per_file {
        reasons.push("per-file");
    }
}

fn push_limit_reasons(
    reasons: &mut Vec<&'static str>,
    source_set: Option<&str>,
    metric_key: &str,
    report: &DiffReport,
) {
    if report
        .over_budget
        .iter()
        .any(|item| item.source_set.as_deref() == source_set && item.metric == metric_key)
    {
        reasons.push("budget");
    }
    if source_set.is_some_and(|ss| {
        report
            .over_new_file_budget
            .iter()
            .any(|item| item.source_set == ss && item.metric == metric_key)
    }) {
        reasons.push("new-file budget");
    }
    if report
        .over_ratchet
        .iter()
        .any(|item| item.source_set.as_deref() == source_set && item.metric == metric_key)
    {
        reasons.push("ratchet");
    }
}

fn diff_metric_status(source_set: Option<&str>, metric_key: &str, report: &DiffReport) -> String {
    let mut reasons = Vec::new();
    push_regression_reasons(&mut reasons, source_set, metric_key, report);
    push_limit_reasons(&mut reasons, source_set, metric_key, report);
    if reasons.is_empty() {
        return "ok".to_owned();
    }
    format!("FAIL ({})", reasons.join(", "))
}

fn count_findings<T>(findings: &Findings<T>) -> usize {
    findings.listed.len().saturating_add(findings.left_out)
}

fn diff_row<T>(
    source_set: Option<&str>,
    name: &str,
    metric_key: &str,
    changes: &MetricChanges<T>,
    report: &DiffReport,
) -> Vec<String> {
    vec![
        name.to_owned(),
        changes.base_ratio.0.to_string(),
        changes.head_ratio.0.to_string(),
        count_findings(&changes.new).to_string(),
        count_findings(&changes.resolved).to_string(),
        diff_metric_status(source_set, metric_key, report),
    ]
}

fn push_optional_row<T>(
    rows: &mut Vec<Vec<String>>,
    source_set: Option<&str>,
    name: &str,
    metric: Metric,
    changes: Option<&MetricChanges<T>>,
    report: &DiffReport,
) {
    if let Some(changes) = changes {
        rows.push(diff_row(source_set, name, metric.key(), changes, report));
    }
}

fn source_set_diff_table(
    source_set: SourceSet,
    changes: &SourceSetChanges,
    report: &DiffReport,
) -> String {
    let key = Some(source_set.key());
    let mut rows = vec![
        diff_row(
            key,
            "Erosion",
            Metric::Erosion.key(),
            &changes.erosion,
            report,
        ),
        diff_row(
            key,
            "Cognitive complexity",
            Metric::CognitiveComplexity.key(),
            &changes.cognitive_complexity,
            report,
        ),
        diff_row(
            key,
            "Duplication",
            Metric::Duplication.key(),
            &changes.duplication.groups,
            report,
        ),
        diff_row(
            key,
            "Verbosity",
            Metric::Verbosity.key(),
            &changes.verbosity,
            report,
        ),
    ];
    push_optional_row(
        &mut rows,
        key,
        "Import cycles",
        Metric::ImportCycles,
        changes.import_cycles.as_ref(),
        report,
    );
    push_optional_row(
        &mut rows,
        key,
        "Boundaries",
        Metric::Boundaries,
        changes.boundaries.as_ref(),
        report,
    );
    push_optional_row(
        &mut rows,
        key,
        "Orphan files",
        Metric::OrphanFiles,
        changes.orphan_files.as_ref(),
        report,
    );
    push_optional_row(
        &mut rows,
        key,
        "Hazards",
        Metric::Hazards,
        changes.hazards.as_ref(),
        report,
    );
    for (metric_key, external) in &changes.external_metrics {
        rows.push(diff_row(
            key,
            &external.name,
            metric_key.as_str(),
            &external.changes,
            report,
        ));
    }
    let mut lines = vec![source_set_heading(source_set).to_owned()];
    lines.extend(align_table(
        &["Metric", "Base", "Head", "New", "Resolved", "Status"],
        &rows,
        &[false, true, true, true, true, false],
    ));
    lines.join("\n")
}

fn dependencies_diff_table(dependencies: &DependenciesChanges, report: &DiffReport) -> String {
    let rows = vec![
        diff_row(
            None,
            "Unused dependencies",
            Metric::UnusedDependencies.key(),
            &dependencies.unused_dependencies,
            report,
        ),
        diff_row(
            None,
            "Unlisted dependencies",
            Metric::UnlistedDependencies.key(),
            &dependencies.unlisted_dependencies,
            report,
        ),
        diff_row(
            None,
            "Dev dependencies in production",
            Metric::DevInProductionDependencies.key(),
            &dependencies.dev_in_production_dependencies,
            report,
        ),
    ];
    let mut lines = vec!["Dependencies".to_owned()];
    lines.extend(align_table(
        &["Metric", "Base", "Head", "New", "Resolved", "Status"],
        &rows,
        &[false, true, true, true, true, false],
    ));
    lines.join("\n")
}

fn push_change_findings<T>(
    blocks: &mut Vec<String>,
    label: &str,
    changes: &MetricChanges<T>,
    show_all: bool,
    format_item: fn(&T) -> String,
) {
    push_findings_lines(
        blocks,
        &format!("New {label} findings"),
        &changes.new,
        show_all,
        format_item,
    );
    if show_all {
        push_findings_lines(
            blocks,
            &format!("Resolved {label} findings"),
            &changes.resolved,
            true,
            format_item,
        );
    }
}

fn push_diff_findings(blocks: &mut Vec<String>, report: &DiffReport) {
    for (source_set, changes) in report.source_sets.iter() {
        let name = source_set_heading(source_set);
        push_change_findings(
            blocks,
            &format!("{name} Erosion"),
            &changes.erosion,
            report.show_all,
            format_eroded_function,
        );
        push_change_findings(
            blocks,
            &format!("{name} Cognitive complexity"),
            &changes.cognitive_complexity,
            report.show_all,
            format_complex_function,
        );
        if let Some(families) = &changes.duplication.clone_families {
            push_findings_lines(
                blocks,
                &format!("New {name} Clone families"),
                &families.new,
                report.show_all,
                format_clone_family,
            );
            if report.show_all {
                push_findings_lines(
                    blocks,
                    &format!("Resolved {name} Clone families"),
                    &families.resolved,
                    true,
                    format_clone_family,
                );
            }
        }
        push_change_findings(
            blocks,
            &format!("{name} Duplication"),
            &changes.duplication.groups,
            report.show_all,
            format_clone_group,
        );
        push_change_findings(
            blocks,
            &format!("{name} Verbosity"),
            &changes.verbosity,
            report.show_all,
            format_flagged_line,
        );
        push_optional_diff_findings(blocks, name, changes, report.show_all);
    }
    if let Some(dependencies) = &report.dependencies {
        push_change_findings(
            blocks,
            "Unused dependencies",
            &dependencies.unused_dependencies,
            report.show_all,
            format_unused_dependency,
        );
        push_change_findings(
            blocks,
            "Unlisted dependencies",
            &dependencies.unlisted_dependencies,
            report.show_all,
            format_imported_dependency,
        );
        push_change_findings(
            blocks,
            "Dev dependencies in production",
            &dependencies.dev_in_production_dependencies,
            report.show_all,
            format_imported_dependency,
        );
    }
}

fn push_optional_diff_findings(
    blocks: &mut Vec<String>,
    name: &str,
    changes: &SourceSetChanges,
    show_all: bool,
) {
    if let Some(cycles) = &changes.import_cycles {
        push_change_findings(
            blocks,
            &format!("{name} Import cycles"),
            cycles,
            show_all,
            format_import_cycle,
        );
    }
    if let Some(boundaries) = &changes.boundaries {
        push_change_findings(
            blocks,
            &format!("{name} Boundaries"),
            boundaries,
            show_all,
            format_boundary_finding,
        );
    }
    if let Some(orphans) = &changes.orphan_files {
        push_change_findings(
            blocks,
            &format!("{name} Orphan files"),
            orphans,
            show_all,
            format_orphan_file,
        );
    }
    if let Some(hazards) = &changes.hazards {
        let heading = format!("{name} Hazards");
        push_change_findings(blocks, &heading, hazards, show_all, format_hazard_line);
    }
    for external in changes.external_metrics.values() {
        let heading = format!("{name} {}", external.name);
        push_change_findings(
            blocks,
            &heading,
            &external.changes,
            show_all,
            format_external_finding,
        );
    }
}

fn collect_diff_notes(report: &DiffReport) -> Vec<String> {
    let mut notes = Vec::new();
    if let Some(note) = &report.test_gap_note {
        notes.push(note.clone());
    }
    if let Some(note) = &report.ratchet_note {
        notes.push(note.clone());
    }
    for warning in &report.manifest_warnings.base {
        notes.push(format!("base warning: {warning}"));
    }
    for warning in &report.manifest_warnings.head {
        notes.push(format!("head warning: {warning}"));
    }
    notes
}
