use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use codeint_config::{
    BudgetOverride, ConfigFileKind, ConfigOverrides, ConfigProvenance, Configuration,
    ConfigurationFileError, DialectConflict, HAZARDS_METRIC_KEY, HazardsConfig, NewFindingsRule,
    RatioRise, RegressionLimit, ResolvedConfiguration, configuration_schema, read_configuration,
    read_resolved_configuration, validate_dialects,
};
use codeint_model::{RepoPath, SourceSet};
use serde_json::{Value, json};

struct TempConfigRoot {
    dir: PathBuf,
}

impl TempConfigRoot {
    fn empty() -> TempConfigRoot {
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        let dir = std::env::temp_dir()
            .join("codeint-sql-hazards-tests")
            .join(std::process::id().to_string())
            .join(COUNTER.fetch_add(1, Ordering::Relaxed).to_string());
        std::fs::create_dir_all(&dir).expect("create temp config root");
        TempConfigRoot { dir }
    }

    fn create(file: Option<(&str, &str)>) -> TempConfigRoot {
        let root = TempConfigRoot::empty();
        if let Some((name, content)) = file {
            std::fs::write(root.dir.join(name), content).expect("write config file");
        }
        root
    }

    fn read(&self) -> Result<Configuration, ConfigurationFileError> {
        read_configuration(&self.dir)
    }

    fn resolve(
        &self,
        overrides: &ConfigOverrides,
    ) -> Result<ResolvedConfiguration, ConfigurationFileError> {
        read_resolved_configuration(&self.dir, overrides)
    }
}

impl Drop for TempConfigRoot {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn repo_path(path: &str) -> RepoPath {
    RepoPath::new(path).expect("valid repo path")
}

fn expect_invalid(
    file_name: &str,
    payload: &Value,
    expected_substrings: &[&str],
) -> ConfigurationFileError {
    let raw = payload.to_string();
    let err = TempConfigRoot::create(Some((file_name, &raw)))
        .read()
        .expect_err("expected config error");
    let message = err.to_string();
    for needle in expected_substrings {
        assert!(
            message.contains(needle),
            "expected {needle:?} in {message:?}"
        );
    }
    err
}

#[test]
fn dialects_config_defaults_empty_and_resolves_folder_file_and_multi_dialect_mappings() {
    let default_config = Configuration::default();
    assert!(default_config.dialects.is_empty());

    let raw = json!({
        "dialects": {
            "sql": [
                { "dialect": "postgresql", "include": ["db/pg", "shared/pg_*.sql"] },
                { "dialect": "postgresql", "include": ["db/pg/migrations/**"] },
                { "dialect": "mysql", "include": ["db/mysql/**"] },
                { "dialect": "sqlite", "include": ["mobile/local.sql"] },
                { "dialect": "duckdb", "include": ["analytics"] }
            ]
        }
    })
    .to_string();

    let resolved = TempConfigRoot::create(Some(("codeint.json", &raw)))
        .resolve(&ConfigOverrides::default())
        .expect("valid multi-dialect config");
    let sql = resolved
        .configuration
        .dialects
        .get("sql")
        .expect("dialects.sql present");
    assert!(!sql.is_empty());
    for (path, expected) in [
        ("db/pg/migrations/001.sql", Some("postgresql")),
        ("shared/pg_views.sql", Some("postgresql")),
        ("db/mysql/init.sql", Some("mysql")),
        ("mobile/local.sql", Some("sqlite")),
        ("analytics/monthly/revenue.sql", Some("duckdb")),
        ("db/pg_other/schema.sql", None),
        ("unmapped/query.sql", None),
    ] {
        assert_eq!(
            sql.dialect_for("sql", &repo_path(path)),
            Ok(expected),
            "{path}"
        );
    }

    let setting = resolved
        .settings
        .iter()
        .find(|item| item.key == "dialects.sql")
        .expect("dialects.sql setting present");
    assert_eq!(setting.source, ConfigProvenance::File);
    assert_eq!(setting.value.as_array().map(Vec::len), Some(5));
}

#[test]
fn dialect_conflict_reports_both_patterns_and_dialects() {
    let raw = json!({
        "dialects": {
            "sql": [
                { "dialect": "postgresql", "include": ["db/**"] },
                { "dialect": "duckdb", "include": ["db/analytics/**"] }
            ]
        }
    })
    .to_string();

    let config = TempConfigRoot::create(Some(("codeint.json", &raw)))
        .read()
        .expect("syntactically valid config");
    let conflict = config
        .dialects
        .get("sql")
        .expect("sql rules")
        .dialect_for("sql", &repo_path("db/analytics/report.sql"))
        .expect_err("overlapping different dialects must conflict");
    assert_eq!(
        *conflict,
        DialectConflict {
            language: "sql".to_owned(),
            path: repo_path("db/analytics/report.sql"),
            first_dialect: "postgresql".to_owned(),
            first_pattern: "db/**".to_owned(),
            second_dialect: "duckdb".to_owned(),
            second_pattern: "db/analytics/**".to_owned(),
        }
    );
    let rendered = conflict.to_string();
    for expected in [
        "db/analytics/report.sql",
        "postgresql",
        "db/**",
        "duckdb",
        "db/analytics/**",
    ] {
        assert!(rendered.contains(expected), "{rendered}");
    }
}

fn assert_invalid_dialect_config(
    payload: &Value,
    expected_substrings: &[&str],
    check: fn(&ConfigurationFileError) -> bool,
) {
    let err = expect_invalid("codeint.json", payload, expected_substrings);
    assert!(check(&err), "unexpected error variant: {err:?}");
}

fn assert_validate_dialects_err(
    file_name: &str,
    kind: ConfigFileKind,
    payload: &Value,
    known: &BTreeMap<String, Vec<String>>,
    check: fn(&ConfigurationFileError) -> bool,
) -> ConfigurationFileError {
    let cfg = TempConfigRoot::create(Some((file_name, &payload.to_string())))
        .read()
        .expect("syntactically valid config");
    let err = validate_dialects(kind, &cfg.dialects, known).expect_err("expected dialect error");
    assert!(check(&err), "unexpected error variant: {err:?}");
    err
}

type ErrorPredicate = fn(&ConfigurationFileError) -> bool;

fn sql_dialect_payload(dialect: &str, include: &[&str]) -> Value {
    json!({ "dialects": { "sql": [{ "dialect": dialect, "include": include }] } })
}

#[test]
fn dialects_rejects_legacy_sql_key_empty_fields_invalid_glob_and_validates_known_dialects() {
    let invalid_cases: [(Value, &[&str], ErrorPredicate); 5] = [
        (
            json!({
                "sql": {
                    "dialects": [{ "dialect": "postgresql", "include": ["db/**"] }]
                }
            }),
            &["sql"],
            |err| matches!(err, ConfigurationFileError::UnknownKey { .. }),
        ),
        (
            sql_dialect_payload("   ", &["db/**"]),
            &["dialects.sql[0].dialect"],
            |err| matches!(err, ConfigurationFileError::EmptyDialectName { .. }),
        ),
        (
            sql_dialect_payload("sqlite", &[]),
            &["dialects.sql[0].include", "sqlite"],
            |err| matches!(err, ConfigurationFileError::EmptyDialectInclude { .. }),
        ),
        (
            sql_dialect_payload("mysql", &["db/**", "   "]),
            &["dialects.sql[0].include[1]"],
            |err| matches!(err, ConfigurationFileError::EmptyDialectPattern { .. }),
        ),
        (
            sql_dialect_payload("duckdb", &["db/[unclosed"]),
            &["dialects.sql[0].include[0]"],
            |err| matches!(err, ConfigurationFileError::InvalidGlob { .. }),
        ),
    ];
    for (payload, expected_substrings, check) in invalid_cases {
        assert_invalid_dialect_config(&payload, expected_substrings, check);
    }

    let known = BTreeMap::from([
        (
            "sql".to_owned(),
            vec![
                "postgresql".to_owned(),
                "mysql".to_owned(),
                "sqlite".to_owned(),
                "duckdb".to_owned(),
            ],
        ),
        ("rust".to_owned(), Vec::new()),
    ]);
    let validation_cases: [(&str, ConfigFileKind, Value, &str, ErrorPredicate); 4] = [
        (
            "codeint.jsonc",
            ConfigFileKind::Jsonc,
            json!({ "dialects": { "sql": [{ "dialect": "oracle", "include": ["db/**"] }] } }),
            "postgresql",
            |err| matches!(err, ConfigurationFileError::UnknownDialect { .. }),
        ),
        (
            "codeint.json",
            ConfigFileKind::Json,
            json!({ "dialects": { "rust": [{ "dialect": "2024", "include": ["src/**"] }] } }),
            "defines no selectable Dialects",
            |err| matches!(err, ConfigurationFileError::UnknownDialect { .. }),
        ),
        (
            "codeint.json",
            ConfigFileKind::Json,
            json!({ "dialects": { "cobol": [{ "dialect": "ansi", "include": ["src/**"] }] } }),
            "cobol",
            |err| matches!(err, ConfigurationFileError::UnknownDialectLanguage { .. }),
        ),
        (
            "codeint.json",
            ConfigFileKind::Json,
            json!({ "dialects": { "cobol": [] } }),
            "cobol",
            |err| matches!(err, ConfigurationFileError::UnknownDialectLanguage { .. }),
        ),
    ];
    for (file_name, kind, payload, expected_text, check) in validation_cases {
        let err = assert_validate_dialects_err(file_name, kind, &payload, &known, check);
        assert!(err.to_string().contains(expected_text), "error: {err}");
    }

    for empty_lang in ["", "   "] {
        let payload = json!({ "dialects": { empty_lang: [] } });
        assert_invalid_dialect_config(&payload, &["dialects must name"], |err| {
            matches!(err, ConfigurationFileError::EmptyDialectLanguage { .. })
                && !err.to_string().contains("dialects.")
        });
    }
}

#[test]
fn hazards_disabled_rules_defaults_empty_parses_and_rejects_empty_entries() {
    assert_eq!(Configuration::default().hazards, HazardsConfig::default());
    assert!(!Configuration::default().hazards.is_disabled("select-star"));
    assert!(
        Configuration::default()
            .hazards
            .disabled_rules_set()
            .is_empty()
    );

    let default_resolved = TempConfigRoot::create(None)
        .resolve(&ConfigOverrides::default())
        .expect("default config");
    let default_setting = default_resolved
        .settings
        .iter()
        .find(|item| item.key == "hazards.disabled_rules")
        .expect("hazards.disabled_rules default setting");
    assert_eq!(default_setting.value, json!([]));
    assert_eq!(default_setting.source, ConfigProvenance::Default);

    let raw = json!({
        "hazards": { "disabled_rules": ["select-star", "large-offset", "select-star"] }
    })
    .to_string();
    let resolved = TempConfigRoot::create(Some(("codeint.json", &raw)))
        .resolve(&ConfigOverrides::default())
        .expect("valid hazards config");
    assert!(resolved.configuration.hazards.is_disabled("select-star"));
    assert!(resolved.configuration.hazards.is_disabled("large-offset"));
    assert!(!resolved.configuration.hazards.is_disabled("drop"));
    assert_eq!(
        resolved.configuration.hazards.disabled_rules_set(),
        BTreeSet::from(["large-offset".to_owned(), "select-star".to_owned()])
    );

    let file_setting = resolved
        .settings
        .iter()
        .find(|item| item.key == "hazards.disabled_rules")
        .expect("hazards.disabled_rules file setting");
    assert_eq!(file_setting.source, ConfigProvenance::File);
    assert_eq!(
        file_setting.value,
        json!(["select-star", "large-offset", "select-star"])
    );

    for empty in ["", "   "] {
        let err = expect_invalid(
            "codeint.jsonc",
            &json!({ "hazards": { "disabled_rules": [empty] } }),
            &["codeint.jsonc", "hazards.disabled_rules[0]"],
        );
        assert!(matches!(
            err,
            ConfigurationFileError::EmptyHazardRuleName { .. }
        ));
    }
}

#[test]
fn hazards_budgets_work_across_all_four_source_sets_and_cli_overrides() {
    assert_eq!(HAZARDS_METRIC_KEY, "hazards");
    let raw = json!({
        "budgets": {
            "production": { "hazards": 0.05 },
            "test": { "hazards": 0.25 },
            "generated": { "hazards": 0.5 },
            "vendored": { "hazards": 1.0 }
        }
    })
    .to_string();
    let config = TempConfigRoot::create(Some(("codeint.json", &raw)))
        .read()
        .expect("valid hazards budgets");
    config
        .validate_metrics(&[])
        .expect("hazards is a known built-in metric key");
    for (source_set, expected) in [
        (SourceSet::Production, 0.05),
        (SourceSet::Test, 0.25),
        (SourceSet::Generated, 0.5),
        (SourceSet::Vendored, 1.0),
    ] {
        assert_eq!(
            config
                .budgets
                .budget_for_key(source_set, HAZARDS_METRIC_KEY)
                .map(|budget| budget.ratio()),
            Some(expected)
        );
    }

    for invalid in [-0.01_f64, 1.01_f64] {
        let err = expect_invalid(
            "codeint.json",
            &json!({ "budgets": { "production": { "hazards": invalid } } }),
            &["budgets.production.hazards"],
        );
        assert!(matches!(err, ConfigurationFileError::OutOfRange { .. }));
    }

    let cli_budgets: Vec<BudgetOverride> = ["production.hazards=0.1", "generated.hazards=0.4"]
        .into_iter()
        .map(|spec| spec.parse().expect("valid cli budget"))
        .collect();
    let overrides = ConfigOverrides {
        budgets: cli_budgets,
        ..ConfigOverrides::default()
    };
    let resolved = TempConfigRoot::create(None)
        .resolve(&overrides)
        .expect("resolved with CLI hazards budgets");
    resolved
        .configuration
        .validate_cli_budget_metrics(&overrides.budgets, &[])
        .expect("hazards CLI overrides are valid");
    resolved
        .configuration
        .validate_metrics(&[])
        .expect("resolved config is valid");
    assert_eq!(
        resolved
            .configuration
            .budgets
            .budget_for_key(SourceSet::Production, HAZARDS_METRIC_KEY)
            .map(|budget| budget.ratio()),
        Some(0.1)
    );
    assert_eq!(
        resolved
            .configuration
            .budgets
            .budget_for_key(SourceSet::Generated, HAZARDS_METRIC_KEY)
            .map(|budget| budget.ratio()),
        Some(0.4)
    );
}

#[test]
fn hazards_regression_limits_default_and_override_globally_and_per_source_set() {
    let default_resolved = TempConfigRoot::create(None)
        .resolve(&ConfigOverrides::default())
        .expect("default");
    for source_set in [
        SourceSet::Production,
        SourceSet::Test,
        SourceSet::Generated,
        SourceSet::Vendored,
    ] {
        assert_eq!(
            default_resolved
                .configuration
                .regression_limits
                .limit_for_key(source_set, HAZARDS_METRIC_KEY),
            RegressionLimit {
                ratio_rise: RatioRise::DEFAULT,
                new_findings: NewFindingsRule::AnyNewFinding,
            }
        );
    }

    let raw = json!({
        "regression_limits": {
            "hazards": { "ratio_rise": 0.02, "new_findings": "any_new_finding" },
            "test": { "hazards": { "new_findings": "never" } }
        }
    })
    .to_string();
    let resolved = TempConfigRoot::create(Some(("codeint.json", &raw)))
        .resolve(&ConfigOverrides::default())
        .expect("valid hazards regression limits");
    resolved
        .configuration
        .validate_metrics(&[])
        .expect("hazards regression limits pass validation");

    let prod_limit = resolved
        .configuration
        .regression_limits
        .limit_for_key(SourceSet::Production, HAZARDS_METRIC_KEY);
    assert_eq!(prod_limit.ratio_rise.ratio(), 0.02);
    assert_eq!(prod_limit.new_findings, NewFindingsRule::AnyNewFinding);

    let test_limit = resolved
        .configuration
        .regression_limits
        .limit_for_key(SourceSet::Test, HAZARDS_METRIC_KEY);
    assert_eq!(test_limit.ratio_rise.ratio(), 0.02);
    assert_eq!(test_limit.new_findings, NewFindingsRule::Never);

    let prod_ratio_settings: Vec<_> = resolved
        .settings
        .iter()
        .filter(|item| item.key == "regression_limits.production.hazards.ratio_rise")
        .collect();
    assert_eq!(prod_ratio_settings.len(), 1);
    assert_eq!(prod_ratio_settings[0].value, json!(0.02));
    assert_eq!(prod_ratio_settings[0].source, ConfigProvenance::File);

    let default_ratio_setting = default_resolved
        .settings
        .iter()
        .find(|item| item.key == "regression_limits.production.hazards.ratio_rise")
        .expect("default hazards ratio_rise present");
    assert_eq!(
        default_ratio_setting.value,
        json!(RatioRise::DEFAULT.ratio())
    );
    assert_eq!(default_ratio_setting.source, ConfigProvenance::Default);
}

#[test]
fn empty_dialects_and_hazards_sections_serialize_identically_to_default() {
    let explicit_empty = TempConfigRoot::create(Some((
        "codeint.json",
        r#"{ "dialects": { "sql": [] }, "hazards": { "disabled_rules": [] } }"#,
    )))
    .read()
    .expect("empty dialects and hazards sections parse");
    assert_eq!(
        serde_json::to_string(&Configuration::default()).expect("serialize default"),
        serde_json::to_string(&explicit_empty).expect("serialize explicit empty")
    );
}

#[test]
fn configuration_schema_exposes_dialects_hazards_budgets_and_regression_limits() {
    let schema: Value = serde_json::from_str(&configuration_schema()).expect("valid JSON schema");

    assert!(schema["properties"].get("sql").is_none());
    assert_eq!(schema["properties"]["dialects"]["type"], "object");
    assert_eq!(
        schema["properties"]["dialects"]["additionalProperties"]["items"]["$ref"],
        "#/$defs/DialectEntryFile"
    );
    assert_eq!(
        schema["properties"]["hazards"]["$ref"],
        "#/$defs/HazardsConfigFile"
    );
    for def_name in [
        "ProductionMetricBudgets",
        "MetricBudgets",
        "RegressionLimitsFile",
        "MetricRegressionLimits",
    ] {
        assert!(
            schema["$defs"][def_name]["properties"]
                .get("hazards")
                .is_some(),
            "missing hazards in {def_name}"
        );
    }
}
