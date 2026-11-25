use std::path::{Path, PathBuf};

use codeint_config::{
    BudgetOverride, ConfigOverrides, Configuration, NewFindingsRule, configuration_schema,
    read_configuration, read_resolved_configuration,
};
use codeint_model::{RepoPath, SourceSet};
use serde_json::Value;

struct TempFolder(PathBuf);

impl TempFolder {
    fn new(name: &str) -> TempFolder {
        let path =
            std::env::temp_dir().join(format!("codeint-config-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).expect("create temp dir");
        TempFolder(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempFolder {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn parses_entry_points_orphan_files_budget_and_regression_limits() {
    let dir = TempFolder::new("entry-points-valid");
    std::fs::write(
        dir.path().join("codeint.json"),
        r#"{
  "entry_points": ["src/pages/**/*.tsx", "scripts/cli.ts"],
  "budgets": {
    "production": {
      "orphan_files": 0.0
    }
  },
  "regression_limits": {
    "orphan_files": {
      "ratio_rise": 0.02
    },
    "production": {
      "orphan_files": {
        "new_findings": "any_new_finding"
      }
    }
  }
}"#,
    )
    .expect("write codeint.json");

    let config = read_configuration(dir.path()).expect("valid configuration");
    config
        .validate_metrics(&[])
        .expect("orphan_files is a known built-in metric");

    let budget = config
        .budgets
        .budget_for_key(SourceSet::Production, "orphan_files")
        .expect("production orphan_files budget");
    assert_eq!(budget.ratio(), 0.0);

    let prod_limit = config
        .regression_limits
        .limit_for_key(SourceSet::Production, "orphan_files");
    assert_eq!(prod_limit.ratio_rise.ratio(), 0.02);
    assert_eq!(prod_limit.new_findings, NewFindingsRule::AnyNewFinding);

    let default_config = Configuration::default();
    let default_limit = default_config
        .regression_limits
        .limit_for_key(SourceSet::Production, "orphan_files");
    assert_eq!(default_limit.new_findings, NewFindingsRule::AnyNewFinding);

    assert!(!config.entry_points.is_empty());
    assert!(
        config
            .entry_points
            .matches(&RepoPath::new("src/pages/blog/post.tsx").expect("path"))
    );
    assert!(
        config
            .entry_points
            .matches(&RepoPath::new("scripts/cli.ts").expect("path"))
    );
    assert!(
        !config
            .entry_points
            .matches(&RepoPath::new("src/lib/unused.ts").expect("path"))
    );
}

#[test]
fn rejects_invalid_entry_points_glob() {
    let dir = TempFolder::new("entry-points-invalid");
    std::fs::write(
        dir.path().join("codeint.json"),
        r#"{ "entry_points": ["src/[unclosed"] }"#,
    )
    .expect("write codeint.json");

    let error = read_configuration(dir.path()).expect_err("invalid glob must fail");
    assert!(
        error.to_string().contains("entry_points[0]"),
        "unexpected error: {error}"
    );
}

#[test]
fn rejects_orphan_files_budgets_outside_production() {
    for source_set in ["test", "generated", "vendored"] {
        let dir = TempFolder::new(&format!("orphan-budget-{source_set}"));
        std::fs::write(
            dir.path().join("codeint.jsonc"),
            format!(r#"{{ "budgets": {{ "{source_set}": {{ "orphan_files": 0.1 }} }} }}"#),
        )
        .expect("write codeint.jsonc");

        let error = read_configuration(dir.path()).expect_err("non-production budget must fail");
        let rendered = error.to_string();
        assert!(
            rendered.contains("codeint.jsonc")
                && rendered.contains(&format!("budgets.{source_set}.orphan_files"))
                && rendered.contains("Orphan files")
                && rendered.contains("Production"),
            "unexpected error for {source_set}: {rendered}"
        );
    }
}

#[test]
fn rejects_orphan_files_regression_limits_outside_production() {
    let cases = [
        (
            "test-ratio",
            r#"{ "regression_limits": { "test": { "orphan_files": { "ratio_rise": 0.05 } } } }"#,
            "regression_limits.test.orphan_files",
        ),
        (
            "test-empty",
            r#"{ "regression_limits": { "test": { "orphan_files": {} } } }"#,
            "regression_limits.test.orphan_files",
        ),
        (
            "generated",
            r#"{ "regression_limits": { "generated": { "orphan_files": { "new_findings": "never" } } } }"#,
            "regression_limits.generated.orphan_files",
        ),
        (
            "vendored",
            r#"{ "regression_limits": { "vendored": { "orphan_files": { "ratio_rise": 1 } } } }"#,
            "regression_limits.vendored.orphan_files",
        ),
    ];

    for (name, raw, expected_key) in cases {
        let dir = TempFolder::new(&format!("orphan-limit-{name}"));
        std::fs::write(dir.path().join("codeint.json"), raw).expect("write codeint.json");

        let error =
            read_configuration(dir.path()).expect_err("non-production regression limit must fail");
        let rendered = error.to_string();
        assert!(
            rendered.contains(expected_key)
                && rendered.contains("Orphan files")
                && rendered.contains("Production"),
            "unexpected error for {name}: {rendered}"
        );
    }
}

#[test]
fn cli_budget_overrides_accept_production_orphan_files_and_reject_other_source_sets() {
    let dir = TempFolder::new("orphan-cli-budget");
    let prod_override: BudgetOverride = "production.orphan_files=0.2".parse().expect("override");
    let overrides = ConfigOverrides {
        budgets: vec![prod_override],
        ..ConfigOverrides::default()
    };
    let resolved =
        read_resolved_configuration(dir.path(), &overrides).expect("resolved with CLI override");
    resolved
        .configuration
        .validate_cli_budget_metrics(&overrides.budgets, &[])
        .expect("production orphan_files CLI override is valid");
    resolved
        .configuration
        .validate_metrics(&[])
        .expect("resolved config is valid");

    for source_set in ["test", "generated", "vendored"] {
        let bad_override: BudgetOverride = format!("{source_set}.orphan_files=0.2")
            .parse()
            .expect("syntactically valid override");
        let error = resolved
            .configuration
            .validate_cli_budget_metrics(&[bad_override], &[])
            .expect_err("non-production CLI orphan_files must fail");
        let rendered = error.to_string();
        assert!(
            rendered.contains(&format!("{source_set}.orphan_files"))
                && rendered.contains("Orphan files")
                && rendered.contains("Production"),
            "unexpected error for {source_set}: {rendered}"
        );
    }
}

#[test]
fn configuration_schema_restricts_orphan_files_to_production_budgets_and_regression_limits() {
    let schema: Value = serde_json::from_str(&configuration_schema()).expect("valid JSON schema");
    let defs = &schema["$defs"];

    for (def_name, should_include_orphan_files) in [
        ("ProductionMetricBudgets", true),
        ("MetricBudgets", false),
        ("RegressionLimitsFile", true),
        ("ProductionMetricRegressionLimits", true),
        ("MetricRegressionLimits", false),
    ] {
        assert_eq!(
            defs[def_name]["properties"].get("orphan_files").is_some(),
            should_include_orphan_files,
            "unexpected orphan_files presence in {def_name}"
        );
    }

    let limit_props = &defs["RegressionLimitsFile"]["properties"];
    assert_eq!(
        limit_props["production"]["$ref"],
        "#/$defs/ProductionMetricRegressionLimits"
    );
    for other in ["test", "generated", "vendored"] {
        assert_eq!(limit_props[other]["$ref"], "#/$defs/MetricRegressionLimits");
    }
}

#[test]
fn resolved_settings_only_include_production_orphan_files_regression_limits() {
    let dir = TempFolder::new("orphan-resolved-settings");
    let resolved = read_resolved_configuration(dir.path(), &ConfigOverrides::default())
        .expect("default config resolves");
    assert!(
        resolved
            .settings
            .iter()
            .any(|setting| setting.key == "regression_limits.production.orphan_files.ratio_rise")
    );
    for source_set in ["test", "generated", "vendored"] {
        let key = format!("regression_limits.{source_set}.orphan_files.ratio_rise");
        assert!(
            resolved.settings.iter().all(|setting| setting.key != key),
            "unexpected setting {key}"
        );
    }
}
