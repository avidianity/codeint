use std::collections::BTreeSet;

use super::{TrendReport, TrendRow};
use crate::markdown::{code, table};

const TITLE: &str = "# codeint trend";
const BASE_COLUMNS: [&str; 7] = [
    "Commit",
    "Committed on",
    "Files",
    "Erosion",
    "Duplication",
    "Verbosity",
    "Import cycles",
];

pub(super) fn render(report: &TrendReport) -> String {
    let mut blocks = vec![TITLE.to_owned(), measured_sentence(report.commits)];
    for (name, rows) in report.source_sets.named() {
        let external_keys: Vec<String> = rows
            .iter()
            .flat_map(|row| row.external_metrics.keys().cloned())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        let mut columns: Vec<&str> = BASE_COLUMNS.to_vec();
        columns.extend(external_keys.iter().map(String::as_str));
        blocks.push(format!("## {name}"));
        blocks.push(table(
            &columns,
            rows.iter()
                .map(|row| row_cells(row, &external_keys))
                .collect(),
        ));
    }
    blocks.join("\n\n")
}

fn measured_sentence(commits: usize) -> String {
    let noun = if commits == 1 { "commit" } else { "commits" };
    format!("codeint measured {commits} {noun} of the main line of HEAD, oldest first.")
}

fn row_cells(row: &TrendRow, external_keys: &[String]) -> Vec<String> {
    let mut cells = vec![
        code(&row.commit),
        row.committed_on.clone(),
        row.files.to_string(),
        row.erosion.0.to_string(),
        row.duplication.0.to_string(),
        row.verbosity.0.to_string(),
        row.import_cycles.0.to_string(),
    ];
    for key in external_keys {
        let value = row
            .external_metrics
            .get(key)
            .map_or(0.0, |decimal| decimal.0);
        cells.push(value.to_string());
    }
    cells
}
