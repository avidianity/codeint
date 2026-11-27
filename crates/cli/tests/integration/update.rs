use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::thread;
use std::time::Duration;

use tempfile::TempDir;

fn run_exe(exe: &Path, args: &[&str], release_base: Option<&Path>) -> Output {
    run_exe_in_dir(exe, exe.parent(), args, release_base)
}

fn run_exe_in_dir(
    exe: &Path,
    working_dir: Option<&Path>,
    args: &[&str],
    release_base: Option<&Path>,
) -> Output {
    for attempt in 0..20 {
        let mut command = Command::new(exe);
        command.args(args);
        if let Some(dir) = working_dir {
            command.current_dir(dir);
        }
        if let Some(base) = release_base {
            command.env("CODEINT_RELEASE_BASE_URL", base);
        }
        match command.output() {
            Ok(output) => return output,
            Err(error) if error.kind() == ErrorKind::ExecutableFileBusy && attempt < 19 => {
                thread::sleep(Duration::from_millis(10));
            }
            Err(error) => panic!("run: {error:?}"),
        }
    }
    unreachable!()
}

fn codeint_binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_codeint"))
}

fn host_archive_name() -> &'static str {
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    return "codeint-aarch64-apple-darwin.tar.gz";
    #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
    return "codeint-x86_64-apple-darwin.tar.gz";
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    return "codeint-x86_64-unknown-linux-musl.tar.gz";
    #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
    return "codeint-aarch64-unknown-linux-musl.tar.gz";
    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    return "codeint-x86_64-pc-windows-msvc.zip";
    #[cfg(all(target_os = "windows", target_arch = "aarch64"))]
    return "codeint-aarch64-pc-windows-msvc.zip";
}

fn stage_release_archive(release_dir: &Path, payload: &[u8]) -> String {
    let bin_name = if cfg!(windows) {
        "codeint.exe"
    } else {
        "codeint"
    };
    let payload_dir = release_dir.join("payload");
    fs::create_dir_all(&payload_dir).expect("payload dir");
    let bin_path = payload_dir.join(bin_name);
    fs::write(&bin_path, payload).expect("write payload");
    let archive_name = host_archive_name();
    let archive_path = release_dir.join(archive_name);

    #[cfg(windows)]
    {
        let status = Command::new("powershell")
            .args([
                "-NoProfile",
                "-Command",
                "Compress-Archive -Path $args[0] -DestinationPath $args[1] -Force",
            ])
            .arg(&bin_path)
            .arg(&archive_path)
            .status()
            .expect("compress archive");
        assert!(status.success());
        let output = Command::new("powershell")
            .args([
                "-NoProfile",
                "-Command",
                "(Get-FileHash -Algorithm SHA256 -LiteralPath $args[0]).Hash.ToLowerInvariant()",
            ])
            .arg(&archive_path)
            .output()
            .expect("hash");
        String::from_utf8_lossy(&output.stdout).trim().to_owned()
    }
    #[cfg(not(windows))]
    {
        let status = Command::new("tar")
            .arg("-C")
            .arg(&payload_dir)
            .arg("-czf")
            .arg(&archive_path)
            .arg(bin_name)
            .status()
            .expect("tar");
        assert!(status.success());
        let output = Command::new("sha256sum")
            .arg(&archive_path)
            .output()
            .or_else(|_| {
                Command::new("shasum")
                    .args(["-a", "256"])
                    .arg(&archive_path)
                    .output()
            })
            .expect("sha256");
        String::from_utf8_lossy(&output.stdout)
            .split_whitespace()
            .next()
            .expect("digest")
            .to_ascii_lowercase()
    }
}

fn copy_codeint_to(dir: &Path) -> PathBuf {
    let bin_name = if cfg!(windows) {
        "codeint.exe"
    } else {
        "codeint"
    };
    let dest = dir.join(bin_name);
    if crate::common::try_hard_link_codeint(&dest) {
        return dest;
    }
    fs::copy(codeint_binary(), &dest).expect("copy codeint");
    dest
}

#[test]
fn update_check_reports_up_to_date_and_available_update_without_modifying_binary() {
    let temp = TempDir::new().expect("temp");
    let install_dir = temp.path().join("bin");
    let release_dir = temp.path().join("release");
    fs::create_dir_all(&install_dir).expect("install dir");
    fs::create_dir_all(&release_dir).expect("release dir");
    let exe = copy_codeint_to(&install_dir);
    let before_len = fs::metadata(&exe).expect("meta").len();

    fs::write(
        release_dir.join("LATEST"),
        format!("v{}\n", env!("CARGO_PKG_VERSION")),
    )
    .expect("latest");
    let up_to_date = run_exe(&exe, &["update", "--check"], Some(&release_dir));
    assert!(up_to_date.status.success(), "{up_to_date:?}");
    assert!(
        String::from_utf8_lossy(&up_to_date.stdout).contains("already up to date"),
        "{}",
        String::from_utf8_lossy(&up_to_date.stdout)
    );

    fs::write(release_dir.join("LATEST"), "v9.9.9\n").expect("latest");
    let available = run_exe(&exe, &["update", "--check"], Some(&release_dir));
    assert!(available.status.success(), "{available:?}");
    assert!(
        String::from_utf8_lossy(&available.stdout).contains("Update available"),
        "{}",
        String::from_utf8_lossy(&available.stdout)
    );
    assert_eq!(fs::metadata(&exe).expect("meta").len(), before_len);
}

#[test]
fn update_verifies_checksum_and_replaces_standalone_binary_in_place() {
    let temp = TempDir::new().expect("temp");
    let install_dir = temp.path().join("bin");
    let release_dir = temp.path().join("release");
    fs::create_dir_all(&install_dir).expect("install dir");
    fs::create_dir_all(&release_dir).expect("release dir");
    let exe = copy_codeint_to(&install_dir);
    let new_payload = b"#!/bin/sh\necho updated-codeint\n";
    let digest = stage_release_archive(&release_dir, new_payload);
    fs::write(release_dir.join("LATEST"), "v9.9.9\n").expect("latest");
    fs::write(
        release_dir.join("SHA256SUMS"),
        format!("{digest}  {}\n", host_archive_name()),
    )
    .expect("sums");

    let output = run_exe(&exe, &["update"], Some(&release_dir));
    assert!(output.status.success(), "{output:?}");
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("Updated codeint to v9.9.9"),
        "{}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert_eq!(fs::read(&exe).expect("read replaced"), new_payload);
}

#[test]
fn update_refuses_checksum_mismatch_and_keeps_original_binary() {
    let temp = TempDir::new().expect("temp");
    let install_dir = temp.path().join("bin");
    let release_dir = temp.path().join("release");
    fs::create_dir_all(&install_dir).expect("install dir");
    fs::create_dir_all(&release_dir).expect("release dir");
    let exe = copy_codeint_to(&install_dir);
    let original_len = fs::metadata(&exe).expect("meta").len();
    let _ = stage_release_archive(&release_dir, b"tampered");
    fs::write(release_dir.join("LATEST"), "v9.9.9\n").expect("latest");
    fs::write(
        release_dir.join("SHA256SUMS"),
        format!(
            "0000000000000000000000000000000000000000000000000000000000000000  {}\n",
            host_archive_name()
        ),
    )
    .expect("sums");

    let output = run_exe(&exe, &["update"], Some(&release_dir));
    assert_eq!(output.status.code(), Some(2));
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("checksum verification failed"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(fs::metadata(&exe).expect("meta").len(), original_len);
}

#[test]
fn update_refuses_to_overwrite_node_modules_install() {
    let temp = TempDir::new().expect("temp");
    let nm_bin = temp
        .path()
        .join("node_modules/@avidian/codeint-darwin-arm64/bin");
    fs::create_dir_all(&nm_bin).expect("nm dir");
    let exe = copy_codeint_to(&nm_bin);

    let output = run_exe(&exe, &["update", "--check"], None);
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("managed by a package manager"), "{stderr}");
    assert!(stderr.contains("npm update -g codeint"), "{stderr}");
}

fn run_update_in_repo(temp: &TempDir, repo_dir: &Path, latest_tag: &str) -> Output {
    let install_dir = temp.path().join("bin");
    let release_dir = temp.path().join("release");
    if release_dir.join("SHA256SUMS").is_file()
        && fs::read_to_string(release_dir.join("LATEST"))
            .is_ok_and(|content| content.trim() == latest_tag)
    {
        let exe = copy_codeint_to(&install_dir);
        return run_exe_in_dir(&exe, Some(repo_dir), &["update"], Some(&release_dir));
    }
    fs::create_dir_all(&install_dir).expect("install dir");
    fs::create_dir_all(&release_dir).expect("release dir");
    let exe = copy_codeint_to(&install_dir);
    let digest = stage_release_archive(&release_dir, b"#!/bin/sh\necho ok\n");
    fs::write(release_dir.join("LATEST"), format!("{latest_tag}\n")).expect("latest");
    fs::write(
        release_dir.join("SHA256SUMS"),
        format!("{digest}  {}\n", host_archive_name()),
    )
    .expect("sums");
    run_exe_in_dir(&exe, Some(repo_dir), &["update"], Some(&release_dir))
}

#[test]
fn update_syncs_versioned_schema_in_codeint_json() {
    let temp = TempDir::new().expect("temp");
    let repo_dir = temp.path().join("repo");
    fs::create_dir_all(repo_dir.join(".git")).expect("git dir");
    let config_path = repo_dir.join("codeint.json");
    fs::write(
        &config_path,
        "{\n  \"$schema\": \"https://raw.githubusercontent.com/avidianity/codeint/v0.1.0/schema/codeint.schema.json\",\n  \"budgets\": {}\n}\n",
    )
    .expect("write codeint.json");

    let output = run_update_in_repo(&temp, &repo_dir, "v9.9.9");
    assert!(output.status.success(), "{output:?}");
    let updated_json = fs::read_to_string(&config_path).expect("read codeint.json");
    assert!(
        updated_json.contains(
            "https://raw.githubusercontent.com/avidianity/codeint/v9.9.9/schema/codeint.schema.json"
        ),
        "{updated_json}"
    );
    assert!(updated_json.contains("\"budgets\": {}"), "{updated_json}");
}

fn assert_update_rewrites(config_name: &str, cases: &[(&str, &str)]) {
    let temp = TempDir::new().expect("temp");
    let repo_dir = temp.path().join("repo");
    fs::create_dir_all(repo_dir.join(".git")).expect("git dir");
    let config_path = repo_dir.join(config_name);
    for (original, expected) in cases {
        fs::write(&config_path, original).expect("write config");
        let output = run_update_in_repo(&temp, &repo_dir, "v9.9.9");
        assert!(output.status.success(), "{output:?}");
        assert_eq!(
            fs::read_to_string(&config_path).expect("read config"),
            *expected
        );
    }
}

#[test]
fn update_syncs_only_active_schema_in_codeint_jsonc_and_preserves_comments() {
    let original = concat!(
        "{\n",
        "  // \"$schema\": \"https://raw.githubusercontent.com/avidianity/codeint/v0.0.1/schema/codeint.schema.json\",\n",
        "  /* \"$schema\": \"https://raw.githubusercontent.com/avidianity/codeint/v0.0.2/schema/codeint.schema.json\" */\n",
        "  \"$schema\": \"https://raw.githubusercontent.com/avidianity/codeint/v0.1.0/schema/codeint.schema.json\",\n",
        "  \"budgets\": {},\n",
        "}\n",
    );
    let expected = concat!(
        "{\n",
        "  // \"$schema\": \"https://raw.githubusercontent.com/avidianity/codeint/v0.0.1/schema/codeint.schema.json\",\n",
        "  /* \"$schema\": \"https://raw.githubusercontent.com/avidianity/codeint/v0.0.2/schema/codeint.schema.json\" */\n",
        "  \"$schema\": \"https://raw.githubusercontent.com/avidianity/codeint/v9.9.9/schema/codeint.schema.json\",\n",
        "  \"budgets\": {},\n",
        "}\n",
    );
    assert_update_rewrites("codeint.jsonc", &[(original, expected)]);
}

#[cfg(unix)]
#[test]
fn update_refuses_to_mutate_codeint_jsonc_when_broken_codeint_json_symlink_exists() {
    use std::os::unix::fs::symlink;

    let temp = TempDir::new().expect("temp");
    let repo_dir = temp.path().join("repo");
    fs::create_dir_all(repo_dir.join(".git")).expect("git dir");
    let config_path = repo_dir.join("codeint.jsonc");
    let original = "{\n  \"$schema\": \"https://raw.githubusercontent.com/avidianity/codeint/v0.1.0/schema/codeint.schema.json\",\n}\n";
    fs::write(&config_path, original).expect("write codeint.jsonc");
    symlink(repo_dir.join("missing.json"), repo_dir.join("codeint.json"))
        .expect("create broken codeint.json symlink");

    let output = run_update_in_repo(&temp, &repo_dir, "v9.9.9");
    assert!(output.status.success(), "{output:?}");
    assert_eq!(
        fs::read_to_string(&config_path).expect("read codeint.jsonc"),
        original
    );
}

#[test]
fn update_handles_duplicate_schema_keys_and_json_escapes() {
    let duplicated = concat!(
        "{\n",
        "  \"$schema\": \"https://raw.githubusercontent.com/avidianity/codeint/v0.1.0/schema/codeint.schema.json\",\n",
        "  \"$schema\": \"https://raw.githubusercontent.com/avidianity/codeint/v0.2.0/schema/codeint.schema.json\",\n",
        "}\n",
    );
    let duplicated_expected = concat!(
        "{\n",
        "  \"$schema\": \"https://raw.githubusercontent.com/avidianity/codeint/v0.1.0/schema/codeint.schema.json\",\n",
        "  \"$schema\": \"https://raw.githubusercontent.com/avidianity/codeint/v9.9.9/schema/codeint.schema.json\",\n",
        "}\n",
    );
    let custom_last = concat!(
        "{\n",
        "  \"$schema\": \"https://raw.githubusercontent.com/avidianity/codeint/v0.1.0/schema/codeint.schema.json\",\n",
        "  \"$schema\": \"https://example.invalid/custom.schema.json\",\n",
        "}\n",
    );
    let escaped_slash_in_ref = "{\n  \"$schema\": \"https://raw.githubusercontent.com/avidianity/codeint/v0.1.0\\u002fextra/schema/codeint.schema.json\",\n}\n";
    let escaped_valid_url = "{\n  \"$schema\": \"https:\\/\\/raw.githubusercontent.com\\/avidianity\\/codeint\\/v0.1.0\\/schema\\/codeint.schema.json\",\n}\n";
    let escaped_valid_expected = "{\n  \"$schema\": \"https://raw.githubusercontent.com/avidianity/codeint/v9.9.9/schema/codeint.schema.json\",\n}\n";
    assert_update_rewrites(
        "codeint.jsonc",
        &[
            (duplicated, duplicated_expected),
            (custom_last, custom_last),
            (escaped_slash_in_ref, escaped_slash_in_ref),
            (escaped_valid_url, escaped_valid_expected),
        ],
    );
}

#[cfg(unix)]
fn write_executable_script(path: &Path, body: &str) {
    use std::os::unix::fs::PermissionsExt;

    fs::write(path, body).expect("write script");
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).expect("chmod script");
}

#[cfg(unix)]
fn find_system_tar() -> PathBuf {
    ["/usr/bin/tar", "/bin/tar"]
        .into_iter()
        .map(PathBuf::from)
        .find(|candidate| candidate.is_file())
        .expect("could not find tar in /usr/bin or /bin")
}

#[cfg(unix)]
#[test]
fn update_passes_hardened_tls_timeout_and_terminator_flags_to_curl() {
    let temp = TempDir::new().expect("temp");
    let install_dir = temp.path().join("bin");
    let fake_bin = temp.path().join("fake-bin");
    let args_log = temp.path().join("curl-args.txt");
    fs::create_dir_all(&install_dir).expect("install dir");
    fs::create_dir_all(&fake_bin).expect("fake bin");
    let exe = copy_codeint_to(&install_dir);

    let script = format!(
        "#!/bin/sh\nprintf '%s\\n' \"$@\" > \"{}\"\nprintf '{{\"tag_name\":\"v9.9.9\"}}'\n",
        args_log.display()
    );
    write_executable_script(&fake_bin.join("curl"), &script);
    let joined_path = format!("{}:/usr/bin:/bin", fake_bin.display());

    let output = Command::new(&exe)
        .current_dir(temp.path())
        .args(["update", "--check"])
        .env("PATH", &joined_path)
        .env_remove("CODEINT_RELEASE_BASE_URL")
        .output()
        .expect("run update");
    assert!(output.status.success(), "{output:?}");

    let recorded = fs::read_to_string(&args_log).expect("read curl args");
    let args: Vec<&str> = recorded.lines().collect();
    let has_flag_pair = |flag: &str, value: &str| {
        args.windows(2)
            .any(|pair| pair[0] == flag && pair[1] == value)
    };
    assert!(
        has_flag_pair("--proto", "=https"),
        "missing --proto =https in curl args: {args:?}"
    );
    assert!(
        args.contains(&"--tlsv1.2"),
        "missing --tlsv1.2 in curl args: {args:?}"
    );
    assert!(
        has_flag_pair("--proto-redir", "=https"),
        "missing --proto-redir =https in curl args: {args:?}"
    );
    assert!(
        args.contains(&"--connect-timeout"),
        "missing --connect-timeout in curl args: {args:?}"
    );
    assert!(
        args.contains(&"--max-time"),
        "missing --max-time in curl args: {args:?}"
    );
    let url_index = args.len().checked_sub(1).expect("at least one arg");
    assert!(
        url_index >= 1 && args[url_index - 1] == "--",
        "missing -- terminator before URL in curl args: {args:?}"
    );
}

#[cfg(unix)]
#[test]
fn update_parses_github_release_json_and_rejects_malformed_payloads() {
    let temp = TempDir::new().expect("temp");
    let install_dir = temp.path().join("bin");
    let fake_bin = temp.path().join("fake-bin");
    let payload_file = temp.path().join("release.json");
    fs::create_dir_all(&install_dir).expect("install dir");
    fs::create_dir_all(&fake_bin).expect("fake bin");
    let exe = copy_codeint_to(&install_dir);

    let script = format!("#!/bin/sh\ncat \"{}\"\n", payload_file.display());
    write_executable_script(&fake_bin.join("curl"), &script);
    let joined_path = format!("{}:/usr/bin:/bin", fake_bin.display());

    fs::write(
        &payload_file,
        r#"{"body":"Mentioned \"tag_name\": \"v0.0.1\" in notes","tag_name":"v9.8.7"}"#,
    )
    .expect("write release json");
    let valid_output = Command::new(&exe)
        .current_dir(temp.path())
        .args(["update", "--check"])
        .env("PATH", &joined_path)
        .env_remove("CODEINT_RELEASE_BASE_URL")
        .output()
        .expect("run update");
    assert!(valid_output.status.success(), "{valid_output:?}");
    let stdout = String::from_utf8_lossy(&valid_output.stdout);
    assert!(
        stdout.contains("v9.8.7"),
        "expected parsed top-level tag v9.8.7, got: {stdout}"
    );

    fs::write(&payload_file, r#"{"tag_name":"v9.8.7""#).expect("write malformed json");
    let malformed_output = Command::new(&exe)
        .current_dir(temp.path())
        .args(["update", "--check"])
        .env("PATH", &joined_path)
        .env_remove("CODEINT_RELEASE_BASE_URL")
        .output()
        .expect("run update on malformed json");
    assert_eq!(
        malformed_output.status.code(),
        Some(2),
        "malformed JSON should fail with exit code 2, got: {malformed_output:?}"
    );
}

#[cfg(unix)]
#[test]
fn update_verifies_checksum_in_process_without_external_checksum_binaries() {
    use std::os::unix::fs::symlink;

    let temp = TempDir::new().expect("temp");
    let install_dir = temp.path().join("bin");
    let release_dir = temp.path().join("release");
    let minimal_bin = temp.path().join("minimal-bin");
    fs::create_dir_all(&install_dir).expect("install dir");
    fs::create_dir_all(&release_dir).expect("release dir");
    fs::create_dir_all(&minimal_bin).expect("minimal bin");

    let tar_path = find_system_tar();
    symlink(&tar_path, minimal_bin.join("tar")).expect("symlink tar");

    let exe = copy_codeint_to(&install_dir);
    let new_payload = b"#!/bin/sh\necho pure-rust-sha256\n";
    let digest = stage_release_archive(&release_dir, new_payload);
    fs::write(release_dir.join("LATEST"), "v9.9.9\n").expect("latest");
    fs::write(
        release_dir.join("SHA256SUMS"),
        format!("{digest}  {}\n", host_archive_name()),
    )
    .expect("sums");

    let output = Command::new(&exe)
        .current_dir(temp.path())
        .arg("update")
        .env("PATH", &minimal_bin)
        .env("CODEINT_RELEASE_BASE_URL", &release_dir)
        .output()
        .expect("run update");
    assert!(
        output.status.success(),
        "expected pure-Rust SHA-256 verification without sha256sum/shasum, got: {output:?}"
    );
    assert_eq!(fs::read(&exe).expect("read replaced"), new_payload);
}

#[test]
fn update_reports_format_specific_schema_directive_notice_for_json_toml_and_yaml() {
    let cases = [
        (
            "codeint.json",
            "{\n  \"$schema\": \"https://raw.githubusercontent.com/avidianity/codeint/v0.1.0/schema/codeint.schema.json\"\n}\n",
            "Updated $schema in",
        ),
        (
            "codeint.jsonc",
            "{\n  \"$schema\": \"https://raw.githubusercontent.com/avidianity/codeint/v0.1.0/schema/codeint.schema.json\",\n}\n",
            "Updated $schema in",
        ),
        (
            "codeint.toml",
            "#:schema https://raw.githubusercontent.com/avidianity/codeint/v0.1.0/schema/codeint.schema.json\n\nformat = \"text\"\n",
            "#:schema",
        ),
        (
            "codeint.yaml",
            "# yaml-language-server: $schema=https://raw.githubusercontent.com/avidianity/codeint/v0.1.0/schema/codeint.schema.json\nformat: text\n",
            "# yaml-language-server: $schema=",
        ),
    ];

    for (file_name, original, expected_directive) in cases {
        let temp = TempDir::new().expect("temp");
        let repo_dir = temp.path().join("repo");
        fs::create_dir_all(repo_dir.join(".git")).expect("git dir");
        fs::write(repo_dir.join(file_name), original).expect("write config");

        let output = run_update_in_repo(&temp, &repo_dir, "v9.9.9");
        assert!(output.status.success(), "{output:?}");
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            stdout.contains(expected_directive),
            "expected {file_name} notice to mention {expected_directive:?}, got: {stdout}"
        );
        assert!(
            stdout.contains("v9.9.9/schema/codeint.schema.json"),
            "expected {file_name} notice to include updated URL, got: {stdout}"
        );
    }
}
