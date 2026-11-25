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

fn build_sandbox_probe_wasm(package: &str, file_name: &str) -> PathBuf {
    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let manifest_path =
        workspace_root.join("crates/plugin-host/tests/fixtures/sandbox-probes/Cargo.toml");
    let target_dir = workspace_root.join("target/wasm-fixtures");
    let status = Command::new("cargo")
        .args([
            "build",
            "-p",
            package,
            "--locked",
            "--offline",
            "--target",
            "wasm32-wasip2",
        ])
        .arg("--manifest-path")
        .arg(manifest_path)
        .arg("--target-dir")
        .arg(&target_dir)
        .status()
        .expect("build sandbox probe wasm");
    assert!(status.success(), "{package} build exited with {status}");
    target_dir.join("wasm32-wasip2/debug").join(file_name)
}

pub(crate) fn folder_import_plugin_wasm() -> &'static Path {
    FOLDER_IMPORT_WASM.get_or_init(|| {
        build_sandbox_probe_wasm(
            "folder-import-language-plugin",
            "folder_import_language_plugin.wasm",
        )
    })
}

pub(crate) fn no_imports_language_plugin_wasm() -> &'static Path {
    NO_IMPORTS_WASM.get_or_init(|| {
        build_sandbox_probe_wasm(
            "no-imports-language-plugin",
            "no_imports_language_plugin.wasm",
        )
    })
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

pub(crate) fn init_git_repo(files: &[(&str, &str)]) -> TempDir {
    let dir = TempDir::new().unwrap();
    let run_git = |args: &[&str]| {
        let status = Command::new("git")
            .args(args)
            .current_dir(dir.path())
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_AUTHOR_NAME", "Test")
            .env("GIT_AUTHOR_EMAIL", "test@example.com")
            .env("GIT_COMMITTER_NAME", "Test")
            .env("GIT_COMMITTER_EMAIL", "test@example.com")
            .status()
            .unwrap();
        assert!(status.success(), "git {args:?} failed");
    };
    run_git(&["init", "-q", "-b", "main"]);
    write_repo_files(dir.path(), files);
    run_git(&["add", "."]);
    run_git(&["commit", "-q", "-m", "initial"]);
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
    serde_json::from_str(response["result"]["content"][0]["text"].as_str().unwrap()).unwrap()
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
