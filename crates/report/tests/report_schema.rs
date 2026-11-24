use std::path::PathBuf;

use codeint_config::{Budgets, DependenciesConfig, DuplicationConfig, FindingsShown, ReportFormat};
use codeint_model::language::Function;
use codeint_model::{
    CodeModel, FileModel, FunctionCoverage, HazardLine, LanguageFacts, ReferenceRanges, RepoPath,
    SourceFile, SourceSet, Workspace,
};
use codeint_report::{
    MeasuredLanguage, OwnershipOptions, RejectedStatementEntry, Report, ReportExtras,
    ReportOptions, SqlSchemaSummary, report_schema,
};
use serde_json::Value;

fn published_report_schema_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../schema/codeint-report.schema.json")
}

fn validate_against_schema(value: &Value, schema: &Value, root: &Value, path: &str) {
    if let Some(allowed) = schema.as_bool() {
        assert!(allowed, "value disallowed by schema at {path}: {value}");
        return;
    }
    if let Some(reference) = schema.get("$ref").and_then(Value::as_str) {
        let def_name = reference
            .strip_prefix("#/$defs/")
            .expect("local $defs reference");
        validate_against_schema(value, &root["$defs"][def_name], root, path);
        return;
    }
    check_json_type(value, schema, path);
    check_number_bounds(value, schema, path);
    check_object_schema(value, schema, root, path);
    check_array_schema(value, schema, root, path);
}

fn check_json_type(value: &Value, schema: &Value, path: &str) {
    let Some(expected_type) = schema.get("type").and_then(Value::as_str) else {
        return;
    };
    let ok = match expected_type {
        "object" => value.is_object(),
        "array" => value.is_array(),
        "string" => value.is_string(),
        "number" => value.is_number(),
        "integer" => value.is_u64() || value.is_i64(),
        "boolean" => value.is_boolean(),
        other => panic!("unsupported schema type {other} at {path}"),
    };
    assert!(ok, "expected {expected_type} at {path}, got {value}");
}

fn check_number_bounds(value: &Value, schema: &Value, path: &str) {
    let Some(number) = value.as_f64() else {
        return;
    };
    if let Some(min) = schema.get("minimum").and_then(Value::as_f64) {
        assert!(
            number >= min,
            "value {number} below minimum {min} at {path}"
        );
    }
    if let Some(max) = schema.get("maximum").and_then(Value::as_f64) {
        assert!(
            number <= max,
            "value {number} above maximum {max} at {path}"
        );
    }
}

fn check_object_schema(value: &Value, schema: &Value, root: &Value, path: &str) {
    let Some(object) = value.as_object() else {
        return;
    };
    for key in schema
        .get("required")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let key_str = key.as_str().expect("required key string");
        assert!(
            object.contains_key(key_str),
            "missing required key {key_str:?} at {path}"
        );
    }
    let properties = schema.get("properties").and_then(Value::as_object);
    for (key, child) in object {
        let child_path = format!("{path}.{key}");
        let child_schema = properties
            .and_then(|props| props.get(key))
            .or_else(|| schema.get("additionalProperties"));
        if let Some(target) = child_schema {
            validate_against_schema(child, target, root, &child_path);
        }
    }
}

fn check_array_schema(value: &Value, schema: &Value, root: &Value, path: &str) {
    let (Some(items), Some(item_schema)) = (value.as_array(), schema.get("items")) else {
        return;
    };
    for (index, item) in items.iter().enumerate() {
        validate_against_schema(item, item_schema, root, &format!("{path}[{index}]"));
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

    let with_gap = Report::summarize_with_extras(
        vec![MeasuredLanguage {
            plugin: "typescript-javascript".to_owned(),
            model: sample_model(true, true),
            reference_ranges: ReferenceRanges::default(),
            has_imports: true,
        }],
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
            unmatched_coverage_paths: &["missing/file.ts".to_owned()],
        },
        ReportExtras::default()
            .with_rejected_statements(vec![RejectedStatementEntry {
                path: "packages/api/db/bad.sql".to_owned(),
                line: 9,
                dialect: "postgresql".to_owned(),
            }])
            .with_sql_schemas(vec![SqlSchemaSummary {
                package: Some("packages/api".to_owned()),
                dialect: "postgresql".to_owned(),
                tables: 3,
            }]),
    )
    .expect("report with test gap and hazards");
    let with_gap_json: Value =
        serde_json::from_str(&with_gap.render(ReportFormat::Json).expect("json"))
            .expect("valid JSON");
    validate_against_schema(&with_gap_json, &schema, &schema, "$");
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
            has_imports: true,
        }],
        Vec::new(),
        FindingsShown::default(),
        &budgets,
    );
    let without_gap_json: Value =
        serde_json::from_str(&without_gap.render(ReportFormat::Json).expect("json"))
            .expect("valid JSON");
    validate_against_schema(&without_gap_json, &schema, &schema, "$");
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
