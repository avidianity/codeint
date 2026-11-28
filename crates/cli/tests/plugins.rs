mod wasm_fixtures;

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

fn built_sample_plugin() -> &'static Path {
    BUILT_SAMPLE_PLUGIN.get_or_init(|| wasm_fixtures::wasm_fixture_path("sample_mini_plugin.wasm"))
}

fn built_sample_metric_plugin() -> &'static Path {
    BUILT_SAMPLE_METRIC_PLUGIN
        .get_or_init(|| wasm_fixtures::wasm_fixture_path("sample_long_functions_plugin.wasm"))
}

fn built_sandbox_probe(file_name: &str) -> PathBuf {
    wasm_fixtures::wasm_fixture_path(file_name)
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

fn assert_check_exits_2(dir: &Path, expected_substrings: &[&str]) {
    let output = run_codeint(dir, &["check"]);
    assert_eq!(
        output.status.code(),
        Some(2),
        "expected exit 2, got {:?}\nstderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    for expected in expected_substrings {
        assert!(
            stderr.contains(expected),
            "expected {expected:?} in stderr, got: {stderr}"
        );
    }
}

fn assert_skipped_reason(report: &Value, expected_path: &str, expected_substrings: &[&str]) {
    assert_eq!(report["skipped"][0]["path"], expected_path);
    let reason = report["skipped"][0]["reason"]
        .as_str()
        .expect("skipped reason string");
    for expected in expected_substrings {
        assert!(
            reason.contains(expected),
            "expected {expected:?} in skipped reason, got: {reason}"
        );
    }
}

fn run_check_json_with_env(dir: &Path, envs: &[(&str, &Path)]) -> Value {
    let output = run_codeint_with_env(dir, &["check", "--format", "json"], envs);
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("valid JSON report")
}

#[test]
fn missing_sha256_hash_is_a_configuration_error_and_exits_2() {
    let repo = init_repo();
    write_file(
        repo.path(),
        "codeint.json",
        r#"{ "plugins": [{ "name": "sample-mini", "path": "plugins/sample.wasm" }] }"#,
    );
    assert_check_exits_2(repo.path(), &["plugins[0]", "sha256"]);
}

#[test]
fn invalid_sha256_hash_is_a_configuration_error_and_exits_2() {
    let repo = init_repo();
    write_file(
        repo.path(),
        "codeint.json",
        r#"{ "plugins": [{ "name": "sample-mini", "path": "plugins/sample.wasm", "sha256": "not-a-hash" }] }"#,
    );
    assert_check_exits_2(repo.path(), &["plugins[0].sha256", "64-character"]);
}

#[test]
fn hash_mismatch_refuses_to_load_the_plugin_and_exits_2() {
    let repo = init_repo();
    let _actual = copy_plugin_into(repo.path(), "plugins/sample.wasm");
    write_plugin_config(
        repo.path(),
        "sample-mini",
        None,
        "plugins/sample.wasm",
        "0000000000000000000000000000000000000000000000000000000000000000",
        "",
    );
    write_file(repo.path(), "src/app.mini", "fn main() {\n  return 1\n}\n");
    assert_check_exits_2(
        repo.path(),
        &["SHA-256 hash mismatch", "sample-mini", "sample.wasm"],
    );
}

#[test]
fn sample_external_plugin_measures_its_fixture_language_end_to_end_through_codeint_check() {
    let repo = init_repo();
    let hash = copy_plugin_into(repo.path(), "plugins/sample.wasm");
    write_plugin_config(
        repo.path(),
        "sample-mini",
        None,
        "plugins/sample.wasm",
        &hash,
        "",
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

    let report = run_check_json_report(&repo.path().join("src"));
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

    assert_check_exits_2(
        repo.path(),
        &[
            "alpha-mini",
            "zeta-mini",
            ".mini",
            "src/sub/app.mini",
            "languages",
        ],
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

    let report = run_check_json_report(repo.path());
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

    let report = run_check_json_report(repo.path());
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

    let _ = run_check_json_report(repo.path());
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
    let report = run_check_json_with_env(repo.path(), &envs);
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

    let second_report =
        run_check_json_with_env(repo.path(), &[("CODEINT_CACHE_DIR", cache_dir.path())]);
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
fn plugin_max_download_mb_env_var_validates_and_enforces_cap() {
    let wasm = built_sample_plugin();
    let hash = sha256_hex(&sha256(&fs::read(wasm).expect("read sample wasm")));
    let oversized_dir = tempfile::tempdir().expect("oversized tempdir");
    let oversized_path = oversized_dir.path().join("oversized.wasm");
    fs::write(&oversized_path, vec![0u8; 2 * 1024 * 1024]).expect("write 2 MiB file");

    let server =
        LocalHttpsServer::start(&[("sample.wasm", wasm), ("oversized.wasm", &oversized_path)]);
    let cache_dir = tempfile::tempdir().expect("cache tempdir");
    let repo = init_repo();
    write_url_plugin_config(
        repo.path(),
        "oversized-remote",
        &server.url("oversized.wasm"),
        "0000000000000000000000000000000000000000000000000000000000000000",
    );
    write_file(repo.path(), "src/app.mini", "fn main() {\n  return 1\n}\n");

    for bad in ["0", "-2", "not-a-number", "8796093022208", "10000000000000"] {
        let output = run_codeint_with_env(
            repo.path(),
            &["check"],
            &[("CODEINT_PLUGIN_MAX_DOWNLOAD_MB", Path::new(bad))],
        );
        assert_eq!(output.status.code(), Some(2));
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains("CODEINT_PLUGIN_MAX_DOWNLOAD_MB"),
            "expected CODEINT_PLUGIN_MAX_DOWNLOAD_MB error for {bad:?}, got: {stderr}"
        );
    }

    let capped = run_codeint_with_env(
        repo.path(),
        &["check"],
        &[
            ("CODEINT_CACHE_DIR", cache_dir.path()),
            ("CODEINT_TEST_CA_CERT", server.cert_path()),
            ("CODEINT_PLUGIN_MAX_DOWNLOAD_MB", Path::new("1")),
        ],
    );
    assert_eq!(capped.status.code(), Some(2));
    let capped_stderr = String::from_utf8_lossy(&capped.stderr);
    assert!(
        capped_stderr.contains("oversized-remote")
            && capped_stderr.contains("1048576 bytes")
            && capped_stderr.contains("CODEINT_PLUGIN_MAX_DOWNLOAD_MB"),
        "expected 1 MiB download cap error and remedy, got: {capped_stderr}"
    );

    write_url_plugin_config(
        repo.path(),
        "sample-mini",
        &server.url("sample.wasm"),
        &hash,
    );
    let allowed = run_check_json_with_env(
        repo.path(),
        &[
            ("CODEINT_CACHE_DIR", cache_dir.path()),
            ("CODEINT_TEST_CA_CERT", server.cert_path()),
            ("CODEINT_PLUGIN_MAX_DOWNLOAD_MB", Path::new("1")),
        ],
    );
    assert_eq!(allowed["files"], 1);
}

fn assert_manifest_paths_probe_error(repo: &Path, expected_snippets: &[&str], context: &str) {
    let output = run_codeint_with_env(repo, &["check"], &[]);
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    for snippet in expected_snippets {
        assert!(
            stderr.contains(snippet),
            "{context}, missing {snippet:?}: {stderr}"
        );
    }
    assert!(
        !stderr.contains("cannot resolve runtime imports"),
        "{context}, got: {stderr}"
    );
}

#[test]
fn manifest_paths_timeout_and_trap_report_language_plugin_error_and_remedy() {
    let (timeout_repo, _hash) = init_single_probe_repo(
        "loop-manifests",
        None,
        "loop_language_plugin.wasm",
        r#""plugin_limits": { "timeout_ms": 50 }"#,
    );
    write_file(timeout_repo.path(), "loop-manifest-paths", "trigger\n");
    write_file(timeout_repo.path(), "src/app.loop", "ok\n");
    assert_manifest_paths_probe_error(
        timeout_repo.path(),
        &["loop-manifests", "50 ms", "plugin_limits.timeout_ms"],
        "expected Language plugin timeout error and plugin_limits remedy",
    );

    let (trap_repo, _trap_hash) = init_single_probe_repo(
        "trap-manifests",
        None,
        "manifest_paths_trap_language_plugin.wasm",
        "",
    );
    write_file(trap_repo.path(), "src/app.probe", "ok\n");
    assert_manifest_paths_probe_error(
        trap_repo.path(),
        &["trap-manifests", "manifest-paths"],
        "expected Language plugin trap error instead of runtime imports error",
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

        assert_check_exits_2(repo.path(), &["insecure-mini", "HTTPS"]);
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
    assert_check_exits_2(neither_repo.path(), &["missing-source", "path", "url"]);

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
    assert_check_exits_2(both_repo.path(), &["both-sources", "not both"]);
}

fn init_single_probe_repo(
    plugin_name: &str,
    kind: Option<&str>,
    wasm_file: &str,
    extra_config: &str,
) -> (TempDir, String) {
    let repo = init_repo();
    let plugin_rel = format!("plugins/{wasm_file}");
    let hash = copy_wasm_into(&built_sandbox_probe(wasm_file), repo.path(), &plugin_rel);
    write_plugin_config(
        repo.path(),
        plugin_name,
        kind,
        &plugin_rel,
        &hash,
        extra_config,
    );
    (repo, hash)
}

#[test]
fn language_plugin_that_loops_forever_skips_the_file_and_finishes_the_run() {
    let (repo, _hash) = init_single_probe_repo(
        "loop-lang",
        None,
        "loop_language_plugin.wasm",
        r#""plugin_limits": { "timeout_ms": 50 }"#,
    );
    write_file(repo.path(), "src/hang.loop", "LOOP_FOREVER\n");
    write_file(repo.path(), "src/healthy.loop", "ok line 1\nok line 2\n");

    let report = run_check_json_report(repo.path());
    assert_eq!(report["files"], 1);
    assert_eq!(report["source_lines"], 2);
    assert_skipped_reason(
        &report,
        "src/hang.loop",
        &["loop-lang", "time limit", "50 ms"],
    );
}

#[test]
fn language_plugin_that_grows_memory_past_cap_skips_the_file_and_raised_limit_takes_effect() {
    let (repo, hash) = init_single_probe_repo(
        "hog-lang",
        None,
        "memory_hog_language_plugin.wasm",
        r#""plugin_limits": { "memory_mb": 2 }"#,
    );
    write_file(repo.path(), "src/heavy.hog", "ALLOC_2MB\n");
    write_file(repo.path(), "src/light.hog", "ok\n");

    let tight_report = run_check_json_report(repo.path());
    assert_eq!(tight_report["files"], 1);
    assert_skipped_reason(
        &tight_report,
        "src/heavy.hog",
        &["hog-lang", "memory cap", "2 MiB"],
    );

    write_plugin_config(
        repo.path(),
        "hog-lang",
        None,
        "plugins/memory_hog_language_plugin.wasm",
        &hash,
        r#""plugin_limits": { "memory_mb": 8 }"#,
    );

    let raised_report = run_check_json_report(repo.path());
    assert_eq!(raised_report["files"], 2);
    assert!(raised_report.get("skipped").is_none());
}

#[test]
fn metric_plugin_that_loops_forever_stops_the_run_with_exit_2_and_names_the_plugin() {
    let (repo, _hash) = init_single_probe_repo(
        "runaway-metric",
        Some("metric"),
        "loop_metric_plugin.wasm",
        r#""plugin_limits": { "timeout_ms": 50 }"#,
    );
    write_file(repo.path(), "src/app.ts", "export const ok = 1;\n");
    assert_check_exits_2(repo.path(), &["runaway-metric", "time limit", "50 ms"]);
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

fn assert_legacy_plugin_stops_check(
    plugin_name: &str,
    kind: &str,
    wasm_file: &str,
    found_interface: &str,
    found_version: &str,
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
            && lines[0].contains(found_version)
            && lines[0].contains(expected_interface)
            && lines[0].contains("0.5.0"),
        "expected stderr line 1 to name plugin and both versions, got: {}",
        lines[0]
    );
    assert!(
        lines[1].to_ascii_lowercase().contains("rebuild") && lines[1].contains("0.5.0"),
        "expected stderr line 2 to instruct rebuilding against 0.5.0, got: {}",
        lines[1]
    );
}

#[test]
fn v0_2_0_through_v0_4_0_language_plugins_stop_check_with_exit_2_and_rebuild_remedy() {
    assert_legacy_plugin_stops_check(
        "legacy-v02-lang",
        "language",
        "legacy_v0_2_0_language_plugin.wasm",
        "codeint:plugin/language@0.2.0",
        "0.2.0",
        "codeint:plugin/language@0.5.0",
    );
    assert_legacy_plugin_stops_check(
        "legacy-v03-lang",
        "language",
        "legacy_v0_3_0_language_plugin.wasm",
        "codeint:plugin/language@0.3.0",
        "0.3.0",
        "codeint:plugin/language@0.5.0",
    );
    assert_legacy_plugin_stops_check(
        "legacy-v04-lang",
        "language",
        "legacy_v0_4_0_language_plugin.wasm",
        "codeint:plugin/language@0.4.0",
        "0.4.0",
        "codeint:plugin/language@0.5.0",
    );
}

#[test]
fn v0_2_0_through_v0_4_0_metric_plugins_stop_check_with_exit_2_and_rebuild_remedy() {
    assert_legacy_plugin_stops_check(
        "legacy-v02-metric",
        "metric",
        "legacy_v0_2_0_metric_plugin.wasm",
        "codeint:plugin/metric@0.2.0",
        "0.2.0",
        "codeint:plugin/metric@0.5.0",
    );
    assert_legacy_plugin_stops_check(
        "legacy-v03-metric",
        "metric",
        "legacy_v0_3_0_metric_plugin.wasm",
        "codeint:plugin/metric@0.3.0",
        "0.3.0",
        "codeint:plugin/metric@0.5.0",
    );
    assert_legacy_plugin_stops_check(
        "legacy-v04-metric",
        "metric",
        "legacy_v0_4_0_metric_plugin.wasm",
        "codeint:plugin/metric@0.4.0",
        "0.4.0",
        "codeint:plugin/metric@0.5.0",
    );
}

#[test]
fn zero_version_metric_plugin_stops_check_with_exit_2_and_actionable_remedy() {
    let output = run_check_with_sandbox_probe(
        "zero-ver-metric",
        Some("metric"),
        "zero_version_metric_plugin.wasm",
    );
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&output.stderr);
    let lines: Vec<&str> = stderr.lines().collect();
    assert!(lines.len() >= 2, "expected error and remedy, got: {stderr}");
    assert!(
        lines[0].contains("zero-ver-metric") && lines[0].contains("version 0"),
        "expected version 0 error, got: {}",
        lines[0]
    );
    assert!(
        lines[1].contains("positive integer") && lines[1].contains("version"),
        "expected actionable version remedy, got: {}",
        lines[1]
    );
}

fn run_check_markdown_stdout(repo_path: &Path) -> String {
    let output = run_codeint(repo_path, &["check", "--format", "markdown"]);
    assert_eq!(output.status.code(), Some(0));
    String::from_utf8_lossy(&output.stdout).into_owned()
}

#[test]
fn stale_reference_range_from_external_language_plugin_hides_stale_range_with_note_without_failing()
{
    let (repo, _hash) = init_single_probe_repo(
        "stale-ranges",
        None,
        "stale_reference_range_language_plugin.wasm",
        "",
    );
    write_file(repo.path(), "src/sample.stale", "alpha\nbeta\n");

    let report = run_check_json_report(repo.path());
    let prod = &report["source_sets"]["production"];
    for (metric, expected_found, expected_current) in [
        ("erosion", "Metric version 2", "Metric version 1"),
        ("verbosity", "Metric version 0", "Metric version 2"),
    ] {
        let note = prod[metric]["reference_range"]
            .as_str()
            .unwrap_or_else(|| panic!("stale {metric} Reference range must be hidden as a note"));
        assert!(
            note.contains(expected_found) && note.contains(expected_current),
            "unexpected stale {metric} note: {note:?}"
        );
    }
    assert_eq!(
        prod["duplication"]["reference_range"]["source"],
        "fresh v2 duplication panel"
    );

    let md_stdout = run_check_markdown_stdout(repo.path());
    for needle in [
        "Metric version 2",
        "Metric version 0",
        "fresh v2 duplication panel",
    ] {
        assert!(
            md_stdout.contains(needle),
            "missing {needle:?} in markdown: {md_stdout}"
        );
    }
}

fn assert_exact_language_findings(findings: &[Value], expected: &[(&str, &str)]) {
    let language_findings: Vec<&Value> = findings
        .iter()
        .filter(|finding| {
            finding["path"]
                .as_str()
                .is_some_and(|path| path.starts_with("languages/"))
        })
        .collect();
    assert_eq!(
        language_findings.len(),
        expected.len(),
        "expected only measured languages in model.languages, got: {language_findings:?}"
    );
    for (path, message) in expected {
        assert!(
            language_findings
                .iter()
                .any(|finding| finding["path"] == *path && finding["message"] == *message),
            "expected ({path}, {message}) in {language_findings:?}"
        );
    }
}

#[test]
fn coverage_reader_metric_plugin_receives_per_language_coverage_statement_from_check() {
    let (repo, _hash) = init_single_probe_repo(
        "coverage-reader",
        Some("metric"),
        "coverage_reader_metric_plugin.wasm",
        "",
    );
    write_file(
        repo.path(),
        "src/app.ts",
        "export function greet(name: string): string { return name; }\n",
    );

    let report = run_check_json_report(repo.path());
    let findings = report["source_sets"]["production"]["coverage_reader"]["findings"]
        .as_array()
        .expect("coverage_reader findings list");
    assert_exact_language_findings(
        findings,
        &[(
            "languages/typescript-javascript",
            "language=typescript-javascript has_coverage=false",
        )],
    );
}

#[test]
fn coverage_reader_metric_plugin_receives_filled_function_and_language_coverage_facts() {
    let (repo, _hash) = init_single_probe_repo(
        "coverage-reader",
        Some("metric"),
        "coverage_reader_metric_plugin.wasm",
        r#""coverage_reports": ["coverage/lcov.info"]"#,
    );
    write_file(
        repo.path(),
        "src/app.ts",
        concat!(
            "export function covered(x: number): number {\n",
            "  const doubled = x * 2;\n",
            "  return doubled;\n",
            "}\n",
            "export function partial(x: number): number {\n",
            "  if (x > 0) {\n",
            "    return x;\n",
            "  }\n",
            "  return 0;\n",
            "}\n",
        ),
    );
    write_file(
        repo.path(),
        "coverage/lcov.info",
        concat!(
            "SF:src/app.ts\n",
            "DA:2,1\n",
            "DA:3,1\n",
            "DA:6,1\n",
            "DA:7,0\n",
            "DA:9,1\n",
            "end_of_record\n",
        ),
    );

    let report = run_check_json_report(repo.path());
    let findings = report["source_sets"]["production"]["coverage_reader"]["findings"]
        .as_array()
        .expect("coverage_reader findings list");
    assert!(
        findings.iter().any(|finding| {
            finding["path"] == "src/app.ts"
                && finding["id"] == "covered:covered"
                && finding["message"] == "listed=2 executed=2"
        }),
        "expected filled coverage for covered(), got: {findings:?}"
    );
    assert!(
        findings.iter().any(|finding| {
            finding["path"] == "src/app.ts"
                && finding["id"] == "covered:partial"
                && finding["message"] == "listed=3 executed=2"
        }),
        "expected filled coverage for partial(), got: {findings:?}"
    );
    assert_exact_language_findings(
        findings,
        &[(
            "languages/typescript-javascript",
            "language=typescript-javascript has_coverage=true",
        )],
    );
}

fn init_program_start_repo() -> (TempDir, String) {
    let (repo, hash) = init_single_probe_repo(
        "program-start",
        None,
        "program_start_language_plugin.wasm",
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

    let report = run_check_json_report(repo.path());
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
        "plugins/program_start_language_plugin.wasm",
        &hash,
        r#""entry_points": ["internal/util/extra_entry.prog"]"#,
    );

    let glob_report = run_check_json_report(repo.path());
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

    write_file(
        repo.path(),
        "scripts/main.prog",
        "program main\nuse internal/util/live.prog\n",
    );
    let tooling_trace: Value = serde_json::from_str(&run_trace(
        repo.path(),
        "scripts/main.prog",
        "internal/util/live.prog",
        "json",
    ))
    .expect("valid tooling trace JSON");
    assert!(tooling_trace.get("from_entry_point").is_none());
    assert!(tooling_trace.get("to_entry_point").is_none());

    let (pkg_repo, _hash) = init_single_probe_repo(
        "folder-import-lang",
        None,
        "folder_import_language_plugin.wasm",
        "",
    );
    write_file(pkg_repo.path(), "fld.workspace", "packages/app\n");
    write_file(
        pkg_repo.path(),
        "packages/app/src/main.fld",
        "program main\nfn main()\n",
    );
    write_file(
        pkg_repo.path(),
        "packages/app/scripts/tool.fld",
        "program main\nfn tool()\n",
    );
    let pkg_trace_out = run_codeint(
        pkg_repo.path(),
        &[
            "trace",
            "--package",
            "packages/app",
            "packages/app/scripts/tool.fld",
            "packages/app/scripts/tool.fld",
            "--format",
            "json",
        ],
    );
    assert_eq!(
        pkg_trace_out.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&pkg_trace_out.stderr)
    );
    let pkg_trace: Value = serde_json::from_slice(&pkg_trace_out.stdout).expect("pkg trace json");
    assert!(pkg_trace.get("from_entry_point").is_none());
    assert!(pkg_trace.get("to_entry_point").is_none());
}

fn init_paired_language_and_metric_repo(
    language_name: &str,
    language_wasm: &str,
    metric_name: &str,
    metric_wasm: &str,
) -> TempDir {
    let repo = init_repo();
    let lang_rel = format!("plugins/{language_wasm}");
    let metric_rel = format!("plugins/{metric_wasm}");
    let lang_hash = copy_wasm_into(&built_sandbox_probe(language_wasm), repo.path(), &lang_rel);
    let metric_hash = copy_wasm_into(&built_sandbox_probe(metric_wasm), repo.path(), &metric_rel);
    write_file(
        repo.path(),
        "codeint.json",
        &format!(
            r#"{{
              "plugins": [
                {{
                  "name": "{language_name}",
                  "kind": "language",
                  "path": "{lang_rel}",
                  "sha256": "{lang_hash}"
                }},
                {{
                  "name": "{metric_name}",
                  "kind": "metric",
                  "path": "{metric_rel}",
                  "sha256": "{metric_hash}"
                }}
              ]
            }}"#
        ),
    );
    repo
}

fn init_hazard_rules_repo_with_metric() -> TempDir {
    init_paired_language_and_metric_repo(
        "hazard-lang",
        "hazard_rules_language_plugin.wasm",
        "hazard-facts",
        "hazard_facts_metric_plugin.wasm",
    )
}

fn run_check_json_report(repo: &Path) -> Value {
    let output = run_codeint(repo, &["check", "--format", "json"]);
    assert_eq!(
        output.status.code(),
        Some(0),
        "expected exit 0, got {:?}\nstderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("parse json report")
}

fn run_paired_language_and_metric_probe_check(
    language_name: &str,
    language_wasm: &str,
    metric_name: &str,
    metric_wasm: &str,
    extra_file: (&str, &str),
) -> Value {
    let repo = init_paired_language_and_metric_repo(
        language_name,
        language_wasm,
        metric_name,
        metric_wasm,
    );
    write_file(
        repo.path(),
        "src/app.ts",
        "export function greet(name: string): string { return name; }\n",
    );
    write_file(repo.path(), extra_file.0, extra_file.1);
    run_check_json_report(repo.path())
}

fn run_check_hazard_facts_findings(repo: &Path) -> Vec<Value> {
    let report = run_check_json_report(repo);
    report["source_sets"]["production"]["hazard_facts"]["findings"]
        .as_array()
        .expect("hazard_facts findings list")
        .clone()
}

fn assert_exit_2_without_stdout(repo: &Path, args: &[&str], expected_substrings: &[&str]) {
    let output = run_codeint(repo, args);
    assert_eq!(
        output.status.code(),
        Some(2),
        "args {args:?}: expected exit 2, got {:?}\nstderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        output.stdout.is_empty(),
        "args {args:?}: expected no report on stdout, got: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    for expected in expected_substrings {
        assert!(
            stderr.contains(expected),
            "args {args:?}: expected {expected:?} in stderr, got: {stderr}"
        );
    }
}

#[test]
fn hazard_facts_metric_plugin_reads_per_language_hazard_rules_from_check() {
    let repo = init_hazard_rules_repo_with_metric();
    write_file(
        repo.path(),
        "src/app.ts",
        "export function greet(name: string): string { return name; }\n",
    );
    write_file(repo.path(), "queries/clean.haz", "SELECT id FROM users;\n");

    let findings = run_check_hazard_facts_findings(repo.path());
    assert_exact_language_findings(
        &findings,
        &[
            (
                "languages/hazard-lang",
                "language=hazard-lang has_hazard_rules=true",
            ),
            (
                "languages/typescript-javascript",
                "language=typescript-javascript has_hazard_rules=false",
            ),
        ],
    );
}

#[test]
fn empty_resolved_import_list_stops_check_with_exit_2_naming_plugin() {
    let (repo, _hash) = init_single_probe_repo(
        "folder-import-lang",
        None,
        "folder_import_language_plugin.wasm",
        "",
    );
    write_file(repo.path(), "src/app.fld", "use empty\nfn main()\n");
    assert_exit_2_without_stdout(
        repo.path(),
        &["check", "--format", "json"],
        &["folder-import-lang", "empty list"],
    );
}

#[test]
fn hazard_line_beyond_end_of_file_stops_check_with_exit_2_naming_plugin_file_and_line() {
    let (repo, _hash) = init_single_probe_repo(
        "hazard-lang",
        Some("language"),
        "hazard_rules_language_plugin.wasm",
        "",
    );
    write_file(
        repo.path(),
        "queries/beyond.haz",
        "SELECT 1;\nHAZARD_BEYOND_EOF\nSELECT 2;\n",
    );
    commit_all(repo.path(), "commit file with out-of-range hazard line");

    for args in [&["check", "--format", "json"][..], &["diff", "HEAD"][..]] {
        assert_exit_2_without_stdout(
            repo.path(),
            args,
            &["hazard-lang", "queries/beyond.haz", "line 4"],
        );
    }

    write_file(
        repo.path(),
        "queries/beyond.haz",
        "SELECT 1;\nHAZARD_LINE_ZERO\n",
    );
    assert_exit_2_without_stdout(
        repo.path(),
        &["check", "--format", "json"],
        &["hazard-lang", "queries/beyond.haz", "line 0"],
    );
}

#[test]
fn hazard_line_on_last_line_of_file_is_accepted_with_and_without_trailing_newline() {
    let repo = init_hazard_rules_repo_with_metric();
    write_file(
        repo.path(),
        "queries/with_newline.haz",
        "SELECT id FROM users;\nDELETE FROM sessions;\n",
    );
    write_file(
        repo.path(),
        "queries/without_newline.haz",
        "SELECT id FROM users;\nDELETE FROM sessions;",
    );

    let findings = run_check_hazard_facts_findings(repo.path());
    for expected_path in ["queries/with_newline.haz", "queries/without_newline.haz"] {
        let expected_id = format!("hazard:{expected_path}:2:delete-without-where");
        assert!(
            findings.iter().any(|finding| {
                finding["path"] == expected_path
                    && finding["line"] == 2
                    && finding["id"] == expected_id
            }),
            "expected last-line hazard for {expected_path}, got: {findings:?}"
        );
    }
}

fn init_unlisted_hazard_probe_repo(source: &str) -> TempDir {
    let (repo, _hash) = init_single_probe_repo(
        "unlisted-hazard-lang",
        None,
        "unlisted_hazard_rule_language_plugin.wasm",
        "",
    );
    write_file(repo.path(), "src/query.uhr", source);
    repo
}

#[test]
fn unlisted_hazard_line_rule_stops_check_with_exit_2_naming_plugin_file_and_rule() {
    let repo = init_unlisted_hazard_probe_repo("SELECT 1;\nUNLISTED_HAZARD\n");
    assert_exit_2_without_stdout(
        repo.path(),
        &["check", "--format", "json"],
        &[
            "unlisted-hazard-lang",
            "src/query.uhr",
            "unlisted-secret-rule",
        ],
    );
}

#[test]
fn listed_hazard_line_rule_is_accepted_by_check() {
    let repo = init_unlisted_hazard_probe_repo("SELECT 1;\nALLOWED_HAZARD\n");
    let report = run_check_json_report(repo.path());
    assert_eq!(report["files"], 1);
}

#[test]
fn no_imports_language_plugin_check_skips_import_calls_and_metric_reads_has_imports() {
    let report = run_paired_language_and_metric_probe_check(
        "no-imports-lang",
        "no_imports_language_plugin.wasm",
        "import-capability",
        "import_capability_metric_plugin.wasm",
        ("queries/users.noimp", "fn select_users()\n"),
    );
    let findings = report["source_sets"]["production"]["import_capability"]["findings"]
        .as_array()
        .expect("import_capability findings list");
    assert_exact_language_findings(
        findings,
        &[
            (
                "languages/no-imports-lang",
                "language=no-imports-lang has_imports=false",
            ),
            (
                "languages/typescript-javascript",
                "language=typescript-javascript has_imports=true",
            ),
        ],
    );
}

#[test]
fn no_imports_only_repo_omits_import_cycles_and_dependencies_across_check_diff_and_trend() {
    let (repo, _hash) = init_single_probe_repo(
        "no-imports-lang",
        None,
        "no_imports_language_plugin.wasm",
        r#""budgets": { "unused_dependencies": 0.0 }"#,
    );
    write_file(
        repo.path(),
        "package.json",
        r#"{"name":"docs-site","dependencies":{"left-pad":"^1.0.0"}}"#,
    );
    write_file(repo.path(), "queries/users.noimp", "fn select_users()\n");
    commit_all(repo.path(), "initial no-imports commit");
    write_file(repo.path(), "queries/orders.noimp", "fn select_orders()\n");

    let check = run_check_json_report(repo.path());
    assert!(
        check["source_sets"]["production"]
            .get("import_cycles")
            .is_none()
    );
    assert!(check.get("dependencies").is_none());

    let diff_out = run_codeint(repo.path(), &["diff", "HEAD", "--format", "json"]);
    assert_eq!(
        diff_out.status.code(),
        Some(0),
        "diff stderr: {}",
        String::from_utf8_lossy(&diff_out.stderr)
    );
    let diff: Value = serde_json::from_slice(&diff_out.stdout).expect("parse diff json");
    assert!(
        diff["source_sets"]["production"]
            .get("import_cycles")
            .is_none()
    );
    assert!(diff.get("dependencies").is_none());

    let trend_out = run_codeint(repo.path(), &["trend", "--format", "json"]);
    assert_eq!(
        trend_out.status.code(),
        Some(0),
        "trend stderr: {}",
        String::from_utf8_lossy(&trend_out.stderr)
    );
    let trend: Value = serde_json::from_slice(&trend_out.stdout).expect("parse trend json");
    assert!(
        trend["source_sets"]["production"][0]
            .get("import_cycles")
            .is_none()
    );
}

fn assert_prod_hazards(report: &Value, hazard_lines: u64, ratio: f64, expected_rules: &[&str]) {
    let section = &report["source_sets"]["production"]["hazards"];
    assert_eq!(section["hazard_lines"], hazard_lines, "report: {report}");
    assert_eq!(section["source_lines"], 4, "report: {report}");
    assert_eq!(section["ratio"], ratio, "report: {report}");
    let actual: Vec<&str> = section["findings"]
        .as_array()
        .expect("hazard findings")
        .iter()
        .filter_map(|item| item["rule"].as_str())
        .collect();
    assert_eq!(actual, expected_rules, "report: {report}");
}

#[test]
fn external_wasm_language_plugin_with_hazard_rules_populates_hazards_metric() {
    let (repo, hash) = init_single_probe_repo(
        "hazard-lang",
        Some("language"),
        "hazard_rules_language_plugin.wasm",
        "",
    );
    write_file(
        repo.path(),
        "src/app.ts",
        "export const a = 1;\nexport const b = 2;\nexport const c = 3;\nexport const d = 4;\n",
    );
    write_file(
        repo.path(),
        "queries/ops.haz",
        "SELECT id FROM users WHERE id = 1;\nDELETE FROM sessions;\nSELECT name FROM users WHERE name LIKE '%foo';\nSELECT 1;\n",
    );

    let report = run_check_json_report(repo.path());
    assert_prod_hazards(
        &report,
        2,
        0.5,
        &["delete-without-where", "leading-wildcard-like"],
    );
    assert!(
        report["languages"]["typescript-javascript"]["source_sets"]["production"]
            .get("hazards")
            .is_none()
    );
    assert_eq!(
        report["languages"]["hazard-lang"]["source_sets"]["production"]["hazards"]["ratio"],
        0.5
    );

    write_plugin_config(
        repo.path(),
        "hazard-lang",
        Some("language"),
        "plugins/hazard_rules_language_plugin.wasm",
        &hash,
        r#""hazards": { "disabled_rules": ["delete-without-where"] }"#,
    );
    let filtered = run_check_json_report(repo.path());
    assert_prod_hazards(&filtered, 1, 0.25, &["leading-wildcard-like"]);
}

#[test]
fn every_built_in_metric_and_source_set_key_from_schema_is_rejected_for_external_plugins() {
    let schema = serde_json::to_value(codeint_cli::schema()).expect("serialize schema");
    for metric in schema["metrics"].as_array().expect("schema metrics") {
        let key = metric["key"].as_str().expect("metric key");
        let error = codeint_plugin_host::validate_metric_key("external-probe", key)
            .expect_err("built-in Metric key from schema must be rejected");
        assert!(
            matches!(
                error,
                codeint_plugin_host::PluginHostError::InvalidMetricKey {
                    key: ref rejected, ..
                } if rejected == key
            ),
            "unexpected error for {key}: {error}"
        );
    }
    for source_set in schema["source_sets"]
        .as_array()
        .expect("schema source_sets")
    {
        let key = source_set.as_str().expect("source set key");
        codeint_plugin_host::validate_metric_key("external-probe", key)
            .expect_err("Source set key from schema must be rejected");
    }
}

#[test]
fn codeint_check_refuses_external_metric_plugin_using_built_in_key() {
    let (repo, _hash) = init_single_probe_repo(
        "reserved-metric",
        Some("metric"),
        "reserved_key_metric_plugin.wasm",
        "",
    );
    write_file(repo.path(), "src/app.ts", "export const x = 1;\n");

    let output = Command::new(env!("CARGO_BIN_EXE_codeint"))
        .current_dir(repo.path())
        .arg("check")
        .output()
        .expect("run codeint check");
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("cognitive_complexity") && stderr.contains("collides"),
        "expected stderr to reject cognitive_complexity collision, got: {stderr}"
    );
}

fn run_diff_head_json(repo: &Path, cache: &Path) -> Value {
    let out = run_codeint_with_env(
        repo,
        &["diff", "HEAD", "--format", "json"],
        &[("CODEINT_CACHE_DIR", cache)],
    );
    assert!(out.status.success());
    serde_json::from_slice(&out.stdout).expect("diff json")
}

#[test]
fn external_metric_plugin_records_metric_version_in_reports_and_commit_cache_keys() {
    let repo = init_repo();
    let cache = tempfile::tempdir().expect("cache dir");
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
    commit_all(repo.path(), "base");

    let check_report = run_check_json_report(repo.path());
    assert_eq!(
        check_report["source_sets"]["production"]["long_functions"]["metric_version"],
        1
    );

    let diff_report = run_diff_head_json(repo.path(), cache.path());
    assert_eq!(
        diff_report["source_sets"]["production"]["long_functions"]["metric_version"],
        1
    );

    #[derive(serde::Deserialize)]
    struct StoredCache {
        settings: String,
        result: Value,
    }

    let only_file = fs::read_dir(cache.path())
        .expect("read cache dir")
        .map(|item| item.expect("item").path())
        .collect::<Vec<_>>();
    assert_eq!(only_file.len(), 1);
    let stored: StoredCache =
        serde_json::from_slice(&fs::read(&only_file[0]).expect("read bytes")).expect("stored");
    let mut cfg: Value = serde_json::from_str(&stored.settings).expect("cfg");
    assert_eq!(cfg["metric_versions"]["long_functions"], 1);
    cfg["metric_versions"]["long_functions"] = serde_json::json!(2);
    let mut poisoned = stored.result;
    poisoned["production"]["external_metrics"]["long_functions"]["measurement"]["ratio"] =
        serde_json::json!(0.9999);

    let seeded_cache = tempfile::tempdir().expect("seeded cache");
    let head_commit = codeint_git::Repository::discover(repo.path())
        .expect("discover")
        .resolve("HEAD")
        .expect("head");
    codeint_git::ResultCache::in_folder(seeded_cache.path().to_path_buf())
        .write(head_commit, &cfg, &poisoned)
        .expect("write poisoned");

    let fresh = run_diff_head_json(repo.path(), seeded_cache.path());
    assert_eq!(
        fresh["source_sets"]["production"]["long_functions"]["base_ratio"],
        0.5
    );
    assert_eq!(fs::read_dir(seeded_cache.path()).expect("list").count(), 2);
}

fn trap_workspace_repo(plugin_name: &str) -> TempDir {
    let (repo, _hash) = init_single_probe_repo(plugin_name, None, "loop_language_plugin.wasm", "");
    write_file(repo.path(), "trapper", "PANIC_TRAP\n");
    write_file(repo.path(), "src/one.loop", "alpha\n");
    commit_all(repo.path(), "trap workspace");
    repo
}

#[test]
fn plugin_trap_in_workspace_packages_stays_fatal_for_every_plugin_name() {
    for plugin_name in ["trapper", "other"] {
        let repo = trap_workspace_repo(plugin_name);

        let output = run_codeint(repo.path(), &["check", "--format", "json"]);

        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.code(), Some(2), "{plugin_name}: {stderr}");
        assert!(
            stderr.contains(&format!(
                "Language plugin \"{plugin_name}\" failed during analyze-manifests"
            )),
            "{plugin_name}: {stderr}"
        );
        assert!(
            !stderr.contains("Skipped the invalid") && output.stdout.is_empty(),
            "{plugin_name}: {stderr}"
        );
        assert!(
            stderr.contains("report it to the Plugin's author")
                && !stderr.contains("Fix the manifest"),
            "{plugin_name}: {stderr}"
        );
    }
}

#[test]
fn external_v05_wasm_plugin_supports_dialects_diagnostics_and_analyze_scope_end_to_end() {
    let extra_config = r#"
      "dialects": {
        "v05-lang": [
          { "dialect": "alpha", "include": ["alpha/**"] },
          { "dialect": "beta", "include": ["beta/**"] }
        ]
      }
    "#;
    let (repo, _hash) = init_single_probe_repo(
        "v05-lang",
        None,
        "v0_5_0_features_language_plugin.wasm",
        extra_config,
    );
    write_file(repo.path(), "alpha/a.v05", "DIAG_NOTE\nSCOPE_RISK\n");
    write_file(repo.path(), "beta/b.v05", "ok\n");
    write_file(repo.path(), "plain/c.v05", "ok\n");

    let report = run_check_json_report(repo.path());
    assert_eq!(report["files"], 3, "report: {report}");
    assert_eq!(report["file_diagnostics"][0]["path"], "alpha/a.v05");
    assert_eq!(report["file_diagnostics"][0]["line"], 1);
    assert_eq!(report["file_diagnostics"][0]["kind"], "note");
    assert_eq!(report["file_diagnostics"][0]["message"], "v05 diagnostic");

    let hazards = report["source_sets"]["production"]["hazards"]["findings"]
        .as_array()
        .expect("hazard findings");
    assert!(
        hazards.iter().any(|f| f["path"] == "alpha/a.v05"
            && f["line"] == 1
            && f["rule"] == "cross-file-risk"),
        "expected cross-file-risk from analyze-scope: {report}"
    );

    let summaries = report["scope_summaries"]
        .as_array()
        .expect("scope_summaries");
    assert_eq!(summaries.len(), 3, "summaries: {summaries:?}");
    let dialects: Vec<&str> = summaries
        .iter()
        .filter_map(|s| s["fields"]["dialect"].as_str())
        .collect();
    assert_eq!(dialects, ["none", "alpha", "beta"]);
    assert!(
        summaries
            .iter()
            .all(|s| s["language"] == "v05-lang" && s["kind"] == "v05-scope")
    );

    let (bad_dialect_repo, _) = init_single_probe_repo(
        "v05-lang",
        None,
        "v0_5_0_features_language_plugin.wasm",
        r#""dialects": { "v05-lang": [{ "dialect": "gamma", "include": ["src/**"] }] }"#,
    );
    write_file(bad_dialect_repo.path(), "src/a.v05", "ok\n");
    assert_check_exits_2(bad_dialect_repo.path(), &["gamma", "alpha, beta"]);

    let (bad_line_repo, _) =
        init_single_probe_repo("v05-lang", None, "v0_5_0_features_language_plugin.wasm", "");
    write_file(bad_line_repo.path(), "src/bad.v05", "SCOPE_BAD_LINE\n");
    assert_check_exits_2(bad_line_repo.path(), &["outside the file's", "4000000000"]);
}
