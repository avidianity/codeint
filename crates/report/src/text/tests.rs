use std::collections::BTreeSet;

use codeint_config::{
    Baseline, Budgets, CalendarDay, DependenciesConfig, DuplicationConfig, FindingsShown, Ratchet,
    RegressionLimits, ReportFormat, ReportRenderFormat, TrendFormat,
};
use codeint_model::language::{FlaggedLine, Function, FunctionCoverage, HazardLine};
use codeint_model::{
    CodeModel, FileModel, ImportResolution, LanguageFacts, Metric, ReferenceRange, ReferenceRanges,
    RepoPath, RuntimeImport, SourceFile, SourceSet, Workspace,
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
            is_tooling_file: false,
            diagnostics: Vec::new(),
            scope_artifact: None,
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
            baseline: None,
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
            baseline: None,
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

#[test]
fn workspace_package_fail_line_includes_failing_non_core_metrics() {
    let make_file = |path: &str, hazards: Vec<HazardLine>, untested: bool| SourceFile {
        path: RepoPath::new(path).expect("repo path"),
        model: FileModel {
            source_set: SourceSet::Production,
            source_lines: 10,
            functions: vec![Function {
                line: 1,
                end_line: 10,
                decision_points: 1,
                source_lines: 10,
                name: Some("run".to_owned()),
                cognitive_complexity: 1,
                coverage: Some(FunctionCoverage {
                    listed_lines: 10,
                    executed_lines: if untested { 0 } else { 10 },
                }),
            }],
            tokens: Vec::new(),
            flagged_lines: Vec::new(),
            hazard_lines: hazards,
            runtime_imports: Vec::new(),
            program_start: false,
            is_tooling_file: false,
            diagnostics: Vec::new(),
            scope_artifact: None,
        },
    };

    let bad_a = RepoPath::new("packages/bad/src/a.ts").expect("a");
    let bad_b = RepoPath::new("packages/bad/src/b.ts").expect("b");
    let model = CodeModel {
        workspace: Workspace::new([
            RepoPath::new("packages/bad").expect("pkg bad"),
            RepoPath::new("packages/ok").expect("pkg ok"),
        ]),
        files: vec![
            make_file(
                "packages/bad/src/a.ts",
                vec![HazardLine {
                    line: 2,
                    rule: "unsafe-query".to_owned(),
                }],
                true,
            ),
            make_file("packages/bad/src/b.ts", Vec::new(), false),
            make_file("packages/ok/src/c.ts", Vec::new(), false),
        ],
        runtime_imports: vec![
            RuntimeImport {
                importer: bad_a.clone(),
                specifier: "./b".to_owned(),
                resolution: ImportResolution::Resolved(vec![bad_b.clone()]),
            },
            RuntimeImport {
                importer: bad_b,
                specifier: "./a".to_owned(),
                resolution: ImportResolution::Resolved(vec![bad_a]),
            },
        ],
        languages: vec![LanguageFacts {
            language: "typescript-javascript".to_owned(),
            has_coverage: true,
            has_hazard_rules: true,
            has_imports: true,
        }],
        ..CodeModel::default()
    };

    let budgets = Budgets::default();
    let dependencies = DependenciesConfig::default();
    let (ratchet, _) = Ratchet::update(
        None,
        &[
            codeint_config::MeasuredRatchetMetric {
                source_set: Some(SourceSet::Production),
                metric: Metric::Hazards.key().to_owned(),
                ratio: 0.0,
                metric_version: Metric::Hazards.version(),
            },
            codeint_config::MeasuredRatchetMetric {
                source_set: Some(SourceSet::Production),
                metric: Metric::ImportCycles.key().to_owned(),
                ratio: 0.0,
                metric_version: Metric::ImportCycles.version(),
            },
            codeint_config::MeasuredRatchetMetric {
                source_set: Some(SourceSet::Production),
                metric: Metric::TestGap.key().to_owned(),
                ratio: 0.0,
                metric_version: Metric::TestGap.version(),
            },
        ],
        true,
        ".codeint/ratchet.json",
    )
    .expect("ratchet");

    let mut report = Report::summarize_with_extras(
        vec![MeasuredLanguage::new(
            "typescript-javascript",
            model,
            ReferenceRanges::default(),
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
            coverage_reports_named: true,
            unmatched_coverage_paths: &[],
            baseline: None,
        },
        ReportExtras::default(),
    )
    .expect("report");
    report.apply_ratchet(&ratchet).expect("apply ratchet");

    let text = report.render(ReportFormat::Text).expect("render text");
    let pkg_line = text
        .lines()
        .find(|line| line.contains("packages/bad (production) [FAIL]:"))
        .unwrap_or_else(|| panic!("missing packages/bad line in:\n{text}"));
    assert!(pkg_line.contains("import_cycles=1"), "line: {pkg_line}");
    assert!(pkg_line.contains("test_gap=0.5"), "line: {pkg_line}");
    assert!(pkg_line.contains("hazards=0.05"), "line: {pkg_line}");
    assert!(!text.contains("packages/ok (production)"));
}
