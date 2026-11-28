use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::OnceLock;

use serde_json::{Value, json};
use tempfile::TempDir;

static FOLDER_IMPORT_WASM: OnceLock<PathBuf> = OnceLock::new();
static NO_IMPORTS_WASM: OnceLock<PathBuf> = OnceLock::new();

struct SharedCodeintBinary {
    _folder: TempDir,
    path: PathBuf,
}

static SHARED_CODEINT_BINARY: OnceLock<Option<SharedCodeintBinary>> = OnceLock::new();

fn shared_temp_codeint_binary() -> Option<&'static Path> {
    SHARED_CODEINT_BINARY
        .get_or_init(|| {
            let folder = TempDir::new().ok()?;
            let file_name = Path::new(env!("CARGO_BIN_EXE_codeint")).file_name()?;
            let path = folder.path().join(file_name);
            std::fs::copy(env!("CARGO_BIN_EXE_codeint"), &path).ok()?;
            Some(SharedCodeintBinary {
                _folder: folder,
                path,
            })
        })
        .as_ref()
        .map(|shared| shared.path.as_path())
}

pub(crate) fn try_hard_link_codeint(destination: &Path) -> bool {
    let _ = std::fs::remove_file(destination);
    if std::fs::hard_link(env!("CARGO_BIN_EXE_codeint"), destination).is_ok() {
        return true;
    }
    shared_temp_codeint_binary()
        .is_some_and(|shared| std::fs::hard_link(shared, destination).is_ok())
}

#[cfg(unix)]
fn write_codeint_launcher(destination: &Path) {
    use std::os::unix::fs::PermissionsExt;

    let quoted = env!("CARGO_BIN_EXE_codeint").replace('\'', "'\\''");
    let script = format!("#!/bin/sh\nexec '{quoted}' \"$@\"\n");
    std::fs::write(destination, script).expect("write codeint launcher");
    std::fs::set_permissions(destination, std::fs::Permissions::from_mode(0o755))
        .expect("make codeint launcher executable");
}

pub(crate) fn stage_codeint_for_packaging(destination: &Path) {
    #[cfg(unix)]
    {
        write_codeint_launcher(destination);
    }
    #[cfg(not(unix))]
    {
        if !try_hard_link_codeint(destination) {
            std::fs::copy(env!("CARGO_BIN_EXE_codeint"), destination)
                .expect("stage codeint binary");
        }
    }
}

pub(crate) fn folder_import_plugin_wasm() -> &'static Path {
    FOLDER_IMPORT_WASM.get_or_init(|| {
        crate::wasm_fixtures::wasm_fixture_path("folder_import_language_plugin.wasm")
    })
}

pub(crate) fn no_imports_language_plugin_wasm() -> &'static Path {
    NO_IMPORTS_WASM
        .get_or_init(|| crate::wasm_fixtures::wasm_fixture_path("no_imports_language_plugin.wasm"))
}

pub(crate) fn eroded_ts(name: &str, branches: usize) -> String {
    let arms: String = (1..=branches)
        .map(|i| format!("  if (x === {i}) return {i};\n"))
        .collect();
    format!("export function {name}(x: number): number {{\n{arms}  return 0;\n}}\n")
}

pub(crate) fn write_repo_files(root: &Path, files: &[(&str, &str)]) {
    for (relative, content) in files {
        let full = root.join(relative);
        if let Some(parent) = full.parent() {
            std::fs::create_dir_all(parent).expect("create parent folder");
        }
        std::fs::write(full, content).expect("write file");
    }
}

pub(crate) fn run_git(dir: &Path, args: &[&str]) {
    let status = Command::new("git")
        .args(args)
        .current_dir(dir)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_AUTHOR_NAME", "Test")
        .env("GIT_AUTHOR_EMAIL", "test@example.com")
        .env("GIT_COMMITTER_NAME", "Test")
        .env("GIT_COMMITTER_EMAIL", "test@example.com")
        .status()
        .unwrap();
    assert!(status.success(), "git {args:?} failed");
}

pub(crate) fn init_git_repo(files: &[(&str, &str)]) -> TempDir {
    let dir = TempDir::new().unwrap();
    run_git(dir.path(), &["init", "-q", "-b", "main"]);
    write_repo_files(dir.path(), files);
    run_git(dir.path(), &["add", "."]);
    run_git(dir.path(), &["commit", "-q", "-m", "initial"]);
    dir
}

pub(crate) fn send_mcp_requests(dir: &Path, requests: &[Value]) -> Vec<Value> {
    let mut child = Command::new(env!("CARGO_BIN_EXE_codeint"))
        .arg("mcp")
        .current_dir(dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    {
        let mut stdin = child.stdin.take().unwrap();
        for req in requests {
            writeln!(stdin, "{req}").unwrap();
        }
    }
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "codeint mcp failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}

pub(crate) fn mcp_payload(response: &Value) -> Value {
    assert_eq!(response["result"]["isError"], false);
    let text = response["result"]["content"][0]["text"].as_str().unwrap();
    toon_format::decode_default(text)
        .unwrap_or_else(|err| panic!("expected valid TOON MCP payload: {err}\n{text}"))
}

pub(crate) fn mcp_tool_call(id: u64, name: &str, arguments: Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "tools/call",
        "params": { "name": name, "arguments": arguments }
    })
}

pub(crate) fn run_codeint_output(dir: &Path, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_codeint"))
        .args(args)
        .current_dir(dir)
        .output()
        .expect("run codeint")
}

pub(crate) fn run_codeint_text(dir: &Path, args: &[&str], expected_code: i32) -> (String, String) {
    let output = run_codeint_output(dir, args);
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

pub(crate) fn run_check_json(dir: &Path) -> Value {
    let (stdout, _) = run_codeint_text(dir, &["check", "--format", "json"], 0);
    serde_json::from_str(&stdout).expect("valid json")
}

pub(crate) fn run_codeint_stdout(dir: &Path, args: &[&str], expected_code: i32) -> String {
    run_codeint_text(dir, args, expected_code).0
}

pub(crate) fn run_codeint_json(dir: &Path, args: &[&str], expected_code: i32) -> Value {
    let mut full = args.to_vec();
    full.extend(["--format", "json"]);
    serde_json::from_str(&run_codeint_stdout(dir, &full, expected_code)).expect("valid JSON output")
}

pub(crate) fn init_git_repo_with(base: &[(&str, &str)], extra: &[(&str, &str)]) -> TempDir {
    let mut files = base.to_vec();
    files.extend_from_slice(extra);
    init_git_repo(&files)
}

pub(crate) fn check_json_with_config(dir: &Path, config_json: &str) -> Value {
    std::fs::write(dir.join("codeint.json"), config_json).expect("write codeint.json");
    run_check_json(dir)
}

pub(crate) fn assert_import_metrics_absent(report: &Value) {
    assert!(
        report["import_metrics_note"]
            .as_str()
            .unwrap_or_default()
            .contains("no imports"),
        "report: {report}"
    );
    assert!(report.get("dependencies").is_none(), "report: {report}");
    let production = &report["source_sets"]["production"];
    for absent in ["import_cycles", "orphan_files", "boundaries"] {
        assert!(
            production.get(absent).is_none(),
            "{absent} present: {report}"
        );
    }
}

pub(crate) fn finding_names(findings: &Value) -> Vec<&str> {
    findings
        .as_array()
        .expect("findings array")
        .iter()
        .filter_map(|finding| finding["name"].as_str())
        .collect()
}

pub(crate) fn finding_lines(findings: &Value) -> Vec<u64> {
    findings
        .as_array()
        .expect("findings array")
        .iter()
        .map(|finding| finding["line"].as_u64().expect("finding line"))
        .collect()
}

pub(crate) fn assert_doctor_counts_language_files(dir: &Path, needle: &str) {
    let summary = doctor_languages_summary(dir);
    assert!(summary.contains(needle), "doctor languages: {summary}");
}

fn doctor_languages_summary(dir: &Path) -> String {
    let doctor: Value =
        serde_json::from_str(&run_codeint_stdout(dir, &["doctor", "--format", "json"], 0))
            .expect("valid doctor json");
    assert_eq!(doctor["summary"]["errors"], 0, "doctor: {doctor}");
    doctor["checks"]
        .as_array()
        .expect("doctor checks")
        .iter()
        .find(|check| check["id"] == "languages")
        .expect("languages check")["summary"]
        .as_str()
        .unwrap_or_default()
        .to_owned()
}
