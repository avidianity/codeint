use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::OnceLock;

use codeint_plugin_host::{sha256, sha256_hex};
use serde_json::Value;
use tempfile::TempDir;

static BUILT_SAMPLE_PLUGIN: OnceLock<PathBuf> = OnceLock::new();
static BUILT_SAMPLE_METRIC_PLUGIN: OnceLock<PathBuf> = OnceLock::new();
static BUILT_SANDBOX_PROBES: OnceLock<PathBuf> = OnceLock::new();

fn build_wasm_fixture(manifest_relative: &str, target_name: &str, workspace: bool) -> PathBuf {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let manifest = root.join(manifest_relative);
    let target = root.join("target").join(target_name);
    let mut command = Command::new("cargo");
    command.arg("build");
    if workspace {
        command.arg("--workspace");
    }
    let status = command
        .args(["--locked", "--offline", "--target", "wasm32-wasip2"])
        .arg("--manifest-path")
        .arg(&manifest)
        .arg("--target-dir")
        .arg(&target)
        .status()
        .expect("run cargo build for wasm fixture");
    assert!(status.success(), "{target_name} build exited with {status}");
    target.join("wasm32-wasip2/debug")
}

fn built_sample_plugin() -> &'static Path {
    BUILT_SAMPLE_PLUGIN.get_or_init(|| {
        build_wasm_fixture(
            "crates/plugin-host/tests/fixtures/sample-plugin/Cargo.toml",
            "wasm-fixtures",
            false,
        )
        .join("sample_mini_plugin.wasm")
    })
}

fn built_sample_metric_plugin() -> &'static Path {
    BUILT_SAMPLE_METRIC_PLUGIN.get_or_init(|| {
        build_wasm_fixture(
            "crates/plugin-host/tests/fixtures/sample-metric-plugin/Cargo.toml",
            "wasm-fixtures",
            false,
        )
        .join("sample_long_functions_plugin.wasm")
    })
}

fn built_sandbox_probe(file_name: &str) -> PathBuf {
    BUILT_SANDBOX_PROBES
        .get_or_init(|| {
            build_wasm_fixture(
                "crates/plugin-host/tests/fixtures/sandbox-probes/Cargo.toml",
                "wasm-fixtures",
                true,
            )
        })
        .join(file_name)
}

fn init_repo() -> TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    let status = Command::new("git")
        .args(["init", "--quiet"])
        .current_dir(dir.path())
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .status()
        .expect("git init");
    assert!(status.success());
    dir
}

fn write_file(root: &Path, relative: &str, text: &str) {
    let full = root.join(relative);
    fs::create_dir_all(full.parent().expect("parent")).expect("create parent");
    fs::write(full, text).expect("write file");
}

fn run_codeint(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_codeint"))
        .args(args)
        .current_dir(dir)
        .output()
        .expect("run codeint")
}

fn copy_wasm_into(source: &Path, repo_root: &Path, relative: &str) -> String {
    let bytes = fs::read(source).expect("read wasm plugin");
    let full = repo_root.join(relative);
    fs::create_dir_all(full.parent().expect("parent")).expect("create parent");
    fs::write(&full, &bytes).expect("copy plugin");
    sha256_hex(&sha256(&bytes))
}

fn copy_plugin_into(repo_root: &Path, relative: &str) -> String {
    copy_wasm_into(built_sample_plugin(), repo_root, relative)
}

fn copy_metric_plugin_into(repo_root: &Path, relative: &str) -> String {
    copy_wasm_into(built_sample_metric_plugin(), repo_root, relative)
}

fn assert_sandbox_probe_exits_2(plugin_name: &str, wasm_file: &str, denied_interface: &str) {
    let repo = init_repo();
    let hash = copy_wasm_into(
        &built_sandbox_probe(wasm_file),
        repo.path(),
        &format!("plugins/{wasm_file}"),
    );
    write_file(
        repo.path(),
        "codeint.json",
        &format!(
            r#"{{
              "plugins": [
                {{
                  "name": "{plugin_name}",
                  "path": "plugins/{wasm_file}",
                  "sha256": "{hash}"
                }}
              ]
            }}"#
        ),
    );
    write_file(repo.path(), "src/app.ts", "export const ok = 1;\n");

    let output = run_codeint(repo.path(), &["check"]);

    assert_eq!(
        output.status.code(),
        Some(2),
        "expected exit 2, got {:?}\nstderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("cannot instantiate Plugin")
            && stderr.contains(plugin_name)
            && stderr.contains(denied_interface)
            && stderr.contains("imports no host capabilities"),
        "expected clear sandbox error naming {plugin_name} and {denied_interface}, got: {stderr}"
    );
}

#[test]
fn missing_sha256_hash_is_a_configuration_error_and_exits_2() {
    let repo = init_repo();
    write_file(
        repo.path(),
        "codeint.json",
        r#"{ "plugins": [{ "name": "sample-mini", "path": "plugins/sample.wasm" }] }"#,
    );

    let output = run_codeint(repo.path(), &["check"]);

    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("plugins[0]") && stderr.contains("sha256"),
        "expected missing sha256 error, got: {stderr}"
    );
}

#[test]
fn invalid_sha256_hash_is_a_configuration_error_and_exits_2() {
    let repo = init_repo();
    write_file(
        repo.path(),
        "codeint.json",
        r#"{ "plugins": [{ "name": "sample-mini", "path": "plugins/sample.wasm", "sha256": "not-a-hash" }] }"#,
    );

    let output = run_codeint(repo.path(), &["check"]);

    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("plugins[0].sha256") && stderr.contains("64-character"),
        "expected invalid sha256 error, got: {stderr}"
    );
}

#[test]
fn hash_mismatch_refuses_to_load_the_plugin_and_exits_2() {
    let repo = init_repo();
    let _actual = copy_plugin_into(repo.path(), "plugins/sample.wasm");
    write_file(
        repo.path(),
        "codeint.json",
        r#"{
          "plugins": [
            {
              "name": "sample-mini",
              "path": "plugins/sample.wasm",
              "sha256": "0000000000000000000000000000000000000000000000000000000000000000"
            }
          ]
        }"#,
    );
    write_file(repo.path(), "src/app.mini", "fn main() {\n  return 1\n}\n");

    let output = run_codeint(repo.path(), &["check"]);

    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("SHA-256 hash mismatch")
            && stderr.contains("sample-mini")
            && stderr.contains("sample.wasm"),
        "expected hash mismatch naming plugin and path, got: {stderr}"
    );
}

#[test]
fn sample_external_plugin_measures_its_fixture_language_end_to_end_through_codeint_check() {
    let repo = init_repo();
    let hash = copy_plugin_into(repo.path(), "plugins/sample.wasm");
    write_file(
        repo.path(),
        "codeint.json",
        &format!(
            r#"{{
              "plugins": [
                {{
                  "name": "sample-mini",
                  "path": "plugins/sample.wasm",
                  "sha256": "{hash}"
                }}
              ]
            }}"#
        ),
    );
    write_file(
        repo.path(),
        "src/a.mini",
        concat!("import \"./b\"\n", "fn alpha() {\n", "  pass;\n", "}\n",),
    );
    write_file(
        repo.path(),
        "src/b.mini",
        concat!("import \"./a\"\n", "fn beta() {\n", "  return 1\n", "}\n",),
    );
    write_file(repo.path(), "src/types.d.mini", "fn extern_decl() {}\n");
    write_file(repo.path(), "src/broken.mini", "SYNTAX_ERROR\n");

    let output = run_codeint(&repo.path().join("src"), &["check", "--format", "json"]);

    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: Value =
        serde_json::from_slice(&output.stdout).expect("codeint check prints valid JSON");
    assert_eq!(report["files"], 2);
    assert_eq!(report["source_lines"], 8);
    assert_eq!(
        report["source_sets"]["production"]["verbosity"]["flagged_lines"],
        1
    );
    assert_eq!(
        report["source_sets"]["production"]["verbosity"]["findings"][0]["rule"],
        "redundant-pass"
    );
    assert_eq!(
        report["source_sets"]["production"]["import_cycles"]["modules_in_import_cycles"],
        2
    );
    assert_eq!(report["declaration_only"][0], "src/types.d.mini");
    assert_eq!(report["skipped"][0]["path"], "src/broken.mini");
    assert_eq!(
        report["source_sets"]["production"]["erosion"]["reference_range"]["source"],
        "sample mini language fixture"
    );
}

#[test]
fn external_plugin_that_reads_files_fails_clearly_names_plugin_and_exits_2() {
    assert_sandbox_probe_exits_2(
        "rogue-file-reader",
        "file_reader_plugin.wasm",
        "wasi:filesystem/",
    );
}

#[test]
fn external_plugin_that_opens_sockets_fails_clearly_names_plugin_and_exits_2() {
    assert_sandbox_probe_exits_2(
        "rogue-socket-opener",
        "socket_opener_plugin.wasm",
        "wasi:sockets/",
    );
}

#[test]
fn external_plugin_that_reads_the_clock_fails_clearly_names_plugin_and_exits_2() {
    assert_sandbox_probe_exits_2(
        "rogue-clock-reader",
        "clock_reader_plugin.wasm",
        "wasi:clocks/",
    );
}

fn commit_all(dir: &Path, message: &str) {
    let add = Command::new("git")
        .args(["add", "-A"])
        .current_dir(dir)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .status()
        .expect("git add");
    assert!(add.success(), "git add exited with {add}");
    let commit = Command::new("git")
        .args(["commit", "--quiet", "-m", message])
        .current_dir(dir)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_AUTHOR_NAME", "Test")
        .env("GIT_AUTHOR_EMAIL", "test@example.com")
        .env("GIT_COMMITTER_NAME", "Test")
        .env("GIT_COMMITTER_EMAIL", "test@example.com")
        .status()
        .expect("git commit");
    assert!(commit.success(), "git commit exited with {commit}");
}

#[test]
fn two_external_plugins_claiming_one_extension_without_languages_exit_2_across_commands() {
    let repo = init_repo();
    let hash = copy_plugin_into(repo.path(), "plugins/sample.wasm");
    write_file(
        repo.path(),
        "codeint.json",
        &format!(
            r#"{{
              "plugins": [
                {{
                  "name": "zeta-mini",
                  "path": "plugins/sample.wasm",
                  "sha256": "{hash}"
                }},
                {{
                  "name": "alpha-mini",
                  "path": "plugins/sample.wasm",
                  "sha256": "{hash}"
                }}
              ]
            }}"#
        ),
    );
    write_file(repo.path(), "src/app.mini", "fn main() {\n  return 1\n}\n");
    commit_all(repo.path(), "initial");

    for args in [&["check"][..], &["diff", "HEAD"][..], &["trend"][..]] {
        let output = run_codeint(repo.path(), args);
        assert_eq!(output.status.code(), Some(2), "args {args:?}");
        let stderr = String::from_utf8_lossy(&output.stderr);
        let alpha_pos = stderr
            .find("alpha-mini")
            .unwrap_or_else(|| panic!("expected alpha-mini in stderr for {args:?}, got: {stderr}"));
        let zeta_pos = stderr
            .find("zeta-mini")
            .unwrap_or_else(|| panic!("expected zeta-mini in stderr for {args:?}, got: {stderr}"));
        assert!(
            alpha_pos < zeta_pos,
            "expected sorted plugin names for {args:?}, got: {stderr}"
        );
        assert!(
            stderr.contains(".mini") && stderr.contains("languages"),
            "expected extension and languages setting for {args:?}, got: {stderr}"
        );
    }
}

#[test]
fn two_external_plugins_claiming_one_extension_in_overlapping_languages_folders_exit_2() {
    let repo = init_repo();
    let hash = copy_plugin_into(repo.path(), "plugins/sample.wasm");
    write_file(
        repo.path(),
        "codeint.json",
        &format!(
            r#"{{
              "plugins": [
                {{
                  "name": "zeta-mini",
                  "path": "plugins/sample.wasm",
                  "sha256": "{hash}"
                }},
                {{
                  "name": "alpha-mini",
                  "path": "plugins/sample.wasm",
                  "sha256": "{hash}"
                }}
              ],
              "languages": [
                {{ "plugin": "zeta-mini", "include": ["src"] }},
                {{ "plugin": "alpha-mini", "include": ["src/sub"] }}
              ]
            }}"#
        ),
    );
    write_file(
        repo.path(),
        "src/sub/app.mini",
        "fn main() {\n  return 1\n}\n",
    );

    let output = run_codeint(repo.path(), &["check"]);

    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("alpha-mini")
            && stderr.contains("zeta-mini")
            && stderr.contains(".mini")
            && stderr.contains("src/sub/app.mini")
            && stderr.contains("languages"),
        "expected extension conflict error, got: {stderr}"
    );
}

#[test]
fn assigning_separate_directories_in_languages_resolves_external_plugin_extension_conflict() {
    let repo = init_repo();
    let hash = copy_plugin_into(repo.path(), "plugins/sample.wasm");
    write_file(
        repo.path(),
        "codeint.json",
        &format!(
            r#"{{
              "plugins": [
                {{
                  "name": "zeta-mini",
                  "path": "plugins/sample.wasm",
                  "sha256": "{hash}"
                }},
                {{
                  "name": "alpha-mini",
                  "path": "plugins/sample.wasm",
                  "sha256": "{hash}"
                }}
              ],
              "languages": [
                {{ "plugin": "alpha-mini", "include": ["service-a"] }},
                {{ "plugin": "zeta-mini", "include": ["service-b"] }}
              ]
            }}"#
        ),
    );
    write_file(
        repo.path(),
        "service-a/a.mini",
        "fn alpha() {\n  return 1\n}\n",
    );
    write_file(
        repo.path(),
        "service-b/b.mini",
        "fn zeta() {\n  pass;\n  return 2\n}\n",
    );

    let output = run_codeint(repo.path(), &["check", "--format", "json"]);

    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: Value =
        serde_json::from_slice(&output.stdout).expect("codeint check prints valid JSON");
    assert_eq!(report["files"], 2, "unexpected report: {report}");
    assert_eq!(
        report["languages"]["alpha-mini"]["files"], 1,
        "unexpected report: {report}"
    );
    assert_eq!(
        report["languages"]["alpha-mini"]["source_lines"], 3,
        "unexpected report: {report}"
    );
    assert_eq!(
        report["languages"]["zeta-mini"]["files"], 1,
        "unexpected report: {report}"
    );
    assert_eq!(
        report["languages"]["zeta-mini"]["source_lines"], 4,
        "unexpected report: {report}"
    );
}

fn long_ts_function(name: &str, seed: u32) -> String {
    let mut body = format!("export function {name}(x: number): number {{\n");
    for step in 1..=9 {
        body.push_str(&format!("  x += {};\n", seed + step));
    }
    body.push_str("  return x;\n}\n");
    body
}

fn short_ts_function(name: &str) -> String {
    format!("export function {name}(x: number): number {{\n  return x + 1;\n}}\n")
}

fn write_metric_plugin_config(repo_root: &Path, hash: &str, extra_fields: &str) {
    let trailing = if extra_fields.is_empty() {
        String::new()
    } else {
        format!(",\n{extra_fields}")
    };
    write_file(
        repo_root,
        "codeint.json",
        &format!(
            r#"{{
              "plugins": [
                {{
                  "name": "long-functions",
                  "kind": "metric",
                  "path": "plugins/long_functions.wasm",
                  "sha256": "{hash}"
                }}
              ]{trailing}
            }}"#
        ),
    );
}

#[test]
fn sample_external_metric_plugin_appears_next_to_builtin_metrics_in_check_reports() {
    let repo = init_repo();
    let hash = copy_metric_plugin_into(repo.path(), "plugins/long_functions.wasm");
    write_metric_plugin_config(repo.path(), &hash, "");
    write_file(
        repo.path(),
        "src/app.ts",
        &format!(
            "{}\n{}",
            short_ts_function("brief"),
            long_ts_function("verbose", 10)
        ),
    );

    let json_output = run_codeint(repo.path(), &["check", "--format", "json"]);
    assert_eq!(
        json_output.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&json_output.stderr)
    );
    let report: Value =
        serde_json::from_slice(&json_output.stdout).expect("codeint check prints valid JSON");
    let prod = &report["source_sets"]["production"];
    assert!(prod.get("erosion").is_some());
    assert!(prod.get("duplication").is_some());
    assert!(prod.get("verbosity").is_some());
    assert!(prod.get("import_cycles").is_some());
    let metric = &prod["long_functions"];
    assert_eq!(metric["ratio"], 0.5);
    assert_eq!(metric["numerator"], 1.0);
    assert_eq!(metric["denominator"], 2.0);
    assert_eq!(metric["findings"].as_array().map(Vec::len), Some(1));
    assert_eq!(metric["findings"][0]["path"], "src/app.ts");
    assert_eq!(metric["findings"][0]["line"], 5);
    assert_eq!(metric["findings"][0]["end_line"], 16);

    let md_output = run_codeint(repo.path(), &["check", "--format", "markdown"]);
    assert_eq!(md_output.status.code(), Some(0));
    let md = String::from_utf8_lossy(&md_output.stdout);
    assert!(md.contains("| Production | Long functions | 0.5 | 1 of 2 |"));
    assert!(md.contains("### Long functions"));
    assert!(md.contains("`src/app.ts:5-16`"));

    let sarif_output = run_codeint(repo.path(), &["check", "--format", "sarif"]);
    assert_eq!(sarif_output.status.code(), Some(0));
    let sarif: Value =
        serde_json::from_slice(&sarif_output.stdout).expect("codeint check prints valid SARIF");
    let rules = sarif["runs"][0]["tool"]["driver"]["rules"]
        .as_array()
        .expect("sarif rules");
    assert!(rules.iter().any(|rule| rule["id"] == "long_functions"));
    let results = sarif["runs"][0]["results"]
        .as_array()
        .expect("sarif results");
    assert!(results.iter().any(|result| {
        result["ruleId"] == "long_functions"
            && result["locations"][0]["physicalLocation"]["artifactLocation"]["uri"] == "src/app.ts"
    }));
}

#[test]
fn external_metric_plugin_works_with_budgets_in_check() {
    let repo = init_repo();
    let hash = copy_metric_plugin_into(repo.path(), "plugins/long_functions.wasm");
    write_file(
        repo.path(),
        "src/app.ts",
        &format!(
            "{}\n{}",
            short_ts_function("brief"),
            long_ts_function("verbose", 10)
        ),
    );
    write_metric_plugin_config(
        repo.path(),
        &hash,
        r#""budgets": { "production": { "long_functions": 0.25 } }"#,
    );

    let over_output = run_codeint(repo.path(), &["check", "--format", "json"]);
    assert_eq!(over_output.status.code(), Some(1));
    let report: Value =
        serde_json::from_slice(&over_output.stdout).expect("over-budget report is valid JSON");
    assert_eq!(report["over_budget"][0]["source_set"], "production");
    assert_eq!(report["over_budget"][0]["metric"], "long_functions");
    assert_eq!(report["over_budget"][0]["ratio"], 0.5);
    assert_eq!(report["over_budget"][0]["budget"], 0.25);
    let stderr = String::from_utf8_lossy(&over_output.stderr);
    assert!(
        stderr.contains("production long_functions ratio 0.5 is over its Budget 0.25"),
        "unexpected stderr: {stderr}"
    );

    write_metric_plugin_config(
        repo.path(),
        &hash,
        r#""budgets": { "production": { "long_functions": 0.5 } }"#,
    );

    let passing_output = run_codeint(repo.path(), &["check", "--format", "json"]);
    assert_eq!(
        passing_output.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&passing_output.stderr)
    );
}

#[test]
fn external_metric_plugin_findings_work_with_diff_budgets_and_regression_limits() {
    let repo = init_repo();
    let hash = copy_metric_plugin_into(repo.path(), "plugins/long_functions.wasm");
    write_metric_plugin_config(repo.path(), &hash, "");
    write_file(
        repo.path(),
        "src/keep.ts",
        &format!(
            "{}\n{}",
            short_ts_function("tiny"),
            long_ts_function("stays", 10)
        ),
    );
    write_file(repo.path(), "src/remove.ts", &long_ts_function("gone", 40));
    commit_all(repo.path(), "base");

    write_file(repo.path(), "src/remove.ts", &short_ts_function("gone"));
    write_file(repo.path(), "src/added.ts", &long_ts_function("fresh", 80));
    write_file(
        repo.path(),
        "src/extra.ts",
        &long_ts_function("second", 120),
    );

    let diff_output = run_codeint(repo.path(), &["diff", "HEAD", "--format", "json"]);
    assert_eq!(diff_output.status.code(), Some(1));
    let report: Value =
        serde_json::from_slice(&diff_output.stdout).expect("codeint diff prints valid JSON");
    let changes = &report["source_sets"]["production"]["long_functions"];
    assert_eq!(changes["base_ratio"], 0.6667);
    assert_eq!(changes["head_ratio"], 0.6);
    assert_eq!(changes["unchanged"], 1);
    assert_eq!(changes["resolved"].as_array().map(Vec::len), Some(1));
    assert_eq!(changes["resolved"][0]["path"], "src/remove.ts");
    assert_eq!(changes["new"].as_array().map(Vec::len), Some(2));
    assert!(
        report["regressions"]
            .as_array()
            .expect("regressions")
            .iter()
            .any(|reg| reg["metric"] == "long_functions")
    );

    let sarif_output = run_codeint(repo.path(), &["diff", "HEAD", "--format", "sarif"]);
    assert_eq!(sarif_output.status.code(), Some(1));
    let sarif: Value =
        serde_json::from_slice(&sarif_output.stdout).expect("codeint diff prints valid SARIF");
    let new_metric_results: Vec<&Value> = sarif["runs"][0]["results"]
        .as_array()
        .expect("sarif results")
        .iter()
        .filter(|result| result["ruleId"] == "long_functions")
        .collect();
    assert_eq!(new_metric_results.len(), 2);

    write_metric_plugin_config(
        repo.path(),
        &hash,
        r#""budgets": { "production": { "long_functions": 0.5 } }, "regression_limits": { "long_functions": { "ratio_rise": 1, "new_findings": "never" } }"#,
    );

    let budget_diff = run_codeint(repo.path(), &["diff", "HEAD", "--format", "json"]);
    assert_eq!(budget_diff.status.code(), Some(1));
    let budget_report: Value =
        serde_json::from_slice(&budget_diff.stdout).expect("budget diff prints valid JSON");
    assert!(budget_report.get("regressions").is_none());
    assert_eq!(budget_report["over_budget"][0]["metric"], "long_functions");
}

#[test]
fn language_plugin_that_loops_forever_skips_the_file_and_finishes_the_run() {
    let repo = init_repo();
    let hash = copy_wasm_into(
        &built_sandbox_probe("loop_language_plugin.wasm"),
        repo.path(),
        "plugins/loop_lang.wasm",
    );
    write_file(
        repo.path(),
        "codeint.json",
        &format!(
            r#"{{
              "plugins": [
                {{
                  "name": "loop-lang",
                  "path": "plugins/loop_lang.wasm",
                  "sha256": "{hash}"
                }}
              ],
              "plugin_limits": {{
                "timeout_ms": 50
              }}
            }}"#
        ),
    );
    write_file(repo.path(), "src/hang.loop", "LOOP_FOREVER\n");
    write_file(repo.path(), "src/healthy.loop", "ok line 1\nok line 2\n");

    let output = run_codeint(repo.path(), &["check", "--format", "json"]);

    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: Value =
        serde_json::from_slice(&output.stdout).expect("codeint check prints valid JSON");
    assert_eq!(report["files"], 1);
    assert_eq!(report["source_lines"], 2);
    assert_eq!(report["skipped"][0]["path"], "src/hang.loop");
    let reason = report["skipped"][0]["reason"]
        .as_str()
        .expect("skipped reason string");
    assert!(
        reason.contains("loop-lang") && reason.contains("time limit") && reason.contains("50 ms"),
        "unexpected skipped reason: {reason}"
    );
}

#[test]
fn language_plugin_that_grows_memory_past_cap_skips_the_file_and_raised_limit_takes_effect() {
    let repo = init_repo();
    let hash = copy_wasm_into(
        &built_sandbox_probe("memory_hog_language_plugin.wasm"),
        repo.path(),
        "plugins/hog_lang.wasm",
    );
    write_file(
        repo.path(),
        "codeint.json",
        &format!(
            r#"{{
              "plugins": [
                {{
                  "name": "hog-lang",
                  "path": "plugins/hog_lang.wasm",
                  "sha256": "{hash}"
                }}
              ],
              "plugin_limits": {{
                "memory_mb": 2
              }}
            }}"#
        ),
    );
    write_file(repo.path(), "src/heavy.hog", "ALLOC_2MB\n");
    write_file(repo.path(), "src/light.hog", "ok\n");

    let tight_output = run_codeint(repo.path(), &["check", "--format", "json"]);
    assert_eq!(
        tight_output.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&tight_output.stderr)
    );
    let tight_report: Value =
        serde_json::from_slice(&tight_output.stdout).expect("codeint check prints valid JSON");
    assert_eq!(tight_report["files"], 1);
    assert_eq!(tight_report["skipped"][0]["path"], "src/heavy.hog");
    let reason = tight_report["skipped"][0]["reason"]
        .as_str()
        .expect("skipped reason string");
    assert!(
        reason.contains("hog-lang") && reason.contains("memory cap") && reason.contains("2 MiB"),
        "unexpected skipped reason: {reason}"
    );

    write_file(
        repo.path(),
        "codeint.json",
        &format!(
            r#"{{
              "plugins": [
                {{
                  "name": "hog-lang",
                  "path": "plugins/hog_lang.wasm",
                  "sha256": "{hash}"
                }}
              ],
              "plugin_limits": {{
                "memory_mb": 8
              }}
            }}"#
        ),
    );

    let raised_output = run_codeint(repo.path(), &["check", "--format", "json"]);
    assert_eq!(
        raised_output.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&raised_output.stderr)
    );
    let raised_report: Value =
        serde_json::from_slice(&raised_output.stdout).expect("codeint check prints valid JSON");
    assert_eq!(raised_report["files"], 2);
    assert!(raised_report.get("skipped").is_none());
}

#[test]
fn metric_plugin_that_loops_forever_stops_the_run_with_exit_2_and_names_the_plugin() {
    let repo = init_repo();
    let hash = copy_wasm_into(
        &built_sandbox_probe("loop_metric_plugin.wasm"),
        repo.path(),
        "plugins/loop_metric.wasm",
    );
    write_file(
        repo.path(),
        "codeint.json",
        &format!(
            r#"{{
              "plugins": [
                {{
                  "name": "runaway-metric",
                  "kind": "metric",
                  "path": "plugins/loop_metric.wasm",
                  "sha256": "{hash}"
                }}
              ],
              "plugin_limits": {{
                "timeout_ms": 50
              }}
            }}"#
        ),
    );
    write_file(repo.path(), "src/app.ts", "export const ok = 1;\n");

    let output = run_codeint(repo.path(), &["check"]);

    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("runaway-metric")
            && stderr.contains("time limit")
            && stderr.contains("50 ms"),
        "expected time limit error naming runaway-metric, got: {stderr}"
    );
}

#[test]
fn invalid_plugin_limits_print_the_offending_key_and_exit_2() {
    let cases = [
        (
            r#"{ "plugin_limits": { "memory_mb": 0 } }"#,
            "plugin_limits.memory_mb",
        ),
        (
            r#"{ "plugin_limits": { "timeout_ms": -10 } }"#,
            "plugin_limits.timeout_ms",
        ),
        (
            r#"{ "plugin_limits": { "timeout_ms": "fast" } }"#,
            "plugin_limits.timeout_ms",
        ),
        (
            r#"{ "plugin_limits": { "memory_gb": 1 } }"#,
            "plugin_limits.memory_gb",
        ),
    ];

    for (config_json, expected_key) in cases {
        let repo = init_repo();
        write_file(repo.path(), "codeint.json", config_json);
        let output = run_codeint(repo.path(), &["check"]);
        assert_eq!(
            output.status.code(),
            Some(2),
            "config {config_json} should exit 2"
        );
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains(expected_key),
            "expected stderr to name {expected_key} for {config_json}, got: {stderr}"
        );
    }
}
