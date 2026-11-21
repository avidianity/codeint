#![cfg(unix)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use tempfile::TempDir;

struct ReleaseArtifacts {
    folder: TempDir,
    archive_name: String,
    archive_sha256: String,
}

impl ReleaseArtifacts {
    fn new() -> ReleaseArtifacts {
        let folder = tempfile::tempdir().expect("create release artifacts folder");
        let stage = tempfile::tempdir().expect("create staging folder");
        let staged_binary = stage.path().join("codeint");
        fs::copy(env!("CARGO_BIN_EXE_codeint"), &staged_binary).expect("stage codeint binary");

        let archive_name = format!("codeint-{}.tar.gz", host_target());
        let archive_path = folder.path().join(&archive_name);
        let tar_status = Command::new("tar")
            .args(["-C"])
            .arg(stage.path())
            .args(["-czf"])
            .arg(&archive_path)
            .arg("codeint")
            .status()
            .expect("create release tar.gz archive");
        assert!(tar_status.success(), "tar exited with {tar_status}");

        let archive_sha256 = sha256_of(&archive_path);
        let checksums = format!(
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa  codeint-other-target.tar.gz\n\
             {archive_sha256}  {archive_name}\n"
        );
        fs::write(folder.path().join("SHA256SUMS"), checksums).expect("write SHA256SUMS");

        ReleaseArtifacts {
            folder,
            archive_name,
            archive_sha256,
        }
    }

    fn path(&self) -> &Path {
        self.folder.path()
    }

    fn file_url(&self) -> String {
        format!("file://{}", self.folder.path().display())
    }

    fn archive_path(&self) -> PathBuf {
        self.folder.path().join(&self.archive_name)
    }
}

fn install_script() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../install.sh")
}

fn host_target() -> &'static str {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("macos", "aarch64") => "aarch64-apple-darwin",
        ("macos", "x86_64") => "x86_64-apple-darwin",
        ("linux", "x86_64") => "x86_64-unknown-linux-musl",
        ("linux", "aarch64") => "aarch64-unknown-linux-musl",
        (os, arch) => panic!("unsupported test host {os}/{arch}"),
    }
}

fn sha256_of(path: &Path) -> String {
    let output = Command::new("sh")
        .args([
            "-c",
            r#"if command -v sha256sum >/dev/null 2>&1; then sha256sum "$1"; else shasum -a 256 "$1"; fi | awk '{ print tolower($1) }'"#,
            "_",
        ])
        .arg(path)
        .output()
        .expect("compute sha256");
    assert!(
        output.status.success(),
        "sha256 command failed: {}",
        text(&output.stderr)
    );
    text(&output.stdout).trim().to_owned()
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

fn run_install(base_url: &str, install_dir: &Path) -> Output {
    Command::new("sh")
        .arg(install_script())
        .env("CODEINT_RELEASE_BASE_URL", base_url)
        .env("CODEINT_INSTALL_DIR", install_dir)
        .output()
        .expect("run install.sh")
}

fn run_with_fake_uname(
    os_name: &str,
    arch_name: &str,
    base_url: &str,
    install_dir: &Path,
) -> Output {
    let fake_bin = tempfile::tempdir().expect("create fake bin folder");
    let fake_uname = fake_bin.path().join("uname");
    let script = format!(
        "#!/bin/sh\nif [ \"${{1:-}}\" = \"-s\" ]; then\n  printf '%s\\n' '{os_name}'\nelse\n  printf '%s\\n' '{arch_name}'\nfi\n"
    );
    fs::write(&fake_uname, script).expect("write fake uname");
    fs::set_permissions(&fake_uname, fs::Permissions::from_mode(0o755))
        .expect("make fake uname executable");

    Command::new("sh")
        .args(["-c", r#"PATH="$1:$PATH" sh "$2""#, "_"])
        .arg(fake_bin.path())
        .arg(install_script())
        .env("CODEINT_RELEASE_BASE_URL", base_url)
        .env("CODEINT_INSTALL_DIR", install_dir)
        .output()
        .expect("run install.sh with fake uname")
}

#[test]
fn install_from_local_release_artifacts_verifies_checksum_and_installs_codeint() {
    let artifacts = ReleaseArtifacts::new();
    let target_root = tempfile::tempdir().expect("create target root");
    let install_dir = target_root.path().join("bin");

    let output = run_install(&artifacts.file_url(), &install_dir);

    assert!(
        output.status.success(),
        "install.sh failed\nstdout: {}\nstderr: {}",
        text(&output.stdout),
        text(&output.stderr)
    );

    let installed = install_dir.join("codeint");
    assert!(
        installed.is_file(),
        "expected installed binary at {}",
        installed.display()
    );
    assert!(
        text(&output.stdout).contains(&installed.display().to_string()),
        "stdout did not report install path: {}",
        text(&output.stdout)
    );

    let version_output = Command::new(&installed)
        .arg("--version")
        .output()
        .expect("run installed codeint --version");
    assert!(
        version_output.status.success(),
        "installed codeint --version failed: {}",
        text(&version_output.stderr)
    );
    assert!(
        text(&version_output.stdout).contains(env!("CARGO_PKG_VERSION")),
        "unexpected --version output: {}",
        text(&version_output.stdout)
    );
}

#[test]
fn install_defaults_to_home_local_bin_when_codeint_install_dir_is_unset() {
    let artifacts = ReleaseArtifacts::new();
    let home = tempfile::tempdir().expect("create temporary HOME");

    let output = Command::new("sh")
        .arg(install_script())
        .env("HOME", home.path())
        .env_remove("CODEINT_INSTALL_DIR")
        .env("CODEINT_RELEASE_BASE_URL", artifacts.path())
        .output()
        .expect("run install.sh");

    assert!(
        output.status.success(),
        "install.sh failed\nstdout: {}\nstderr: {}",
        text(&output.stdout),
        text(&output.stderr)
    );

    let installed = home.path().join(".local/bin/codeint");
    assert!(
        installed.is_file(),
        "expected binary at {}",
        installed.display()
    );

    let version_output = Command::new(&installed)
        .arg("--version")
        .output()
        .expect("run installed codeint --version");
    assert!(version_output.status.success());
}

#[test]
fn install_stops_on_checksum_mismatch_and_leaves_no_binary_installed() {
    let artifacts = ReleaseArtifacts::new();
    let mut tampered = fs::read(artifacts.archive_path()).expect("read release archive");
    tampered.push(0xFF);
    fs::write(artifacts.archive_path(), tampered).expect("write tampered archive");
    let tampered_sha256 = sha256_of(&artifacts.archive_path());

    let target_root = tempfile::tempdir().expect("create target root");
    let install_dir = target_root.path().join("bin");

    let output = run_install(&artifacts.file_url(), &install_dir);

    assert!(
        !output.status.success(),
        "expected non-zero exit on checksum mismatch; stdout: {}",
        text(&output.stdout)
    );
    let stderr = text(&output.stderr);
    assert!(
        stderr.contains("SHA-256 checksum mismatch"),
        "expected checksum mismatch message in stderr: {stderr}"
    );
    assert!(
        stderr.contains(&artifacts.archive_sha256),
        "expected expected-hash in stderr: {stderr}"
    );
    assert!(
        stderr.contains(&tampered_sha256),
        "expected actual-hash in stderr: {stderr}"
    );
    assert!(
        !install_dir.join("codeint").exists(),
        "checksum mismatch must not install binary"
    );
    assert!(
        !install_dir.exists(),
        "checksum mismatch must not create install directory"
    );
}

#[test]
fn install_rejects_unsupported_os_and_architecture() {
    let artifacts = ReleaseArtifacts::new();
    let target_root = tempfile::tempdir().expect("create target root");
    let install_dir = target_root.path().join("bin");

    let os_output = run_with_fake_uname("FreeBSD", "x86_64", &artifacts.file_url(), &install_dir);
    assert!(!os_output.status.success());
    assert!(
        text(&os_output.stderr).contains("unsupported operating system: FreeBSD"),
        "stderr: {}",
        text(&os_output.stderr)
    );

    let arch_output = run_with_fake_uname("Linux", "riscv64", &artifacts.file_url(), &install_dir);
    assert!(!arch_output.status.success());
    assert!(
        text(&arch_output.stderr).contains("unsupported architecture: riscv64"),
        "stderr: {}",
        text(&arch_output.stderr)
    );
    assert!(!install_dir.exists());
}
