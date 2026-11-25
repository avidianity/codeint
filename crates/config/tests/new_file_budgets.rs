use std::path::PathBuf;

use codeint_config::{
    ConfigOverrides, Configuration, ConfigurationFileError, ResolvedConfiguration,
    configuration_schema, read_configuration, read_resolved_configuration,
};
use codeint_model::{Metric, SourceSet};
use serde_json::{Value, json};

struct DirGuard(PathBuf);

impl Drop for DirGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn fresh_dir(label: &str) -> DirGuard {
    let path = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("nfb-{label}"));
    let _ = std::fs::remove_dir_all(&path);
    std::fs::create_dir_all(&path).expect("mkdir");
    DirGuard(path)
}

fn load_ok(label: &str, body: &str) -> Configuration {
    let guard = fresh_dir(label);
    std::fs::write(guard.0.join("codeint.json"), body).expect("write");
    let cfg = read_configuration(&guard.0).expect("valid config");
    cfg.validate_metrics(&[]).expect("valid metrics");
    cfg
}

fn load_err(label: &str, file_name: &str, body: &str) -> ConfigurationFileError {
    let guard = fresh_dir(label);
    std::fs::write(guard.0.join(file_name), body).expect("write");
    read_configuration(&guard.0).expect_err("expected config error")
}

fn resolve(label: &str, body: Option<&str>) -> ResolvedConfiguration {
    let guard = fresh_dir(label);
    if let Some(text) = body {
        std::fs::write(guard.0.join("codeint.json"), text).expect("write");
    }
    read_resolved_configuration(&guard.0, &ConfigOverrides::default()).expect("resolve config")
}

#[test]
fn parses_valid_new_file_budgets_across_source_sets_and_defaults_empty() {
    let guard = fresh_dir("empty");
    let default_cfg = read_configuration(&guard.0).expect("default config");
    assert!(default_cfg.new_file_budgets.is_empty());

    let cfg = load_ok(
        "valid",
        r#"{
  "new_file_budgets": {
    "production": {
      "erosion": 0.0,
      "cognitive_complexity": 0.05,
      "duplication": 0.0,
      "verbosity": 0.02,
      "import_cycles": 0.0,
      "boundaries": 0.0,
      "orphan_files": 0.0,
      "hazards": 0.0,
      "test_gap": 0.1
    },
    "test": { "erosion": 0.2, "hazards": 0.05 },
    "generated": { "duplication": 0.5 },
    "vendored": { "verbosity": 0.4 }
  }
}"#,
    );
    assert!(!cfg.new_file_budgets.is_empty());
    let expected = [
        (SourceSet::Production, Metric::Erosion, 0.0),
        (SourceSet::Production, Metric::CognitiveComplexity, 0.05),
        (SourceSet::Production, Metric::OrphanFiles, 0.0),
        (SourceSet::Production, Metric::TestGap, 0.1),
        (SourceSet::Test, Metric::Erosion, 0.2),
        (SourceSet::Generated, Metric::Duplication, 0.5),
        (SourceSet::Vendored, Metric::Verbosity, 0.4),
    ];
    for (set, metric, ratio) in expected {
        assert_eq!(
            cfg.new_file_budgets.budget(set, metric).map(|b| b.ratio()),
            Some(ratio)
        );
    }
}

#[test]
fn rejects_production_only_repository_wide_and_unknown_new_file_budget_keys() {
    let disallowed_cases = [
        (
            "test-gap-test",
            "codeint.jsonc",
            r#"{ "new_file_budgets": { "test": { "test_gap": 0.1 } } }"#,
            "new_file_budgets.test.test_gap",
            "Production",
        ),
        (
            "test-gap-gen",
            "codeint.jsonc",
            r#"{ "new_file_budgets": { "generated": { "test_gap": 0.1 } } }"#,
            "new_file_budgets.generated.test_gap",
            "Production",
        ),
        (
            "test-gap-vend",
            "codeint.jsonc",
            r#"{ "new_file_budgets": { "vendored": { "test_gap": 0.1 } } }"#,
            "new_file_budgets.vendored.test_gap",
            "Production",
        ),
        (
            "orphan-test",
            "codeint.jsonc",
            r#"{ "new_file_budgets": { "test": { "orphan_files": 0.0 } } }"#,
            "new_file_budgets.test.orphan_files",
            "Orphan files",
        ),
        (
            "orphan-gen",
            "codeint.jsonc",
            r#"{ "new_file_budgets": { "generated": { "orphan_files": 0.0 } } }"#,
            "new_file_budgets.generated.orphan_files",
            "Orphan files",
        ),
        (
            "orphan-vend",
            "codeint.jsonc",
            r#"{ "new_file_budgets": { "vendored": { "orphan_files": 0.0 } } }"#,
            "new_file_budgets.vendored.orphan_files",
            "Orphan files",
        ),
        (
            "dep-top-unused",
            "codeint.json",
            r#"{ "new_file_budgets": { "unused_dependencies": 0.0 } }"#,
            "new_file_budgets.unused_dependencies",
            "unknown key",
        ),
        (
            "dep-top-unlisted",
            "codeint.json",
            r#"{ "new_file_budgets": { "unlisted_dependencies": 0.0 } }"#,
            "new_file_budgets.unlisted_dependencies",
            "unknown key",
        ),
        (
            "dep-top-dev",
            "codeint.json",
            r#"{ "new_file_budgets": { "dev_in_production_dependencies": 0.0 } }"#,
            "new_file_budgets.dev_in_production_dependencies",
            "unknown key",
        ),
        (
            "dep-nested-unused",
            "codeint.json",
            r#"{ "new_file_budgets": { "production": { "unused_dependencies": 0.0 } } }"#,
            "new_file_budgets.production.unused_dependencies",
            "unknown key",
        ),
    ];
    for (label, file_name, body, key, phrase) in disallowed_cases {
        let text = load_err(label, file_name, body).to_string();
        assert!(
            text.contains(file_name) && text.contains(key) && text.contains(phrase),
            "unexpected error for {label}: {text}"
        );
    }

    let guard = fresh_dir("unknown-metric");
    std::fs::write(
        guard.0.join("codeint.json"),
        r#"{ "new_file_budgets": { "production": { "not_a_metric": 0.1 } } }"#,
    )
    .expect("write");
    let unknown_cfg = read_configuration(&guard.0).expect("shape accepts external metric");
    let err = unknown_cfg
        .validate_metrics(&[])
        .expect_err("unknown metric must fail");
    assert!(
        err.to_string()
            .contains("new_file_budgets.production.not_a_metric")
    );
}

#[test]
fn configuration_schema_and_resolved_config_expose_new_file_budgets() {
    let schema: Value = serde_json::from_str(&configuration_schema()).expect("schema json");
    assert_eq!(
        schema["properties"]["new_file_budgets"]["$ref"],
        "#/$defs/NewFileBudgetsFile"
    );
    let def = &schema["$defs"]["NewFileBudgetsFile"];
    assert_eq!(def["additionalProperties"], json!(false));
    assert_eq!(
        def["properties"]["production"]["$ref"],
        "#/$defs/ProductionMetricBudgets"
    );
    for set in ["test", "generated", "vendored"] {
        assert_eq!(def["properties"][set]["$ref"], "#/$defs/MetricBudgets");
    }
    assert!(def["properties"].get("unused_dependencies").is_none());

    let default_resolved = resolve("resolved-default", None);
    let default_setting = default_resolved
        .settings
        .iter()
        .find(|item| item.key == "new_file_budgets")
        .expect("default new_file_budgets setting");
    assert_eq!(default_setting.value, json!({}));

    let file_resolved = resolve(
        "resolved-file",
        Some(
            r#"{ "new_file_budgets": { "production": { "erosion": 0.0 }, "test": { "verbosity": 0.1 } } }"#,
        ),
    );
    let pairs = [
        ("new_file_budgets.production.erosion", json!(0.0)),
        ("new_file_budgets.test.verbosity", json!(0.1)),
    ];
    for (key, value) in pairs {
        let found = file_resolved
            .settings
            .iter()
            .find(|item| item.key == key)
            .unwrap_or_else(|| panic!("missing setting {key}"));
        assert_eq!(found.value, value);
    }
}
