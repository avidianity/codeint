use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
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

fn run_check_with_sandbox_probe(plugin_name: &str, kind: Option<&str>, wasm_file: &str) -> Output {
    let repo = init_repo();
    let hash = copy_wasm_into(
        &built_sandbox_probe(wasm_file),
        repo.path(),
        &format!("plugins/{wasm_file}"),
    );
    let kind_line = kind.map_or_else(String::new, |kind| format!(r#""kind": "{kind}","#));
    write_file(
        repo.path(),
        "codeint.json",
        &format!(
            r#"{{
              "plugins": [
                {{
                  "name": "{plugin_name}",
                  {kind_line}
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
    output
}

fn assert_sandbox_probe_exits_2(plugin_name: &str, wasm_file: &str, denied_interface: &str) {
    let output = run_check_with_sandbox_probe(plugin_name, None, wasm_file);
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

fn write_plugin_config(
    repo_root: &Path,
    name: &str,
    kind: Option<&str>,
    path: &str,
    hash: &str,
    extra_fields: &str,
) {
    let kind_line = kind.map_or_else(String::new, |kind| format!(r#""kind": "{kind}","#));
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
                  "name": "{name}",
                  {kind_line}
                  "path": "{path}",
                  "sha256": "{hash}"
                }}
              ]{trailing}
            }}"#
        ),
    );
}

fn write_metric_plugin_config(repo_root: &Path, hash: &str, extra_fields: &str) {
    write_plugin_config(
        repo_root,
        "long-functions",
        Some("metric"),
        "plugins/long_functions.wasm",
        hash,
        extra_fields,
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

struct LocalHttpsServer {
    _dir: TempDir,
    cert_path: PathBuf,
    port: u16,
    child: Option<Child>,
}

impl LocalHttpsServer {
    fn start(files: &[(&str, &Path)]) -> LocalHttpsServer {
        let dir = tempfile::tempdir().expect("tempdir for https server");
        let www = dir.path().join("www");
        fs::create_dir_all(&www).expect("create www dir");
        for (name, source) in files {
            fs::copy(source, www.join(name)).expect("copy served file");
        }
        let cert_path = dir.path().join("cert.pem");
        let key_path = dir.path().join("key.pem");
        let config_path = dir.path().join("openssl.cnf");
        fs::write(
            &config_path,
            concat!(
                "[req]\n",
                "distinguished_name = dn\n",
                "x509_extensions = v3_req\n",
                "prompt = no\n",
                "[dn]\n",
                "CN = 127.0.0.1\n",
                "[v3_req]\n",
                "subjectAltName = IP:127.0.0.1\n",
            ),
        )
        .expect("write openssl config");

        let openssl = Command::new("openssl")
            .args([
                "req", "-x509", "-newkey", "rsa:2048", "-nodes", "-days", "1", "-keyout",
            ])
            .arg(&key_path)
            .arg("-out")
            .arg(&cert_path)
            .arg("-config")
            .arg(&config_path)
            .output()
            .expect("run openssl to generate self-signed test certificate");
        assert!(
            openssl.status.success(),
            "openssl failed: {}",
            String::from_utf8_lossy(&openssl.stderr)
        );

        let script = concat!(
            "import http.server, os, ssl, sys\n",
            "os.chdir(sys.argv[1])\n",
            "class QuietHandler(http.server.SimpleHTTPRequestHandler):\n",
            "    def log_message(self, fmt, *args):\n",
            "        pass\n",
            "server = http.server.ThreadingHTTPServer(('127.0.0.1', 0), QuietHandler)\n",
            "ctx = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)\n",
            "ctx.load_cert_chain(certfile=sys.argv[2], keyfile=sys.argv[3])\n",
            "server.socket = ctx.wrap_socket(server.socket, server_side=True)\n",
            "print(server.server_address[1], flush=True)\n",
            "server.serve_forever()\n",
        );

        let mut child = Command::new("python3")
            .args(["-c", script])
            .arg(&www)
            .arg(&cert_path)
            .arg(&key_path)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn local python3 HTTPS server");
        let stdout = child.stdout.take().expect("server stdout");
        let mut reader = BufReader::new(stdout);
        let mut line = String::new();
        reader
            .read_line(&mut line)
            .expect("read bound port from local HTTPS server");
        let port: u16 = line.trim().parse().unwrap_or_else(|error| {
            panic!("expected port number from local HTTPS server, got {line:?}: {error}")
        });

        LocalHttpsServer {
            _dir: dir,
            cert_path,
            port,
            child: Some(child),
        }
    }

    fn url(&self, file_name: &str) -> String {
        format!("https://127.0.0.1:{}/{file_name}", self.port)
    }

    fn cert_path(&self) -> &Path {
        &self.cert_path
    }

    fn stop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

impl Drop for LocalHttpsServer {
    fn drop(&mut self) {
        self.stop();
    }
}

fn run_codeint_with_env(dir: &Path, args: &[&str], envs: &[(&str, &Path)]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_codeint"));
    command.args(args).current_dir(dir);
    for (key, value) in envs {
        command.env(key, value);
    }
    command.output().expect("run codeint")
}

fn write_url_plugin_config(repo_root: &Path, name: &str, url: &str, hash: &str) {
    write_file(
        repo_root,
        "codeint.json",
        &format!(
            r#"{{
              "plugins": [
                {{
                  "name": "{name}",
                  "url": "{url}",
                  "sha256": "{hash}"
                }}
              ]
            }}"#
        ),
    );
}

#[test]
fn https_url_plugin_downloads_once_caches_by_sha256_and_runs_offline_after_server_stops() {
    let wasm = built_sample_plugin();
    let hash = sha256_hex(&sha256(&fs::read(wasm).expect("read sample wasm")));
    let mut server = LocalHttpsServer::start(&[("sample.wasm", wasm)]);
    let cache_dir = tempfile::tempdir().expect("cache tempdir");
    let repo = init_repo();
    write_url_plugin_config(
        repo.path(),
        "sample-mini",
        &server.url("sample.wasm"),
        &hash,
    );
    write_file(
        repo.path(),
        "src/app.mini",
        "fn alpha() {\n  pass;\n  return 1\n}\n",
    );

    let envs = [
        ("CODEINT_CACHE_DIR", cache_dir.path()),
        ("CODEINT_TEST_CA_CERT", server.cert_path()),
    ];
    let first = run_codeint_with_env(repo.path(), &["check", "--format", "json"], &envs);
    assert_eq!(
        first.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&first.stderr)
    );
    let report: Value = serde_json::from_slice(&first.stdout).expect("valid JSON report");
    assert_eq!(report["files"], 1);
    assert_eq!(
        report["source_sets"]["production"]["verbosity"]["flagged_lines"],
        1
    );

    let cached_file = cache_dir
        .path()
        .join("plugins")
        .join(format!("{hash}.wasm"));
    assert!(
        cached_file.is_file(),
        "expected cached plugin at {}",
        cached_file.display()
    );

    server.stop();

    let second = run_codeint_with_env(
        repo.path(),
        &["check", "--format", "json"],
        &[("CODEINT_CACHE_DIR", cache_dir.path())],
    );
    assert_eq!(
        second.status.code(),
        Some(0),
        "expected cache hit after server stopped, stderr: {}",
        String::from_utf8_lossy(&second.stderr)
    );
    let second_report: Value =
        serde_json::from_slice(&second.stdout).expect("valid JSON report from cache");
    assert_eq!(second_report["files"], 1);
    assert_eq!(
        second_report["source_sets"]["production"]["verbosity"]["flagged_lines"],
        1
    );
}

#[test]
fn https_url_plugin_hash_mismatch_refuses_download_leaves_cache_empty_and_exits_2() {
    let wasm = built_sample_plugin();
    let server = LocalHttpsServer::start(&[("sample.wasm", wasm)]);
    let cache_dir = tempfile::tempdir().expect("cache tempdir");
    let repo = init_repo();
    write_url_plugin_config(
        repo.path(),
        "tampered-mini",
        &server.url("sample.wasm"),
        "0000000000000000000000000000000000000000000000000000000000000000",
    );
    write_file(repo.path(), "src/app.mini", "fn main() {\n  return 1\n}\n");

    let output = run_codeint_with_env(
        repo.path(),
        &["check"],
        &[
            ("CODEINT_CACHE_DIR", cache_dir.path()),
            ("CODEINT_TEST_CA_CERT", server.cert_path()),
        ],
    );

    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("SHA-256 hash mismatch") && stderr.contains("tampered-mini"),
        "expected hash mismatch error naming plugin, got: {stderr}"
    );
    let plugins_cache = cache_dir.path().join("plugins");
    let remaining: Vec<_> = fs::read_dir(&plugins_cache)
        .map(|entries| entries.filter_map(Result::ok).collect())
        .unwrap_or_default();
    assert!(
        remaining.is_empty(),
        "expected no cached or temp files after hash mismatch, found: {remaining:?}"
    );
}

#[test]
fn plain_http_and_non_https_plugin_urls_are_refused_with_exit_2() {
    for bad_url in [
        "http://127.0.0.1:8080/sample.wasm",
        "file:///tmp/sample.wasm",
        "ftp://example.com/sample.wasm",
    ] {
        let repo = init_repo();
        write_url_plugin_config(
            repo.path(),
            "insecure-mini",
            bad_url,
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
        );

        let output = run_codeint(repo.path(), &["check"]);

        assert_eq!(output.status.code(), Some(2), "url {bad_url}");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains("insecure-mini") && stderr.contains("HTTPS"),
            "expected HTTPS-required error naming plugin for {bad_url}, got: {stderr}"
        );
    }
}

#[test]
fn plugin_entry_must_specify_either_path_or_url_not_neither_or_both() {
    let neither_repo = init_repo();
    write_file(
        neither_repo.path(),
        "codeint.json",
        r#"{
          "plugins": [
            {
              "name": "missing-source",
              "sha256": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
            }
          ]
        }"#,
    );
    let neither = run_codeint(neither_repo.path(), &["check"]);
    assert_eq!(neither.status.code(), Some(2));
    let neither_stderr = String::from_utf8_lossy(&neither.stderr);
    assert!(
        neither_stderr.contains("missing-source")
            && neither_stderr.contains("path")
            && neither_stderr.contains("url"),
        "expected missing source error, got: {neither_stderr}"
    );

    let both_repo = init_repo();
    write_file(
        both_repo.path(),
        "codeint.json",
        r#"{
          "plugins": [
            {
              "name": "both-sources",
              "path": "plugins/sample.wasm",
              "url": "https://example.com/sample.wasm",
              "sha256": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
            }
          ]
        }"#,
    );
    let both = run_codeint(both_repo.path(), &["check"]);
    assert_eq!(both.status.code(), Some(2));
    let both_stderr = String::from_utf8_lossy(&both.stderr);
    assert!(
        both_stderr.contains("both-sources") && both_stderr.contains("not both"),
        "expected ambiguous source error, got: {both_stderr}"
    );
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

fn assert_legacy_v0_2_0_plugin_stops_check(
    plugin_name: &str,
    kind: &str,
    wasm_file: &str,
    found_interface: &str,
    expected_interface: &str,
) {
    let output = run_check_with_sandbox_probe(plugin_name, Some(kind), wasm_file);
    assert!(
        output.stdout.is_empty(),
        "expected empty stdout before any report is written, got: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    let lines: Vec<&str> = stderr.lines().collect();
    assert!(
        lines.len() >= 2,
        "expected error line and rebuild remedy line, got: {stderr}"
    );
    assert!(
        lines[0].contains(plugin_name)
            && lines[0].contains(found_interface)
            && lines[0].contains("0.2.0")
            && lines[0].contains(expected_interface)
            && lines[0].contains("0.3.0"),
        "expected stderr line 1 to name plugin and both versions, got: {}",
        lines[0]
    );
    assert!(
        lines[1].to_ascii_lowercase().contains("rebuild") && lines[1].contains("0.3.0"),
        "expected stderr line 2 to instruct rebuilding against 0.3.0, got: {}",
        lines[1]
    );
}

#[test]
fn v0_2_0_language_plugin_stops_check_with_exit_2_and_rebuild_remedy() {
    assert_legacy_v0_2_0_plugin_stops_check(
        "legacy-v02-lang",
        "language",
        "legacy_v0_2_0_language_plugin.wasm",
        "codeint:plugin/language@0.2.0",
        "codeint:plugin/language@0.3.0",
    );
}

#[test]
fn v0_2_0_metric_plugin_stops_check_with_exit_2_and_rebuild_remedy() {
    assert_legacy_v0_2_0_plugin_stops_check(
        "legacy-v02-metric",
        "metric",
        "legacy_v0_2_0_metric_plugin.wasm",
        "codeint:plugin/metric@0.2.0",
        "codeint:plugin/metric@0.3.0",
    );
}

#[test]
fn coverage_reader_metric_plugin_receives_per_language_coverage_statement_from_check() {
    let repo = init_repo();
    let hash = copy_wasm_into(
        &built_sandbox_probe("coverage_reader_metric_plugin.wasm"),
        repo.path(),
        "plugins/coverage_reader_metric_plugin.wasm",
    );
    write_plugin_config(
        repo.path(),
        "coverage-reader",
        Some("metric"),
        "plugins/coverage_reader_metric_plugin.wasm",
        &hash,
        "",
    );
    write_file(
        repo.path(),
        "src/app.ts",
        "export function greet(name: string): string { return name; }\n",
    );

    let output = run_codeint(repo.path(), &["check", "--format", "json"]);
    assert_eq!(
        output.status.code(),
        Some(0),
        "expected exit 0, got {:?}\nstderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let report: Value = serde_json::from_slice(&output.stdout).expect("parse json report");
    let findings = report["source_sets"]["production"]["coverage_reader"]["findings"]
        .as_array()
        .expect("coverage_reader findings list");
    assert!(
        findings.iter().any(|finding| {
            finding["path"] == "languages/typescript-javascript"
                && finding["message"] == "language=typescript-javascript has_coverage=false"
        }),
        "expected typescript-javascript has_coverage=false finding, got: {findings:?}"
    );
}

fn init_program_start_repo() -> (TempDir, String) {
    let repo = init_repo();
    let hash = copy_wasm_into(
        &built_sandbox_probe("program_start_language_plugin.wasm"),
        repo.path(),
        "plugins/program_start.wasm",
    );
    write_plugin_config(
        repo.path(),
        "program-start",
        None,
        "plugins/program_start.wasm",
        &hash,
        "",
    );
    write_file(
        repo.path(),
        "cmd/app/main.prog",
        "program main\nuse internal/util/live.prog\n",
    );
    write_file(repo.path(), "internal/util/live.prog", "library live\n");
    (repo, hash)
}

fn run_trace(dir: &Path, from: &str, to: &str, format: &str) -> String {
    let output = run_codeint(dir, &["trace", from, to, "--format", format]);
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("utf8")
}

#[test]
fn marked_production_file_is_an_entry_point_for_orphan_files_alongside_globs_while_marked_tests_stay_out()
 {
    let (repo, hash) = init_program_start_repo();
    write_file(repo.path(), "internal/util/orphan.prog", "library orphan\n");
    write_file(
        repo.path(),
        "internal/util/test_only.prog",
        "library test_only\n",
    );
    write_file(
        repo.path(),
        "cmd/app/main.test.prog",
        "program main\nuse internal/util/test_only.prog\n",
    );
    write_file(repo.path(), "cmd/app/extra_test.prog", "program main\n");
    write_file(repo.path(), "dist/gen.prog", "program main\n");
    write_file(repo.path(), "vendor/third.prog", "program main\n");

    let output = run_codeint(repo.path(), &["check", "--format", "json"]);
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: Value = serde_json::from_slice(&output.stdout).expect("valid JSON report");
    let orphan = &report["source_sets"]["production"]["orphan_files"];
    assert_eq!(orphan["entry_points"], 1, "report: {report}");
    assert_eq!(orphan["evaluated_files"], 4, "report: {report}");
    assert_eq!(orphan["orphan_files"], 2, "report: {report}");
    assert_eq!(orphan["unreferenced"], 1, "report: {report}");
    assert_eq!(orphan["test_only"], 1, "report: {report}");
    let findings = orphan["findings"].as_array().expect("orphan findings");
    assert_eq!(findings.len(), 2);
    assert_eq!(findings[0]["path"], "internal/util/orphan.prog");
    assert_eq!(findings[0]["kind"], "unreferenced");
    assert_eq!(findings[1]["path"], "internal/util/test_only.prog");
    assert_eq!(findings[1]["kind"], "test-only");
    assert_eq!(
        findings[1]["test_importers"],
        serde_json::json!(["cmd/app/main.test.prog"])
    );

    write_file(
        repo.path(),
        "internal/util/extra_entry.prog",
        "library extra_entry\nuse ./via_glob.prog\n",
    );
    write_file(
        repo.path(),
        "internal/util/via_glob.prog",
        "library via_glob\n",
    );
    write_plugin_config(
        repo.path(),
        "program-start",
        None,
        "plugins/program_start.wasm",
        &hash,
        r#""entry_points": ["internal/util/extra_entry.prog"]"#,
    );

    let with_glob = run_codeint(repo.path(), &["check", "--format", "json"]);
    assert_eq!(
        with_glob.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&with_glob.stderr)
    );
    let glob_report: Value = serde_json::from_slice(&with_glob.stdout).expect("valid JSON report");
    let glob_orphan = &glob_report["source_sets"]["production"]["orphan_files"];
    assert_eq!(glob_orphan["entry_points"], 2, "report: {glob_report}");
    assert_eq!(glob_orphan["evaluated_files"], 6, "report: {glob_report}");
    assert_eq!(glob_orphan["orphan_files"], 2, "report: {glob_report}");
    let glob_paths: Vec<&str> = glob_orphan["findings"]
        .as_array()
        .expect("orphan findings")
        .iter()
        .map(|item| item["path"].as_str().expect("path"))
        .collect();
    assert_eq!(
        glob_paths,
        vec!["internal/util/orphan.prog", "internal/util/test_only.prog"]
    );
}

#[test]
fn trace_names_program_start_mark_for_marked_production_file_and_omits_it_for_test_files() {
    let (repo, _hash) = init_program_start_repo();
    write_file(
        repo.path(),
        "cmd/app/main.test.prog",
        "program main\nuse internal/util/live.prog\n",
    );

    let traced: Value = serde_json::from_str(&run_trace(
        repo.path(),
        "cmd/app/main.prog",
        "internal/util/live.prog",
        "json",
    ))
    .expect("valid trace JSON");
    assert_eq!(traced["from"], "cmd/app/main.prog");
    assert_eq!(traced["to"], "internal/util/live.prog");
    assert_eq!(traced["from_entry_point"], "program-start mark");
    assert!(traced.get("to_entry_point").is_none());
    assert_eq!(traced["connected"], true);
    assert_eq!(
        traced["paths"],
        serde_json::json!([["cmd/app/main.prog", "internal/util/live.prog"]])
    );

    let text = run_trace(
        repo.path(),
        "cmd/app/main.prog",
        "internal/util/live.prog",
        "text",
    );
    assert_eq!(
        text.trim(),
        "Entry point: cmd/app/main.prog (program-start mark)\ncmd/app/main.prog -> internal/util/live.prog"
    );

    let toon = run_trace(
        repo.path(),
        "cmd/app/main.prog",
        "internal/util/live.prog",
        "toon",
    );
    assert!(
        toon.contains("from_entry_point: program-start mark"),
        "toon: {toon}"
    );
    assert!(!toon.contains("to_entry_point:"), "toon: {toon}");

    let reverse: Value = serde_json::from_str(&run_trace(
        repo.path(),
        "internal/util/live.prog",
        "cmd/app/main.prog",
        "json",
    ))
    .expect("valid reverse trace JSON");
    assert!(reverse.get("from_entry_point").is_none());
    assert_eq!(reverse["to_entry_point"], "program-start mark");
    assert_eq!(reverse["connected"], false);

    let reverse_text = run_trace(
        repo.path(),
        "internal/util/live.prog",
        "cmd/app/main.prog",
        "text",
    );
    assert_eq!(
        reverse_text.trim(),
        "Entry point: cmd/app/main.prog (program-start mark)\nNo runtime import path from internal/util/live.prog to cmd/app/main.prog"
    );

    let test_file_trace: Value = serde_json::from_str(&run_trace(
        repo.path(),
        "cmd/app/main.test.prog",
        "internal/util/live.prog",
        "json",
    ))
    .expect("valid test trace JSON");
    assert!(test_file_trace.get("from_entry_point").is_none());
    assert!(test_file_trace.get("to_entry_point").is_none());

    let test_file_text = run_trace(
        repo.path(),
        "cmd/app/main.test.prog",
        "internal/util/live.prog",
        "text",
    );
    assert_eq!(
        test_file_text.trim(),
        "cmd/app/main.test.prog -> internal/util/live.prog"
    );
}
