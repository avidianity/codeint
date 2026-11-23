use std::fs;
use std::path::Path;
use std::process::Command;

use codeint_cli::{
    CheckOptions, DiffOptions, ReportFormat, TrendFormat, TrendOptions, check, diff, init, trend,
};
use tempfile::TempDir;

fn git(dir: &Path, args: &[&str]) {
    let status = Command::new("git")
        .args(args)
        .current_dir(dir)
        .status()
        .expect("run git");
    assert!(status.success(), "git {args:?} failed");
}

fn setup_repo() -> TempDir {
    let dir = TempDir::new().expect("tempdir");
    git(dir.path(), &["init", "-b", "main"]);
    git(dir.path(), &["config", "user.email", "dev@example.com"]);
    git(dir.path(), &["config", "user.name", "Dev"]);
    fs::create_dir_all(dir.path().join("src")).expect("src dir");
    fs::write(
        dir.path().join("src/app.ts"),
        "export function ok(x: boolean): boolean {\n  return x;\n}\n",
    )
    .expect("write app.ts");
    git(dir.path(), &["add", "."]);
    git(dir.path(), &["commit", "-m", "initial"]);
    dir
}

#[test]
fn rust_sdk_supports_init_check_diff_trend_and_budget_failures() {
    let dir = setup_repo();

    let created = init(dir.path()).expect("init succeeds");
    assert!(created.ends_with("codeint.jsonc"));

    let checked = check(dir.path(), &CheckOptions::default()).expect("check succeeds");
    assert!(checked.passed);
    assert!(checked.failures.is_empty());
    let json = checked
        .report
        .render(ReportFormat::Json)
        .expect("render json");
    assert!(json.contains("\"files\": 1"));

    fs::write(
        dir.path().join("codeint.jsonc"),
        "{\"budgets\":{\"production\":{\"verbosity\":0}}}\n",
    )
    .expect("write tight budget");
    fs::write(
        dir.path().join("src/app.ts"),
        "export function verbose(x: boolean): boolean {\n  if (x) {\n    return true;\n  } else {\n    return false;\n  }\n}\n",
    )
    .expect("write verbose");

    let failed_check = check(dir.path(), &CheckOptions::default()).expect("returns outcome");
    assert!(!failed_check.passed);
    assert!(!failed_check.failures.is_empty());

    let diffed = diff(dir.path(), "HEAD", &DiffOptions::default()).expect("diff succeeds");
    assert!(!diffed.passed);
    assert!(!diffed.failures.is_empty());

    let trend_report = trend(dir.path(), &TrendOptions::default()).expect("trend succeeds");
    let trend_json = trend_report
        .render(TrendFormat::Json)
        .expect("render trend");
    assert!(trend_json.contains("\"commits\""));

    let outside = TempDir::new().expect("outside");
    let err = check(outside.path(), &CheckOptions::default())
        .err()
        .expect("operational error");
    assert!(err.message().contains("not inside a Git repository"));
    assert!(!err.remedy().is_empty());
}
