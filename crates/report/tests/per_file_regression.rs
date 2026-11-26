use std::collections::BTreeSet;

use codeint_config::{Budgets, FindingsShown, RegressionLimits, ReportFormat, ReportRenderFormat};
use codeint_model::{
    CodeModel, FileModel, LanguageFacts, RepoPath, SourceFile, SourceSet, language::FlaggedLine,
};
use codeint_report::{
    BaseCommit, CommitMeasurement, DiffReport, DiffReportOptions, Head, OwnershipOptions,
    SavedReport,
};

fn flagged_source(path: &str, lines: &[u32]) -> SourceFile {
    let flagged_lines = lines
        .iter()
        .map(|&line| FlaggedLine {
            line,
            rule: "redundant-else".to_owned(),
        })
        .collect();
    SourceFile {
        path: RepoPath::new(path).expect("valid path"),
        model: FileModel {
            source_set: SourceSet::Production,
            source_lines: 20,
            functions: Vec::new(),
            tokens: Vec::new(),
            flagged_lines,
            hazard_lines: Vec::new(),
            runtime_imports: Vec::new(),
            program_start: false,
        },
    }
}

fn ts_model(files: Vec<SourceFile>) -> CodeModel {
    CodeModel {
        files,
        languages: vec![LanguageFacts {
            language: "typescript-javascript".to_owned(),
            has_coverage: false,
            has_hazard_rules: false,
            has_imports: true,
        }],
        ..CodeModel::default()
    }
}

#[test]
fn diff_report_evaluates_per_file_regressions_and_round_trips_formats() {
    let base_model = ts_model(vec![
        flagged_source("src/shrank.ts", &[2, 4, 6]),
        flagged_source("src/worsened.ts", &[]),
        flagged_source("src/untouched.ts", &[3]),
    ]);
    let head_model = ts_model(vec![
        flagged_source("src/shrank.ts", &[]),
        flagged_source("src/worsened.ts", &[5]),
        flagged_source("src/untouched.ts", &[3]),
    ]);
    let changed_files = BTreeSet::from(["src/shrank.ts".to_owned(), "src/worsened.ts".to_owned()]);
    let limits = RegressionLimits::default().with_per_file(true);
    let diff = DiffReport::compare_with_options(
        BaseCommit {
            git_ref: "main".to_owned(),
            merge_base: "abc".to_owned(),
        },
        Head::WorkingTree,
        CommitMeasurement::of(base_model),
        CommitMeasurement::of(head_model),
        DiffReportOptions {
            findings: FindingsShown::default(),
            budgets: &Budgets::default(),
            new_file_budgets: &Budgets::default(),
            new_files: None,
            changed_files,
            limits: &limits,
            ownership: OwnershipOptions::default(),
            baseline: None,
            include_test_gap_note: false,
        },
    );
    let failures = diff.failure_messages();
    assert_eq!(failures.len(), 1, "unexpected failures: {failures:?}");
    assert!(
        failures[0].contains("src/worsened.ts")
            && failures[0].contains("verbosity")
            && failures[0].contains("0 to 1"),
        "unexpected failure: {}",
        failures[0]
    );

    let saved =
        SavedReport::from_json(&diff.render(ReportFormat::Json).expect("json")).expect("saved");
    for (render_fmt, direct_fmt) in [
        (ReportRenderFormat::Markdown, ReportFormat::Markdown),
        (ReportRenderFormat::Sarif, ReportFormat::Sarif),
        (ReportRenderFormat::Toon, ReportFormat::Toon),
    ] {
        assert_eq!(
            saved.render(render_fmt).expect("rerender"),
            diff.render(direct_fmt).expect("direct")
        );
    }
}
