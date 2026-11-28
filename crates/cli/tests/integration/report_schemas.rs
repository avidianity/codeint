use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use codeint_report::{ReportSchemaKind, validate_json_against_schema};
use serde_json::Value;
use tempfile::TempDir;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn load_published_schema(kind: ReportSchemaKind) -> Value {
    let path = repo_root().join(kind.relative_path());
    let text = fs::read_to_string(&path).expect("read published report schema");
    serde_json::from_str(&text).expect("parse published report schema")
}

fn run_cli_json(dir: &Path, args: &[&str], expected_codes: &[i32]) -> Value {
    let output = Command::new(env!("CARGO_BIN_EXE_codeint"))
        .args(args)
        .current_dir(dir)
        .output()
        .expect("run codeint cli");
    let code = output.status.code().unwrap_or(-1);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        expected_codes.contains(&code),
        "codeint {args:?} exited with {code} (expected {expected_codes:?})\nstdout: {stdout}\nstderr: {stderr}"
    );
    serde_json::from_str(&stdout)
        .unwrap_or_else(|err| panic!("codeint {args:?} did not emit valid JSON: {err}\n{stdout}"))
}

fn assert_conforms(kind: ReportSchemaKind, value: &Value) {
    let schema = load_published_schema(kind);
    if let Err(error) = validate_json_against_schema(value, &schema) {
        panic!(
            "{} output violated {}: {error}",
            kind.name(),
            kind.relative_path()
        );
    }
}

fn setup_two_commit_repo() -> TempDir {
    let dir = crate::sdk::setup_repo();
    fs::write(
        dir.path().join("src/app.ts"),
        "import { flag } from './helper';\nexport function ok(x: boolean): boolean {\n  if (flag(x)) {\n    return true;\n  } else {\n    return false;\n  }\n}\n",
    )
    .expect("update app.ts");
    crate::sdk::git(dir.path(), &["add", "."]);
    crate::sdk::git(dir.path(), &["commit", "-m", "second"]);
    dir
}

#[test]
fn live_cli_json_outputs_conform_to_published_report_schemas() {
    let dir = setup_two_commit_repo();
    let root = dir.path();

    let check_json = run_cli_json(root, &["check", "--format", "json"], &[0, 1]);
    assert_conforms(ReportSchemaKind::Check, &check_json);

    let saved_check = root.join("saved-check.json");
    fs::write(&saved_check, check_json.to_string()).expect("write saved check");
    let rerendered_check = run_cli_json(
        root,
        &[
            "report",
            saved_check.to_str().expect("utf8 path"),
            "--format",
            "json",
        ],
        &[0],
    );
    assert_conforms(ReportSchemaKind::Check, &rerendered_check);

    let diff_json = run_cli_json(root, &["diff", "HEAD~1", "--format", "json"], &[0, 1]);
    assert_conforms(ReportSchemaKind::Diff, &diff_json);

    let saved_diff = root.join("saved-diff.json");
    fs::write(&saved_diff, diff_json.to_string()).expect("write saved diff");
    let rerendered_diff = run_cli_json(
        root,
        &[
            "report",
            saved_diff.to_str().expect("utf8 path"),
            "--format",
            "json",
        ],
        &[0],
    );
    assert_conforms(ReportSchemaKind::Diff, &rerendered_diff);

    let trend_json = run_cli_json(root, &["trend", "--format", "json"], &[0]);
    assert_conforms(ReportSchemaKind::Trend, &trend_json);

    let impact_json = run_cli_json(root, &["impact", "src/helper.ts", "--format", "json"], &[0]);
    assert_conforms(ReportSchemaKind::Impact, &impact_json);

    let trace_json = run_cli_json(
        root,
        &["trace", "src/app.ts", "src/helper.ts", "--format", "json"],
        &[0],
    );
    assert_conforms(ReportSchemaKind::Trace, &trace_json);

    let guard_json = run_cli_json(root, &["guard", "src/app.ts", "--format", "json"], &[0]);
    assert_conforms(ReportSchemaKind::Guard, &guard_json);

    let schema_json = run_cli_json(root, &["schema", "--format", "json"], &[0]);
    assert_conforms(ReportSchemaKind::Schema, &schema_json);

    let config_json = run_cli_json(root, &["config", "--resolved", "--format", "json"], &[0]);
    assert_conforms(ReportSchemaKind::Config, &config_json);

    let doctor_json = run_cli_json(root, &["doctor", "--format", "json"], &[0, 1]);
    assert_conforms(ReportSchemaKind::Doctor, &doctor_json);

    let agent_home = TempDir::new().expect("agent home");
    let home_str = agent_home.path().to_str().expect("utf8 home");
    let agent_status_json = run_cli_json(
        root,
        &[
            "agent",
            "status",
            "--config-root",
            home_str,
            "--format",
            "json",
        ],
        &[0],
    );
    assert_conforms(ReportSchemaKind::AgentStatus, &agent_status_json);

    let agent_install_json = run_cli_json(
        root,
        &[
            "agent",
            "install",
            "--client",
            "claude-code",
            "--config-root",
            home_str,
            "--dry-run",
            "--format",
            "json",
        ],
        &[0],
    );
    assert_conforms(ReportSchemaKind::AgentInstall, &agent_install_json);

    let churn_json = run_cli_json(root, &["churn", "--format", "json"], &[0]);
    assert_conforms(ReportSchemaKind::Churn, &churn_json);

    let coupling_json = run_cli_json(root, &["coupling", "--format", "json"], &[0]);
    assert_conforms(ReportSchemaKind::Coupling, &coupling_json);

    let ratchet_json = run_cli_json(root, &["ratchet", "update", "--format", "json"], &[0]);
    assert_conforms(ReportSchemaKind::RatchetUpdate, &ratchet_json);

    let baseline_json = run_cli_json(root, &["baseline", "update", "--format", "json"], &[0]);
    assert_conforms(ReportSchemaKind::BaselineUpdate, &baseline_json);

    let plugin_path = crate::wasm_fixtures::wasm_fixture_path("sample_mini_plugin.wasm");
    let plugin_json = run_cli_json(
        root,
        &[
            "plugin",
            "test",
            plugin_path.to_str().expect("utf8 wasm"),
            "--kind",
            "language",
            "--format",
            "json",
        ],
        &[0, 1],
    );
    assert_conforms(ReportSchemaKind::PluginTest, &plugin_json);
}
