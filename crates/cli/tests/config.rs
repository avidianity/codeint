use std::fs;
use std::path::Path;
use std::process::{Command, Output};

use serde_json::{Value, json};
use tempfile::TempDir;

fn init_repo_with_config(config: Option<Value>) -> TempDir {
    let dir = TempDir::new().expect("tempdir");
    let status = Command::new("git")
        .current_dir(dir.path())
        .args(["init", "--quiet", "--initial-branch", "main"])
        .status()
        .expect("run git init");
    assert!(status.success(), "git init failed");
    if let Some(body) = config {
        fs::write(dir.path().join("codeint.json"), body.to_string()).expect("write codeint.json");
    }
    dir
}

fn invoke(cwd: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_codeint"))
        .current_dir(cwd)
        .args(args)
        .output()
        .expect("invoke codeint")
}

fn invoke_ok_stdout(cwd: &Path, args: &[&str]) -> String {
    let output = invoke(cwd, args);
    assert_eq!(
        output.status.code(),
        Some(0),
        "expected exit 0, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("utf8 stdout")
}

fn resolved_json(cwd: &Path, extra_args: &[&str]) -> Value {
    let mut args = vec!["config", "--resolved", "--format", "json"];
    args.extend_from_slice(extra_args);
    let stdout = invoke_ok_stdout(cwd, &args);
    serde_json::from_str(&stdout).expect("valid JSON")
}

fn assert_setting(payload: &Value, key: &str, expected_value: Value, expected_source: &str) {
    let settings = payload["settings"]
        .as_array()
        .expect("settings array in resolved config");
    let entry = settings
        .iter()
        .find(|item| item["key"] == key)
        .unwrap_or_else(|| panic!("missing setting {key} in {payload}"));
    assert_eq!(entry["value"], expected_value, "wrong value for {key}");
    assert_eq!(entry["source"], expected_source, "wrong source for {key}");
}

#[test]
fn config_resolved_reports_defaults_only_across_text_json_and_toon_formats() {
    let repo = init_repo_with_config(None);

    let payload = resolved_json(repo.path(), &[]);
    assert_eq!(payload["config_file_present"], false);
    assert!(
        payload["config_file"]
            .as_str()
            .expect("config_file string")
            .ends_with("codeint.json")
    );
    assert_setting(&payload, "source_sets.production", json!([]), "default");
    assert_setting(&payload, "source_sets.test", json!([]), "default");
    assert_setting(&payload, "source_sets.generated", json!([]), "default");
    assert_setting(&payload, "source_sets.vendored", json!([]), "default");
    assert_setting(&payload, "budgets", json!({}), "default");
    assert_setting(
        &payload,
        "regression_limits.production.erosion.ratio_rise",
        json!(0.01),
        "default",
    );
    assert_setting(
        &payload,
        "regression_limits.production.erosion.new_findings",
        json!("numerator_rise_with_new_findings"),
        "default",
    );
    assert_setting(
        &payload,
        "regression_limits.production.import_cycles.new_findings",
        json!("any_new_finding"),
        "default",
    );
    assert_setting(&payload, "languages", Value::Null, "default");
    assert_setting(&payload, "plugins", json!([]), "default");
    assert_setting(&payload, "plugin_limits.memory_mb", json!(64), "default");
    assert_setting(&payload, "plugin_limits.timeout_ms", json!(5000), "default");
    assert_setting(&payload, "boundaries", Value::Null, "default");

    let text = invoke_ok_stdout(repo.path(), &["config", "--resolved"]);
    assert!(text.contains("source_sets.production = [] [default]"));
    assert!(text.contains("plugin_limits.memory_mb = 64 [default]"));
    assert!(text.contains("plugin_limits.timeout_ms = 5000 [default]"));

    let toon = invoke_ok_stdout(repo.path(), &["config", "--resolved", "--format", "toon"]);
    let decoded: Value = toon_format::decode_default(&toon).expect("valid TOON");
    assert_eq!(decoded["config_file_present"], false);
    assert_setting(&decoded, "plugin_limits.memory_mb", json!(64), "default");
}

#[test]
fn config_resolved_merges_file_and_cli_overrides_with_per_field_provenance() {
    let repo = init_repo_with_config(Some(json!({
        "source_sets": {
            "test": ["spec/**"]
        },
        "budgets": {
            "production": {
                "erosion": 0.25,
                "duplication": 0.1
            }
        },
        "regression_limits": {
            "erosion": {
                "ratio_rise": 0.05
            },
            "test": {
                "erosion": {
                    "new_findings": "never"
                }
            }
        },
        "plugin_limits": {
            "memory_mb": 128
        },
        "boundaries": {
            "preset": "layered",
            "strict_zones": true
        }
    })));

    let file_only = resolved_json(repo.path(), &[]);
    assert_eq!(file_only["config_file_present"], true);
    assert_setting(&file_only, "source_sets.test", json!(["spec/**"]), "file");
    assert_setting(&file_only, "source_sets.production", json!([]), "default");
    assert_setting(
        &file_only,
        "budgets.production.erosion",
        json!(0.25),
        "file",
    );
    assert_setting(
        &file_only,
        "budgets.production.duplication",
        json!(0.1),
        "file",
    );
    assert_setting(
        &file_only,
        "regression_limits.production.erosion.ratio_rise",
        json!(0.05),
        "file",
    );
    assert_setting(
        &file_only,
        "regression_limits.production.erosion.new_findings",
        json!("numerator_rise_with_new_findings"),
        "default",
    );
    assert_setting(
        &file_only,
        "regression_limits.test.erosion.new_findings",
        json!("never"),
        "file",
    );
    assert_setting(&file_only, "plugin_limits.memory_mb", json!(128), "file");
    assert_setting(
        &file_only,
        "plugin_limits.timeout_ms",
        json!(5000),
        "default",
    );
    assert_setting(&file_only, "boundaries.preset", json!("layered"), "file");
    assert_setting(&file_only, "boundaries.strict_zones", json!(true), "file");
    assert_setting(
        &file_only,
        "boundaries.source_sets",
        json!(["production"]),
        "default",
    );

    let with_cli = resolved_json(
        repo.path(),
        &[
            "--budget",
            "production.erosion=0.15",
            "--budget",
            "test.verbosity=0.3",
            "--source-set",
            "test=e2e/**",
            "--plugin-memory-mb",
            "256",
            "--plugin-timeout-ms",
            "9000",
        ],
    );
    assert_setting(&with_cli, "budgets.production.erosion", json!(0.15), "cli");
    assert_setting(
        &with_cli,
        "budgets.production.duplication",
        json!(0.1),
        "file",
    );
    assert_setting(&with_cli, "budgets.test.verbosity", json!(0.3), "cli");
    assert_setting(&with_cli, "source_sets.test", json!(["e2e/**"]), "cli");
    assert_setting(&with_cli, "source_sets.production", json!([]), "default");
    assert_setting(&with_cli, "plugin_limits.memory_mb", json!(256), "cli");
    assert_setting(&with_cli, "plugin_limits.timeout_ms", json!(9000), "cli");

    let text = invoke_ok_stdout(
        repo.path(),
        &[
            "config",
            "--resolved",
            "--budget",
            "production.erosion=0.15",
            "--plugin-memory-mb",
            "256",
        ],
    );
    assert!(text.contains("budgets.production.erosion = 0.15 [cli]"));
    assert!(text.contains("budgets.production.duplication = 0.1 [file]"));
    assert!(text.contains("plugin_limits.memory_mb = 256 [cli]"));
    assert!(text.contains("plugin_limits.timeout_ms = 5000 [default]"));
}

#[test]
fn config_resolved_rejects_invalid_config_invalid_override_and_non_repository() {
    let bad_repo = init_repo_with_config(Some(json!({ "unknown_key": true })));
    let bad_cfg = invoke(bad_repo.path(), &["config", "--resolved"]);
    assert_eq!(bad_cfg.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&bad_cfg.stderr).contains("unknown_key"));

    let good_repo = init_repo_with_config(None);
    let bad_budget = invoke(
        good_repo.path(),
        &["config", "--resolved", "--budget", "production.erosion=1.5"],
    );
    assert_eq!(bad_budget.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&bad_budget.stderr).contains("between 0 and 1"));

    let missing_flag = invoke(good_repo.path(), &["config"]);
    assert_eq!(missing_flag.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&missing_flag.stderr).contains("--resolved"));

    let non_repo = TempDir::new().expect("tempdir outside git");
    let outside = invoke(non_repo.path(), &["config", "--resolved"]);
    assert_eq!(outside.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&outside.stderr).contains("not inside a Git repository"));
}

#[test]
fn config_resolved_accepts_subdirectory_path_argument() {
    let repo = init_repo_with_config(Some(json!({
        "plugin_limits": { "memory_mb": 96 }
    })));
    fs::create_dir_all(repo.path().join("src")).expect("create src dir");

    let payload = resolved_json(repo.path(), &["src"]);
    assert_eq!(payload["config_file_present"], true);
    assert_setting(&payload, "plugin_limits.memory_mb", json!(96), "file");
}
