use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use tempfile::TempDir;

use crate::common::{
    eroded_ts, init_git_repo, run_git, stage_codeint_for_packaging, write_repo_files,
};

fn action_script() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../scripts/action-diff.sh")
        .canonicalize()
        .expect("locate scripts/action-diff.sh")
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
fn action_diff_runs_the_real_cli_from_the_path_input_folder_and_reports_a_pass() {
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
        action.contains("bash \"$ACTION_PATH/scripts/action-diff.sh\""),
        "action.yml no longer runs scripts/action-diff.sh"
    );
}
