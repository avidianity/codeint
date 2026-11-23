use std::fmt::Write as _;

use super::{ChurnReport, DuplicationChurnSection, ErosionChurnSection};

pub(super) fn render(report: &ChurnReport) -> String {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "codeint churn (window: {}, commits analyzed: {})",
        report.window, report.commits_analyzed
    );
    if let Some(warning) = &report.warning {
        let _ = writeln!(out, "warning: {warning}");
    }
    for (name, section) in report.named_source_sets() {
        let _ = writeln!(out, "\n{name} ({} files)", section.files);
        write_erosion(&mut out, &section.erosion);
        write_duplication(&mut out, &section.duplication);
    }
    out.trim_end_matches('\n').to_owned()
}

fn write_erosion(out: &mut String, section: &ErosionChurnSection) {
    let _ = writeln!(
        out,
        "  Erosion (ratio {}, mass {}/{}):",
        section.ratio.0, section.eroded_function_mass.0, section.total_function_mass.0
    );
    if section.findings.listed.is_empty() {
        let _ = writeln!(out, "    none");
        return;
    }
    for row in &section.findings.listed {
        let _ = writeln!(
            out,
            "    {}  commits={}  eroded_functions={}  mass={}  max_cc={}",
            row.path,
            row.commits,
            row.eroded_functions,
            row.eroded_function_mass.0,
            row.max_cyclomatic_complexity
        );
    }
    if section.findings.left_out > 0 {
        let _ = writeln!(out, "    ... {} more", section.findings.left_out);
    }
}

fn write_duplication(out: &mut String, section: &DuplicationChurnSection) {
    let _ = writeln!(
        out,
        "  Duplication (ratio {}, clone_lines {}):",
        section.ratio.0, section.clone_lines
    );
    if section.findings.listed.is_empty() {
        let _ = writeln!(out, "    none");
        return;
    }
    for row in &section.findings.listed {
        let _ = writeln!(
            out,
            "    {}  commits={}  clone_lines={}  clone_groups={}",
            row.path, row.commits, row.clone_lines, row.clone_groups
        );
    }
    if section.findings.left_out > 0 {
        let _ = writeln!(out, "    ... {} more", section.findings.left_out);
    }
}
