use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use tempfile::TempDir;

use crate::common::{
    eroded_ts, host_target, init_git_repo, run_git, sha256_of, stage_codeint_for_packaging,
    write_repo_files,
};

fn script_path(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../scripts")
        .join(name)
}

fn action_script() -> PathBuf {
    script_path("action-diff.sh")
        .canonicalize()
        .expect("locate scripts/action-diff.sh")
}

fn install_action_script() -> PathBuf {
    script_path("action-install.sh")
}

fn enforce_exit_script() -> PathBuf {
    script_path("action-enforce-exit.sh")
}

fn write_executable(path: &Path, content: &str) {
    fs::write(path, content).expect("write executable script");
    fs::set_permissions(path, fs::Permissions::from_mode(0o755))
        .expect("set executable permissions");
}

fn build_stub_release_dir(dir: &Path, reported_version: &str) {
    fs::create_dir_all(dir).expect("create release dir");
    let stage = TempDir::new().expect("create staging dir");
    let stub_bin = stage.path().join("codeint");
    write_executable(
        &stub_bin,
        &format!(
            "#!/bin/sh\nif [ \"${{1:-}}\" = \"--version\" ]; then\n  printf 'codeint {reported_version}\\n'\n  exit 0\nfi\nexit 0\n"
        ),
    );

    let archive_name = format!("codeint-{}.tar.gz", host_target());
    let archive_path = dir.join(&archive_name);
    let tar_status = Command::new("sh")
        .args(["-c", r#"tar -C "$1" -cf - codeint | gzip -1 > "$2""#, "_"])
        .arg(stage.path())
        .arg(&archive_path)
        .status()
        .expect("create stub release archive");
    assert!(tar_status.success(), "tar | gzip exited with {tar_status}");

    let digest = sha256_of(&archive_path);
    fs::write(
        dir.join("SHA256SUMS"),
        format!("{digest}  {archive_name}\n"),
    )
    .expect("write SHA256SUMS");

    let repo_install = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../install.sh");
    fs::copy(&repo_install, dir.join("install.sh")).expect("copy install.sh into stub release");
}

fn run_action_install_with_stub_releases(
    version: &str,
    releases_root: &Path,
    inherited_codeint_version: Option<&str>,
) -> (Output, PathBuf, PathBuf, TempDir) {
    let sandbox = TempDir::new().expect("create install sandbox");
    let home = sandbox.path().join("home");
    fs::create_dir_all(&home).expect("create home");
    let fake_bin = sandbox.path().join("bin");
    fs::create_dir_all(&fake_bin).expect("create fake bin");
    let github_path = sandbox.path().join("github-path");
    fs::write(&github_path, "").expect("create GITHUB_PATH");
    let curl_log = sandbox.path().join("curl-urls.log");
    fs::write(&curl_log, "").expect("create curl log");

    let fake_curl = fake_bin.join("curl");
    write_executable(
        &fake_curl,
        &format!(
            "#!/usr/bin/env bash\nset -euo pipefail\nOUT=\"\"\nURL=\"\"\nwhile [ \"$#\" -gt 0 ]; do\n  case \"$1\" in\n    -o)\n      OUT=\"$2\"\n      shift 2\n      ;;\n    http://*|https://*)\n      URL=\"$1\"\n      shift\n      ;;\n    *)\n      shift\n      ;;\n  esac\ndone\nprintf '%s\\n' \"$URL\" >> '{log}'\nPREFIX='https://github.com/avidianity/codeint/releases/'\nREL=\"${{URL#$PREFIX}}\"\nSRC='{root}'\"/$REL\"\nif [ ! -f \"$SRC\" ]; then\n  printf 'stub curl: missing %s\\n' \"$SRC\" >&2\n  exit 22\nfi\nif [ -n \"$OUT\" ]; then\n  cp \"$SRC\" \"$OUT\"\nelse\n  cat \"$SRC\"\nfi\n",
            log = curl_log.display(),
            root = releases_root.display(),
        ),
    );

    let search_path = format!("{}:/usr/bin:/bin", fake_bin.display());
    let mut cmd = Command::new("bash");
    cmd.arg(install_action_script())
        .env("PATH", search_path)
        .env("HOME", &home)
        .env("INPUT_VERSION", version)
        .env("GITHUB_PATH", &github_path)
        .env_remove("CODEINT_INSTALL_DIR")
        .env_remove("CODEINT_RELEASE_BASE_URL");
    if let Some(inherited) = inherited_codeint_version {
        cmd.env("CODEINT_VERSION", inherited);
    } else {
        cmd.env_remove("CODEINT_VERSION");
    }

    let output = cmd.output().expect("run scripts/action-install.sh");
    (output, curl_log, github_path, sandbox)
}

fn run_enforce_exit(
    diff_exit_code: Option<&str>,
    fail_on_regression: &str,
    upload_sarif: bool,
    sarif_exit_code: Option<&str>,
) -> Output {
    let mut cmd = Command::new("bash");
    cmd.arg(enforce_exit_script())
        .env("FAIL_ON_REGRESSION", fail_on_regression)
        .env(
            "INPUT_UPLOAD_SARIF",
            if upload_sarif { "true" } else { "false" },
        );
    match diff_exit_code {
        Some(code) => {
            cmd.env("DIFF_EXIT_CODE", code);
        }
        None => {
            cmd.env_remove("DIFF_EXIT_CODE");
        }
    }
    match sarif_exit_code {
        Some(code) => {
            cmd.env("SARIF_EXIT_CODE", code);
        }
        None => {
            cmd.env_remove("SARIF_EXIT_CODE");
        }
    }
    cmd.output().expect("run scripts/action-enforce-exit.sh")
}

fn run_action_diff_with_stub_codeint(
    markdown_exit: i32,
    sarif_exit: i32,
    upload_sarif: bool,
) -> ActionRun {
    let workspace = init_git_repo(&[("pkg/src/tidy.ts", "export const tidy = 1;\n")]);
    let tools = TempDir::new().expect("tool folder");
    let stub = tools.path().join("codeint");
    write_executable(
        &stub,
        &format!(
            "#!/usr/bin/env bash\nset -euo pipefail\nFORMAT=\"\"\nOUTPUT=\"\"\nwhile [ \"$#\" -gt 0 ]; do\n  case \"$1\" in\n    --format) FORMAT=\"$2\"; shift 2 ;;\n    --output) OUTPUT=\"$2\"; shift 2 ;;\n    *) shift ;;\n  esac\ndone\nif [ \"$FORMAT\" = \"sarif\" ]; then\n  if [ {sarif_exit} -eq 0 ] || [ {sarif_exit} -eq 1 ]; then\n    printf '{{}}' > \"$OUTPUT\"\n  fi\n  exit {sarif_exit}\nfi\nif [ -n \"$OUTPUT\" ]; then\n  printf '# report\\n' > \"$OUTPUT\"\nfi\nexit {markdown_exit}\n"
        ),
    );
    let github_output = tools.path().join("github-output");
    fs::write(&github_output, "").expect("create GITHUB_OUTPUT");
    let search_path = format!("{}:/usr/bin:/bin", tools.path().display());
    let output = Command::new("bash")
        .arg(action_script())
        .current_dir(workspace.path())
        .env("PATH", search_path)
        .env("INPUT_BASE_REF", "main")
        .env("INPUT_PATH", ".")
        .env(
            "INPUT_UPLOAD_SARIF",
            if upload_sarif { "true" } else { "false" },
        )
        .env("GITHUB_BASE_REF", "")
        .env("GITHUB_OUTPUT", &github_output)
        .output()
        .expect("run scripts/action-diff.sh");
    let outputs = fs::read_to_string(&github_output).expect("read GITHUB_OUTPUT");
    ActionRun {
        output,
        outputs,
        workspace,
        _tools: tools,
    }
}

struct ActionRun {
    output: Output,
    outputs: String,
    workspace: TempDir,
    _tools: TempDir,
}

fn run_action_diff(path_input: &str, upload_sarif: bool, edit: impl FnOnce(&Path)) -> ActionRun {
    let workspace = init_git_repo(&[("pkg/src/tidy.ts", "export const tidy = 1;\n")]);
    run_git(workspace.path(), &["checkout", "-q", "-b", "feature"]);
    edit(workspace.path());
    run_git(workspace.path(), &["add", "."]);
    run_git(workspace.path(), &["commit", "-q", "-m", "change"]);

    let tools = TempDir::new().expect("tool folder");
    let launcher = tools.path().join(if cfg!(windows) {
        "codeint.exe"
    } else {
        "codeint"
    });
    stage_codeint_for_packaging(&launcher);
    let github_output = tools.path().join("github-output");
    fs::write(&github_output, "").expect("create GITHUB_OUTPUT");
    let search_path = format!("{}:/usr/bin:/bin", tools.path().display());
    let output = Command::new("bash")
        .arg(action_script())
        .current_dir(workspace.path())
        .env("PATH", search_path)
        .env("INPUT_BASE_REF", "main")
        .env("INPUT_PATH", path_input)
        .env(
            "INPUT_UPLOAD_SARIF",
            if upload_sarif { "true" } else { "false" },
        )
        .env("GITHUB_BASE_REF", "")
        .env("GITHUB_OUTPUT", &github_output)
        .output()
        .expect("run the action script");
    let outputs = fs::read_to_string(&github_output).expect("read GITHUB_OUTPUT");
    ActionRun {
        output,
        outputs,
        workspace,
        _tools: tools,
    }
}

#[test]
fn action_diff_runs_the_real_cli_from_the_path_input_folder_of_the_repository_and_reports_a_pass() {
    let run = run_action_diff("pkg", true, |root| {
        write_repo_files(root, &[("pkg/src/other.ts", "export const other = 2;\n")]);
    });

    let stderr = String::from_utf8_lossy(&run.output.stderr);
    assert!(run.output.status.success(), "script failed: {stderr}");
    assert!(
        !stderr.contains("unexpected argument"),
        "codeint rejected its arguments: {stderr}"
    );
    assert!(
        run.outputs.contains("exit-code=0\n"),
        "got: {}",
        run.outputs
    );
    assert!(
        run.outputs.contains("markdown-file=codeint-report.md\n")
            && run.outputs.contains("sarif-file=codeint.sarif\n"),
        "got: {}",
        run.outputs
    );
    let report = fs::read_to_string(run.workspace.path().join("codeint-report.md"))
        .expect("the Markdown report is written in the workspace root");
    assert!(!report.trim().is_empty());
    assert!(run.workspace.path().join("codeint.sarif").is_file());
}

#[test]
fn action_diff_reports_exit_code_one_for_a_regression() {
    let run = run_action_diff(".", false, |root| {
        write_repo_files(root, &[("pkg/src/worse.ts", &eroded_ts("worse", 16))]);
    });

    assert!(run.output.status.success());
    assert!(
        run.outputs.contains("exit-code=1\n"),
        "got: {}",
        run.outputs
    );
    assert!(!run.outputs.contains("sarif-file"), "got: {}", run.outputs);
}

#[test]
fn the_action_runs_its_diff_step_through_the_script_the_test_executes() {
    let action = fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../action.yml"))
        .expect("read action.yml");

    assert!(
        action.contains("bash \"$ACTION_PATH/scripts/action-install.sh\""),
        "action.yml does not run scripts/action-install.sh"
    );
    assert!(
        action.contains("bash \"$ACTION_PATH/scripts/action-diff.sh\""),
        "action.yml no longer runs scripts/action-diff.sh"
    );
    assert!(
        action.contains("bash \"$ACTION_PATH/scripts/action-enforce-exit.sh\""),
        "action.yml does not run scripts/action-enforce-exit.sh"
    );
}

#[test]
fn enforce_exit_accepts_zero_and_advisory_one_and_rejects_every_unexpected_status() {
    assert!(
        run_enforce_exit(Some("0"), "true", false, None)
            .status
            .success()
    );
    assert!(
        run_enforce_exit(Some("0"), "false", false, None)
            .status
            .success()
    );

    let fail_on_reg_true = run_enforce_exit(Some("1"), "true", false, None);
    assert_eq!(fail_on_reg_true.status.code(), Some(1));

    let fail_on_reg_false = run_enforce_exit(Some("1"), "false", false, None);
    assert!(fail_on_reg_false.status.success());

    for unexpected in ["2", "101", "127", "137", "139"] {
        let expected_code: i32 = unexpected.parse().expect("numeric code");
        for fail_on_regression in ["true", "false"] {
            let output = run_enforce_exit(Some(unexpected), fail_on_regression, false, None);
            assert_eq!(
                output.status.code(),
                Some(expected_code),
                "expected exit {expected_code} for DIFF_EXIT_CODE={unexpected} fail-on-regression={fail_on_regression}, got {:?}, stderr={}",
                output.status.code(),
                String::from_utf8_lossy(&output.stderr)
            );
        }
    }

    for invalid in [None, Some(""), Some("abc"), Some("-1"), Some("256")] {
        for fail_on_regression in ["true", "false"] {
            let output = run_enforce_exit(invalid, fail_on_regression, false, None);
            assert!(
                !output.status.success(),
                "expected failure for DIFF_EXIT_CODE={invalid:?} fail-on-regression={fail_on_regression}"
            );
        }
    }
}

#[test]
fn sarif_pass_unexpected_exit_code_fails_diff_wrapper_and_exit_enforcement() {
    for ok_sarif in [0, 1] {
        let run = run_action_diff_with_stub_codeint(0, ok_sarif, true);
        assert!(
            run.output.status.success(),
            "sarif exit {ok_sarif} should succeed: {}",
            String::from_utf8_lossy(&run.output.stderr)
        );
        assert!(
            run.outputs
                .contains(&format!("sarif-exit-code={ok_sarif}\n")),
            "missing sarif-exit-code output: {}",
            run.outputs
        );
        assert!(run.outputs.contains("sarif-file=codeint.sarif\n"));
        assert!(
            run_enforce_exit(Some("0"), "true", true, Some(&ok_sarif.to_string()))
                .status
                .success()
        );
    }

    for bad_sarif in [2, 101, 137] {
        let run = run_action_diff_with_stub_codeint(0, bad_sarif, true);
        assert_eq!(
            run.output.status.code(),
            Some(bad_sarif),
            "expected action-diff.sh to fail with {bad_sarif} on unexpected SARIF exit, got {:?}, stderr={}",
            run.output.status.code(),
            String::from_utf8_lossy(&run.output.stderr)
        );
        assert!(
            !run.outputs.contains("sarif-file="),
            "broken SARIF pass must not emit sarif-file output: {}",
            run.outputs
        );

        let enforced = run_enforce_exit(Some("0"), "false", true, Some(&bad_sarif.to_string()));
        assert_eq!(enforced.status.code(), Some(bad_sarif));
    }

    assert!(
        !run_enforce_exit(Some("0"), "false", true, None)
            .status
            .success()
    );
    assert!(
        !run_enforce_exit(Some("0"), "false", true, Some("not-a-number"))
            .status
            .success()
    );
}

#[test]
fn action_install_pins_version_tag_through_installer_and_verifies_binary_version() {
    let releases = TempDir::new().expect("create stub releases");
    build_stub_release_dir(&releases.path().join("download/v0.8.0"), "0.8.0");
    build_stub_release_dir(&releases.path().join("latest/download"), "0.9.0");

    let (output, curl_log, github_path, sandbox) =
        run_action_install_with_stub_releases("v0.8.0", releases.path(), None);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "pinned install failed: stdout={}, stderr={stderr}",
        String::from_utf8_lossy(&output.stdout)
    );

    let urls = fs::read_to_string(&curl_log).expect("read curl log");
    let archive_name = format!("codeint-{}.tar.gz", host_target());
    assert!(
        urls.contains("https://github.com/avidianity/codeint/releases/download/v0.8.0/install.sh"),
        "expected tagged installer URL, got:\n{urls}"
    );
    assert!(
        urls.contains("https://github.com/avidianity/codeint/releases/download/v0.8.0/SHA256SUMS"),
        "expected tagged SHA256SUMS URL, got:\n{urls}"
    );
    assert!(
        urls.contains(&format!(
            "https://github.com/avidianity/codeint/releases/download/v0.8.0/{archive_name}"
        )),
        "expected tagged archive URL, got:\n{urls}"
    );
    assert!(
        !urls.contains("releases/latest/download"),
        "pinned install must not touch latest release:\n{urls}"
    );

    let expected_bin_dir = sandbox.path().join("home/.local/bin");
    let path_entries = fs::read_to_string(&github_path).expect("read GITHUB_PATH");
    assert!(
        path_entries.contains(&expected_bin_dir.display().to_string()),
        "GITHUB_PATH missing install dir: {path_entries}"
    );
}

#[test]
fn action_install_fails_when_installed_binary_version_mismatches_requested_tag() {
    let releases = TempDir::new().expect("create stub releases");
    build_stub_release_dir(&releases.path().join("download/v0.8.0"), "0.9.0");

    let (output, _curl_log, _github_path, _sandbox) =
        run_action_install_with_stub_releases("v0.8.0", releases.path(), None);
    assert!(
        !output.status.success(),
        "expected version mismatch to fail, stdout={}",
        String::from_utf8_lossy(&output.stdout)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("0.8.0") && stderr.contains("0.9.0"),
        "expected version mismatch diagnostic in stderr, got: {stderr}"
    );
}

#[test]
fn action_install_latest_uses_latest_release_with_codeint_version_unset() {
    let releases = TempDir::new().expect("create stub releases");
    build_stub_release_dir(&releases.path().join("latest/download"), "0.9.0");
    build_stub_release_dir(&releases.path().join("download/v0.8.0"), "0.8.0");

    let (output, curl_log, _github_path, _sandbox) =
        run_action_install_with_stub_releases("latest", releases.path(), Some("v0.8.0"));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "latest install failed: stdout={}, stderr={stderr}",
        String::from_utf8_lossy(&output.stdout)
    );

    let urls = fs::read_to_string(&curl_log).expect("read curl log");
    let archive_name = format!("codeint-{}.tar.gz", host_target());
    assert!(
        urls.contains("https://github.com/avidianity/codeint/releases/latest/download/install.sh"),
        "expected latest installer URL, got:\n{urls}"
    );
    assert!(
        urls.contains("https://github.com/avidianity/codeint/releases/latest/download/SHA256SUMS"),
        "expected latest SHA256SUMS URL, got:\n{urls}"
    );
    assert!(
        urls.contains(&format!(
            "https://github.com/avidianity/codeint/releases/latest/download/{archive_name}"
        )),
        "expected latest archive URL, got:\n{urls}"
    );
    assert!(
        !urls.contains("releases/download/v0.8.0"),
        "latest install must unset CODEINT_VERSION:\n{urls}"
    );
}
