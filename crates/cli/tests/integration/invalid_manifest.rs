use std::fs;
use std::path::Path;

use serde_json::{Value, json};

use crate::common::{
    init_git_repo, mcp_payload, mcp_tool_call, run_codeint_text, send_mcp_requests,
};

const TS_SOURCE: &str = "export function add(a: number, b: number): number {\n  return a + b;\n}\n";
const GO_SOURCE: &str = "package tools\n\nfunc One() int {\n\treturn 1\n}\n";
const PYTHON_SOURCE: &str = "def one():\n    return 1\n";

fn monorepo_with_invalid_go_modules() -> tempfile::TempDir {
    init_git_repo(&[
        (
            "package.json",
            r#"{ "name": "mono", "private": true, "workspaces": ["packages/*"] }"#,
        ),
        ("packages/app/package.json", r#"{ "name": "app" }"#),
        ("packages/app/src/index.ts", TS_SOURCE),
        ("packages/app/fixtures/empty/go.mod", ""),
        ("packages/app/fixtures/empty/one.go", GO_SOURCE),
        (
            "packages/app/fixtures/garbage/go.mod",
            "\u{1b}[38;2;249;38;114mmodule example.com/garbage\n",
        ),
        ("packages/app/fixtures/garbage/two.go", GO_SOURCE),
        ("packages/lib/package.json", r#"{ "name": "lib" }"#),
        ("packages/lib/src/index.ts", TS_SOURCE),
    ])
}

fn web_repo() -> tempfile::TempDir {
    init_git_repo(&[
        ("package.json", r#"{ "name": "web" }"#),
        ("a.ts", TS_SOURCE),
    ])
}

fn json_of(dir: &Path, args: &[&str]) -> Value {
    let (stdout, _) = run_codeint_text(dir, args, 0);
    serde_json::from_str(&stdout).expect("json output")
}

fn check_report(dir: &Path) -> Value {
    json_of(dir, &["check", "--format", "json"])
}

fn text_of(dir: &Path, args: &[&str]) -> String {
    run_codeint_text(dir, args, 0).0
}

fn warnings(report: &Value) -> Vec<String> {
    report["manifest_warnings"]
        .as_array()
        .map(|entries| {
            entries
                .iter()
                .map(|entry| entry.as_str().expect("warning text").to_owned())
                .collect()
        })
        .unwrap_or_default()
}

#[test]
fn check_skips_invalid_go_modules_with_warnings_and_measures_the_rest() {
    let repo = monorepo_with_invalid_go_modules();

    let report = check_report(repo.path());

    assert_eq!(report["files"], 4);
    let found = warnings(&report);
    assert_eq!(found.len(), 2, "{found:?}");
    assert!(
        found[0].contains("packages/app/fixtures/empty/go.mod")
            && found[0].contains("missing module directive"),
        "{}",
        found[0]
    );
    assert!(
        found[1].contains("packages/app/fixtures/garbage/go.mod")
            && found[1].contains("unknown directive"),
        "{}",
        found[1]
    );
}

#[test]
fn files_under_a_skipped_manifest_count_toward_the_surrounding_workspace_package() {
    let repo = monorepo_with_invalid_go_modules();

    let report = json_of(
        repo.path(),
        &["check", "--package", "packages/app", "--format", "json"],
    );

    assert_eq!(report["files"], 3);
    assert_eq!(warnings(&report).len(), 2);
}

#[test]
fn every_output_format_carries_the_warning_without_changing_the_exit_code() {
    let repo = monorepo_with_invalid_go_modules();
    let saved = repo.path().join("saved.json");

    let json = text_of(repo.path(), &["check", "--format", "json"]);
    fs::write(&saved, &json).expect("save report");

    let toon = text_of(repo.path(), &["check", "--format", "toon"]);
    assert!(toon.contains("manifest_warnings"), "{toon}");
    assert!(
        toon.contains("packages/app/fixtures/empty/go.mod"),
        "{toon}"
    );

    let markdown = text_of(repo.path(), &["check", "--format", "markdown"]);
    assert!(
        markdown.contains("Skipped the invalid go manifest packages/app/fixtures/empty/go.mod"),
        "{markdown}"
    );

    let text = text_of(
        repo.path(),
        &["report", saved.to_str().expect("path"), "--format", "text"],
    );
    assert!(
        text.contains("packages/app/fixtures/garbage/go.mod"),
        "{text}"
    );

    let sarif = text_of(repo.path(), &["check", "--format", "sarif"]);
    serde_json::from_str::<Value>(&sarif).expect("sarif stays valid JSON");
}

#[test]
fn invalid_manifest_of_a_second_language_is_skipped_in_a_typescript_repository() {
    let repo = init_git_repo(&[
        ("package.json", r#"{ "name": "web" }"#),
        ("src/index.ts", TS_SOURCE),
        ("pyproject.toml", "[[["),
        ("tools/one.py", PYTHON_SOURCE),
    ]);

    let report = check_report(repo.path());

    assert_eq!(report["files"], 2);
    let found = warnings(&report);
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(
        found[0].starts_with("Skipped the invalid python manifest pyproject.toml: "),
        "{}",
        found[0]
    );
}

#[test]
fn repository_without_invalid_manifests_has_no_warnings() {
    let repo = web_repo();

    let report = check_report(repo.path());

    assert!(report.get("manifest_warnings").is_none());
}

#[test]
fn diff_carries_the_warning_for_the_head_commit() {
    let repo = web_repo();
    crate::common::run_git(repo.path(), &["checkout", "-q", "-b", "feature"]);
    fs::create_dir_all(repo.path().join("fixtures")).expect("mkdir");
    fs::write(repo.path().join("fixtures/go.mod"), "").expect("write go.mod");
    fs::write(repo.path().join("fixtures/one.go"), GO_SOURCE).expect("write go file");
    crate::common::run_git(repo.path(), &["add", "-A"]);
    crate::common::run_git(repo.path(), &["commit", "-q", "-m", "add fixtures"]);

    let report = json_of(repo.path(), &["diff", "main", "--format", "json"]);

    let found = warnings(&report);
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(found[0].contains("fixtures/go.mod"), "{}", found[0]);

    let markdown = text_of(repo.path(), &["diff", "main", "--format", "markdown"]);
    assert!(markdown.contains("fixtures/go.mod"), "{markdown}");
}

#[test]
fn mcp_check_tool_carries_the_warning() {
    let repo = monorepo_with_invalid_go_modules();

    let responses = send_mcp_requests(repo.path(), &[mcp_tool_call(1, "check", json!({}))]);
    let payload = mcp_payload(&responses[0]);

    assert_eq!(payload["passed"], true);
    assert_eq!(warnings(&payload["report"]).len(), 2);
}

#[test]
fn doctor_warns_instead_of_failing_on_an_invalid_manifest() {
    let repo = init_git_repo(&[("package.json", r#"{ "name": "web" }"#), ("go.mod", "")]);

    let report = json_of(repo.path(), &["doctor", "--format", "json"]);
    let workspace = report["checks"]
        .as_array()
        .expect("checks")
        .iter()
        .find(|check| check["id"] == "workspace")
        .expect("workspace check");
    assert_eq!(workspace["status"], "warn", "{workspace}");
}
