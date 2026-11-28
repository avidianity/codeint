use std::collections::BTreeMap;

use codeint_config::RegressionLimit;

use super::{MetricChanges, RegressionRule};
use crate::baseline::BudgetedRatio;
use crate::{Decimal, Findings};

fn ratio_changes(base_ratio: f64, head_ratio: f64) -> MetricChanges<()> {
    let head_ratio = Decimal::rounded(head_ratio);
    MetricChanges {
        metric_version: 1,
        base_ratio: Decimal::rounded(base_ratio),
        head_ratio,
        head_budgeted_ratio: BudgetedRatio::from(head_ratio.0),
        base_numerator: Decimal(1.0),
        head_numerator: Decimal(1.0),
        base_file_numerators: BTreeMap::new(),
        head_file_numerators: BTreeMap::new(),
        new_findings_by_file: BTreeMap::new(),
        new: Findings::listing(Vec::new(), 0),
        resolved: Findings::listing(Vec::new(), 0),
        unchanged: 0,
    }
}

#[test]
fn a_displayed_rise_of_0_01_passes_even_when_the_float_difference_is_above_0_01() {
    let changes = ratio_changes(0.0042, 0.0142);

    assert!(!changes.breaks(RegressionRule::RatioRise, RegressionLimit::DEFAULT));
}

#[test]
fn a_displayed_rise_of_0_0101_is_a_regression() {
    let changes = ratio_changes(0.0042, 0.0143);

    assert!(changes.breaks(RegressionRule::RatioRise, RegressionLimit::DEFAULT));
}

#[test]
fn new_findings_saturates_when_left_out_is_usize_max() {
    let mut changes = ratio_changes(0.0, 0.0);
    changes.new = Findings::listing(vec![()], usize::MAX);

    assert_eq!(changes.new_findings(), usize::MAX);
}

#[test]
fn numerator_rise_compares_to_four_decimal_places_and_respects_rule_modes() {
    use codeint_config::{NewFindingsRule, RatioRise};

    let mut flat_after_rounding = ratio_changes(0.5, 0.5);
    flat_after_rounding.base_numerator = Decimal(10.00001);
    flat_after_rounding.head_numerator = Decimal(10.00004);
    assert!(!flat_after_rounding.breaks(RegressionRule::NumeratorRise, RegressionLimit::DEFAULT));

    let mut rises_without_new_findings = ratio_changes(0.5, 0.5);
    rises_without_new_findings.base_numerator = Decimal(10.0);
    rises_without_new_findings.head_numerator = Decimal(10.0001);
    assert!(
        rises_without_new_findings.breaks(RegressionRule::NumeratorRise, RegressionLimit::DEFAULT)
    );
    assert!(rises_without_new_findings.breaks(
        RegressionRule::NumeratorRise,
        RegressionLimit {
            ratio_rise: RatioRise::DEFAULT,
            new_findings: NewFindingsRule::AnyNewFinding,
        }
    ));
    assert!(!rises_without_new_findings.breaks(
        RegressionRule::NumeratorRise,
        RegressionLimit {
            ratio_rise: RatioRise::DEFAULT,
            new_findings: NewFindingsRule::Never,
        }
    ));
}

fn single_file_model(
    path: &str,
    language: &str,
    has_imports: bool,
    program_start: bool,
) -> (codeint_model::RepoPath, codeint_model::CodeModel) {
    use codeint_model::{CodeModel, FileModel, LanguageFacts, RepoPath, SourceFile, SourceSet};

    let repo_path = RepoPath::new(path).expect("path");
    let model = CodeModel {
        files: vec![SourceFile {
            path: repo_path.clone(),
            model: FileModel {
                source_set: SourceSet::Production,
                source_lines: 2,
                functions: Vec::new(),
                tokens: Vec::new(),
                flagged_lines: Vec::new(),
                hazard_lines: Vec::new(),
                runtime_imports: Vec::new(),
                program_start,
                is_tooling_file: false,
                diagnostics: Vec::new(),
                scope_artifact: None,
            },
        }],
        languages: vec![LanguageFacts {
            language: language.to_owned(),
            has_coverage: false,
            has_hazard_rules: false,
            has_imports,
        }],
        ..CodeModel::default()
    };
    (repo_path, model)
}

#[test]
fn no_imports_only_models_omit_import_cycles_and_dependencies_in_diff_and_trend() {
    use codeint_config::{
        Budgets, CalendarDay, FindingsShown, RegressionLimits, ReportFormat, TrendFormat,
    };
    use codeint_model::{CodeModel, DeclaredDep, DepKind, DependencyManifest, RepoPath};

    use crate::{
        BaseCommit, CommitMeasurement, DiffReport, Head, TraceReport, TrendPoint, TrendReport,
    };

    let (sql_path, no_imports_model) = single_file_model("queries/users.sql", "sql", false, true);
    let unmeasured_js_with_manifest = CodeModel {
        dependency_manifests: vec![DependencyManifest {
            package_root: String::new(),
            path: RepoPath::new("package.json").expect("manifest path"),
        }],
        declared_dependencies: vec![DeclaredDep {
            package_root: String::new(),
            name: "lodash".to_owned(),
            kind: DepKind::Runtime,
        }],
        ..CodeModel::default()
    };

    let measurement = |models: Vec<CodeModel>| {
        CommitMeasurement::of_languages(models, crate::CommitMeasurementOptions::default())
            .expect("commit measurement")
    };
    let base_m = measurement(vec![
        no_imports_model.clone(),
        unmeasured_js_with_manifest.clone(),
    ]);
    let head_m = measurement(vec![
        no_imports_model.clone(),
        unmeasured_js_with_manifest.clone(),
    ]);
    let budgets = Budgets::default();
    let diff = DiffReport::compare(
        BaseCommit {
            git_ref: "main".to_owned(),
            merge_base: "abc".to_owned(),
        },
        Head::WorkingTree,
        base_m,
        head_m,
        FindingsShown::default(),
        &budgets,
        &RegressionLimits::default(),
    );
    assert!(diff.failure_messages().is_empty());
    let diff_json: serde_json::Value =
        serde_json::from_str(&diff.render(ReportFormat::Json).expect("diff json")).expect("json");
    assert!(diff_json.get("dependencies").is_none());
    assert!(
        diff_json["source_sets"]["production"]
            .get("import_cycles")
            .is_none()
    );
    let diff_md = diff.render(ReportFormat::Markdown).expect("diff markdown");
    assert!(!diff_md.contains("Import cycles"));

    let trend = TrendReport::of(vec![TrendPoint {
        commit: "abc1234".to_owned(),
        committed_on: CalendarDay::containing(1_700_000_000),
        measurement: measurement(vec![no_imports_model.clone(), unmeasured_js_with_manifest]),
    }]);
    let trend_json: serde_json::Value =
        serde_json::from_str(&trend.render(TrendFormat::Json).expect("trend json")).expect("json");
    assert!(
        trend_json["source_sets"]["production"][0]
            .get("import_cycles")
            .is_none()
    );
    let trend_md = trend.render(TrendFormat::Markdown).expect("trend markdown");
    assert!(!trend_md.contains("Import cycles"));

    let traced = TraceReport::trace(&no_imports_model, &sql_path, &sql_path, false, 5)
        .expect("trace no-imports");
    assert!(traced.from_entry_point.is_none());
    assert!(traced.to_entry_point.is_none());
}

#[test]
fn trace_omits_program_start_mark_for_non_runtime_tooling_paths() {
    use crate::TraceReport;

    let (script, model) = single_file_model("scripts/main.prog", "program-start", true, true);

    let traced = TraceReport::trace(&model, &script, &script, false, 5).expect("trace");
    assert!(traced.from_entry_point.is_none());
    assert!(traced.to_entry_point.is_none());

    let (config, mut plugin_model) =
        single_file_model("vite.config.prog", "program-start", true, true);
    plugin_model.files[0].model.is_tooling_file = true;

    let traced_plugin =
        TraceReport::trace(&plugin_model, &config, &config, false, 5).expect("trace plugin");
    assert!(traced_plugin.from_entry_point.is_none());
    assert!(traced_plugin.to_entry_point.is_none());
}

#[test]
fn diff_report_evaluates_new_file_budgets_and_round_trips_saved_report() {
    use codeint_config::{
        Budgets, FindingsShown, RegressionLimits, ReportFormat, ReportRenderFormat,
        read_configuration,
    };
    use codeint_model::{
        CodeModel, FileModel, LanguageFacts, RepoPath, SourceFile, SourceSet, language::FlaggedLine,
    };

    use crate::{BaseCommit, CommitMeasurement, DiffReport, DiffReportOptions, Head, SavedReport};

    let dir = std::env::temp_dir().join(format!("codeint-report-nfb-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("mkdir");
    std::fs::write(
        dir.join("codeint.json"),
        r#"{ "new_file_budgets": { "production": { "verbosity": 0.1 } } }"#,
    )
    .expect("write config");
    let config = read_configuration(&dir).expect("read config");
    let _ = std::fs::remove_dir_all(&dir);

    let new_file_model = CodeModel {
        files: vec![SourceFile {
            path: RepoPath::new("src/new.ts").expect("path"),
            model: FileModel {
                source_set: SourceSet::Production,
                source_lines: 4,
                functions: Vec::new(),
                tokens: Vec::new(),
                flagged_lines: vec![FlaggedLine {
                    line: 2,
                    rule: "redundant-else".to_owned(),
                }],
                hazard_lines: Vec::new(),
                runtime_imports: Vec::new(),
                program_start: false,
                is_tooling_file: false,
                diagnostics: Vec::new(),
                scope_artifact: None,
            },
        }],
        languages: vec![LanguageFacts {
            language: "typescript-javascript".to_owned(),
            has_coverage: false,
            has_hazard_rules: false,
            has_imports: true,
        }],
        ..CodeModel::default()
    };

    let budgets = Budgets::default();
    let limits = RegressionLimits::default();
    let diff = DiffReport::compare_with_options(
        BaseCommit {
            git_ref: "main".to_owned(),
            merge_base: "abc".to_owned(),
        },
        Head::WorkingTree,
        CommitMeasurement::of(CodeModel::default()),
        CommitMeasurement::of(new_file_model.clone()),
        DiffReportOptions {
            new_file_budgets: Some(&config.new_file_budgets),
            new_files: Some(CommitMeasurement::of(new_file_model)),
            ..DiffReportOptions::new(FindingsShown::default(), &budgets, &limits)
        },
    );
    assert_eq!(diff.over_new_file_budget().len(), 1);
    assert!(
        diff.failure_messages()
            .iter()
            .any(|msg| msg.contains("new-file production verbosity ratio 0.25"))
    );

    let json_text = diff.render(ReportFormat::Json).expect("json");
    let saved = SavedReport::from_json(&json_text).expect("parse saved diff");
    assert_eq!(
        saved.render(ReportRenderFormat::Markdown).expect("md"),
        diff.render(ReportFormat::Markdown).expect("direct md")
    );
    assert_eq!(
        saved.render(ReportRenderFormat::Sarif).expect("sarif"),
        diff.render(ReportFormat::Sarif).expect("direct sarif")
    );
}
