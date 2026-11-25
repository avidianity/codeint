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
    web_repo_with(&[])
}

fn web_repo_with(extra: &[(&str, &str)]) -> tempfile::TempDir {
    let mut files = vec![
        ("package.json", r#"{ "name": "web" }"#),
        ("a.ts", TS_SOURCE),
    ];
    files.extend_from_slice(extra);
    init_git_repo(&files)
}

fn assert_contains(text: &str, needle: &str) {
    assert!(text.contains(needle), "missing {needle:?} in {text}");
}

fn assert_one_warning_mentions(report: &Value, key: &str, needle: &str) {
    let found = warnings_under(report, key);
    assert_eq!(found.len(), 1, "{report}");
    assert_contains(&found[0], needle);
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
    warnings_under(report, "manifest_warnings")
}

fn warnings_under(report: &Value, key: &str) -> Vec<String> {
    report[key]
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

fn commit_on_feature_branch(repo: &Path, change: impl FnOnce(&Path)) {
    crate::common::run_git(repo, &["checkout", "-q", "-b", "feature"]);
    change(repo);
    crate::common::run_git(repo, &["add", "-A"]);
    crate::common::run_git(repo, &["commit", "-q", "-m", "change"]);
}

fn assert_diff_warns_on_one_side(repo: &Path, side: &str, other_side: &str) {
    let key = format!("{}_manifest_warnings", side.to_lowercase());
    let other_key = format!("{}_manifest_warnings", other_side.to_lowercase());

    let json = json_of(repo, &["diff", "main", "--format", "json"]);
    assert_one_warning_mentions(&json, &key, "fixtures/go.mod");
    assert!(json.get(&other_key).is_none(), "{json}");
    assert!(json.get("manifest_warnings").is_none(), "{json}");

    let toon = text_of(repo, &["diff", "main"]);
    assert_contains(&toon, &key);
    assert!(!toon.contains(&other_key), "{toon}");

    let markdown = text_of(repo, &["diff", "main", "--format", "markdown"]);
    assert_contains(
        &markdown,
        &format!("{side} commit: Skipped the invalid go manifest fixtures/go.mod"),
    );
    assert!(
        !markdown.contains(&format!("{other_side} commit:")),
        "{markdown}"
    );
}

#[test]
fn diff_keeps_a_head_only_warning_on_the_head_side_in_every_format() {
    let repo = web_repo();
    commit_on_feature_branch(repo.path(), |root| {
        fs::create_dir_all(root.join("fixtures")).expect("mkdir");
        fs::write(root.join("fixtures/go.mod"), "").expect("write go.mod");
        fs::write(root.join("fixtures/one.go"), GO_SOURCE).expect("write go file");
    });

    assert_diff_warns_on_one_side(repo.path(), "Head", "Base");
}

fn repository_that_drops_an_invalid_go_mod_on_a_feature_branch() -> tempfile::TempDir {
    let repo = web_repo_with(&[("fixtures/go.mod", ""), ("fixtures/one.go", GO_SOURCE)]);
    commit_on_feature_branch(repo.path(), |root| {
        fs::remove_file(root.join("fixtures/go.mod")).expect("remove go.mod");
    });
    repo
}

#[test]
fn diff_keeps_a_base_only_warning_on_the_base_side_in_every_format() {
    let repo = repository_that_drops_an_invalid_go_mod_on_a_feature_branch();

    assert_diff_warns_on_one_side(repo.path(), "Base", "Head");
}

fn main_commit(repo: &Path) -> String {
    let output = std::process::Command::new("git")
        .args(["rev-parse", "main"])
        .current_dir(repo)
        .output()
        .expect("rev-parse");
    String::from_utf8(output.stdout)
        .expect("utf8")
        .trim()
        .to_owned()
}

fn trend_output(dir: &Path, cache: &Path, format: &str) -> String {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_codeint"))
        .args(["trend", "--format", format])
        .current_dir(dir)
        .env("CODEINT_CACHE_DIR", cache)
        .output()
        .expect("run codeint trend");
    assert_eq!(output.status.code(), Some(0), "{:?}", output.stderr);
    String::from_utf8(output.stdout).expect("utf8")
}

#[test]
fn trend_lists_the_warnings_of_each_commit_in_every_format() {
    let cache = tempfile::tempdir().expect("cache folder");
    let repo = repository_that_drops_an_invalid_go_mod_on_a_feature_branch();
    let first = main_commit(repo.path());

    let json: Value =
        serde_json::from_str(&trend_output(repo.path(), cache.path(), "json")).expect("json");
    let entries = json["manifest_warnings"].as_array().expect("warnings");
    assert_eq!(entries.len(), 1, "{json}");
    assert_eq!(entries[0]["commit"], first);
    assert_contains(
        entries[0]["warnings"][0].as_str().expect("text"),
        "fixtures/go.mod",
    );

    let toon = trend_output(repo.path(), cache.path(), "toon");
    assert_contains(&toon, "manifest_warnings");
    assert_contains(&toon, "fixtures/go.mod");

    let markdown = trend_output(repo.path(), cache.path(), "markdown");
    assert_contains(
        &markdown,
        &format!("Commit {first}: Skipped the invalid go manifest fixtures/go.mod"),
    );
}

#[test]
fn package_selection_names_the_skipped_manifest_of_that_folder() {
    let repo = web_repo_with(&[("sub/go.mod", ""), ("sub/one.go", GO_SOURCE)]);

    let (_, stderr) = run_codeint_text(repo.path(), &["check", "--package", "sub"], 2);

    assert_contains(&stderr, "sub/go.mod");
    assert_contains(&stderr, "missing module directive");
}

#[test]
fn warnings_carry_no_raw_control_characters_in_markdown_or_text() {
    let repo = web_repo_with(&[
        ("pyproject.toml", "[tool]\n\u{1b}[31mx = [\n"),
        ("tools/one.py", PYTHON_SOURCE),
    ]);

    for args in [
        &["check", "--format", "markdown"][..],
        &["check", "--format", "toon"][..],
        &["check", "--format", "json"][..],
        &["doctor"][..],
    ] {
        let (stdout, _) = run_codeint_text(repo.path(), args, 0);
        assert!(
            !stdout.chars().any(|c| c.is_control() && c != '\n'),
            "{args:?}: {stdout:?}"
        );
    }
}

#[test]
fn doctor_text_indents_every_line_of_the_skipped_manifest_detail() {
    let repo = init_git_repo(&[
        ("package.json", r#"{ "name": "web" }"#),
        ("a/go.mod", ""),
        ("b/go.mod", ""),
    ]);

    let (stdout, _) = run_codeint_text(repo.path(), &["doctor"], 0);

    for line in stdout.lines().filter(|line| line.contains("go.mod")) {
        assert!(line.starts_with("    "), "{line:?} in {stdout}");
    }
    assert_eq!(
        stdout
            .lines()
            .filter(|line| line.contains("go.mod"))
            .count(),
        2,
        "{stdout}"
    );
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
