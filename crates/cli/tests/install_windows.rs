#![cfg(windows)]

use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use tempfile::TempDir;

fn windows_target() -> &'static str {
    if cfg!(target_arch = "x86_64") {
        "x86_64-pc-windows-msvc"
    } else if cfg!(target_arch = "aarch64") {
        "aarch64-pc-windows-msvc"
    } else {
        panic!("unsupported Windows architecture for install.ps1 test")
    }
}

fn install_script_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crates folder")
        .parent()
        .expect("repository root")
        .join("install.ps1")
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

fn package_release_archive(artifacts_dir: &Path) -> (String, String) {
    let archive_name = format!("codeint-{}.zip", windows_target());
    let archive_path = artifacts_dir.join(&archive_name);
    let output = Command::new("pwsh")
        .args([
            "-NoProfile",
            "-Command",
            "$ErrorActionPreference = 'Stop'; Compress-Archive -LiteralPath $env:SOURCE_BIN -DestinationPath $env:ARCHIVE_PATH -Force; (Get-FileHash -LiteralPath $env:ARCHIVE_PATH -Algorithm SHA256).Hash.ToLowerInvariant()",
        ])
        .env("SOURCE_BIN", env!("CARGO_BIN_EXE_codeint"))
        .env("ARCHIVE_PATH", &archive_path)
        .output()
        .expect("package release archive with pwsh");
    assert!(
        output.status.success(),
        "packaging failed:\nstdout: {}\nstderr: {}",
        text(&output.stdout),
        text(&output.stderr)
    );
    let hash = text(&output.stdout).trim().to_string();
    assert_eq!(
        hash.len(),
        64,
        "expected 64-character SHA-256 hex digest, got: {hash}"
    );
    (archive_name, hash)
}

fn run_installer<V: AsRef<OsStr>>(env_vars: &[(&str, V)]) -> Output {
    let mut command = Command::new("pwsh");
    command
        .args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-File"])
        .arg(install_script_path())
        .env_remove("CODEINT_ARTIFACTS_DIR")
        .env_remove("CODEINT_RELEASE_BASE_URL")
        .env_remove("CODEINT_VERSION")
        .env_remove("CODEINT_INSTALL_DIR");
    for (key, value) in env_vars {
        command.env(key, value);
    }
    command.output().expect("run install.ps1 with pwsh")
}

fn file_uri(path: &Path) -> String {
    let normalized = path.to_string_lossy().replace('\\', "/");
    format!("file:///{normalized}")
}

#[test]
fn installs_from_local_artifacts_and_runs_codeint_version() {
    let workspace = TempDir::new().expect("create temp workspace");
    let artifacts_dir = workspace.path().join("artifacts");
    let install_dir = workspace.path().join("install").join("bin");
    fs::create_dir_all(&artifacts_dir).expect("create artifacts dir");

    let (archive_name, hash) = package_release_archive(&artifacts_dir);
    fs::write(
        artifacts_dir.join("SHA256SUMS"),
        format!("{hash}  {archive_name}\n"),
    )
    .expect("write SHA256SUMS");

    let output = run_installer(&[
        ("CODEINT_ARTIFACTS_DIR", artifacts_dir.as_os_str()),
        ("CODEINT_INSTALL_DIR", install_dir.as_os_str()),
    ]);
    assert!(
        output.status.success(),
        "install.ps1 failed:\nstdout: {}\nstderr: {}",
        text(&output.stdout),
        text(&output.stderr)
    );

    let installed = install_dir.join("codeint.exe");
    assert!(
        installed.is_file(),
        "expected installed binary at {}",
        installed.display()
    );

    let version_output = Command::new(&installed)
        .arg("--version")
        .output()
        .expect("run installed codeint.exe --version");
    assert!(
        version_output.status.success(),
        "codeint.exe --version failed:\nstdout: {}\nstderr: {}",
        text(&version_output.stdout),
        text(&version_output.stderr)
    );
    let version_stdout = text(&version_output.stdout);
    let expected_version = format!("codeint {}", env!("CARGO_PKG_VERSION"));
    assert!(
        version_stdout.contains(&expected_version),
        "expected stdout to contain {expected_version:?}, got {version_stdout:?}"
    );
}

#[test]
fn installs_from_file_uri_release_base_url() {
    let workspace = TempDir::new().expect("create temp workspace");
    let artifacts_dir = workspace.path().join("artifacts");
    let install_dir = workspace.path().join("install");
    fs::create_dir_all(&artifacts_dir).expect("create artifacts dir");

    let (archive_name, hash) = package_release_archive(&artifacts_dir);
    fs::write(
        artifacts_dir.join("SHA256SUMS"),
        format!("{hash}  {archive_name}\n"),
    )
    .expect("write SHA256SUMS");

    let base_url = file_uri(&artifacts_dir);
    let install_dir_str = install_dir.to_string_lossy().into_owned();
    let output = run_installer(&[
        ("CODEINT_RELEASE_BASE_URL", base_url.as_str()),
        ("CODEINT_INSTALL_DIR", install_dir_str.as_str()),
    ]);
    assert!(
        output.status.success(),
        "install.ps1 via file:// failed:\nstdout: {}\nstderr: {}",
        text(&output.stdout),
        text(&output.stderr)
    );
    assert!(install_dir.join("codeint.exe").is_file());
}

#[test]
fn checksum_mismatch_stops_install_and_leaves_no_binary() {
    let workspace = TempDir::new().expect("create temp workspace");
    let artifacts_dir = workspace.path().join("artifacts");
    let install_dir = workspace.path().join("install");
    fs::create_dir_all(&artifacts_dir).expect("create artifacts dir");

    let (archive_name, _actual_hash) = package_release_archive(&artifacts_dir);
    let wrong_hash = "0".repeat(64);
    fs::write(
        artifacts_dir.join("SHA256SUMS"),
        format!("{wrong_hash}  {archive_name}\n"),
    )
    .expect("write mismatched SHA256SUMS");

    let output = run_installer(&[
        ("CODEINT_ARTIFACTS_DIR", artifacts_dir.as_os_str()),
        ("CODEINT_INSTALL_DIR", install_dir.as_os_str()),
    ]);
    assert!(
        !output.status.success(),
        "expected install.ps1 to fail on checksum mismatch, stdout: {}",
        text(&output.stdout)
    );
    let stderr = text(&output.stderr).to_lowercase();
    assert!(
        stderr.contains("checksum mismatch"),
        "expected checksum mismatch error in stderr, got: {stderr}"
    );
    assert!(
        !install_dir.join("codeint.exe").exists(),
        "checksum mismatch must leave no binary installed"
    );
}

#[test]
fn missing_checksum_entry_stops_install_and_leaves_no_binary() {
    let workspace = TempDir::new().expect("create temp workspace");
    let artifacts_dir = workspace.path().join("artifacts");
    let install_dir = workspace.path().join("install");
    fs::create_dir_all(&artifacts_dir).expect("create artifacts dir");

    let (_archive_name, hash) = package_release_archive(&artifacts_dir);
    fs::write(
        artifacts_dir.join("SHA256SUMS"),
        format!("{hash}  codeint-other-target.zip\n"),
    )
    .expect("write SHA256SUMS without matching entry");

    let output = run_installer(&[
        ("CODEINT_ARTIFACTS_DIR", artifacts_dir.as_os_str()),
        ("CODEINT_INSTALL_DIR", install_dir.as_os_str()),
    ]);
    assert!(
        !output.status.success(),
        "expected install.ps1 to fail on missing SHA256SUMS entry, stdout: {}",
        text(&output.stdout)
    );
    let stderr = text(&output.stderr);
    assert!(
        stderr.contains("SHA256SUMS"),
        "expected SHA256SUMS error in stderr, got: {stderr}"
    );
    assert!(
        !install_dir.join("codeint.exe").exists(),
        "missing checksum entry must leave no binary installed"
    );
}
