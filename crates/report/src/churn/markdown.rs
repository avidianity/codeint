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
    left_out: usize,
) {
    blocks.push(heading);
    if rows.is_empty() {
        blocks.push(empty_note.to_owned());
    } else {
        blocks.push(table(headers, rows));
    }
    if left_out > 0 {
        let noun = if left_out == 1 { "file" } else { "files" };
        blocks.push(format!(
            "{left_out} more {noun} omitted (pass `--all` to show every file)."
        ));
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
        section.findings.left_out,
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
        section.findings.left_out,
    );
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::churn::{DuplicatedChurnFile, ErodedChurnFile, SourceSetChurnSection, SourceSetKey};
    use crate::{Decimal, Findings};

    #[test]
    fn markdown_emits_truncation_notice_when_findings_are_left_out() {
        let mut source_sets = BTreeMap::new();
        source_sets.insert(
            SourceSetKey::Production,
            SourceSetChurnSection {
                files: 5,
                erosion: ErosionChurnSection {
                    ratio: Decimal(0.5),
                    eroded_function_mass: Decimal(10.0),
                    total_function_mass: Decimal(20.0),
                    findings: Findings::listing(
                        vec![ErodedChurnFile {
                            path: "src/hot.ts".to_owned(),
                            commits: 4,
                            eroded_functions: 1,
                            eroded_function_mass: Decimal(10.0),
                            max_cyclomatic_complexity: 12,
                        }],
                        1,
                    ),
                },
                duplication: DuplicationChurnSection {
                    ratio: Decimal(0.25),
                    clone_lines: 30,
                    findings: Findings::listing(
                        vec![DuplicatedChurnFile {
                            path: "src/dup.ts".to_owned(),
                            commits: 3,
                            clone_lines: 15,
                            clone_groups: 1,
                        }],
                        2,
                    ),
                },
            },
        );
        let report = ChurnReport {
            window: "90d".to_owned(),
            commits_analyzed: 4,
            shallow: false,
            warning: None,
            source_sets,
        };
        let rendered = render(&report);
        assert!(
            rendered.contains("1 more file omitted (pass `--all` to show every file)."),
            "rendered:\n{rendered}"
        );
        assert!(
            rendered.contains("2 more files omitted (pass `--all` to show every file)."),
            "rendered:\n{rendered}"
        );
    }
}
