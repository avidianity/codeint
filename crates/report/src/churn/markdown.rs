use super::{ChurnReport, DuplicationChurnSection, ErosionChurnSection};
use crate::markdown::{code, table};

const TITLE: &str = "# codeint churn";

pub(super) fn render(report: &ChurnReport) -> String {
    let mut blocks = vec![TITLE.to_owned(), summary_line(report)];
    if let Some(warning) = &report.warning {
        blocks.push(format!("> Warning: {warning}"));
    }
    for (name, section) in report.named_source_sets() {
        blocks.push(format!("## {name}"));
        push_erosion_blocks(&mut blocks, &section.erosion);
        push_duplication_blocks(&mut blocks, &section.duplication);
    }
    blocks.join("\n\n")
}

fn summary_line(report: &ChurnReport) -> String {
    let noun = if report.commits_analyzed == 1 {
        "commit"
    } else {
        "commits"
    };
    format!(
        "codeint analyzed {} {noun} (window: `{}`).",
        report.commits_analyzed, report.window
    )
}

fn push_table_section(
    blocks: &mut Vec<String>,
    heading: String,
    empty_note: &str,
    headers: &[&str],
    rows: Vec<Vec<String>>,
) {
    blocks.push(heading);
    if rows.is_empty() {
        blocks.push(empty_note.to_owned());
    } else {
        blocks.push(table(headers, rows));
    }
}

fn push_erosion_blocks(blocks: &mut Vec<String>, section: &ErosionChurnSection) {
    let rows = section
        .findings
        .listed
        .iter()
        .map(|row| {
            vec![
                code(&row.path),
                row.commits.to_string(),
                row.eroded_functions.to_string(),
                row.eroded_function_mass.0.to_string(),
                row.max_cyclomatic_complexity.to_string(),
            ]
        })
        .collect();
    push_table_section(
        blocks,
        format!(
            "### Erosion (ratio {}, mass {}/{})",
            section.ratio.0, section.eroded_function_mass.0, section.total_function_mass.0
        ),
        "No eroded files.",
        &[
            "File",
            "Commits",
            "Eroded functions",
            "Eroded Function mass",
            "Max CC",
        ],
        rows,
    );
}

fn push_duplication_blocks(blocks: &mut Vec<String>, section: &DuplicationChurnSection) {
    let rows = section
        .findings
        .listed
        .iter()
        .map(|row| {
            vec![
                code(&row.path),
                row.commits.to_string(),
                row.clone_lines.to_string(),
                row.clone_groups.to_string(),
            ]
        })
        .collect();
    push_table_section(
        blocks,
        format!(
            "### Duplication (ratio {}, {} clone lines)",
            section.ratio.0, section.clone_lines
        ),
        "No duplicated files.",
        &["File", "Commits", "Clone lines", "Clone groups"],
        rows,
    );
}
