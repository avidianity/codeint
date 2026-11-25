use std::fs;
use std::path::Path;
use std::process::Command;

use codeint_plugin_host::{sha256, sha256_hex};
use serde_json::{Value, json};
use tempfile::TempDir;

use crate::common::{
    init_git_repo, run_codeint_output, run_codeint_text, run_git, write_repo_files,
};

fn read_json_file(path: &Path) -> Value {
    let text = fs::read_to_string(path).expect("read json file");
    serde_json::from_str(&text).expect("valid json file")
}

fn branching_ts_function(name: &str, branches: usize) -> String {
    let mut body = String::new();
    for index in 1..=branches {
        body.push_str(&format!("  if (value === {index}) return {index};\n"));
    }
    format!("export function {name}(value: number): number {{\n{body}  return 0;\n}}\n")
}

fn run_diff_text(
    dir: &Path,
    cache_dir: &Path,
    args: &[&str],
    expected_code: i32,
) -> (String, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_codeint"))
        .args(args)
        .current_dir(dir)
        .env("CODEINT_CACHE_DIR", cache_dir)
        .output()
        .expect("run codeint diff");
    let stdout = String::from_utf8(output.stdout).expect("utf8 stdout");
    let stderr = String::from_utf8(output.stderr).expect("utf8 stderr");
    assert_eq!(
        output.status.code(),
        Some(expected_code),
        "codeint {args:?} exited {:?}\nstdout: {stdout}\nstderr: {stderr}",
        output.status.code()
    );
    (stdout, stderr)
}

#[test]
fn config_discovery_supports_all_four_paths_and_rejects_any_ambiguity() {
    let jsonc_body =
        "{\n  // custom test glob\n  \"source_sets\": { \"test\": [\"spec/**\",], },\n}\n";
    for relative_path in [
        "codeint.json",
        "codeint.jsonc",
        ".codeint/config.json",
        ".codeint/config.jsonc",
    ] {
        let repo = init_git_repo(&[
            (relative_path, jsonc_body),
            ("src/app.ts", "export const app = 1;\n"),
            ("spec/app.ts", "export const spec = 1;\n"),
        ]);

        let (check_out, _) = run_codeint_text(repo.path(), &["check", "--format", "json"], 0);
        let report: Value = serde_json::from_str(&check_out).expect("valid check json");
        assert_eq!(report["source_sets"]["production"]["files"], 1);
        assert_eq!(report["source_sets"]["test"]["files"], 1);

        let (config_out, _) = run_codeint_text(
            repo.path(),
            &["config", "--resolved", "--format", "json"],
            0,
        );
        let resolved: Value = serde_json::from_str(&config_out).expect("valid config json");
        assert_eq!(resolved["config_file_present"], true);
        let config_file = resolved["config_file"]
            .as_str()
            .expect("config_file string")
            .replace('\\', "/");
        assert!(
            config_file.ends_with(relative_path),
            "expected config_file ending with {relative_path}, got {config_file}"
        );

        let (doctor_out, _) = run_codeint_text(repo.path(), &["doctor", "--format", "json"], 0);
        let doctor: Value = serde_json::from_str(&doctor_out).expect("valid doctor json");
        let config_check = doctor["checks"]
            .as_array()
            .expect("checks array")
            .iter()
            .find(|check| check["id"] == "config")
            .expect("config check");
        assert_eq!(config_check["status"], "pass");
        assert_eq!(config_check["summary"], format!("{relative_path} is valid"));

        let (_, init_err) = run_codeint_text(repo.path(), &["init"], 2);
        assert!(
            init_err.contains(relative_path),
            "expected init error to name {relative_path}, got: {init_err}"
        );
    }

    let conflict_repo = init_git_repo(&[
        ("codeint.json", "{}\n"),
        (".codeint/config.jsonc", "{}\n"),
        ("src/app.ts", "export const app = 1;\n"),
    ]);
    let (_, conflict_err) = run_codeint_text(conflict_repo.path(), &["check"], 2);
    assert!(
        conflict_err.contains("codeint.json") && conflict_err.contains(".codeint/config.jsonc"),
        "expected both conflicting files in stderr, got: {conflict_err}"
    );

    write_repo_files(conflict_repo.path(), &[(".codeint/config.json", "{}\n")]);
    let (_, triple_err) = run_codeint_text(conflict_repo.path(), &["config", "--resolved"], 2);
    assert!(
        triple_err.contains("codeint.json")
            && triple_err.contains(".codeint/config.json")
            && triple_err.contains(".codeint/config.jsonc"),
        "expected all three conflicting files in stderr, got: {triple_err}"
    );
}

#[test]
fn dot_codeint_config_validation_errors_name_exact_active_path() {
    let repo = init_git_repo(&[
        (
            ".codeint/config.jsonc",
            "{\n  \"unknown_setting\": true,\n}\n",
        ),
        ("src/app.ts", "export const app = 1;\n"),
    ]);

    let (_, stderr) = run_codeint_text(repo.path(), &["check"], 2);
    assert!(
        stderr.contains(".codeint/config.jsonc has an unknown key unknown_setting"),
        "expected exact .codeint/config.jsonc path in stderr, got: {stderr}"
    );
}

#[test]
fn ratchet_update_creates_tightens_refuses_regression_resets_and_refreshes_version_changes() {
    let repo = init_git_repo(&[
        (
            "src/main.ts",
            "import { calc } from './calc';\nexport const out = calc(1);\n",
        ),
        ("src/calc.ts", &branching_ts_function("calc", 12)),
        (
            "package.json",
            r#"{ "name": "demo", "dependencies": { "unused-pkg": "1.0.0" } }"#,
        ),
    ]);

    let (create_out, _) = run_codeint_text(repo.path(), &["ratchet", "update"], 0);
    assert!(
        create_out.contains(".codeint/ratchet.json"),
        "expected create summary to mention ratchet path, got: {create_out}"
    );

    let ratchet_path = repo.path().join(".codeint/ratchet.json");
    assert!(ratchet_path.is_file());
    let created = read_json_file(&ratchet_path);
    assert_eq!(created["schema_version"], 1);
    let initial_erosion = created["source_sets"]["production"]["erosion"]["ceiling"]
        .as_f64()
        .expect("erosion ceiling");
    assert!(initial_erosion > 0.0);
    assert_eq!(
        created["source_sets"]["production"]["erosion"]["metric_version"],
        1
    );
    assert_eq!(
        created["source_sets"]["production"]["verbosity"]["metric_version"],
        2
    );
    assert_eq!(
        created["dependencies"]["unused_dependencies"]["ceiling"],
        1.0
    );
    assert_eq!(
        created["dependencies"]["unused_dependencies"]["metric_version"],
        1
    );

    let (_, _) = run_codeint_text(repo.path(), &["check", "--format", "json"], 0);

    write_repo_files(
        repo.path(),
        &[(
            "src/calc.ts",
            "export function calc(value: number): number {\n  return value + 1;\n}\n",
        )],
    );
    let _ = run_codeint_text(repo.path(), &["ratchet", "update"], 0);
    let tightened = read_json_file(&ratchet_path);
    assert_eq!(
        tightened["source_sets"]["production"]["erosion"]["ceiling"],
        0.0
    );

    write_repo_files(
        repo.path(),
        &[("src/calc.ts", &branching_ts_function("calc", 14))],
    );
    let (_, refuse_err) = run_codeint_text(repo.path(), &["ratchet", "update"], 2);
    assert!(
        refuse_err.contains("erosion") && refuse_err.contains("codeint ratchet update --reset"),
        "expected refusal naming erosion and --reset, got: {refuse_err}"
    );
    let unchanged_after_refusal = read_json_file(&ratchet_path);
    assert_eq!(
        unchanged_after_refusal["source_sets"]["production"]["erosion"]["ceiling"],
        0.0
    );

    let _ = run_codeint_text(repo.path(), &["ratchet", "update", "--reset"], 0);
    let reset_ratchet = read_json_file(&ratchet_path);
    assert!(
        reset_ratchet["source_sets"]["production"]["erosion"]["ceiling"]
            .as_f64()
            .expect("reset ceiling")
            > 0.0
    );

    let mut stale_version = read_json_file(&ratchet_path);
    stale_version["source_sets"]["production"]["verbosity"] =
        json!({ "ceiling": 0.0, "metric_version": 1 });
    fs::write(
        &ratchet_path,
        format!(
            "{}\n",
            serde_json::to_string_pretty(&stale_version).expect("serialize stale ratchet")
        ),
    )
    .expect("write stale ratchet");

    let _ = run_codeint_text(repo.path(), &["ratchet", "update"], 0);
    let refreshed = read_json_file(&ratchet_path);
    assert_eq!(
        refreshed["source_sets"]["production"]["verbosity"]["metric_version"],
        2
    );
}

#[test]
fn check_and_diff_enforce_ratchet_report_failures_and_fail_loudly_on_version_mismatch() {
    let repo = init_git_repo(&[
        (
            "src/main.ts",
            "import { calc } from './calc';\nexport const out = calc(1);\n",
        ),
        (
            "src/calc.ts",
            "export function calc(value: number): number {\n  return value + 1;\n}\n",
        ),
    ]);
    let cache = TempDir::new().expect("cache dir");

    let _ = run_codeint_text(repo.path(), &["ratchet", "update"], 0);
    run_git(repo.path(), &["add", ".codeint/ratchet.json"]);
    run_git(repo.path(), &["commit", "-q", "-m", "add ratchet"]);
    run_git(repo.path(), &["checkout", "-q", "-b", "feature"]);

    write_repo_files(
        repo.path(),
        &[("src/calc.ts", &branching_ts_function("calc", 14))],
    );

    let (check_json_out, check_err) =
        run_codeint_text(repo.path(), &["check", "--format", "json"], 1);
    let check_report: Value = serde_json::from_str(&check_json_out).expect("check json");
    let over_ratchet = check_report["over_ratchet"]
        .as_array()
        .expect("over_ratchet array");
    assert!(
        over_ratchet
            .iter()
            .any(|item| item["source_set"] == "production" && item["metric"] == "erosion"),
        "expected production.erosion in over_ratchet: {over_ratchet:?}"
    );
    assert!(
        check_err.contains("Ratchet ceiling"),
        "expected Ratchet ceiling message in stderr, got: {check_err}"
    );

    let saved_path = repo.path().join("saved-check.json");
    fs::write(&saved_path, &check_json_out).expect("write saved check json");
    let (rerendered_md, _) = run_codeint_text(
        repo.path(),
        &["report", "saved-check.json", "--format", "markdown"],
        0,
    );
    assert!(
        rerendered_md.contains("## Over Ratchet"),
        "expected Markdown Over Ratchet section, got:\n{rerendered_md}"
    );
    let (rerendered_sarif, _) = run_codeint_text(
        repo.path(),
        &["report", "saved-check.json", "--format", "sarif"],
        0,
    );
    let sarif: Value = serde_json::from_str(&rerendered_sarif).expect("valid sarif");
    assert!(
        sarif["runs"][0]["properties"]["ratchetFailures"]
            .as_array()
            .is_some_and(|items| !items.is_empty()),
        "expected ratchetFailures in SARIF properties: {sarif}"
    );

    let (diff_json_out, diff_err) = run_diff_text(
        repo.path(),
        cache.path(),
        &["diff", "main", "--format", "json"],
        1,
    );
    let diff_report: Value = serde_json::from_str(&diff_json_out).expect("diff json");
    assert!(
        diff_report["over_ratchet"]
            .as_array()
            .is_some_and(|items| items
                .iter()
                .any(|item| item["source_set"] == "production" && item["metric"] == "erosion")),
        "expected production.erosion in diff over_ratchet: {diff_report}"
    );
    assert!(
        diff_err.contains("Ratchet ceiling"),
        "expected Ratchet ceiling in diff stderr, got: {diff_err}"
    );

    let ratchet_path = repo.path().join(".codeint/ratchet.json");
    let mut stale = read_json_file(&ratchet_path);
    stale["source_sets"]["production"]["verbosity"]["metric_version"] = json!(1);
    fs::write(
        &ratchet_path,
        format!("{}\n", serde_json::to_string_pretty(&stale).expect("json")),
    )
    .expect("write stale ratchet");

    let (_, check_mismatch_err) = run_codeint_text(repo.path(), &["check"], 2);
    assert!(
        check_mismatch_err.contains("verbosity")
            && check_mismatch_err.contains("codeint ratchet update"),
        "expected loud version mismatch naming verbosity and re-baseline command, got: {check_mismatch_err}"
    );

    let (_, diff_mismatch_err) = run_diff_text(repo.path(), cache.path(), &["diff", "main"], 2);
    assert!(
        diff_mismatch_err.contains("verbosity")
            && diff_mismatch_err.contains("codeint ratchet update"),
        "expected loud version mismatch in diff, got: {diff_mismatch_err}"
    );
}

#[test]
fn package_scoped_check_and_diff_skip_ratchet_with_clear_note() {
    let repo = init_git_repo(&[
        (
            "package.json",
            r#"{ "name": "mono", "private": true, "workspaces": ["packages/*"] }"#,
        ),
        (
            "packages/a/package.json",
            r#"{ "name": "@mono/a", "version": "1.0.0" }"#,
        ),
        (
            "packages/a/src/index.ts",
            "export function ok(value: number): number {\n  return value + 1;\n}\n",
        ),
        (
            "packages/b/package.json",
            r#"{ "name": "@mono/b", "version": "1.0.0" }"#,
        ),
        (
            "packages/b/src/index.ts",
            "export function okB(value: number): number {\n  return value + 2;\n}\n",
        ),
    ]);
    let cache = TempDir::new().expect("cache dir");

    let _ = run_codeint_text(repo.path(), &["ratchet", "update"], 0);
    run_git(repo.path(), &["add", ".codeint/ratchet.json"]);
    run_git(repo.path(), &["commit", "-q", "-m", "add ratchet"]);
    run_git(repo.path(), &["checkout", "-q", "-b", "feature"]);

    write_repo_files(
        repo.path(),
        &[(
            "packages/b/src/index.ts",
            &branching_ts_function("erodedB", 14),
        )],
    );

    let (check_out, _) = run_codeint_text(
        repo.path(),
        &["check", "--package", "packages/a", "--format", "json"],
        0,
    );
    let check_json: Value = serde_json::from_str(&check_out).expect("check json");
    assert_ratchet_package_note(&check_json, "check");

    let (diff_out, _) = run_diff_text(
        repo.path(),
        cache.path(),
        &[
            "diff",
            "main",
            "--package",
            "packages/a",
            "--format",
            "json",
        ],
        0,
    );
    let diff_json: Value = serde_json::from_str(&diff_out).expect("diff json");
    assert_ratchet_package_note(&diff_json, "diff");
}

fn assert_ratchet_package_note(report: &Value, command: &str) {
    assert!(
        report["ratchet_note"]
            .as_str()
            .is_some_and(|note| note.contains("--package")),
        "expected ratchet_note in package {command} json: {report}"
    );
}

#[test]
fn ratchet_includes_external_metric_plugin_and_production_test_gap() {
    let wasm_path = crate::wasm_fixtures::wasm_fixture_path("sample_long_functions_plugin.wasm");
    let wasm_bytes = fs::read(&wasm_path).expect("read sample_long_functions_plugin.wasm");
    let hash = sha256_hex(&sha256(&wasm_bytes));

    let long_body: String = (1..=10)
        .map(|step| format!("  total += step + {step};\n"))
        .collect();
    let app_source = format!(
        "export function tiny(step: number): number {{\n  return step + 1;\n}}\nexport function stretch(step: number): number {{\n  let total = 0;\n{long_body}  return total;\n}}\n"
    );
    let lcov_lines: String = (5..=16).map(|line| format!("DA:{line},0\n")).collect();
    let lcov_content = format!("SF:src/app.ts\nDA:2,1\n{lcov_lines}end_of_record\n");
    let repo = init_git_repo(&[
        ("src/app.ts", &app_source),
        ("coverage/lcov.info", &lcov_content),
    ]);
    let plugin_dest = repo.path().join("plugins/long_functions.wasm");
    fs::create_dir_all(plugin_dest.parent().expect("parent")).expect("mkdir");
    fs::copy(&wasm_path, &plugin_dest).expect("copy wasm");
    write_repo_files(
        repo.path(),
        &[(
            ".codeint/config.json",
            &format!(
                r#"{{
  "coverage_reports": ["coverage/lcov.info"],
  "plugins": [
    {{
      "name": "long-functions",
      "kind": "metric",
      "path": "plugins/long_functions.wasm",
      "sha256": "{hash}"
    }}
  ]
}}
"#
            ),
        )],
    );

    let _ = run_codeint_text(repo.path(), &["ratchet", "update"], 0);
    let ratchet = read_json_file(&repo.path().join(".codeint/ratchet.json"));
    assert_eq!(
        ratchet["source_sets"]["production"]["long_functions"]["ceiling"],
        0.5
    );
    assert_eq!(
        ratchet["source_sets"]["production"]["long_functions"]["metric_version"],
        1
    );
    assert!(
        ratchet["source_sets"]["production"]["test_gap"]["ceiling"]
            .as_f64()
            .is_some_and(|ratio| ratio > 0.0)
    );
    assert_eq!(
        ratchet["source_sets"]["production"]["test_gap"]["metric_version"],
        1
    );
}

#[test]
fn custom_ratchet_path_works_and_repo_escaping_paths_are_rejected() {
    let repo = init_git_repo(&[
        (
            "codeint.json",
            r#"{ "ratchet": { "path": "quality/ratchet.json" } }"#,
        ),
        ("src/app.ts", "export const app = 1;\n"),
    ]);

    let _ = run_codeint_text(repo.path(), &["ratchet", "update"], 0);
    assert!(repo.path().join("quality/ratchet.json").is_file());
    assert!(!repo.path().join(".codeint/ratchet.json").exists());

    for bad_path in ["", "../escape.json", "/tmp/escape.json"] {
        write_repo_files(
            repo.path(),
            &[(
                "codeint.json",
                &format!(r#"{{ "ratchet": {{ "path": "{bad_path}" }} }}"#),
            )],
        );
        let output = run_codeint_output(repo.path(), &["check"]);
        assert_eq!(
            output.status.code(),
            Some(2),
            "expected exit 2 for bad ratchet.path {bad_path:?}, stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[cfg(unix)]
#[test]
fn symlinked_ratchet_file_outside_repository_is_rejected() {
    let outside = TempDir::new().expect("outside dir");
    let external_file = outside.path().join("external-ratchet.json");
    fs::write(&external_file, r#"{ "schema_version": 1 }"#).expect("write external file");

    let repo = init_git_repo(&[("src/app.ts", "export const app = 1;\n")]);
    fs::create_dir_all(repo.path().join(".codeint")).expect("mkdir .codeint");
    std::os::unix::fs::symlink(&external_file, repo.path().join(".codeint/ratchet.json"))
        .expect("create symlink");

    let (_, stderr) = run_codeint_text(repo.path(), &["check"], 2);
    assert!(
        stderr.contains("outside the Git repository"),
        "expected outside-repo error, got: {stderr}"
    );
}

#[test]
fn ratchet_update_reset_regenerates_an_invalid_ratchet_file() {
    for corrupt in ["{", r#"{"schema_version":2}"#] {
        let repo = init_git_repo(&[("src/app.ts", "export const app = 1;\n")]);
        write_repo_files(repo.path(), &[(".codeint/ratchet.json", corrupt)]);

        let (_, stderr) = run_codeint_text(repo.path(), &["ratchet", "update"], 2);
        assert!(
            stderr.contains("regenerate it with codeint ratchet update --reset"),
            "expected the advice to name --reset, got: {stderr}"
        );
        assert_eq!(
            fs::read_to_string(repo.path().join(".codeint/ratchet.json")).expect("read"),
            corrupt
        );

        let (stdout, _) = run_codeint_text(repo.path(), &["ratchet", "update", "--reset"], 0);
        assert!(
            stdout.contains("Replaced invalid .codeint/ratchet.json"),
            "expected the summary to say the invalid file was replaced, got: {stdout}"
        );
        let stored = read_json_file(&repo.path().join(".codeint/ratchet.json"));
        assert_eq!(stored["schema_version"], 1);
        run_codeint_text(repo.path(), &["check"], 0);
    }
}
