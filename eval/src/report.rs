use std::fmt::Write as _;

use crate::audit::AuditCategory;
use crate::corpus::SizeBucket;
use crate::recall::{LanguageRecall, RecallCount};
use crate::review::AuditReview;
use crate::summary::{
    CaseSnapshot, CaseStudyResult, EvaluationSummary, LanguageSummary, OracleStatus, PanelSummary,
    RepositoryResult,
};

const MEBIBYTE: f64 = 1024.0 * 1024.0;
const GIBIBYTE: f64 = 1024.0 * MEBIBYTE;

fn verdict(passed: bool) -> &'static str {
    if passed { "PASS" } else { "FAIL" }
}

fn or_dash<T: ToString>(value: Option<T>) -> String {
    value.map_or_else(|| "-".to_owned(), |value| value.to_string())
}

fn mebibytes(bytes: Option<u64>) -> String {
    or_dash(bytes.map(|bytes| format!("{:.1} MiB", bytes as f64 / MEBIBYTE)))
}

fn seconds(value: Option<f64>) -> String {
    or_dash(value.map(|seconds| format!("{seconds:.3} s")))
}

fn percent(count: &RecallCount) -> String {
    let hundredths = if count.total == 0 {
        10_000
    } else {
        u128::from(count.matched) * 10_000 / u128::from(count.total)
    };
    format!(
        "{}.{:02}% ({}/{})",
        hundredths / 100,
        hundredths % 100,
        count.matched,
        count.total
    )
}

fn bucket_label(bucket: SizeBucket) -> String {
    format!("{bucket:?}").to_lowercase()
}

fn short_commit(commit: &str) -> &str {
    commit.get(..12).unwrap_or(commit)
}

pub(crate) fn render_markdown(summary: &EvaluationSummary) -> String {
    let mut out = String::new();
    write_header(&mut out, summary);
    write_panels(&mut out, &summary.panels);
    write_recall(&mut out, summary);
    write_performance(&mut out, summary);
    write_case_studies(&mut out, &summary.case_studies);
    write_repositories(&mut out, &summary.repositories);
    write_audit(&mut out, summary);
    out
}

fn write_header(out: &mut String, summary: &EvaluationSummary) {
    let machine = &summary.machine;
    let _ = writeln!(out, "# codeint Evaluation\n");
    let _ = writeln!(out, "Result: {}.", verdict(summary.passed));
    let _ = writeln!(out, "Corpus measured on {}.", summary.measured_on);
    let _ = writeln!(out, "Binary: `{}`.\n", summary.codeint_version);
    let _ = writeln!(out, "## Machine profile\n");
    let _ = writeln!(
        out,
        "- Operating system: {} on {}",
        machine.os, machine.arch
    );
    let _ = writeln!(out, "- CPU: {}", or_dash(machine.cpu_model.as_deref()));
    let _ = writeln!(out, "- Logical CPUs: {}", machine.logical_cpus);
    let memory = machine
        .memory_bytes
        .map(|bytes| format!("{:.1} GiB", bytes as f64 / GIBIBYTE));
    let _ = writeln!(out, "- Memory: {}", or_dash(memory));
    let _ = writeln!(
        out,
        "- Runs per repository: {}\n",
        summary.runs_per_repository
    );
}

const METRIC_TITLES: [(&str, &str); 4] = [
    ("erosion", "Erosion"),
    ("verbosity", "Verbosity"),
    ("duplication", "Duplication"),
    ("import_cycles", "Import cycles"),
];

fn panel_title(panel: &PanelSummary) -> String {
    match &panel.panel {
        Some(name) => format!("{} ({name} Framework profile)", panel.language),
        None => panel.language.clone(),
    }
}

fn write_panels(out: &mut String, panels: &[PanelSummary]) {
    if panels.is_empty() {
        return;
    }
    let _ = writeln!(out, "## Reference range panels\n");
    let _ = writeln!(
        out,
        "Each Reference range is the 10th to 90th percentile (linear interpolation) of the Production Source set ratios across the panel.\nThe standard deviation is the population standard deviation.\n"
    );
    for panel in panels {
        let _ = writeln!(
            out,
            "### {}\n\n{} repositories: {} Small, {} Medium, {} Large.\n",
            panel_title(panel),
            panel.repositories,
            panel.small,
            panel.medium,
            panel.large
        );
        let _ = writeln!(
            out,
            "| Metric | Version | Mean | Median | Standard deviation | 10th percentile | 90th percentile | Lowest | Highest |"
        );
        let _ = writeln!(
            out,
            "| --- | --- | --- | --- | --- | --- | --- | --- | --- |"
        );
        for (key, title) in METRIC_TITLES {
            let Some(metric) = panel.metrics.get(key) else {
                continue;
            };
            let d = &metric.distribution;
            let _ = writeln!(
                out,
                "| {title} | {} | {:.3} | {:.3} | {:.3} | {:.3} | {:.3} | {:.3} | {:.3} |",
                metric.version,
                d.mean,
                d.median,
                d.standard_deviation,
                d.p10,
                d.p90,
                d.lowest,
                d.highest
            );
        }
        out.push('\n');
    }
}

fn ratio_cell(snapshot: &CaseSnapshot, key: &str) -> String {
    or_dash(
        snapshot
            .metrics
            .get(key)
            .map(|reading| format!("{:.4}", reading.ratio)),
    )
}

fn write_case_studies(out: &mut String, studies: &[CaseStudyResult]) {
    if studies.is_empty() {
        return;
    }
    let _ = writeln!(out, "## Before-after case studies\n");
    let _ = writeln!(
        out,
        "Each study measures the Production Source set at two pinned commits with the binary above.\nThe report holds commit references and Metric ratios only, never source excerpts.\n"
    );
    for study in studies {
        let _ = writeln!(
            out,
            "### {}\n\n{}\n\nRepository: {}\n",
            study.name, study.change, study.url
        );
        if let Some(error) = &study.error {
            let _ = writeln!(out, "FAIL: {}\n", error.replace('\n', " "));
        }
        let (Some(before), Some(after)) = (&study.before, &study.after) else {
            continue;
        };
        let _ = writeln!(
            out,
            "Before: `{}`; after: `{}`.\n",
            before.commit, after.commit
        );
        let _ = writeln!(out, "| Metric | Before | After |");
        let _ = writeln!(out, "| --- | --- | --- |");
        let _ = writeln!(
            out,
            "| Production lines | {} | {} |",
            or_dash(before.production_lines),
            or_dash(after.production_lines)
        );
        for (key, title) in METRIC_TITLES {
            let _ = writeln!(
                out,
                "| {title} | {} | {} |",
                ratio_cell(before, key),
                ratio_cell(after, key)
            );
        }
        out.push('\n');
    }
}

fn recall_cells(recall: Option<&LanguageRecall>) -> String {
    recall.map_or_else(
        || "- | - | - | - | - | -".to_owned(),
        |recall| {
            format!(
                "{} | {} | {} | {} | {} | {}",
                percent(&recall.functions),
                percent(&recall.decision_points),
                percent(&recall.imports),
                recall.oracle_unparsable_files.len(),
                recall.unexpected_unparsable_files.len(),
                recall.unmeasured_files.len()
            )
        },
    )
}

fn oracle_label(language: &LanguageSummary) -> String {
    match language.oracle_status {
        OracleStatus::Measured => "measured".to_owned(),
        OracleStatus::NotApplicable => "no oracle".to_owned(),
        OracleStatus::Skipped => format!("skipped: {}", or_dash(language.oracle_reason.as_deref())),
    }
}

fn write_recall(out: &mut String, summary: &EvaluationSummary) {
    let _ = writeln!(out, "## Native-parser recall\n");
    let _ = writeln!(
        out,
        "Each bar is {:.0}% recall on functions, decision points, and static imports, with no unexpected unparsable files.\nA file that the oracle parses and codeint did not measure counts as a miss on every function, decision point, and import in it.\n",
        summary.recall_bar * 100.0
    );
    let _ = writeln!(
        out,
        "| Language | Oracle | Functions | Decision points | Static imports | Oracle-unparsable files | Unexpected unparsable files | Unmeasured files | Result |"
    );
    let _ = writeln!(
        out,
        "| --- | --- | --- | --- | --- | --- | --- | --- | --- |"
    );
    for language in &summary.languages {
        let _ = writeln!(
            out,
            "| {} | {} | {} | {} |",
            language.language,
            oracle_label(language),
            recall_cells(language.recall.as_ref()),
            verdict(language.passed)
        );
    }
    out.push('\n');
    write_unmeasured(out, &summary.languages);
}

fn write_unmeasured(out: &mut String, languages: &[LanguageSummary]) {
    for language in languages {
        let Some(recall) = language
            .recall
            .as_ref()
            .filter(|recall| !recall.unmeasured_files.is_empty())
        else {
            continue;
        };
        let _ = writeln!(
            out,
            "Files the {} oracle parses and codeint did not measure count as recall misses:\n",
            language.language
        );
        for path in &recall.unmeasured_files {
            let _ = writeln!(out, "- `{path}`");
        }
        out.push('\n');
    }
}

fn write_performance(out: &mut String, summary: &EvaluationSummary) {
    let _ = writeln!(out, "## Speed and memory\n");
    let _ = writeln!(
        out,
        "| Repository | Language | Declared bucket | Measured bucket | Production lines | Median wall time | Peak RSS | Production lines per second |"
    );
    let _ = writeln!(out, "| --- | --- | --- | --- | --- | --- | --- | --- |");
    for result in &summary.repositories {
        let _ = writeln!(
            out,
            "| {} | {} | {} | {} | {} | {} | {} | {} |",
            result.name,
            result.language,
            bucket_label(result.declared_bucket),
            or_dash(result.measured_bucket.map(bucket_label)),
            or_dash(result.production_lines),
            seconds(result.median_wall_seconds),
            mebibytes(result.peak_rss_bytes),
            or_dash(
                result
                    .production_lines_per_second
                    .map(|rate| format!("{rate:.0}"))
            )
        );
    }
    let performance = &summary.performance;
    let _ = writeln!(
        out,
        "\nLargest repository: {} with {} Production lines, {} median wall time, and {} peak RSS.\n",
        or_dash(performance.largest_repository.as_deref()),
        or_dash(performance.largest_production_lines),
        seconds(performance.largest_median_wall_seconds),
        mebibytes(performance.largest_peak_rss_bytes)
    );
}

fn repository_outcome(result: &RepositoryResult) -> String {
    match &result.error {
        Some(error) => format!("FAIL: {}", error.replace('|', "/").replace('\n', " ")),
        None => format!(
            "{} (exit code {})",
            verdict(result.passed),
            or_dash(result.exit_code)
        ),
    }
}

fn write_repositories(out: &mut String, repositories: &[RepositoryResult]) {
    let _ = writeln!(out, "## Repositories\n");
    let _ = writeln!(out, "| Repository | Commit | Result |");
    let _ = writeln!(out, "| --- | --- | --- |");
    for result in repositories {
        let _ = writeln!(
            out,
            "| [{}]({}) | `{}` | {} |",
            result.name,
            result.url,
            short_commit(&result.commit),
            repository_outcome(result)
        );
    }
    out.push('\n');
}

fn category_title(category: AuditCategory) -> &'static str {
    match category {
        AuditCategory::SourceSets => "Source sets",
        AuditCategory::FlaggedLines => "Flagged lines",
        AuditCategory::OrphanFiles => "Orphan files",
    }
}

fn write_audit_review(out: &mut String, review: Option<&AuditReview>) {
    let Some(review) = review else {
        return;
    };
    let _ = writeln!(
        out,
        "\nReviewer: {}. These are agent-reviewed false positive rates; a person can recheck every verdict in the verdicts file.\n",
        review.reviewer
    );
    let _ = writeln!(
        out,
        "| Category | Sampled | Reviewed | False positives | False positive rate |"
    );
    let _ = writeln!(out, "| --- | --- | --- | --- | --- |");
    for category in &review.categories {
        let _ = writeln!(
            out,
            "| {} | {} | {} | {} | {:.2}% |",
            category_title(category.category),
            category.sampled,
            category.reviewed,
            category.false_positives,
            category.false_positive_percent
        );
    }
}

fn write_audit(out: &mut String, summary: &EvaluationSummary) {
    let _ = writeln!(out, "## Audit sample\n");
    let _ = writeln!(
        out,
        "Each sample is drawn with a seed derived from the pinned commits, so the same corpus gives the same sample."
    );
    write_audit_review(out, summary.audit_review.as_ref());
    for sample in &summary.audit {
        let _ = writeln!(
            out,
            "\n### {}\n\nSeed `{}`; {} of {} Findings.",
            category_title(sample.category),
            sample.seed,
            sample.items.len(),
            sample.population
        );
        if !sample.items.is_empty() {
            out.push('\n');
        }
        for item in &sample.items {
            let _ = writeln!(out, "- `{item}`");
        }
    }
}

#[cfg(test)]
mod tests;
