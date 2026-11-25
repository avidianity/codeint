use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use codeint_config::{
    BudgetOverride, ConfigOverrides, ConfigProvenance, Configuration, ConfigurationFileError,
    HAZARDS_METRIC_KEY, HazardsConfig, KNOWN_SQL_DIALECTS, NewFindingsRule, RatioRise,
    RegressionLimit, ResolvedConfiguration, SqlDialect, SqlDialectConflict, configuration_schema,
    read_configuration, read_resolved_configuration,
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
fn sql_dialects_defaults_to_generic_and_resolves_folder_file_and_multi_dialect_mappings() {
    assert_eq!(
        KNOWN_SQL_DIALECTS,
        ["postgresql", "mysql", "sqlite", "duckdb"]
    );
    assert_eq!(SqlDialect::default(), SqlDialect::Generic);
    assert_eq!(SqlDialect::Generic.as_str(), "generic");
    for (name, expected) in [
        ("postgresql", Some(SqlDialect::Postgresql)),
        ("mysql", Some(SqlDialect::Mysql)),
        ("sqlite", Some(SqlDialect::Sqlite)),
        ("duckdb", Some(SqlDialect::Duckdb)),
        ("generic", None),
        ("oracle", None),
    ] {
        assert_eq!(SqlDialect::parse_configured(name), expected);
    }

    let default_config = Configuration::default();
    assert!(default_config.sql.is_empty());
    assert_eq!(
        default_config.sql.dialect_for(&repo_path("db/query.sql")),
        Ok(SqlDialect::Generic)
    );

    let raw = json!({
        "sql": {
            "dialects": [
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
    let sql = &resolved.configuration.sql;
    assert!(!sql.is_empty());
    for (path, expected) in [
        ("db/pg/migrations/001.sql", SqlDialect::Postgresql),
        ("shared/pg_views.sql", SqlDialect::Postgresql),
        ("db/mysql/init.sql", SqlDialect::Mysql),
        ("mobile/local.sql", SqlDialect::Sqlite),
        ("analytics/monthly/revenue.sql", SqlDialect::Duckdb),
        ("db/pg_other/schema.sql", SqlDialect::Generic),
        ("unmapped/query.sql", SqlDialect::Generic),
    ] {
        assert_eq!(sql.dialect_for(&repo_path(path)), Ok(expected), "{path}");
    }

    let setting = resolved
        .settings
        .iter()
        .find(|item| item.key == "sql.dialects")
        .expect("sql.dialects setting present");
    assert_eq!(setting.source, ConfigProvenance::File);
    assert_eq!(setting.value.as_array().map(Vec::len), Some(5));
}

#[test]
fn sql_dialect_conflict_reports_both_patterns_and_dialects() {
    let raw = json!({
        "sql": {
            "dialects": [
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
        .sql
        .dialect_for(&repo_path("db/analytics/report.sql"))
        .expect_err("overlapping different dialects must conflict");
    assert_eq!(
        conflict,
        SqlDialectConflict {
            path: repo_path("db/analytics/report.sql"),
            first_dialect: SqlDialect::Postgresql,
            first_pattern: "db/**".to_owned(),
            second_dialect: SqlDialect::Duckdb,
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

#[test]
fn sql_dialects_rejects_unknown_dialect_empty_include_empty_pattern_and_invalid_glob() {
    let err = expect_invalid(
        "codeint.jsonc",
        &json!({
            "sql": {
                "dialects": [
                    { "dialect": "postgresql", "include": ["db/pg/**"] },
                    { "dialect": "oracle", "include": ["db/ora/**"] }
                ]
            }
        }),
        &[
            "codeint.jsonc",
            "sql.dialects[1].dialect",
            "postgresql, mysql, sqlite, duckdb",
            "oracle",
        ],
    );
    assert!(matches!(err, ConfigurationFileError::NotOneOf { .. }));

    let generic_err = expect_invalid(
        "codeint.json",
        &json!({ "sql": { "dialects": [{ "dialect": "generic", "include": ["db/**"] }] } }),
        &[
            "sql.dialects[0].dialect",
            "postgresql, mysql, sqlite, duckdb",
        ],
    );
    assert!(matches!(
        generic_err,
        ConfigurationFileError::NotOneOf { .. }
    ));

    let empty_include_err = expect_invalid(
        "codeint.json",
        &json!({ "sql": { "dialects": [{ "dialect": "sqlite", "include": [] }] } }),
        &["sql.dialects[0].include", "sqlite"],
    );
    assert!(matches!(
        empty_include_err,
        ConfigurationFileError::EmptySqlDialectInclude { .. }
    ));

    let empty_pattern_err = expect_invalid(
        "codeint.json",
        &json!({ "sql": { "dialects": [{ "dialect": "mysql", "include": ["db/**", "   "] }] } }),
        &["sql.dialects[0].include[1]"],
    );
    assert!(matches!(
        empty_pattern_err,
        ConfigurationFileError::EmptySqlDialectPattern { .. }
    ));

    let bad_glob_err = expect_invalid(
        "codeint.json",
        &json!({ "sql": { "dialects": [{ "dialect": "duckdb", "include": ["db/[unclosed"] }] } }),
        &["sql.dialects[0].include[0]"],
    );
    assert!(matches!(
        bad_glob_err,
        ConfigurationFileError::InvalidGlob { .. }
    ));
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
fn empty_sql_and_hazards_sections_serialize_identically_to_default() {
    let explicit_empty = TempConfigRoot::create(Some((
        "codeint.json",
        r#"{ "sql": { "dialects": [] }, "hazards": { "disabled_rules": [] } }"#,
    )))
    .read()
    .expect("empty sql and hazards sections parse");
    assert_eq!(
        serde_json::to_string(&Configuration::default()).expect("serialize default"),
        serde_json::to_string(&explicit_empty).expect("serialize explicit empty")
    );
}

#[test]
fn configuration_schema_exposes_sql_hazards_budgets_and_regression_limits() {
    let schema: Value = serde_json::from_str(&configuration_schema()).expect("valid JSON schema");

    assert_eq!(schema["properties"]["sql"]["$ref"], "#/$defs/SqlConfigFile");
    assert_eq!(
        schema["properties"]["hazards"]["$ref"],
        "#/$defs/HazardsConfigFile"
    );
    assert_eq!(
        schema["$defs"]["ConfiguredSqlDialect"]["enum"],
        json!(["postgresql", "mysql", "sqlite", "duckdb"])
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
