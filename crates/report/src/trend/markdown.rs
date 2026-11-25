use std::collections::BTreeSet;

use super::{TrendReport, TrendRow};
use crate::markdown::{code, prose, table};

const TITLE: &str = "# codeint trend";
const BASE_COLUMNS: [&str; 7] = [
    "Commit",
    "Committed on",
    "Files",
    "Erosion",
    "Cognitive complexity",
    "Duplication",
    "Verbosity",
];

pub(super) fn render(report: &TrendReport) -> String {
    let mut blocks = vec![TITLE.to_owned(), measured_sentence(report.commits)];
    if let Some(note) = &report.test_gap_note {
        blocks.push(prose(note));
    }
    for entry in &report.manifest_warnings {
        for warning in &entry.warnings {
            blocks.push(prose(&format!("Commit {}: {warning}", entry.commit)));
        }
    }
    for (name, rows) in report.source_sets.named() {
        let external_keys: Vec<String> = rows
            .iter()
            .flat_map(|row| row.external_metrics.keys().cloned())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        let show_import_cycles = rows.iter().any(|row| row.import_cycles.is_some());
        let show_hazards = rows.iter().any(|row| row.hazards.is_some());
        let mut columns: Vec<&str> = BASE_COLUMNS.to_vec();
        if show_import_cycles {
            columns.push("Import cycles");
        }
        if show_hazards {
            columns.push("Hazards");
        }
        columns.extend(external_keys.iter().map(String::as_str));
        blocks.push(format!("## {name}"));
        blocks.push(table(
            &columns,
            rows.iter()
                .map(|row| row_cells(row, show_import_cycles, show_hazards, &external_keys))
                .collect(),
        ));
    }
    blocks.join("\n\n")
}

fn measured_sentence(commits: usize) -> String {
    let noun = if commits == 1 { "commit" } else { "commits" };
    format!("codeint measured {commits} {noun} of the main line of HEAD, oldest first.")
}

fn row_cells(
    row: &TrendRow,
    show_import_cycles: bool,
    show_hazards: bool,
    external_keys: &[String],
) -> Vec<String> {
    let mut cells = vec![
        code(&row.commit),
        row.committed_on.clone(),
        row.files.to_string(),
        row.erosion.0.to_string(),
        row.cognitive_complexity.0.to_string(),
        row.duplication.0.to_string(),
        row.verbosity.0.to_string(),
    ];
    if show_import_cycles {
        let value = row
            .import_cycles
            .as_ref()
            .map_or_else(|| "-".to_owned(), |decimal| decimal.0.to_string());
        cells.push(value);
    }
    if show_hazards {
        let value = row
            .hazards
            .as_ref()
            .map_or_else(|| "-".to_owned(), |decimal| decimal.0.to_string());
        cells.push(value);
    }
    for key in external_keys {
        let value = row
            .external_metrics
            .get(key)
            .map_or(0.0, |decimal| decimal.0);
        cells.push(value.to_string());
    }
    cells
}
