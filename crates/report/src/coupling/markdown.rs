use super::CouplingReport;
use crate::markdown::{code, table};

const TITLE: &str = "# codeint coupling";

pub(super) fn render(report: &CouplingReport) -> String {
    let mut blocks = vec![TITLE.to_owned(), summary_line(report)];
    if let Some(warning) = &report.warning {
        blocks.push(format!("> Warning: {warning}"));
    }
    let rows: Vec<Vec<String>> = report
        .pairs
        .listed
        .iter()
        .map(|row| {
            vec![
                code(&row.file_a),
                code(&row.file_b),
                row.co_change_count.to_string(),
                row.coupling_ratio.0.to_string(),
                row.file_a_commits.to_string(),
                row.file_b_commits.to_string(),
                if row.has_direct_import {
                    "yes".to_owned()
                } else {
                    "no (hidden)".to_owned()
                },
            ]
        })
        .collect();
    if rows.is_empty() {
        blocks.push("No coupled file pairs met the configured limits.".to_owned());
    } else {
        blocks.push(table(
            &[
                "File A",
                "File B",
                "Co-changes",
                "Coupling ratio",
                "File A commits",
                "File B commits",
                "Direct import",
            ],
            rows,
        ));
    }
    if report.pairs.left_out > 0 {
        blocks.push(format!(
            "{} additional coupled pairs omitted (pass `--all` to show every pair).",
            report.pairs.left_out
        ));
    }
    blocks.join("\n\n")
}

fn summary_line(report: &CouplingReport) -> String {
    let noun = if report.commits_analyzed == 1 {
        "commit"
    } else {
        "commits"
    };
    format!(
        "codeint analyzed {} {noun} (window: `{}`, excluded bulk commits > {} files: {}, Source sets: `{}`).",
        report.commits_analyzed,
        report.window,
        report.max_commit_files,
        report.commits_excluded_by_bulk_limit,
        report.source_sets.join(", ")
    )
}
