use std::path::{Path, PathBuf};

use codeint_config::{
    ConfigOverrides, ConfigProvenance, ConfigurationFileError, configuration_schema,
    read_configuration, read_resolved_configuration,
};
use serde_json::{Value, json};

fn with_temp_repo<T>(tag: &str, file: Option<(&str, &str)>, run: impl FnOnce(&Path) -> T) -> T {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("per-file-regression")
        .join(tag);
    if dir.exists() {
        std::fs::remove_dir_all(&dir).expect("clear test folder");
    }
    std::fs::create_dir_all(&dir).expect("create test folder");
    file.into_iter().for_each(|(name, content)| {
        std::fs::write(dir.join(name), content).expect("write config file");
    });
    let output = run(&dir);
    let _ = std::fs::remove_dir_all(&dir);
    output
}

#[test]
fn per_file_defaults_to_false_and_parses_true_and_false() {
    let default_cfg = with_temp_repo("default", None, |root| {
        read_configuration(root).expect("default config")
    });
    assert!(!default_cfg.regression_limits.per_file());

    let false_cfg = with_temp_repo(
        "explicit-false",
        Some((
            "codeint.json",
            r#"{ "regression_limits": { "per_file": false } }"#,
        )),
        |root| read_configuration(root).expect("false config"),
    );
    assert!(!false_cfg.regression_limits.per_file());

    let true_cfg = with_temp_repo(
        "explicit-true",
        Some((
            "codeint.jsonc",
            "{\n  // per-file check\n  \"regression_limits\": { \"per_file\": true, },\n}\n",
        )),
        |root| {
            let cfg = read_configuration(root).expect("true config");
            cfg.validate_metrics(&[]).expect("valid metrics");
            cfg
        },
    );
    assert!(true_cfg.regression_limits.per_file());

    let default_json = serde_json::to_string(&default_cfg).expect("serialize default");
    assert_eq!(
        default_json,
        serde_json::to_string(&false_cfg).expect("serialize false")
    );
    assert_ne!(
        default_json,
        serde_json::to_string(&true_cfg).expect("serialize true")
    );
}

#[test]
fn per_file_rejects_non_boolean_values_and_nested_placement() {
    for (tag, body) in [
        (
            "string",
            r#"{ "regression_limits": { "per_file": "true" } }"#,
        ),
        ("number", r#"{ "regression_limits": { "per_file": 1 } }"#),
        (
            "nested",
            r#"{ "regression_limits": { "production": { "per_file": true } } }"#,
        ),
    ] {
        let err = with_temp_repo(tag, Some(("codeint.json", body)), |root| {
            read_configuration(root).expect_err("invalid per_file must fail")
        });
        assert!(
            matches!(err, ConfigurationFileError::WrongType { .. }),
            "unexpected error for {tag}: {err}"
        );
    }
}

#[test]
fn configuration_schema_and_resolved_settings_expose_per_file() {
    let schema: Value = serde_json::from_str(&configuration_schema()).expect("schema json");
    let per_file_schema = &schema["$defs"]["RegressionLimitsFile"]["properties"]["per_file"];
    assert_eq!(per_file_schema["type"], json!("boolean"));
    assert!(
        per_file_schema["description"]
            .as_str()
            .is_some_and(|desc| desc.contains("modified file"))
    );

    for (tag, file, expected_val, expected_src) in [
        ("res-default", None, json!(false), ConfigProvenance::Default),
        (
            "res-file",
            Some((
                "codeint.json",
                r#"{ "regression_limits": { "per_file": true } }"#,
            )),
            json!(true),
            ConfigProvenance::File,
        ),
    ] {
        let resolved = with_temp_repo(tag, file, |root| {
            read_resolved_configuration(root, &ConfigOverrides::default()).expect("resolve")
        });
        let setting = resolved
            .settings
            .iter()
            .find(|item| item.key == "regression_limits.per_file")
            .expect("regression_limits.per_file setting");
        assert_eq!(setting.value, expected_val);
        assert_eq!(setting.source, expected_src);
    }
}
