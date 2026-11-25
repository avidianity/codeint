use std::collections::{BTreeMap, BTreeSet};

use codeint_config::{
    Budgets, CalendarDay, CliSchema, Codeowners, DependenciesConfig, DuplicationConfig,
    FindingsShown, OwnerFilter, RegressionLimits, ReportFormat, ReportRenderFormat, SchemaFormat,
    TrendFormat,
};
use codeint_model::{
    CodeModel, FileModel, HazardLine, LanguageFacts, ReferenceRanges, RepoPath, SkippedFile,
    SourceFile, SourceSet, Token, TokenKind, Workspace, token_text_hash,
};
use serde_json::Value;

use crate::{
    BaseCommit, CommitMeasurement, CommitMeasurementOptions, DiffReport, Head, LanguageSchema,
    MeasuredLanguage, OwnershipOptions, RejectedStatementEntry, Report, ReportExtras,
    ReportOptions, SavedReport, SchemaReport, SqlSchemaSummary, TrendPoint, TrendReport,
};

fn sql_file(
    path: &str,
    source_set: SourceSet,
    source_lines: u32,
    hazards: &[(u32, &str)],
    tokens: Vec<Token>,
) -> SourceFile {
    SourceFile {
        path: RepoPath::new(path).expect("valid path"),
        model: FileModel {
            source_set,
            source_lines,
            functions: Vec::new(),
            tokens,
            flagged_lines: Vec::new(),
            hazard_lines: hazards
                .iter()
                .map(|&(line, rule)| HazardLine {
                    line,
                    rule: rule.to_owned(),
                })
                .collect(),
            runtime_imports: Vec::new(),
            program_start: false,
        },
    }
}

fn plain_file(path: &str, source_set: SourceSet, source_lines: u32) -> SourceFile {
    sql_file(path, source_set, source_lines, &[], Vec::new())
}

fn sql_model(files: Vec<SourceFile>, workspace: Workspace) -> CodeModel {
    CodeModel {
        workspace,
        files,
        languages: vec![LanguageFacts {
            language: "sql".to_owned(),
            has_coverage: false,
            has_hazard_rules: true,
            has_imports: false,
        }],
        ..CodeModel::default()
    }
}

fn rust_model(files: Vec<SourceFile>, workspace: Workspace) -> CodeModel {
    CodeModel {
        workspace,
        files,
        languages: vec![LanguageFacts {
            language: "rust".to_owned(),
            has_coverage: false,
            has_hazard_rules: false,
            has_imports: true,
        }],
        ..CodeModel::default()
    }
}

fn measured_sql(model: CodeModel) -> MeasuredLanguage {
    MeasuredLanguage {
        plugin: "sql".to_owned(),
        model,
        reference_ranges: ReferenceRanges::default(),
        reference_range_note: None,
        has_imports: false,
        framework_profiles: Vec::new(),
        framework_profiles_by_package: BTreeMap::new(),
    }
}

fn measured_rust(model: CodeModel) -> MeasuredLanguage {
    MeasuredLanguage {
        plugin: "rust".to_owned(),
        model,
        reference_ranges: ReferenceRanges::default(),
        reference_range_note: None,
        has_imports: true,
        framework_profiles: Vec::new(),
        framework_profiles_by_package: BTreeMap::new(),
    }
}

fn default_report_options<'a>(
    budgets: &'a Budgets,
    dependencies: &'a DependenciesConfig,
) -> ReportOptions<'a> {
    ReportOptions {
        findings: FindingsShown::default(),
        budgets,
        metric_plugins: &[],
        boundaries: None,
        dependencies,
        dependency_hygiene: None,
        dependency_hygiene_by_language: None,
        duplication: DuplicationConfig::default(),
        orphan_files: None,
        orphan_files_by_language: None,
        ownership: OwnershipOptions::default(),
        coverage_reports_named: false,
        unmatched_coverage_paths: &[],
        baseline: None,
    }
}

fn config_from_json(json_text: &str) -> codeint_config::Configuration {
    use std::sync::atomic::{AtomicU64, Ordering};

    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!(
        "codeint-report-hazards-test-{}-{unique}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    std::fs::write(dir.join("codeint.json"), json_text).expect("write config");
    let loaded = codeint_config::read_configuration(&dir);
    let _ = std::fs::remove_dir_all(&dir);
    loaded.expect("load config")
}

fn find_entry<'a>(array: &'a Value, key: &str, expected: &str) -> &'a Value {
    array
        .as_array()
        .expect("array")
        .iter()
        .find(|entry| entry[key] == expected)
        .expect("matching entry")
}

fn parse_json(text: &str) -> Value {
    serde_json::from_str(text).expect("valid JSON")
}

fn array_len(value: &Value) -> usize {
    value.as_array().expect("JSON array").len()
}

fn assert_hazards_summary(section: &Value, hazard_lines: u64, source_lines: u64, ratio: f64) {
    assert_eq!(section["hazard_lines"], hazard_lines);
    assert_eq!(section["source_lines"], source_lines);
    assert_eq!(section["ratio"], ratio);
}

fn budgets_from_json(json_text: &str) -> Budgets {
    config_from_json(json_text).budgets
}

fn limits_from_json(json_text: &str) -> RegressionLimits {
    config_from_json(json_text).regression_limits
}

#[test]
fn non_hazard_languages_omit_hazards_and_pass_hazards_budget() {
    let budgets = budgets_from_json(r#"{ "budgets": { "production": { "hazards": 0.0 } } }"#);
    let model = rust_model(
        vec![plain_file("src/lib.rs", SourceSet::Production, 50)],
        Workspace::default(),
    );
    let empty_sql = sql_model(Vec::new(), Workspace::default());
    let report = Report::summarize(
        vec![measured_rust(model), measured_sql(empty_sql)],
        Vec::new(),
        FindingsShown::default(),
        &budgets,
    );

    assert!(report.over_budget().is_empty());
    let json: Value =
        serde_json::from_str(&report.render(ReportFormat::Json).expect("json")).expect("parse");
    assert!(
        json["source_sets"]["production"].get("hazards").is_none(),
        "non-hazard repo must omit hazards even when sql plugin has zero files"
    );
    let markdown = report.render(ReportFormat::Markdown).expect("markdown");
    assert!(!markdown.contains("Hazards"));
    let sarif: Value =
        serde_json::from_str(&report.render(ReportFormat::Sarif).expect("sarif")).expect("sarif");
    assert!(
        sarif["runs"][0]["results"]
            .as_array()
            .expect("results")
            .is_empty()
    );
}

#[test]
fn multi_language_hazards_denominator_excludes_languages_without_hazard_rules() {
    let workspace = Workspace::new([
        RepoPath::new("packages/api").expect("api"),
        RepoPath::new("packages/web").expect("web"),
    ]);
    let rust = rust_model(
        vec![
            plain_file("packages/api/src/lib.rs", SourceSet::Production, 100),
            plain_file("packages/api/tests/it.rs", SourceSet::Test, 50),
            plain_file("packages/web/src/app.rs", SourceSet::Production, 80),
        ],
        workspace.clone(),
    );
    let sql = sql_model(
        vec![
            sql_file(
                "packages/api/db/query.sql",
                SourceSet::Production,
                10,
                &[(2, "select-star"), (2, "order-by-random"), (5, "drop")],
                Vec::new(),
            ),
            sql_file(
                "packages/api/db/dump.sql",
                SourceSet::Generated,
                4,
                &[(1, "truncate")],
                Vec::new(),
            ),
        ],
        workspace,
    );
    let report = Report::summarize(
        vec![measured_rust(rust), measured_sql(sql)],
        Vec::new(),
        FindingsShown::default(),
        &Budgets::default(),
    );

    let json = parse_json(&report.render(ReportFormat::Json).expect("json"));
    let prod_hazards = &json["source_sets"]["production"]["hazards"];
    assert_hazards_summary(prod_hazards, 2, 10, 0.2);
    assert_eq!(
        array_len(&prod_hazards["findings"]),
        3,
        "same line with two rules counts as one hazard line and two findings"
    );
    assert!(
        json["source_sets"]["test"].get("hazards").is_none(),
        "test source set has only rust files so hazards must be absent"
    );
    assert_hazards_summary(&json["source_sets"]["generated"]["hazards"], 1, 4, 0.25);

    assert!(
        json["languages"]["rust"]["source_sets"]["production"]
            .get("hazards")
            .is_none()
    );
    assert_eq!(
        json["languages"]["sql"]["source_sets"]["production"]["hazards"]["ratio"],
        0.2
    );

    let packages = &json["source_sets"]["production"]["packages"];
    let api_pkg = find_entry(packages, "path", "packages/api");
    assert_eq!(api_pkg["hazards"]["hazard_lines"], 2);
    assert_eq!(api_pkg["hazards"]["source_lines"], 10);
    assert_eq!(api_pkg["hazards"]["ratio"], 0.2);
    let web_pkg = find_entry(packages, "path", "packages/web");
    assert!(web_pkg.get("hazards").is_none());

    let markdown = report.render(ReportFormat::Markdown).expect("markdown");
    assert!(
        markdown.contains("| Production | Hazards | 0.2 | 2 of 10 Hazard lines | not applicable |")
    );
    assert!(markdown.contains("### Hazards"));
    assert!(markdown.contains("| Path | Line | Rule |"));
    assert!(markdown.contains("| `packages/api/db/query.sql` | 2 | select-star |"));
    assert!(markdown.contains("| `packages/web` |"));
    assert!(markdown.contains("| not measured |"));
}

#[test]
fn disabled_hazard_rules_and_budgets_equal_and_over() {
    let model = sql_model(
        vec![sql_file(
            "db/queries.sql",
            SourceSet::Production,
            10,
            &[(2, "select-star"), (6, "delete-without-filter")],
            Vec::new(),
        )],
        Workspace::default(),
    );
    let dependencies = DependenciesConfig::default();

    let equal_budget = budgets_from_json(r#"{ "budgets": { "production": { "hazards": 0.2 } } }"#);
    let equal_report = Report::summarize_with_extras(
        vec![measured_sql(model.clone())],
        Vec::new(),
        default_report_options(&equal_budget, &dependencies),
        ReportExtras::default(),
    )
    .expect("equal budget report");
    assert!(equal_report.over_budget().is_empty());

    let tighter_budget =
        budgets_from_json(r#"{ "budgets": { "production": { "hazards": 0.1 } } }"#);
    let over_report = Report::summarize_with_extras(
        vec![measured_sql(model.clone())],
        Vec::new(),
        default_report_options(&tighter_budget, &dependencies),
        ReportExtras::default(),
    )
    .expect("over budget report");
    assert_eq!(over_report.over_budget().len(), 1);
    assert!(
        over_report.over_budget()[0]
            .to_string()
            .contains("production hazards ratio 0.2 is over its Budget 0.1")
    );

    let disabled = BTreeSet::from(["select-star".to_owned()]);
    let disabled_report = Report::summarize_with_extras(
        vec![measured_sql(model)],
        Vec::new(),
        default_report_options(&tighter_budget, &dependencies),
        ReportExtras::default().with_disabled_hazard_rules(disabled),
    )
    .expect("disabled rule report");
    assert!(disabled_report.over_budget().is_empty());
    let json: Value =
        serde_json::from_str(&disabled_report.render(ReportFormat::Json).expect("json"))
            .expect("parse");
    assert_eq!(json["source_sets"]["production"]["hazards"]["ratio"], 0.1);
    assert_eq!(
        json["source_sets"]["production"]["hazards"]["findings"]
            .as_array()
            .map(Vec::len),
        Some(1)
    );
}

#[test]
fn rejected_statements_sql_schemas_templated_skipped_and_zero_secret_leakage() {
    let secret = "SUPER_SECRET_PASSWORD_987654";
    let mut model = sql_model(
        vec![sql_file(
            "db/auth.sql",
            SourceSet::Production,
            5,
            &[(3, "plain-text-secret")],
            vec![Token {
                kind: TokenKind::Literal,
                text_hash: token_text_hash(secret),
                line: 3,
            }],
        )],
        Workspace::default(),
    );
    model.skipped.push(SkippedFile {
        path: RepoPath::new("db/template.sql").expect("path"),
        reason: "templated SQL".to_owned(),
    });
    let budgets = Budgets::default();
    let dependencies = DependenciesConfig::default();
    let codeowners = Codeowners::parse("db/* @data-team\n");
    let owner_filter = OwnerFilter::new("@data-team").expect("owner filter");
    let mut options = default_report_options(&budgets, &dependencies);
    options.ownership = OwnershipOptions {
        codeowners: Some(&codeowners),
        owner_filter: Some(&owner_filter),
    };
    let extras = ReportExtras::default()
        .with_rejected_statements(vec![RejectedStatementEntry {
            path: "db/broken.sql".to_owned(),
            line: 14,
            dialect: "postgresql".to_owned(),
        }])
        .with_sql_schemas(vec![
            SqlSchemaSummary {
                package: None,
                dialect: "postgresql".to_owned(),
                tables: 4,
            },
            SqlSchemaSummary {
                package: Some("packages/billing".to_owned()),
                dialect: "generic".to_owned(),
                tables: 2,
            },
        ]);

    let report =
        Report::summarize_with_extras(vec![measured_sql(model)], Vec::new(), options, extras)
            .expect("summarize");

    let json_text = report.render(ReportFormat::Json).expect("json");
    let toon_text = report.render(ReportFormat::Toon).expect("toon");
    let md_text = report.render(ReportFormat::Markdown).expect("markdown");
    let sarif_text = report.render(ReportFormat::Sarif).expect("sarif");

    for (label, output) in [
        ("json", &json_text),
        ("toon", &toon_text),
        ("markdown", &md_text),
        ("sarif", &sarif_text),
    ] {
        assert!(!output.contains(secret), "{label} leaked secret text");
    }

    let json: Value = serde_json::from_str(&json_text).expect("parse json");
    assert_eq!(json["rejected_statements"][0]["path"], "db/broken.sql");
    assert_eq!(json["rejected_statements"][0]["line"], 14);
    assert_eq!(json["rejected_statements"][0]["dialect"], "postgresql");
    assert!(json["sql_schemas"][0].get("package").is_none());
    assert_eq!(json["sql_schemas"][0]["dialect"], "postgresql");
    assert_eq!(json["sql_schemas"][0]["tables"], 4);
    assert_eq!(json["sql_schemas"][1]["package"], "packages/billing");
    assert_eq!(json["skipped"][0]["reason"], "templated SQL");

    let finding = &json["source_sets"]["production"]["hazards"]["findings"][0];
    let finding_keys: BTreeSet<&str> = finding
        .as_object()
        .expect("finding object")
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(
        finding_keys,
        BTreeSet::from(["id", "path", "line", "rule", "owners"])
    );

    assert!(md_text.contains("| Path | Line | Rule | Owners |"));
    assert!(md_text.contains("| `db/auth.sql` | 3 | plain-text-secret | `@data-team` |"));
    assert!(md_text.contains("## Rejected statements"));
    assert!(md_text.contains("- `db/broken.sql:14`: dialect `postgresql`"));
    assert!(md_text.contains("## SQL schemas"));
    assert!(md_text.contains("| root | `postgresql` | 4 |"));
    assert!(md_text.contains("| `packages/billing` | `generic` | 2 |"));
    assert!(md_text.contains("- `db/template.sql`: templated SQL"));

    let saved = SavedReport::from_json(&json_text).expect("saved report round trip");
    assert_eq!(
        saved.render(ReportRenderFormat::Markdown).expect("md"),
        md_text
    );
    assert_eq!(
        saved.render(ReportRenderFormat::Sarif).expect("sarif"),
        sarif_text
    );
}

#[test]
fn diff_and_trend_measure_hazards_only_on_hazard_languages_with_limits_budgets_and_sarif() {
    let rust = rust_model(
        vec![plain_file("src/main.rs", SourceSet::Production, 100)],
        Workspace::default(),
    );
    let base_sql = sql_model(
        vec![sql_file(
            "db/q.sql",
            SourceSet::Production,
            10,
            &[(2, "select-star")],
            vec![Token {
                kind: TokenKind::Other,
                text_hash: token_text_hash("select"),
                line: 2,
            }],
        )],
        Workspace::default(),
    );
    let head_sql = sql_model(
        vec![sql_file(
            "db/q.sql",
            SourceSet::Production,
            10,
            &[(2, "select-star"), (5, "delete-without-filter")],
            vec![
                Token {
                    kind: TokenKind::Other,
                    text_hash: token_text_hash("select"),
                    line: 2,
                },
                Token {
                    kind: TokenKind::Other,
                    text_hash: token_text_hash("delete"),
                    line: 5,
                },
            ],
        )],
        Workspace::default(),
    );
    let dependencies = DependenciesConfig::default();
    let empty_disabled = BTreeSet::new();
    let measure = |models: Vec<CodeModel>, disabled: &BTreeSet<String>| {
        CommitMeasurement::of_languages_with_full_options(
            models,
            CommitMeasurementOptions {
                metric_plugins: &[],
                boundaries: None,
                dependencies: &dependencies,
                duplication: DuplicationConfig::default(),
                orphan_files: None,
                dependency_hygiene: None,
                disabled_hazard_rules: disabled,
            },
        )
        .expect("commit measurement")
    };

    let base_m = measure(vec![rust.clone(), base_sql], &empty_disabled);
    let head_m = measure(vec![rust.clone(), head_sql.clone()], &empty_disabled);
    let budgets = budgets_from_json(r#"{ "budgets": { "production": { "hazards": 0.15 } } }"#);
    let limits = limits_from_json(
        r#"{ "regression_limits": { "hazards": { "ratio_rise": 0.05, "new_findings": "any_new_finding" } } }"#,
    );

    let diff = DiffReport::compare(
        BaseCommit {
            git_ref: "main".to_owned(),
            merge_base: "abc1234".to_owned(),
        },
        Head::WorkingTree,
        base_m,
        head_m,
        FindingsShown::default(),
        &budgets,
        &limits,
    );

    let diff_json_text = diff.render(ReportFormat::Json).expect("diff json");
    let diff_json = parse_json(&diff_json_text);
    let hazards = &diff_json["source_sets"]["production"]["hazards"];
    assert_eq!(
        (
            hazards["base_ratio"].as_f64(),
            hazards["head_ratio"].as_f64(),
            hazards["unchanged"].as_u64(),
            array_len(&hazards["new"]),
            array_len(&hazards["resolved"]),
        ),
        (Some(0.1), Some(0.2), Some(1), 1, 0)
    );
    assert_eq!(hazards["new"][0]["rule"], "delete-without-filter");

    let failures = diff.failure_messages();
    assert!(
        failures
            .iter()
            .any(|msg| msg.contains("production hazards ratio 0.2 is over its Budget 0.15"))
    );
    assert!(
        failures
            .iter()
            .any(|msg| msg.contains("production hazards ratio rose from 0.1 to 0.2"))
    );
    assert!(
        failures
            .iter()
            .any(|msg| msg.contains("production hazards Metric has 1 new Finding"))
    );

    let diff_md = diff.render(ReportFormat::Markdown).expect("diff md");
    assert!(diff_md.contains("| Production | Hazards | 0.1 | 0.2 | 1 | 0 | 1 |"));
    assert!(diff_md.contains("### New Hazards Findings"));
    assert!(diff_md.contains("| `db/q.sql` | 5 | delete-without-filter |"));

    let diff_sarif: Value =
        serde_json::from_str(&diff.render(ReportFormat::Sarif).expect("diff sarif"))
            .expect("parse sarif");
    let sarif_results = diff_sarif["runs"][0]["results"]
        .as_array()
        .expect("sarif results");
    assert_eq!(sarif_results.len(), 1);
    assert_eq!(sarif_results[0]["ruleId"], "delete-without-filter");
    assert_eq!(
        sarif_results[0]["locations"][0]["physicalLocation"]["region"]["startLine"],
        5
    );
    let rule_index = sarif_results[0]["ruleIndex"].as_u64().expect("ruleIndex") as usize;
    assert_eq!(
        diff_sarif["runs"][0]["tool"]["driver"]["rules"][rule_index]["id"],
        "delete-without-filter"
    );

    let saved_diff = SavedReport::from_json(&diff_json_text).expect("saved diff");
    assert_eq!(
        saved_diff.render(ReportRenderFormat::Sarif).expect("sarif"),
        diff.render(ReportFormat::Sarif).expect("sarif")
    );

    let disabled = BTreeSet::from(["delete-without-filter".to_owned()]);
    let disabled_head = measure(vec![rust.clone(), head_sql.clone()], &disabled);
    let disabled_trend = TrendReport::of(vec![
        TrendPoint {
            commit: "1111111".to_owned(),
            committed_on: CalendarDay::containing(1_700_000_000),
            measurement: measure(vec![rust], &empty_disabled),
        },
        TrendPoint {
            commit: "2222222".to_owned(),
            committed_on: CalendarDay::containing(1_700_086_400),
            measurement: disabled_head,
        },
    ]);
    let trend_json: Value = serde_json::from_str(
        &disabled_trend
            .render(TrendFormat::Json)
            .expect("trend json"),
    )
    .expect("parse trend json");
    assert!(
        trend_json["source_sets"]["production"][0]
            .get("hazards")
            .is_none()
    );
    assert_eq!(trend_json["source_sets"]["production"][1]["hazards"], 0.1);
    let trend_md = disabled_trend
        .render(TrendFormat::Markdown)
        .expect("trend md");
    assert!(trend_md.contains("| Hazards |"));
    assert!(trend_md.contains("| `1111111` |"));
    assert!(trend_md.contains("| - |"));
    assert!(trend_md.contains("| 0.1 |"));
}

#[test]
fn schema_report_lists_hazards_metric_and_language_hazard_rules() {
    let schema = SchemaReport::build(
        CliSchema {
            name: "codeint".to_owned(),
            version: "0.6.0".to_owned(),
            description: "Measure entropy and hazards".to_owned(),
            commands: Vec::new(),
            output_formats: Vec::new(),
        },
        vec![
            LanguageSchema {
                name: "rust".to_owned(),
                extensions: vec!["rs".to_owned()],
            },
            LanguageSchema {
                name: "sql".to_owned(),
                extensions: vec!["sql".to_owned()],
            },
        ],
        Vec::new(),
        Vec::new(),
    )
    .with_language_hazard_rules(BTreeMap::from([(
        "sql".to_owned(),
        vec!["select-star".to_owned(), "drop".to_owned()],
    )]));

    let json: Value =
        serde_json::from_str(&schema.render(SchemaFormat::Json).expect("schema json"))
            .expect("parse schema");
    let hazards_metric = find_entry(&json["metrics"], "key", "hazards");
    assert_eq!(hazards_metric["name"], "Hazards");
    assert!(hazards_metric["default_ratio_rise_limit"].is_number());
    assert!(hazards_metric["default_new_findings_rule"].is_string());

    let rust = find_entry(&json["languages"], "name", "rust");
    assert!(rust.get("hazard_rules").is_none());
    let sql = find_entry(&json["languages"], "name", "sql");
    assert_eq!(
        sql["hazard_rules"],
        serde_json::json!(["select-star", "drop"])
    );
}
