use std::collections::BTreeSet;

use codeint_config::{
    Baseline, Budgets, CalendarDay, DependenciesConfig, DuplicationConfig, FindingsShown, Ratchet,
    RegressionLimits, ReportFormat, ReportRenderFormat, TrendFormat,
};
use codeint_model::language::{FlaggedLine, Function};
use codeint_model::{
    CodeModel, FileModel, Metric, ReferenceRange, ReferenceRanges, RepoPath, SourceFile, SourceSet,
};

use crate::{
    BaseCommit, CommitMeasurement, DiffReport, DiffReportOptions, Head, MeasuredLanguage,
    OwnershipOptions, Report, ReportExtras, ReportOptions, SavedReport, TrendPoint, TrendReport,
};

fn sample_file(path: &str, decision_points: u32, flagged: bool) -> SourceFile {
    let flagged_lines = if flagged {
        vec![FlaggedLine {
            line: 4,
            rule: "else-after-return".to_owned(),
        }]
    } else {
        Vec::new()
    };
    SourceFile {
        path: RepoPath::new(path).expect("repo path"),
        model: FileModel {
            source_set: SourceSet::Production,
            source_lines: 20,
            functions: vec![Function {
                line: 1,
                end_line: 20,
                decision_points,
                source_lines: 20,
                name: Some("handle".to_owned()),
                cognitive_complexity: decision_points,
                coverage: None,
            }],
            tokens: Vec::new(),
            flagged_lines,
            hazard_lines: Vec::new(),
            runtime_imports: Vec::new(),
            program_start: false,
        },
    }
}

fn single_language_ranges() -> ReferenceRanges {
    ReferenceRanges::new(vec![ReferenceRange {
        metric: Metric::Erosion.key().to_owned(),
        low: 0.0,
        high: 0.15,
        source: "panel".to_owned(),
        metric_version: Metric::Erosion.version(),
    }])
    .expect("valid reference ranges")
}

#[test]
fn check_text_snapshot_shows_status_header_reference_ranges_ratchet_baseline_and_no_ansi() {
    let mut model = CodeModel::default();
    model.files.push(sample_file("src/a.ts", 11, true));

    let budgets = Budgets::default();
    let dependencies = DependenciesConfig::default();
    let baseline_source = Report::summarize(
        vec![MeasuredLanguage::new(
            "typescript-javascript",
            model.clone(),
            single_language_ranges(),
            true,
        )],
        Vec::new(),
        FindingsShown::All,
        &budgets,
    );
    let (baseline, _) = Baseline::update(None, &baseline_source.baseline_metrics(), false);
    let (ratchet, _) = Ratchet::update(
        None,
        &[codeint_config::MeasuredRatchetMetric {
            source_set: Some(SourceSet::Production),
            metric: Metric::Erosion.key().to_owned(),
            ratio: 0.05,
            metric_version: Metric::Erosion.version(),
        }],
        true,
        ".codeint/ratchet.json",
    )
    .expect("ratchet");

    let mut report = Report::summarize_with_extras(
        vec![MeasuredLanguage::new(
            "typescript-javascript",
            model,
            single_language_ranges(),
            true,
        )],
        Vec::new(),
        ReportOptions {
            findings: FindingsShown::default(),
            budgets: &budgets,
            metric_plugins: &[],
            boundaries: None,
            dependencies: &dependencies,
            dependency_hygiene: None,
            dependency_hygiene_by_language: None,
            duplication: DuplicationConfig::default(),
            orphan_files: None,
            orphan_files_by_language: None,
            ownership: OwnershipOptions::default(),
            coverage_reports_named: false,
            unmatched_coverage_paths: &[],
            baseline: Some(&baseline),
        },
        ReportExtras::default(),
    )
    .expect("report");
    report.apply_ratchet(&ratchet).expect("apply ratchet");

    let text = report.render(ReportFormat::Text).expect("render text");
    assert!(!text.contains('\u{1b}'));
    assert!(
        text.starts_with(
            "codeint check: FAIL (1 file, 20 source lines; 1 over Ratchet, 2 hidden by Baseline)"
        ),
        "got:\n{text}"
    );
    assert!(text.contains(
        "Over Ratchet:\n  - the production erosion ratio 1 exceeds its Ratchet ceiling 0.05"
    ));
    assert!(text.contains("Metric                Ratio  Reference  Status"));
    assert!(text.contains("Erosion                   1  0-0.15     FAIL (ratchet 0.05)"));
    assert!(text.contains("Verbosity              0.05  -          ok (1 baselined)"));
    assert!(!text.contains("Production Erosion findings"));

    let json = report.render(ReportFormat::Json).expect("json");
    let saved = SavedReport::from_json(&json).expect("saved");
    assert_eq!(
        saved.render(ReportRenderFormat::Text).expect("saved text"),
        text
    );
}

#[test]
fn diff_and_trend_text_render_aligned_tables_and_respect_all_without_ansi() {
    let mut base_model = CodeModel::default();
    base_model.files.push(sample_file("src/old.ts", 11, false));

    let mut head_model = CodeModel::default();
    head_model.files.push(sample_file("src/old.ts", 2, false));
    head_model.files.push(sample_file("src/new.ts", 13, true));

    let budgets = Budgets::default();
    let limits = RegressionLimits::default();
    let diff = DiffReport::compare_with_options(
        BaseCommit {
            git_ref: "main".to_owned(),
            merge_base: "abcdef1234567890".to_owned(),
        },
        Head::WorkingTree,
        CommitMeasurement::of(base_model.clone()),
        CommitMeasurement::of(head_model.clone()),
        DiffReportOptions {
            findings: FindingsShown::default(),
            budgets: &budgets,
            new_file_budgets: &budgets,
            new_files: None,
            changed_files: BTreeSet::new(),
            limits: &limits,
            ownership: OwnershipOptions::default(),
            include_test_gap_note: false,
        },
    );
    let diff_text = diff.render(ReportFormat::Text).expect("diff text");
    assert!(!diff_text.contains('\u{1b}'));
    assert!(
        diff_text.starts_with("codeint diff: FAIL against main (abcdef1)"),
        "got:\n{diff_text}"
    );
    assert!(diff_text.contains("Metric                Base    Head  New  Resolved  Status"));
    assert!(diff_text.contains("New Production Erosion findings (1):"));
    assert!(diff_text.contains("src/new.ts:1  handle  cc=14"));
    assert!(!diff_text.contains("Resolved Production Erosion findings"));

    let diff_all = DiffReport::compare_with_options(
        BaseCommit {
            git_ref: "main".to_owned(),
            merge_base: "abcdef1234567890".to_owned(),
        },
        Head::WorkingTree,
        CommitMeasurement::of(base_model.clone()),
        CommitMeasurement::of(head_model.clone()),
        DiffReportOptions {
            findings: FindingsShown::All,
            budgets: &budgets,
            new_file_budgets: &budgets,
            new_files: None,
            changed_files: BTreeSet::new(),
            limits: &limits,
            ownership: OwnershipOptions::default(),
            include_test_gap_note: false,
        },
    );
    let diff_all_text = diff_all.render(ReportFormat::Text).expect("diff all text");
    assert!(diff_all_text.contains("Resolved Production Erosion findings (1):"));
    assert!(diff_all_text.contains("src/old.ts:1  handle  cc=12"));

    let trend = TrendReport::of(vec![
        TrendPoint {
            commit: "1111111aaaaaaa".to_owned(),
            committed_on: CalendarDay::containing(1_793_577_600),
            measurement: CommitMeasurement::of(base_model),
        },
        TrendPoint {
            commit: "2222222bbbbbbb".to_owned(),
            committed_on: CalendarDay::containing(1_794_182_400),
            measurement: CommitMeasurement::of(head_model),
        },
    ]);
    let trend_text = trend.render(TrendFormat::Text).expect("trend text");
    assert!(!trend_text.contains('\u{1b}'));
    assert!(trend_text.starts_with("codeint trend (2 commits, oldest first)"));
    assert!(trend_text.contains("1111111"));
    assert!(trend_text.contains("2222222"));
    assert!(!trend_text.contains("1111111aaaaaaa"));
}
