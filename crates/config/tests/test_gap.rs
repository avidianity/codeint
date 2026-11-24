use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};

use codeint_config::{
    BudgetOverride, ConfigOverrides, ConfigProvenance, Configuration, ConfigurationFileError,
    ResolvedConfiguration, TEST_GAP_METRIC_KEY, configuration_schema, read_configuration,
    read_resolved_configuration,
};
use codeint_model::SourceSet;
use serde_json::{Value, json};

static NEXT_CASE_ID: AtomicUsize = AtomicUsize::new(0);

fn with_config_dir<T>(file: Option<(&str, &str)>, action: impl FnOnce(&Path) -> T) -> T {
    let case_id = NEXT_CASE_ID.fetch_add(1, Ordering::Relaxed);
    let root =
        std::env::temp_dir().join(format!("codeint-test-gap-{}-{case_id}", std::process::id()));
    std::fs::create_dir_all(&root).expect("create test folder");
    if let Some((file_name, text)) = file {
        std::fs::write(root.join(file_name), text).expect("write config");
    }
    let result = action(&root);
    let _ = std::fs::remove_dir_all(&root);
    result
}

fn parse_file(file_name: &str, text: &str) -> Result<Configuration, ConfigurationFileError> {
    with_config_dir(Some((file_name, text)), read_configuration)
}

fn parse_resolved(
    file: Option<(&str, &str)>,
    overrides: &ConfigOverrides,
) -> Result<ResolvedConfiguration, ConfigurationFileError> {
    with_config_dir(file, |root| read_resolved_configuration(root, overrides))
}

fn assert_rejected(
    file_name: &str,
    raw: &str,
    expected_fragments: &[&str],
    check: impl FnOnce(&ConfigurationFileError) -> bool,
) {
    let error = parse_file(file_name, raw).expect_err("configuration must be rejected");
    assert!(check(&error), "unexpected error variant for {raw}: {error}");
    let rendered = error.to_string();
    for fragment in expected_fragments {
        assert!(
            rendered.contains(fragment),
            "expected {fragment:?} in error: {rendered}"
        );
    }
}

#[test]
fn coverage_reports_defaults_empty_and_parses_relative_paths_from_json_and_jsonc() {
    assert!(Configuration::default().coverage_reports.is_empty());

    let resolved_default =
        parse_resolved(None, &ConfigOverrides::default()).expect("missing config uses defaults");
    assert!(resolved_default.configuration.coverage_reports.is_empty());
    let default_setting = resolved_default
        .settings
        .iter()
        .find(|setting| setting.key == "coverage_reports")
        .expect("coverage_reports setting present");
    assert_eq!(default_setting.value, json!([]));
    assert_eq!(default_setting.source, ConfigProvenance::Default);

    let jsonc = concat!(
        "{\n",
        "  // LCOV and Cobertura reports\n",
        "  \"coverage_reports\": [\n",
        "    \"coverage/lcov.info\",\n",
        "    \"./reports/cobertura.xml\",\n",
        "    \"coverage/..hidden/lcov.info\",\n",
        "  ],\n",
        "}\n",
    );
    let resolved = parse_resolved(Some(("codeint.jsonc", jsonc)), &ConfigOverrides::default())
        .expect("valid jsonc coverage_reports");
    let expected = [
        "coverage/lcov.info",
        "./reports/cobertura.xml",
        "coverage/..hidden/lcov.info",
    ];
    assert_eq!(resolved.configuration.coverage_reports, expected);
    let file_setting = resolved
        .settings
        .iter()
        .find(|setting| setting.key == "coverage_reports")
        .expect("coverage_reports setting present");
    assert_eq!(file_setting.value, json!(expected));
    assert_eq!(file_setting.source, ConfigProvenance::File);

    let default_json = serde_json::to_string(&resolved_default.configuration).expect("serialize");
    let configured_json = serde_json::to_string(&resolved.configuration).expect("serialize");
    assert_ne!(default_json, configured_json);
    assert!(configured_json.contains("\"coverage_reports\""));
}

#[test]
fn coverage_reports_serialization_matches_default_when_explicitly_empty() {
    let explicit_empty =
        parse_file("codeint.json", r#"{ "coverage_reports": [] }"#).expect("empty array parses");
    assert_eq!(
        serde_json::to_string(&Configuration::default()).expect("serialize default"),
        serde_json::to_string(&explicit_empty).expect("serialize explicit empty")
    );
}

#[test]
fn coverage_reports_rejects_non_array_non_string_empty_absolute_and_parent_traversal() {
    for raw in [
        r#"{ "coverage_reports": "coverage/lcov.info" }"#,
        r#"{ "coverage_reports": 42 }"#,
        r#"{ "coverage_reports": {} }"#,
    ] {
        assert_rejected("codeint.json", raw, &["coverage_reports"], |error| {
            matches!(error, ConfigurationFileError::WrongType { .. })
        });
    }

    for raw in [
        r#"{ "coverage_reports": [123] }"#,
        r#"{ "coverage_reports": [null] }"#,
        r#"{ "coverage_reports": [true] }"#,
    ] {
        assert_rejected("codeint.json", raw, &["coverage_reports[0]"], |error| {
            matches!(error, ConfigurationFileError::WrongType { .. })
        });
    }

    for empty in ["", "   ", "\t"] {
        let raw = json!({ "coverage_reports": [empty] }).to_string();
        assert_rejected(
            "codeint.jsonc",
            &raw,
            &["codeint.jsonc", "coverage_reports[0]"],
            |error| {
                matches!(
                    error,
                    ConfigurationFileError::EmptyCoverageReportPath { .. }
                )
            },
        );
    }

    for absolute in [
        "/tmp/lcov.info",
        "\\server\\share\\lcov.info",
        "C:\\coverage\\lcov.info",
        "C:/coverage/lcov.info",
    ] {
        let raw = json!({ "coverage_reports": [absolute] }).to_string();
        assert_rejected("codeint.json", &raw, &["coverage_reports[0]"], |error| {
            matches!(
                error,
                ConfigurationFileError::AbsoluteCoverageReportPath { .. }
            )
        });
    }

    for traversal in [
        "../lcov.info",
        "coverage/../../lcov.info",
        "..",
        "a\\..\\b.info",
    ] {
        let raw = json!({ "coverage_reports": [traversal] }).to_string();
        assert_rejected("codeint.json", &raw, &["coverage_reports[0]"], |error| {
            matches!(
                error,
                ConfigurationFileError::ParentTraversalCoverageReportPath { .. }
            )
        });
    }
}

#[test]
fn budgets_production_test_gap_accepts_unit_interval_and_exposes_accessors() {
    assert_eq!(Configuration::default().budgets.production_test_gap(), None);
    assert_eq!(
        Configuration::default()
            .budgets
            .budget_for_key(SourceSet::Production, TEST_GAP_METRIC_KEY),
        None
    );

    for (raw_ratio, expected) in [("0", 0.0), ("-0.0", 0.0), ("0.25", 0.25), ("1.0", 1.0)] {
        let raw = format!(r#"{{ "budgets": {{ "production": {{ "test_gap": {raw_ratio} }} }} }}"#);
        let config = parse_file("codeint.json", &raw).expect("valid production test_gap budget");
        config
            .validate_metrics(&[])
            .expect("production test_gap passes validate_metrics");
        assert_eq!(config.budgets.production_test_gap(), Some(expected));
        assert_eq!(
            config
                .budgets
                .budget_for_key(SourceSet::Production, TEST_GAP_METRIC_KEY)
                .map(|budget| budget.ratio()),
            Some(expected)
        );
        for other in [SourceSet::Test, SourceSet::Generated, SourceSet::Vendored] {
            assert_eq!(
                config.budgets.budget_for_key(other, TEST_GAP_METRIC_KEY),
                None
            );
        }
    }

    for invalid in ["-0.01", "1.01"] {
        let raw = format!(r#"{{ "budgets": {{ "production": {{ "test_gap": {invalid} }} }} }}"#);
        assert_rejected(
            "codeint.json",
            &raw,
            &["budgets.production.test_gap"],
            |error| matches!(error, ConfigurationFileError::OutOfRange { .. }),
        );
    }

    assert_rejected(
        "codeint.json",
        r#"{ "budgets": { "production": { "test_gap": "20%" } } }"#,
        &["budgets.production.test_gap"],
        |error| matches!(error, ConfigurationFileError::WrongType { .. }),
    );
}

#[test]
fn budgets_test_gap_under_non_production_source_sets_fails_with_full_key_and_explanation() {
    for source_set in ["test", "generated", "vendored"] {
        let raw = format!(r#"{{ "budgets": {{ "{source_set}": {{ "test_gap": 0.2 }} }} }}"#);
        let expected_key = format!("budgets.{source_set}.test_gap");
        assert_rejected(
            "codeint.jsonc",
            &raw,
            &["codeint.jsonc", &expected_key, "Production"],
            |error| {
                matches!(
                    error,
                    ConfigurationFileError::TestGapBudgetOutsideProduction { .. }
                )
            },
        );
    }
}

#[test]
fn cli_budget_overrides_accept_production_test_gap_and_reject_other_source_sets() {
    let prod_override: BudgetOverride = "production.test_gap=0.35".parse().expect("valid override");
    let overrides = ConfigOverrides {
        budgets: vec![prod_override],
        ..ConfigOverrides::default()
    };
    let resolved = parse_resolved(None, &overrides).expect("resolved with CLI override");
    resolved
        .configuration
        .validate_cli_budget_metrics(&overrides.budgets, &[])
        .expect("production test_gap CLI override is valid");
    resolved
        .configuration
        .validate_metrics(&[])
        .expect("resolved config is valid");
    assert_eq!(
        resolved.configuration.budgets.production_test_gap(),
        Some(0.35)
    );

    for source_set in ["test", "generated", "vendored"] {
        let bad_override: BudgetOverride = format!("{source_set}.test_gap=0.35")
            .parse()
            .expect("syntactically valid override");
        let error = resolved
            .configuration
            .validate_cli_budget_metrics(&[bad_override], &[])
            .expect_err("non-production CLI test_gap must fail");
        assert!(
            matches!(
                error,
                ConfigurationFileError::CliTestGapBudgetOutsideProduction { .. }
            ),
            "expected CliTestGapBudgetOutsideProduction for {source_set}, got: {error}"
        );
        let rendered = error.to_string();
        assert!(
            rendered.contains(&format!("{source_set}.test_gap")),
            "{rendered}"
        );
        assert!(rendered.contains("Production"), "{rendered}");
    }
}

#[test]
fn regression_limits_rejects_test_gap_at_top_level_and_every_source_set() {
    let cases = [
        (
            r#"{ "regression_limits": { "test_gap": { "ratio_rise": 0.05 } } }"#,
            "regression_limits.test_gap",
        ),
        (
            r#"{ "regression_limits": { "test_gap": {} } }"#,
            "regression_limits.test_gap",
        ),
        (
            r#"{ "regression_limits": { "production": { "test_gap": { "ratio_rise": 0.05 } } } }"#,
            "regression_limits.production.test_gap",
        ),
        (
            r#"{ "regression_limits": { "production": { "test_gap": {} } } }"#,
            "regression_limits.production.test_gap",
        ),
        (
            r#"{ "regression_limits": { "test": { "test_gap": { "new_findings": "never" } } } }"#,
            "regression_limits.test.test_gap",
        ),
        (
            r#"{ "regression_limits": { "generated": { "test_gap": { "ratio_rise": 0.1 } } } }"#,
            "regression_limits.generated.test_gap",
        ),
        (
            r#"{ "regression_limits": { "vendored": { "test_gap": { "ratio_rise": 0.1 } } } }"#,
            "regression_limits.vendored.test_gap",
        ),
    ];

    for (raw, expected_key) in cases {
        assert_rejected(
            "codeint.json",
            raw,
            &[expected_key, "Regression"],
            |error| matches!(error, ConfigurationFileError::TestGapRegressionLimit { .. }),
        );
    }
}

#[test]
fn configuration_schema_exposes_coverage_reports_and_production_only_test_gap_budget() {
    let schema: Value = serde_json::from_str(&configuration_schema()).expect("valid JSON schema");

    assert_eq!(schema["properties"]["coverage_reports"]["type"], "array");
    assert_eq!(
        schema["properties"]["coverage_reports"]["items"]["type"],
        "string"
    );

    let budget_props = &schema["$defs"]["SourceSetBudgets"]["properties"];
    assert_eq!(
        budget_props["production"]["$ref"],
        "#/$defs/ProductionMetricBudgets"
    );
    for other in ["test", "generated", "vendored"] {
        assert_eq!(budget_props[other]["$ref"], "#/$defs/MetricBudgets");
    }

    assert!(
        schema["$defs"]["ProductionMetricBudgets"]["properties"]
            .get("test_gap")
            .is_some()
    );
    assert!(
        schema["$defs"]["MetricBudgets"]["properties"]
            .get("test_gap")
            .is_none()
    );
    assert!(
        schema["$defs"]["RegressionLimitsFile"]["properties"]
            .get("test_gap")
            .is_none()
    );
    assert!(
        schema["$defs"]["MetricRegressionLimits"]["properties"]
            .get("test_gap")
            .is_none()
    );
}
