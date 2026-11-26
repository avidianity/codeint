use std::fs;
use std::path::Path;
use std::process::{Command, Output};

use serde_json::Value;
use tempfile::TempDir;

use crate::common::{eroded_ts, init_git_repo, write_repo_files};

fn run_codeint(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_codeint"))
        .args(args)
        .current_dir(dir)
        .output()
        .expect("run codeint")
}

fn stdout_text(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr_text(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

#[test]
fn init_and_init_adopt_support_jsonc_json_toml_and_yaml_end_to_end() {
    let formats = [
        ("jsonc", "codeint.jsonc"),
        ("json", "codeint.json"),
        ("toml", "codeint.toml"),
        ("yaml", "codeint.yaml"),
    ];

    for (flag, file_name) in formats {
        let repo = init_git_repo(&[("src/app.ts", "export const ok = 1;\n")]);
        let init_out = run_codeint(repo.path(), &["init", "--format", flag]);
        assert_eq!(
            init_out.status.code(),
            Some(0),
            "init --format {flag} stderr: {}",
            stderr_text(&init_out)
        );
        assert!(repo.path().join(file_name).is_file(), "{file_name} created");
        if flag == "json" {
            let body = fs::read_to_string(repo.path().join(file_name)).expect("read json");
            serde_json::from_str::<Value>(&body).expect("strict valid JSON");
        }

        let doctor_out = run_codeint(repo.path(), &["doctor", "--format", "json"]);
        assert_eq!(doctor_out.status.code(), Some(0));
        let doctor: Value = serde_json::from_slice(&doctor_out.stdout).expect("doctor json");
        assert_eq!(doctor["status"], "pass");

        let resolved_out = run_codeint(repo.path(), &["config", "--resolved", "--format", "json"]);
        assert_eq!(resolved_out.status.code(), Some(0));
        let resolved: Value = serde_json::from_slice(&resolved_out.stdout).expect("resolved json");
        assert!(
            resolved["config_file"]
                .as_str()
                .unwrap_or_default()
                .ends_with(file_name),
            "resolved config_file: {}",
            resolved["config_file"]
        );

        let check_out = run_codeint(repo.path(), &["check", "--format", "json"]);
        assert_eq!(check_out.status.code(), Some(0));

        let adopt_repo = init_git_repo(&[("src/legacy.ts", &eroded_ts("legacy", 12))]);
        let adopt_out = run_codeint(adopt_repo.path(), &["init", "--adopt", "--format", flag]);
        assert_eq!(
            adopt_out.status.code(),
            Some(0),
            "init --adopt --format {flag} stderr: {}",
            stderr_text(&adopt_out)
        );
        let adopt_stdout = stdout_text(&adopt_out);
        assert!(adopt_stdout.contains(file_name), "stdout: {adopt_stdout}");
        assert!(adopt_repo.path().join(file_name).is_file());
        assert!(adopt_repo.path().join(".codeint/ratchet.json").is_file());
        assert!(adopt_repo.path().join(".codeint/baseline.json").is_file());

        let post_adopt_check = run_codeint(adopt_repo.path(), &["check", "--format", "json"]);
        assert_eq!(
            post_adopt_check.status.code(),
            Some(0),
            "post-adopt check stderr: {}",
            stderr_text(&post_adopt_check)
        );
    }
}

#[test]
fn check_and_doctor_enforce_toml_and_yaml_safety_and_schema_rules() {
    let cases = [
        (
            "codeint.yaml",
            "format: &a text\n",
            "is not valid YAML",
            "anchors",
        ),
        (
            "codeint.yaml",
            "format: text\n---\nformat: json\n",
            "is not valid YAML",
            "single mapping document",
        ),
        (
            "codeint.yaml",
            "123: text\n",
            "is not valid YAML",
            "string keys",
        ),
        (
            "codeint.toml",
            "[budgets.production]\nerosion = 2026-11-26T00:00:00Z\n",
            "TOML date-time",
            "Replace the TOML date-time value",
        ),
        (
            "codeint.toml",
            "#:schema https://raw.githubusercontent.com/avidianity/codeint/v0.1.0/schema/codeint.schema.json\n\"$schema\" = \"https://raw.githubusercontent.com/avidianity/codeint/v0.2.0/schema/codeint.schema.json\"\n",
            "conflicting schema URLs",
            "Keep only one schema URL",
        ),
        (
            "codeint.yaml",
            "# yaml-language-server: $schema=https://raw.githubusercontent.com/avidianity/codeint/v99.0.0/schema/codeint.schema.json\nformat: text\n",
            "newer than this binary",
            "codeint update",
        ),
    ];

    for (file_name, body, expected_msg, expected_remedy) in cases {
        let repo = init_git_repo(&[("src/app.ts", "export const ok = 1;\n"), (file_name, body)]);
        let check_out = run_codeint(repo.path(), &["check"]);
        assert_eq!(
            check_out.status.code(),
            Some(2),
            "expected exit 2 for {file_name} ({body:?}), got stderr: {}",
            stderr_text(&check_out)
        );
        let err = stderr_text(&check_out);
        assert!(err.contains(expected_msg), "stderr: {err}");
        assert!(err.contains(expected_remedy), "stderr: {err}");

        let doctor_out = run_codeint(repo.path(), &["doctor", "--format", "json"]);
        assert_eq!(doctor_out.status.code(), Some(1));
    }

    let ambiguous = init_git_repo(&[
        ("src/app.ts", "export const ok = 1;\n"),
        ("codeint.toml", "format = \"text\"\n"),
        (".codeint/config.yml", "format: text\n"),
    ]);
    let ambig_out = run_codeint(ambiguous.path(), &["check"]);
    assert_eq!(ambig_out.status.code(), Some(2));
    assert!(stderr_text(&ambig_out).contains("codeint.toml, .codeint/config.yml"));
}

#[test]
fn update_rewrites_schema_directives_in_toml_and_yaml_configs() {
    let cases = [
        (
            "codeint.toml",
            "#:schema https://raw.githubusercontent.com/avidianity/codeint/v0.1.0/schema/codeint.schema.json\n\nformat = \"text\"\n",
            "#:schema https://raw.githubusercontent.com/avidianity/codeint/v9.9.9/schema/codeint.schema.json\n\nformat = \"text\"\n",
        ),
        (
            "codeint.yaml",
            "# yaml-language-server: $schema=https://raw.githubusercontent.com/avidianity/codeint/v0.1.0/schema/codeint.schema.json\nformat: text\n",
            "# yaml-language-server: $schema=https://raw.githubusercontent.com/avidianity/codeint/v9.9.9/schema/codeint.schema.json\nformat: text\n",
        ),
    ];

    let current_tag = format!("v{}", env!("CARGO_PKG_VERSION"));
    for (file_name, original_template, _) in cases {
        let original = original_template;
        let expected = original.replace("v0.1.0", &current_tag);
        let temp = TempDir::new().expect("temp dir");
        let repo_dir = temp.path().join("repo");
        fs::create_dir_all(repo_dir.join(".git")).expect("git dir");
        write_repo_files(&repo_dir, &[(file_name, original)]);
        let release_dir = temp.path().join("release");
        fs::create_dir_all(&release_dir).expect("release dir");
        fs::write(release_dir.join("LATEST"), format!("{current_tag}\n")).expect("write LATEST");

        let output = Command::new(env!("CARGO_BIN_EXE_codeint"))
            .arg("update")
            .current_dir(&repo_dir)
            .env("CODEINT_RELEASE_BASE_URL", release_dir.as_os_str())
            .output()
            .expect("run codeint update");
        assert_eq!(
            output.status.code(),
            Some(0),
            "update stderr: {}",
            stderr_text(&output)
        );
        let actual = fs::read_to_string(repo_dir.join(file_name)).expect("read updated config");
        assert_eq!(actual, expected);
    }
}
