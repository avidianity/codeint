use crate::NewFindingsRule;

fn read_json_config(json: &str) -> Result<crate::Configuration, crate::ConfigurationFileError> {
    crate::codeint_json::parse(json)
}

#[test]
fn parses_dependencies_ignore_budgets_and_regression_limits() {
    let config = read_json_config(
        r#"{
  "dependencies": {
    "ignore": ["tailwindcss", " @types/node "]
  },
  "budgets": {
    "unused_dependencies": 0.1,
    "unlisted_dependencies": 0,
    "dev_in_production_dependencies": 0
  },
  "regression_limits": {
    "unused_dependencies": { "ratio_rise": 0.05, "new_findings": "never" },
    "unlisted_dependencies": { "ratio_rise": 0, "new_findings": "any_new_finding" },
    "dev_in_production_dependencies": { "ratio_rise": 0, "new_findings": "any_new_finding" }
  }
}"#,
    )
    .expect("valid configuration");

    assert!(config.dependencies.is_ignored("tailwindcss"));
    assert!(config.dependencies.is_ignored("@types/node"));
    assert!(!config.dependencies.is_ignored("lodash"));

    assert_eq!(
        config
            .budgets
            .dependency_budget("unused_dependencies")
            .map(|b| b.ratio()),
        Some(0.1)
    );
    assert_eq!(
        config
            .budgets
            .dependency_budget("unlisted_dependencies")
            .map(|b| b.ratio()),
        Some(0.0)
    );
    assert_eq!(
        config
            .budgets
            .dependency_budget("dev_in_production_dependencies")
            .map(|b| b.ratio()),
        Some(0.0)
    );

    let unused_limit = config
        .regression_limits
        .limit_for_dependency("unused_dependencies");
    assert_eq!(unused_limit.ratio_rise.ratio(), 0.05);
    assert_eq!(unused_limit.new_findings, NewFindingsRule::Never);
}

#[test]
fn rejects_empty_dependencies_ignore_entry_and_unknown_dependencies_key() {
    let empty_err = read_json_config(r#"{ "dependencies": { "ignore": ["   "] } }"#).unwrap_err();
    assert!(empty_err.to_string().contains("dependencies.ignore[0]"));

    let unknown_err = read_json_config(r#"{ "dependencies": { "allow": ["foo"] } }"#).unwrap_err();
    assert!(unknown_err.to_string().contains("dependencies.allow"));
}

#[test]
fn defaults_dependency_regression_limits_to_any_new_finding() {
    let config = read_json_config("{}").expect("empty config");
    for key in [
        "unused_dependencies",
        "unlisted_dependencies",
        "dev_in_production_dependencies",
    ] {
        let limit = config.regression_limits.limit_for_dependency(key);
        assert_eq!(limit.new_findings, NewFindingsRule::AnyNewFinding);
    }
}
