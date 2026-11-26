use super::{TrendReport, TrendRow};
use crate::text::{align_table, plural, push_notes_section, short_hash};

pub(super) fn render(report: &TrendReport) -> String {
    let mut blocks = vec![format!(
        "codeint trend ({} {}, oldest first)",
        report.commits,
        plural(report.commits, "commit", "commits")
    )];
    let mut notes = Vec::new();
    if let Some(note) = &report.test_gap_note {
        notes.push(note.clone());
    }
    for entry in &report.manifest_warnings {
        for warning in &entry.warnings {
            notes.push(format!(
                "warning ({}): {warning}",
                short_hash(&entry.commit)
            ));
        }
    }
    push_notes_section(&mut blocks, notes);
    for (name, rows) in report.source_sets.named() {
        blocks.push(source_set_trend_table(name, rows));
    }
    blocks.join("\n\n")
}

fn source_set_trend_table(name: &str, rows: &[TrendRow]) -> String {
    let (show_cycles, show_hazards, external_keys) = TrendReport::column_flags(rows);
    let mut headers: Vec<&str> = vec![
        "Commit",
        "Date",
        "Files",
        "Erosion",
        "Cognitive",
        "Duplication",
        "Verbosity",
    ];
    let mut right_align = vec![false, false, true, true, true, true, true];
    if show_cycles {
        headers.push("Cycles");
        right_align.push(true);
    }
    if show_hazards {
        headers.push("Hazards");
        right_align.push(true);
    }
    for key in &external_keys {
        headers.push(key.as_str());
        right_align.push(true);
    }
    let table_rows: Vec<Vec<String>> = rows
        .iter()
        .map(|row| {
            row.metric_cells(
                short_hash(&row.commit).to_owned(),
                show_cycles,
                show_hazards,
                &external_keys,
            )
        })
        .collect();
    let mut lines = vec![name.to_owned()];
    lines.extend(align_table(&headers, &table_rows, &right_align));
    lines.join("\n")
}
