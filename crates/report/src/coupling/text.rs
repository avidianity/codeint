use std::fmt::Write as _;

use super::CouplingReport;

pub(super) fn render(report: &CouplingReport) -> String {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "codeint coupling (window: {}, commits analyzed: {}, excluded bulk commits: {}, max_commit_files: {}, source_sets: {})",
        report.window,
        report.commits_analyzed,
        report.commits_excluded_by_bulk_limit,
        report.max_commit_files,
        report.source_sets.join(", ")
    );
    if let Some(warning) = &report.warning {
        let _ = writeln!(out, "warning: {warning}");
    }
    if report.pairs.listed.is_empty() {
        let _ = writeln!(
            out,
            "  none (min_co_changes={}, min_ratio={})",
            report.min_co_changes, report.min_ratio.0
        );
        return out.trim_end_matches('\n').to_owned();
    }
    for row in &report.pairs.listed {
        let edge = if row.has_direct_import {
            "direct"
        } else {
            "hidden"
        };
        let _ = writeln!(
            out,
            "  {} <-> {}  co_changes={}  ratio={}  commits={}/{}  import={}",
            row.file_a,
            row.file_b,
            row.co_change_count,
            row.coupling_ratio.0,
            row.file_a_commits,
            row.file_b_commits,
            edge
        );
    }
    if report.pairs.left_out > 0 {
        let _ = writeln!(out, "  ... {} more", report.pairs.left_out);
    }
    out.trim_end_matches('\n').to_owned()
}
