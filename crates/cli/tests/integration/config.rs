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
        json!("numerator_rise"),
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
    assert_setting(&payload, "dependencies.ignore", json!([]), "default");
    assert_setting(&payload, "duplication.near_clones", json!(false), "default");
    assert_setting(
        &payload,
        "duplication.min_similarity",
        json!(0.85),
        "default",
    );
    assert_setting(&payload, "entry_points", json!([]), "default");
    assert_setting(
        &payload,
        "regression_limits.production.cognitive_complexity.new_findings",
        json!("numerator_rise"),
        "default",
    );
    assert_setting(
        &payload,
        "regression_limits.production.orphan_files.new_findings",
        json!("any_new_finding"),
        "default",
    );
    assert_setting(
        &payload,
        "regression_limits.unused_dependencies.new_findings",
        json!("any_new_finding"),
        "default",
    );

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
            },
            "unused_dependencies": 0.0
        },
        "regression_limits": {
            "erosion": {
                "ratio_rise": 0.05
            },
            "unused_dependencies": {
                "ratio_rise": 0.02
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
        },
        "dependencies": {
            "ignore": ["tailwindcss"]
        },
        "duplication": {
            "near_clones": true,
            "min_similarity": 0.9
        },
        "entry_points": ["src/worker.ts"]
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
        "budgets.unused_dependencies",
        json!(0.0),
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
        "regression_limits.unused_dependencies.ratio_rise",
        json!(0.02),
        "file",
    );
    assert_setting(
        &file_only,
        "regression_limits.production.erosion.new_findings",
        json!("numerator_rise"),
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
    assert_setting(
        &file_only,
        "dependencies.ignore",
        json!(["tailwindcss"]),
        "file",
    );
    assert_setting(&file_only, "duplication.near_clones", json!(true), "file");
    assert_setting(&file_only, "duplication.min_similarity", json!(0.9), "file");
    assert_setting(&file_only, "entry_points", json!(["src/worker.ts"]), "file");

    let with_cli = resolved_json(
        repo.path(),
        &[
            "--budget",
            "production.erosion=0.15",
            "--budget",
            "production.cognitive_complexity=0.2",
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
        "budgets.production.cognitive_complexity",
        json!(0.2),
        "cli",
    );
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

fn assert_config_rejected(
    cwd: &Path,
    args: &[&str],
    expected_stderr: &str,
    forbidden_stderr: Option<&str>,
) {
    let output = invoke(cwd, args);
    assert_eq!(output.status.code(), Some(2), "args={args:?}");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains(expected_stderr),
        "expected {expected_stderr:?} in stderr: {stderr}"
    );
    if let Some(forbidden) = forbidden_stderr {
        assert!(
            !stderr.contains(forbidden),
            "did not expect {forbidden:?} in stderr: {stderr}"
        );
    }
}

#[test]
fn config_resolved_tracks_boundary_provenance_normalizes_negative_zero_and_validates_keys() {
    let empty_boundaries = init_repo_with_config(Some(json!({ "boundaries": {} })));
    let empty_payload = resolved_json(empty_boundaries.path(), &[]);
    assert_setting(&empty_payload, "boundaries.preset", Value::Null, "default");
    assert_setting(&empty_payload, "boundaries.zones", json!({}), "default");
    assert_setting(&empty_payload, "boundaries.rules", json!([]), "default");

    let zones_only = init_repo_with_config(Some(json!({
        "boundaries": {
            "zones": {
                "core": { "patterns": ["src/**"] }
            }
        }
    })));
    let zones_payload = resolved_json(zones_only.path(), &[]);
    assert_setting(
        &zones_payload,
        "boundaries.zones",
        json!({ "core": { "patterns": ["src/**"] } }),
        "file",
    );
    assert_setting(&zones_payload, "boundaries.rules", json!([]), "default");

    let neg_zero_repo = init_repo_with_config(None);
    fs::write(
        neg_zero_repo.path().join("codeint.json"),
        r#"{"budgets":{"production":{"erosion":-0.0}},"regression_limits":{"production":{"erosion":{"ratio_rise":-0.0}}}}"#,
    )
    .expect("write negative zero config");
    let neg_stdout = invoke_ok_stdout(
        neg_zero_repo.path(),
        &[
            "config",
            "--resolved",
            "--format",
            "json",
            "--budget",
            "production.duplication=-0.0",
        ],
    );
    assert!(
        !neg_stdout.contains("-0.0"),
        "unexpected -0.0: {neg_stdout}"
    );
    let neg_payload: Value = serde_json::from_str(&neg_stdout).expect("valid json");
    assert_setting(
        &neg_payload,
        "budgets.production.erosion",
        json!(0.0),
        "file",
    );
    assert_setting(
        &neg_payload,
        "budgets.production.duplication",
        json!(0.0),
        "cli",
    );
    assert_setting(
        &neg_payload,
        "regression_limits.production.erosion.ratio_rise",
        json!(0.0),
        "file",
    );

    let unknown_file_metric = init_repo_with_config(Some(json!({
        "budgets": { "production": { "bogus_metric": 0.2 } }
    })));
    assert_config_rejected(
        unknown_file_metric.path(),
        &["config", "--resolved"],
        "bogus_metric",
        None,
    );

    let unknown_lang = init_repo_with_config(Some(json!({
        "languages": [{ "plugin": "nonexistent-lang", "include": ["src/**"] }]
    })));
    assert_config_rejected(
        unknown_lang.path(),
        &["config", "--resolved"],
        "nonexistent-lang",
        None,
    );

    let with_metric_plugin = init_repo_with_config(Some(json!({
        "plugins": [{
            "name": "long-functions",
            "kind": "metric",
            "path": "plugins/long_functions.wasm",
            "sha256": "0000000000000000000000000000000000000000000000000000000000000000"
        }],
        "budgets": { "production": { "long_functions": 0.25 } }
    })));
    let plugin_payload = resolved_json(with_metric_plugin.path(), &[]);
    assert_setting(
        &plugin_payload,
        "budgets.production.long_functions",
        json!(0.25),
        "file",
    );

    let clean_repo = init_repo_with_config(None);
    assert_config_rejected(
        clean_repo.path(),
        &["config", "--resolved", "--budget", "production.bogus=0.2"],
        "--budget",
        Some("codeint.json"),
    );
    assert_config_rejected(
        clean_repo.path(),
        &[
            "config",
            "--resolved",
            "--budget",
            "production. erosion =0.2",
        ],
        "not a valid Metric key",
        None,
    );
    assert_config_rejected(
        clean_repo.path(),
        &["config", "--resolved", "--source-set", "test=[unclosed"],
        "not a valid glob",
        Some("codeint.json"),
    );
}

#[cfg(unix)]
fn assert_broken_config_symlink(file_name: &str, missing_target: &str) {
    use std::os::unix::fs::symlink;

    let repo = init_repo_with_config(None);
    symlink(
        repo.path().join(missing_target),
        repo.path().join(file_name),
    )
    .expect("create broken config symlink");

    let output = invoke(repo.path(), &["config", "--resolved"]);
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    let expected = format!("{file_name} is a broken symlink");
    assert!(
        stderr.contains(&expected),
        "expected {expected:?}, got: {stderr}"
    );
}

#[cfg(unix)]
#[test]
fn config_resolved_reports_broken_symlink_for_codeint_json() {
    assert_broken_config_symlink("codeint.json", "missing-target.json");
}

#[test]
fn config_resolved_reads_codeint_jsonc_and_rejects_ambiguous_config_files() {
    let repo = init_repo_with_config(None);
    fs::write(
        repo.path().join("codeint.jsonc"),
        concat!(
            "// jsonc config\n",
            "{\n",
            "  /* plugin memory override */\n",
            "  \"plugin_limits\": {\n",
            "    \"memory_mb\": 128,\n",
            "  },\n",
            "}\n",
        ),
    )
    .expect("write codeint.jsonc");

    let payload = resolved_json(repo.path(), &[]);
    assert_eq!(payload["config_file_present"], true);
    assert!(
        payload["config_file"]
            .as_str()
            .expect("config_file string")
            .ends_with("codeint.jsonc")
    );
    assert_setting(&payload, "plugin_limits.memory_mb", json!(128), "file");

    fs::write(repo.path().join("codeint.json"), "{}\n").expect("write codeint.json");
    assert_config_rejected(
        repo.path(),
        &["config", "--resolved"],
        "both codeint.json and codeint.jsonc exist at the repository root",
        None,
    );
}

#[cfg(unix)]
#[test]
fn config_resolved_reports_broken_symlink_for_codeint_jsonc() {
    assert_broken_config_symlink("codeint.jsonc", "missing-target.jsonc");
}

#[test]
fn config_resolved_canonicalizes_deprecated_numerator_rise_alias_and_shows_hazards_default() {
    let default_repo = init_repo_with_config(None);
    let default_payload = resolved_json(default_repo.path(), &[]);
    assert_setting(
        &default_payload,
        "regression_limits.production.erosion.new_findings",
        json!("numerator_rise"),
        "default",
    );
    assert_setting(
        &default_payload,
        "regression_limits.production.hazards.new_findings",
        json!("any_new_finding"),
        "default",
    );

    let alias_repo = init_repo_with_config(Some(json!({
        "regression_limits": {
            "erosion": { "new_findings": "numerator_rise_with_new_findings" }
        }
    })));
    let alias_payload = resolved_json(alias_repo.path(), &[]);
    assert_setting(
        &alias_payload,
        "regression_limits.production.erosion.new_findings",
        json!("numerator_rise"),
        "file",
    );
}

#[test]
fn config_resolved_reports_new_file_budgets_and_rejects_invalid_keys_at_exit_code_2() {
    let default_repo = init_repo_with_config(None);
    let default_payload = resolved_json(default_repo.path(), &[]);
    assert_setting(&default_payload, "new_file_budgets", json!({}), "default");

    let configured_repo = init_repo_with_config(Some(json!({
        "new_file_budgets": {
            "production": {
                "erosion": 0.0,
                "orphan_files": 0.0
            },
            "test": {
                "verbosity": 0.1
            }
        }
    })));
    let payload = resolved_json(configured_repo.path(), &[]);
    assert_setting(
        &payload,
        "new_file_budgets.production.erosion",
        json!(0.0),
        "file",
    );
    assert_setting(
        &payload,
        "new_file_budgets.production.orphan_files",
        json!(0.0),
        "file",
    );
    assert_setting(
        &payload,
        "new_file_budgets.test.verbosity",
        json!(0.1),
        "file",
    );

    let bad_cases = [
        (
            json!({ "new_file_budgets": { "test": { "test_gap": 0.1 } } }),
            "new_file_budgets.test.test_gap",
            "new_file_budgets.production.test_gap",
        ),
        (
            json!({ "new_file_budgets": { "generated": { "orphan_files": 0.0 } } }),
            "new_file_budgets.generated.orphan_files",
            "new_file_budgets.production",
        ),
        (
            json!({ "new_file_budgets": { "unused_dependencies": 0.0 } }),
            "new_file_budgets.unused_dependencies",
            "allowed keys",
        ),
        (
            json!({ "new_file_budgets": { "production": { "bogus_metric": 0.1 } } }),
            "new_file_budgets.production.bogus_metric",
            "allowed keys",
        ),
    ];

    for (config, expected_key, expected_remedy) in bad_cases {
        let repo = init_repo_with_config(Some(config));
        let output = invoke(repo.path(), &["config", "--resolved"]);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.code(), Some(2), "stderr: {stderr}");
        assert!(
            stderr.contains(expected_key) && stderr.contains(expected_remedy),
            "missing {expected_key} or {expected_remedy} in stderr: {stderr}"
        );
    }
}
