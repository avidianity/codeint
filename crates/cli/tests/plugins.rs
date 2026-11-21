use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::OnceLock;

use codeint_plugin_host::{sha256, sha256_hex};
use serde_json::Value;
use tempfile::TempDir;

static BUILT_SAMPLE_PLUGIN: OnceLock<PathBuf> = OnceLock::new();
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
            "sample-mini-plugin",
            false,
        )
        .join("sample_mini_plugin.wasm")
    })
}

fn built_sandbox_probe(file_name: &str) -> PathBuf {
    BUILT_SANDBOX_PROBES
        .get_or_init(|| {
            build_wasm_fixture(
                "crates/plugin-host/tests/fixtures/sandbox-probes/Cargo.toml",
                "sandbox-probes",
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
        stderr.contains("cannot instantiate Language plugin")
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
