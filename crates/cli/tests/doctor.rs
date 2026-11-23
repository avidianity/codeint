use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::process::{Command, Output};

use codeint_plugin_host::{sha256, sha256_hex};
use serde_json::{Value, json};
use tempfile::TempDir;

struct DoctorFixture {
    dir: TempDir,
    cache: TempDir,
}

impl DoctorFixture {
    fn plain() -> DoctorFixture {
        DoctorFixture {
            dir: TempDir::new().expect("tempdir"),
            cache: TempDir::new().expect("cache dir"),
        }
    }

    fn git_repo() -> DoctorFixture {
        let fixture = DoctorFixture::plain();
        let status = Command::new("git")
            .args(["init", "-q", "-b", "main"])
            .current_dir(fixture.root())
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .status()
            .expect("git init");
        assert!(status.success(), "git init failed");
        fixture
    }

    fn git_repo_with_ts() -> DoctorFixture {
        let fixture = DoctorFixture::git_repo();
        fixture.write("src/app.ts", "export const ok = 1;\n");
        fixture
    }

    fn root(&self) -> &Path {
        self.dir.path()
    }

    fn write(&self, relative: &str, contents: impl AsRef<[u8]>) {
        let target = self.dir.path().join(relative);
        let parent = target.parent().expect("relative path has parent");
        fs::create_dir_all(parent).expect("create parent dirs");
        fs::write(&target, contents.as_ref()).expect("write file");
    }

    fn write_config(&self, config: &Value) {
        self.write("codeint.json", config.to_string());
    }

    fn write_plugin_config(&self, plugin: Value) {
        self.write_config(&json!({
            "$schema": current_schema_url(),
            "plugins": [plugin]
        }));
    }

    fn write_local_plugin(&self, name: &str, path: &str, sha256: &str) {
        self.write_plugin_config(json!({
            "name": name,
            "path": path,
            "sha256": sha256
        }));
    }

    fn write_sample_remote_metric_plugin(&self, sha256: &str) {
        self.write_plugin_config(json!({
            "name": "remote-plugin",
            "kind": "metric",
            "url": "https://example.invalid/plugins/remote.wasm",
            "sha256": sha256
        }));
    }

    fn run(&self, args: &[&str]) -> Output {
        let mut command = Command::new(env!("CARGO_BIN_EXE_codeint"));
        command.current_dir(self.dir.path());
        command.env("CODEINT_CACHE_DIR", self.cache.path());
        command.arg("doctor").args(args);
        command.output().expect("run codeint doctor")
    }

    fn run_json(&self, extra_args: &[&str], expected_exit: i32) -> Value {
        let mut args = extra_args.to_vec();
        args.extend(["--format", "json"]);
        let output = self.run(&args);
        assert_eq!(
            output.status.code(),
            Some(expected_exit),
            "stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
            panic!(
                "expected valid JSON stdout, got {error}\nstdout: {}\nstderr: {}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            )
        })
    }

    fn run_toon(&self, expected_exit: i32) -> Value {
        let output = self.run(&["--format", "toon"]);
        assert_eq!(output.status.code(), Some(expected_exit));
        toon_format::decode_default(std::str::from_utf8(&output.stdout).expect("utf8"))
            .expect("valid TOON")
    }

    fn run_text(&self, expected_exit: i32) -> String {
        let output = self.run(&[]);
        assert_eq!(output.status.code(), Some(expected_exit));
        String::from_utf8(output.stdout).expect("utf8")
    }

    fn snapshot(&self) -> BTreeMap<String, Vec<u8>> {
        let mut files = BTreeMap::new();
        collect_tree(self.root(), self.root(), &mut files);
        files
    }
}

fn collect_tree(root: &Path, current: &Path, files: &mut BTreeMap<String, Vec<u8>>) {
    let mut entries: Vec<_> = fs::read_dir(current)
        .expect("read dir")
        .map(|entry| entry.expect("dir entry"))
        .collect();
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let path = entry.path();
        if path.file_name().and_then(|name| name.to_str()) == Some(".git") {
            continue;
        }
        let file_type = entry.file_type().expect("file type");
        if file_type.is_dir() {
            collect_tree(root, &path, files);
        } else if file_type.is_file() {
            let relative = path
                .strip_prefix(root)
                .expect("relative path")
                .to_string_lossy()
                .into_owned();
            files.insert(relative, fs::read(&path).expect("read file"));
        }
    }
}

fn find_check<'a>(report: &'a Value, id: &str) -> &'a Value {
    report["checks"]
        .as_array()
        .expect("checks array")
        .iter()
        .find(|check| check["id"] == id)
        .unwrap_or_else(|| panic!("expected check {id:?} in report: {report}"))
}

fn assert_check_field(report: &Value, id: &str, status: &str, field: &str, expected: &str) {
    let check = find_check(report, id);
    assert_eq!(check["status"], status, "check: {check}");
    let text = check[field].as_str().unwrap_or_default();
    assert!(
        text.contains(expected),
        "expected {id}.{field} to contain {expected:?}, got {text:?}"
    );
}

fn assert_non_blocking_summary(report: &Value, expected_status: &str, expect_warnings: bool) {
    assert_eq!(report["status"], expected_status);
    assert_eq!(report["summary"]["errors"], 0);
    let warnings = report["summary"]["warnings"].as_u64().unwrap_or(0);
    if expect_warnings {
        assert!(warnings >= 1, "expected warnings in {report}");
    } else {
        assert_eq!(warnings, 0, "unexpected warnings in {report}");
    }
}

fn current_schema_url() -> String {
    format!(
        "https://raw.githubusercontent.com/avidianity/codeint/v{}/schema/codeint.schema.json",
        env!("CARGO_PKG_VERSION")
    )
}

#[test]
fn doctor_reports_healthy_repo_in_text_json_and_toon_and_is_strictly_read_only() {
    let fixture = DoctorFixture::git_repo();
    fixture.write_config(&json!({ "$schema": current_schema_url() }));
    fixture.write(
        "package.json",
        r#"{ "name": "root", "private": true, "workspaces": ["packages/*"] }"#,
    );
    fixture.write("packages/app/package.json", r#"{ "name": "app" }"#);
    fixture.write(
        "packages/app/src/index.ts",
        "export function greet(name: string): string {\n  return name;\n}\n",
    );

    let before = fixture.snapshot();

    let report = fixture.run_json(&[], 0);
    assert_eq!(report["version"], env!("CARGO_PKG_VERSION"));
    assert_non_blocking_summary(&report, "pass", false);
    assert!(report["summary"]["passed"].as_u64().unwrap_or(0) >= 6);
    for id in ["git", "config", "schema", "plugins"] {
        assert_eq!(find_check(&report, id)["status"], "pass");
    }
    assert_check_field(
        &report,
        "languages",
        "pass",
        "summary",
        "typescript-javascript",
    );
    assert_check_field(&report, "workspace", "pass", "detail", "packages/app");

    let text = fixture.run_text(0);
    assert!(text.contains("Passing checks:"), "text: {text}");
    assert!(text.contains("[PASS] git:"), "text: {text}");
    assert!(!text.contains("Warnings:"), "text: {text}");
    assert!(!text.contains("Blocking errors:"), "text: {text}");

    let decoded = fixture.run_toon(0);
    assert_eq!(decoded["status"], "pass");
    assert_eq!(decoded["summary"]["errors"], 0);

    let sub_report = fixture.run_json(&["packages/app"], 0);
    assert_eq!(sub_report["status"], "pass");

    assert_eq!(
        before,
        fixture.snapshot(),
        "codeint doctor must never modify repository files"
    );
}

#[test]
fn doctor_reports_blocking_error_for_invalid_codeint_json_and_exits_nonzero() {
    let fixture = DoctorFixture::git_repo_with_ts();
    fixture.write("codeint.json", "{ \"not_valid\": ");

    let syntax_report = fixture.run_json(&[], 1);
    assert_eq!(syntax_report["status"], "error");
    assert!(syntax_report["summary"]["errors"].as_u64().unwrap_or(0) >= 1);
    assert_check_field(
        &syntax_report,
        "config",
        "error",
        "summary",
        "not valid JSON",
    );

    fixture.write("codeint.json", r#"{ "unknown_key": true }"#);
    let text = fixture.run_text(1);
    assert!(text.contains("Blocking errors:"), "text: {text}");
    assert!(text.contains("[ERROR] config:"), "text: {text}");
    assert!(text.contains("unknown_key"), "text: {text}");
}

fn assert_blocking_error(
    fixture: &DoctorFixture,
    id: &str,
    expected_summary: &str,
    expected_remedy: &str,
) -> Value {
    let report = fixture.run_json(&[], 1);
    assert_eq!(report["status"], "error");
    assert_check_field(&report, id, "error", "summary", expected_summary);
    assert_check_field(&report, id, "error", "remedy", expected_remedy);
    report
}

#[test]
fn doctor_reports_blocking_error_for_future_schema_version_and_exits_nonzero() {
    let fixture = DoctorFixture::git_repo_with_ts();
    fixture.write_config(&json!({
        "$schema": "https://raw.githubusercontent.com/avidianity/codeint/v99.0.0/schema/codeint.schema.json"
    }));

    assert_blocking_error(&fixture, "schema", "v99.0.0", "codeint update");
}

#[test]
fn doctor_warns_outside_git_repository_without_blocking_and_remains_read_only() {
    let fixture = DoctorFixture::plain();
    fixture.write("src/app.ts", "export const ok = 1;\n");
    let before = fixture.snapshot();

    let report = fixture.run_json(&[], 0);
    assert_non_blocking_summary(&report, "warn", true);
    assert_eq!(find_check(&report, "git")["status"], "warn");

    let text = fixture.run_text(0);
    assert!(text.contains("Warnings:"), "text: {text}");
    assert!(text.contains("[WARN] git:"), "text: {text}");
    assert!(!text.contains("Blocking errors:"), "text: {text}");

    assert_eq!(before, fixture.snapshot(), "non-Git doctor must not write");
    assert!(!fixture.root().join(".git").exists());
}

#[test]
fn doctor_verifies_external_wasm_plugins_and_fails_on_missing_or_mismatched_sha256() {
    let fixture = DoctorFixture::git_repo_with_ts();

    fixture.write_plugin_config(json!({ "name": "missing-hash", "path": "plugins/sample.wasm" }));
    let missing_hash_report = fixture.run_json(&[], 1);
    assert_check_field(&missing_hash_report, "config", "error", "summary", "sha256");

    let zeros = "0000000000000000000000000000000000000000000000000000000000000000";
    fixture.write_local_plugin("sample-plugin", "plugins/missing.wasm", zeros);
    let missing_file_report = fixture.run_json(&[], 1);
    assert_eq!(
        find_check(&missing_file_report, "plugin:sample-plugin")["status"],
        "error"
    );

    let wasm_bytes = b"\0asm\x01\0\0\0";
    fixture.write("plugins/sample.wasm", wasm_bytes);
    let actual_hash = sha256_hex(&sha256(wasm_bytes));

    fixture.write_local_plugin("sample-plugin", "plugins/sample.wasm", zeros);
    let mismatch_report = fixture.run_json(&[], 1);
    assert_check_field(
        &mismatch_report,
        "plugin:sample-plugin",
        "error",
        "detail",
        &actual_hash,
    );

    fixture.write_local_plugin("sample-plugin", "plugins/sample.wasm", &actual_hash);
    let valid_report = fixture.run_json(&[], 0);
    assert_eq!(valid_report["status"], "pass");
    assert_eq!(
        find_check(&valid_report, "plugin:sample-plugin")["status"],
        "pass"
    );

    fixture.write_sample_remote_metric_plugin(&actual_hash);
    let remote_report = fixture.run_json(&[], 0);
    assert_eq!(remote_report["status"], "warn");
    assert_eq!(
        find_check(&remote_report, "plugin:remote-plugin")["status"],
        "warn"
    );
    assert!(
        fs::read_dir(fixture.cache.path())
            .expect("read cache dir")
            .next()
            .is_none(),
        "doctor must not write to cache directory"
    );
}

#[cfg(unix)]
fn assert_doctor_broken_config_symlink(file_name: &str, missing_target: &str) -> Value {
    use std::os::unix::fs::symlink;

    let fixture = DoctorFixture::git_repo_with_ts();
    symlink(
        fixture.root().join(missing_target),
        fixture.root().join(file_name),
    )
    .expect("create broken config symlink");

    assert_blocking_error(
        &fixture,
        "config",
        &format!("{file_name} is a broken symlink"),
        file_name,
    )
}

#[cfg(unix)]
#[test]
fn doctor_reports_broken_symlink_for_codeint_json_and_cached_remote_plugin() {
    use std::os::unix::fs::symlink;

    let broken_config_report =
        assert_doctor_broken_config_symlink("codeint.json", "missing-config.json");
    let checks = broken_config_report["checks"]
        .as_array()
        .expect("checks array");
    assert!(!checks.iter().any(|check| check["id"] == "plugins"));

    let remote_fixture = DoctorFixture::git_repo_with_ts();
    let expected_hash = "0000000000000000000000000000000000000000000000000000000000000000";
    remote_fixture.write_sample_remote_metric_plugin(expected_hash);
    let plugins_cache_dir = remote_fixture.cache.path().join("plugins");
    fs::create_dir_all(&plugins_cache_dir).expect("create plugins cache dir");
    symlink(
        plugins_cache_dir.join("missing.wasm"),
        plugins_cache_dir.join(format!("{expected_hash}.wasm")),
    )
    .expect("create broken cached plugin symlink");

    let broken_cache_report = remote_fixture.run_json(&[], 1);
    assert_eq!(broken_cache_report["status"], "error");
    assert_check_field(
        &broken_cache_report,
        "plugin:remote-plugin",
        "error",
        "summary",
        "broken symlink",
    );
}

#[test]
fn doctor_validates_codeint_jsonc_and_reports_ambiguous_config_files() {
    let fixture = DoctorFixture::git_repo_with_ts();
    fixture.write(
        "codeint.jsonc",
        format!(
            concat!(
                "// config with comments and trailing commas\n",
                "{{\n",
                "  \"$schema\": \"{}\",\n",
                "  /* valid budgets */\n",
                "  \"budgets\": {{\n",
                "    \"production\": {{\n",
                "      \"erosion\": 0.25,\n",
                "    }},\n",
                "  }},\n",
                "}}\n",
            ),
            current_schema_url()
        ),
    );

    let report = fixture.run_json(&[], 0);
    assert_non_blocking_summary(&report, "pass", false);
    assert_check_field(
        &report,
        "config",
        "pass",
        "summary",
        "codeint.jsonc is valid",
    );

    fixture.write("codeint.json", "{}\n");
    assert_blocking_error(
        &fixture,
        "config",
        "both codeint.json and codeint.jsonc exist at the repository root",
        "Remove either codeint.json or codeint.jsonc",
    );
}

#[cfg(unix)]
#[test]
fn doctor_reports_broken_symlink_for_codeint_jsonc() {
    let _ = assert_doctor_broken_config_symlink("codeint.jsonc", "missing-config.jsonc");
}

#[test]
fn doctor_plugin_error_remedies_name_codeint_jsonc_when_active() {
    let fixture = DoctorFixture::git_repo_with_ts();
    let zeros = "0000000000000000000000000000000000000000000000000000000000000000";
    fixture.write("plugins/sample.wasm", b"\0asm\x01\0\0\0");

    let cases = [
        (
            "plugin:missing-plugin",
            format!(
                "{{\n  // missing local plugin\n  \"plugins\": [{{ \"name\": \"missing-plugin\", \"path\": \"plugins/missing.wasm\", \"sha256\": \"{zeros}\", }}],\n}}\n"
            ),
        ),
        (
            "plugin:sample-plugin",
            format!(
                "{{\n  // mismatched digest\n  \"plugins\": [{{ \"name\": \"sample-plugin\", \"path\": \"plugins/sample.wasm\", \"sha256\": \"{zeros}\", }}],\n}}\n"
            ),
        ),
        (
            "plugin:typescript-javascript",
            format!(
                "{{\n  // collides with built-in plugin\n  \"plugins\": [{{ \"name\": \"typescript-javascript\", \"path\": \"plugins/sample.wasm\", \"sha256\": \"{zeros}\", }}],\n}}\n"
            ),
        ),
    ];

    for (check_id, body) in cases {
        fixture.write("codeint.jsonc", body);
        let report = fixture.run_json(&[], 1);
        let check = find_check(&report, check_id);
        assert_eq!(check["status"], "error", "check: {check}");
        let remedy = check["remedy"].as_str().unwrap_or_default();
        assert!(
            remedy.contains("codeint.jsonc"),
            "expected codeint.jsonc in remedy for {check_id}: {remedy:?}"
        );
        assert!(
            !remedy.replace("codeint.jsonc", "").contains("codeint.json"),
            "unexpected codeint.json in remedy for {check_id}: {remedy:?}"
        );
    }
}
