use std::collections::BTreeMap;
use std::path::PathBuf;

use codeint_config::{
    Budgets, CalendarDay, DependenciesConfig, FindingsShown, RegressionLimits, ReportFormat,
    TrendFormat,
};
use codeint_model::language::Function;
use codeint_model::{
    CodeModel, FileModel, FunctionCoverage, HazardLine, LanguageFacts, ReferenceRanges, RepoPath,
    SourceFile, SourceSet, Workspace,
};
use codeint_report::{
    BaseCommit, CommitMeasurement, CommitMeasurementOptions, DiffReport, FileDiagnosticEntry, Head,
    MeasuredLanguage, Report, ReportExtras, ReportOptions, ReportSchemaKind, ScopeSummaryEntry,
    TrendPoint, TrendReport, all_report_schemas, report_schema, schema_for,
    validate_json_against_schema, validate_report_json,
};
use serde_json::{Value, json};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn published_report_schema_path() -> PathBuf {
    repo_root().join(ReportSchemaKind::Check.relative_path())
}

fn validate_against_schema(value: &Value, schema: &Value) {
    if let Err(message) = validate_json_against_schema(value, schema) {
        panic!("{message}");
    }
}

fn sample_model(has_coverage: bool, has_hazard_rules: bool) -> CodeModel {
    let coverage = has_coverage.then_some(FunctionCoverage {
        listed_lines: 4,
        executed_lines: 1,
    });
    let hazard_lines = if has_hazard_rules {
        vec![HazardLine {
            line: 2,
            rule: "select-star".to_owned(),
        }]
    } else {
        Vec::new()
    };
    CodeModel {
        workspace: Workspace::new([RepoPath::new("packages/api").expect("pkg")]),
        files: vec![SourceFile {
            path: RepoPath::new("packages/api/src/app.ts").expect("path"),
            model: FileModel {
                source_set: SourceSet::Production,
                source_lines: 6,
                functions: vec![Function {
                    name: Some("compute".to_owned()),
                    line: 1,
                    end_line: 6,
                    source_lines: 6,
                    decision_points: 1,
                    cognitive_complexity: 1,
                    coverage,
                }],
                tokens: Vec::new(),
                flagged_lines: Vec::new(),
                hazard_lines,
                runtime_imports: Vec::new(),
                program_start: false,
                is_tooling_file: false,
                diagnostics: Vec::new(),
                scope_artifact: None,
            },
        }],
        languages: vec![LanguageFacts {
            language: "typescript-javascript".to_owned(),
            has_coverage,
            has_hazard_rules,
            has_imports: true,
        }],
        ..CodeModel::default()
    }
}

#[test]
fn published_report_schema_matches_generated_and_validates_reports_with_and_without_test_gap() {
    let generated = report_schema();
    let path = published_report_schema_path();
    let published = std::fs::read_to_string(&path).unwrap_or_default();
    if published != generated {
        std::fs::write(&path, &generated).expect("write published report schema");
        panic!(
            "{} drifted from report_schema(); it has been rewritten, review and re-run",
            path.display()
        );
    }
    let schema: Value = serde_json::from_str(&published).expect("report schema is JSON");
    let dependencies = DependenciesConfig::default();
    let budgets = Budgets::default();

    let unmatched = ["missing/file.ts".to_owned()];
    let with_gap_options = ReportOptions {
        dependencies: Some(&dependencies),
        coverage_reports_named: true,
        unmatched_coverage_paths: &unmatched,
        extras: ReportExtras::default()
            .with_file_diagnostics(vec![FileDiagnosticEntry {
                path: "packages/api/db/bad.sql".to_owned(),
                line: 9,
                kind: "rejected-statement".to_owned(),
                message: "could not parse statement as postgresql".to_owned(),
            }])
            .with_scope_summaries(vec![ScopeSummaryEntry {
                language: "sql".to_owned(),
                package: Some("packages/api".to_owned()),
                dialect: Some("postgresql".to_owned()),
                kind: "sql-schema".to_owned(),
                fields: BTreeMap::from([
                    ("dialect".to_owned(), "postgresql".to_owned()),
                    ("tables".to_owned(), "3".to_owned()),
                ]),
            }]),
        ..ReportOptions::new(FindingsShown::default(), &budgets)
    };
    let with_gap = Report::summarize_with_options(
        vec![MeasuredLanguage {
            plugin: "typescript-javascript".to_owned(),
            model: sample_model(true, true),
            reference_ranges: ReferenceRanges::default(),
            reference_range_note: None,
            has_imports: true,
            framework_profiles: vec!["react".to_owned()],
            framework_profiles_by_package: BTreeMap::from([(
                "packages/api".to_owned(),
                vec!["react".to_owned()],
            )]),
        }],
        Vec::new(),
        with_gap_options,
    )
    .expect("report with test gap and hazards");
    let with_gap_json: Value =
        serde_json::from_str(&with_gap.render(ReportFormat::Json).expect("json"))
            .expect("valid JSON");
    validate_against_schema(&with_gap_json, &schema);
    assert_eq!(
        with_gap_json["source_sets"]["production"]["test_gap"]["ratio"],
        0.75
    );
    assert_eq!(
        with_gap_json["source_sets"]["production"]["hazards"]["hazard_lines"],
        1
    );
    let api_package = with_gap_json["source_sets"]["production"]["packages"]
        .as_array()
        .expect("packages")
        .iter()
        .find(|pkg| pkg["path"] == "packages/api")
        .expect("packages/api entry");
    assert_eq!(api_package["test_gap"]["ratio"], 0.75);
    assert_eq!(api_package["hazards"]["hazard_lines"], 1);

    let without_gap = Report::summarize(
        vec![MeasuredLanguage {
            plugin: "typescript-javascript".to_owned(),
            model: sample_model(false, false),
            reference_ranges: ReferenceRanges::default(),
            reference_range_note: None,
            has_imports: true,
            framework_profiles: Vec::new(),
            framework_profiles_by_package: BTreeMap::new(),
        }],
        Vec::new(),
        FindingsShown::default(),
        &budgets,
    );
    let without_gap_json: Value =
        serde_json::from_str(&without_gap.render(ReportFormat::Json).expect("json"))
            .expect("valid JSON");
    validate_against_schema(&without_gap_json, &schema);
    assert!(
        without_gap_json["source_sets"]["production"]
            .get("test_gap")
            .is_none()
    );
    assert!(
        without_gap_json["source_sets"]["production"]
            .get("hazards")
            .is_none()
    );
    assert!(without_gap_json["test_gap_note"].is_string());
}

#[test]
fn report_schema_validates_external_package_boundary_findings() {
    let published =
        std::fs::read_to_string(published_report_schema_path()).expect("read report schema");
    let schema: Value = serde_json::from_str(&published).expect("report schema is JSON");
    let config_dir = std::env::temp_dir().join(format!(
        "codeint-report-schema-external-boundaries-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&config_dir).expect("create temp config dir");
    std::fs::write(
        config_dir.join("codeint.json"),
        r#"{
  "boundaries": {
    "zones": {
      "domain": { "patterns": ["packages/api/src/**"] }
    },
    "rules": [
      {
        "from": "domain",
        "deny_external": ["prisma"],
        "reason": "Domain must stay persistence-free"
      }
    ]
  }
}"#,
    )
    .expect("write config");
    let configuration = codeint_config::read_configuration(&config_dir).expect("read config");
    let _ = std::fs::remove_dir_all(&config_dir);
    let mut model = sample_model(false, false);
    model.runtime_imports.push(codeint_model::RuntimeImport {
        importer: RepoPath::new("packages/api/src/app.ts").expect("path"),
        specifier: "prisma".to_owned(),
        resolution: codeint_model::ImportResolution::External("prisma".to_owned()),
    });
    let dependencies = DependenciesConfig::default();
    let budgets = Budgets::default();
    let report = Report::summarize_with_options(
        vec![MeasuredLanguage {
            plugin: "typescript-javascript".to_owned(),
            model,
            reference_ranges: ReferenceRanges::default(),
            reference_range_note: None,
            has_imports: true,
            framework_profiles: Vec::new(),
            framework_profiles_by_package: BTreeMap::new(),
        }],
        Vec::new(),
        ReportOptions {
            boundaries: configuration.boundaries.as_ref(),
            dependencies: Some(&dependencies),
            ..ReportOptions::new(FindingsShown::default(), &budgets)
        },
    )
    .expect("report with external boundary finding");
    let report_json: Value =
        serde_json::from_str(&report.render(ReportFormat::Json).expect("json")).expect("json");
    validate_against_schema(&report_json, &schema);
    let finding = &report_json["source_sets"]["production"]["boundaries"]["findings"][0];
    assert_eq!(finding["kind"], "external_package");
    assert_eq!(finding["external_package"], "prisma");
}

struct VersionedExternalMetricPlugin;

impl codeint_model::MetricPlugin for VersionedExternalMetricPlugin {
    fn key(&self) -> &str {
        "custom_depth"
    }

    fn name(&self) -> &str {
        "Custom depth"
    }

    fn description(&self) -> &str {
        "Sample external Metric with version 3"
    }

    fn version(&self) -> u32 {
        3
    }

    fn measure(
        &self,
        _model: &CodeModel,
    ) -> Result<codeint_model::ExternalMetricMeasurement, codeint_model::MetricPluginError> {
        Ok(codeint_model::ExternalMetricMeasurement {
            ratio: 0.25,
            numerator: 1.0,
            denominator: 4.0,
            findings: Vec::new(),
        })
    }
}

#[test]
fn report_schema_validates_metric_versions_for_built_ins_dependencies_and_external_plugins() {
    let schema: Value = serde_json::from_str(&report_schema()).expect("report schema is JSON");
    let mut model = sample_model(true, true);
    model
        .dependency_manifests
        .push(codeint_model::DependencyManifest {
            package_root: "packages/api".to_owned(),
            path: RepoPath::new("packages/api/package.json").expect("manifest path"),
        });
    model
        .declared_dependencies
        .push(codeint_model::DeclaredDep {
            package_root: "packages/api".to_owned(),
            name: "lodash".to_owned(),
            kind: codeint_model::DepKind::Runtime,
        });
    let plugin = VersionedExternalMetricPlugin;
    let metric_plugins: [&dyn codeint_model::MetricPlugin; 1] = [&plugin];
    let budgets = Budgets::default();
    let report = Report::summarize_with_options(
        vec![MeasuredLanguage::new(
            "typescript-javascript",
            model,
            ReferenceRanges::default(),
            true,
        )],
        Vec::new(),
        ReportOptions {
            metric_plugins: &metric_plugins,
            ..ReportOptions::new(FindingsShown::default(), &budgets)
        },
    )
    .expect("report with external metric plugin");

    let json_text = report.render(ReportFormat::Json).expect("json");
    let report_json: Value = serde_json::from_str(&json_text).expect("valid JSON");
    validate_against_schema(&report_json, &schema);

    let prod = &report_json["source_sets"]["production"];
    for metric in [
        "erosion",
        "cognitive_complexity",
        "import_cycles",
        "hazards",
    ] {
        assert_eq!(prod[metric]["metric_version"], 1, "{metric}");
    }
    assert_eq!(prod["duplication"]["metric_version"], 2);
    assert_eq!(prod["verbosity"]["metric_version"], 2);
    assert_eq!(prod["test_gap"]["metric_version"], 2);
    assert!(prod["verbosity"].get("clone_lines").is_none());
    assert_eq!(prod["custom_depth"]["metric_version"], 3);
    for dep_metric in [
        "unused_dependencies",
        "unlisted_dependencies",
        "dev_in_production_dependencies",
    ] {
        assert_eq!(
            report_json["dependencies"][dep_metric]["metric_version"], 1,
            "{dep_metric}"
        );
    }

    let toon_text = report.render(ReportFormat::Toon).expect("toon");
    assert!(toon_text.contains("metric_version: 1"));
    assert!(toon_text.contains("metric_version: 2"));
    assert!(toon_text.contains("metric_version: 3"));
}

#[test]
fn published_report_schemas_match_generated_output_for_all_commands() {
    let root = repo_root();
    let mut drifted = Vec::new();
    for (kind, generated) in all_report_schemas() {
        let path = root.join(kind.relative_path());
        let published = std::fs::read_to_string(&path).unwrap_or_default();
        if published != generated {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).expect("create schema dir");
            }
            std::fs::write(&path, &generated).expect("write published report schema");
            drifted.push(path.display().to_string());
        }
        let parsed: Value = serde_json::from_str(&generated).expect("schema is valid JSON");
        assert_eq!(
            parsed["$schema"],
            "https://json-schema.org/draft/2020-12/schema"
        );
    }
    assert!(
        drifted.is_empty(),
        "report schemas drifted and were rewritten: {drifted:?}; review and re-run"
    );
}

fn parse_and_validate(kind: ReportSchemaKind, rendered: String) -> Value {
    let schema: Value = serde_json::from_str(&schema_for(kind)).expect("schema");
    let value: Value = serde_json::from_str(&rendered).expect("rendered json");
    validate_against_schema(&value, &schema);
    value
}

fn sample_diff_with_external_metric() -> DiffReport {
    let plugin = VersionedExternalMetricPlugin;
    let metric_plugins: [&dyn codeint_model::MetricPlugin; 1] = [&plugin];
    let budgets = Budgets::default();
    let limits = RegressionLimits::default();
    let options = CommitMeasurementOptions {
        metric_plugins: &metric_plugins,
        ..CommitMeasurementOptions::default()
    };
    let base_measurement =
        CommitMeasurement::of_languages(vec![sample_model(false, true)], options)
            .expect("base measurement");
    let head_measurement =
        CommitMeasurement::of_languages(vec![sample_model(false, true)], options)
            .expect("head measurement");
    DiffReport::compare(
        BaseCommit {
            git_ref: "main".to_owned(),
            merge_base: "abc1234".to_owned(),
        },
        Head::WorkingTree,
        base_measurement,
        head_measurement,
        FindingsShown::default(),
        &budgets,
        &limits,
    )
}

#[test]
fn diff_and_trend_schemas_validate_reports_with_external_metrics() {
    let plugin = VersionedExternalMetricPlugin;
    let metric_plugins: [&dyn codeint_model::MetricPlugin; 1] = [&plugin];
    let diff = sample_diff_with_external_metric();
    let diff_json = parse_and_validate(
        ReportSchemaKind::Diff,
        diff.render(ReportFormat::Json).expect("diff json"),
    );
    assert_eq!(
        diff_json["source_sets"]["production"]["custom_depth"]["metric_version"],
        3
    );

    let trend_measurement = CommitMeasurement::of_languages(
        vec![sample_model(false, true)],
        CommitMeasurementOptions {
            metric_plugins: &metric_plugins,
            ..CommitMeasurementOptions::default()
        },
    )
    .expect("trend measurement");
    let trend = TrendReport::of(vec![TrendPoint {
        commit: "abc1234".to_owned(),
        committed_on: CalendarDay::containing(1_700_000_000),
        measurement: trend_measurement,
    }]);
    let trend_json = parse_and_validate(
        ReportSchemaKind::Trend,
        trend.render(TrendFormat::Json).expect("trend json"),
    );
    assert_eq!(trend_json["metric_versions"]["custom_depth"], 3);
}

#[test]
fn report_schemas_and_validator_reject_malformed_payloads_and_invalid_schemas() {
    assert!(validate_json_against_schema(&json!({"a": 1}), &Value::Null).is_err());
    assert!(validate_json_against_schema(&json!({"a": 1}), &json!(42)).is_err());
    assert!(validate_json_against_schema(&json!({"a": 1}), &json!("object")).is_err());
    assert!(validate_json_against_schema(&json!({"a": 1}), &json!([])).is_err());
    assert!(validate_json_against_schema(&json!({"a": 1}), &json!(false)).is_err());
    assert!(validate_json_against_schema(&json!({"a": 1}), &json!(true)).is_ok());

    let plugin = VersionedExternalMetricPlugin;
    let metric_plugins: [&dyn codeint_model::MetricPlugin; 1] = [&plugin];
    let budgets = Budgets::default();
    let report = Report::summarize_with_options(
        vec![MeasuredLanguage::new(
            "typescript-javascript",
            sample_model(true, true),
            ReferenceRanges::default(),
            true,
        )],
        Vec::new(),
        ReportOptions {
            metric_plugins: &metric_plugins,
            ..ReportOptions::new(FindingsShown::default(), &budgets)
        },
    )
    .expect("report");
    let valid_check: Value =
        serde_json::from_str(&report.render(ReportFormat::Json).expect("json")).expect("json");
    validate_report_json(ReportSchemaKind::Check, &valid_check).expect("valid check report");

    let mut bad_erosion_findings = valid_check.clone();
    bad_erosion_findings["source_sets"]["production"]["erosion"]["findings"] =
        json!("not-an-array");
    assert!(validate_report_json(ReportSchemaKind::Check, &bad_erosion_findings).is_err());

    let mut bad_external_findings = valid_check.clone();
    bad_external_findings["source_sets"]["production"]["custom_depth"]["findings"] =
        json!([123, false]);
    assert!(validate_report_json(ReportSchemaKind::Check, &bad_external_findings).is_err());

    let mut bad_external_key = valid_check.clone();
    bad_external_key["source_sets"]["production"]["custom_depth"]["bogus"] = json!("anything");
    assert!(validate_report_json(ReportSchemaKind::Check, &bad_external_key).is_err());

    let mut unknown_top_level = valid_check.clone();
    unknown_top_level["unknown_top_level_key"] = json!(true);
    assert!(validate_report_json(ReportSchemaKind::Check, &unknown_top_level).is_err());

    let mut empty_package_erosion = valid_check.clone();
    empty_package_erosion["source_sets"]["production"]["packages"][0]["erosion"] = json!({});
    assert!(validate_report_json(ReportSchemaKind::Check, &empty_package_erosion).is_err());

    let diff = sample_diff_with_external_metric();
    let mut bad_diff: Value =
        serde_json::from_str(&diff.render(ReportFormat::Json).expect("diff json")).expect("json");
    bad_diff["source_sets"]["production"]["erosion"]["new"] = json!([{"completely_bogus": 42}]);
    assert!(validate_report_json(ReportSchemaKind::Diff, &bad_diff).is_err());
}
