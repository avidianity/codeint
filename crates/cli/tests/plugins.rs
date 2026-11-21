use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::OnceLock;

use codeint_plugin_host::{sha256, sha256_hex};
use serde_json::Value;
use tempfile::TempDir;

static BUILT_SAMPLE_PLUGIN: OnceLock<PathBuf> = OnceLock::new();

fn built_sample_plugin() -> &'static Path {
    BUILT_SAMPLE_PLUGIN.get_or_init(|| {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let manifest = root.join("crates/plugin-host/tests/fixtures/sample-plugin/Cargo.toml");
        let target = root.join("target/sample-mini-plugin");
        let status = Command::new("cargo")
            .args([
                "build",
                "--release",
                "--locked",
                "--offline",
                "--target",
                "wasm32-wasip2",
            ])
            .arg("--manifest-path")
            .arg(&manifest)
            .arg("--target-dir")
            .arg(&target)
            .status()
            .expect("run cargo build for sample plugin");
        assert!(status.success(), "sample plugin build exited with {status}");
        target.join("wasm32-wasip2/release/sample_mini_plugin.wasm")
    })
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

fn copy_plugin_into(repo_root: &Path, relative: &str) -> String {
    let bytes = fs::read(built_sample_plugin()).expect("read sample plugin");
    let full = repo_root.join(relative);
    fs::create_dir_all(full.parent().expect("parent")).expect("create parent");
    fs::write(&full, &bytes).expect("copy plugin");
    sha256_hex(&sha256(&bytes))
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
