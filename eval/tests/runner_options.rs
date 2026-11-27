use std::path::Path;
use std::process::Command;

use serde_json::{Value, json};
use tempfile::TempDir;

fn git(folder: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(["-c", "commit.gpgsign=false"])
        .args(args)
        .current_dir(folder)
        .output()
        .expect("run git");
    assert!(output.status.success(), "{output:?}");
    String::from_utf8(output.stdout).expect("utf8")
}

fn init_repo(root: &Path, name: &str, relative: &str, body: &str) -> (std::path::PathBuf, String) {
    let folder = root.join(name);
    let target = folder.join(relative);
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent).expect("mkdir");
    }
    std::fs::write(target, body).expect("write");
    git(&folder, &["init", "--initial-branch=main"]);
    git(&folder, &["config", "user.name", "Test"]);
    git(&folder, &["config", "user.email", "test@example.com"]);
    git(&folder, &["add", "."]);
    git(&folder, &["commit", "-m", "initial"]);
    let commit = git(&folder, &["rev-parse", "HEAD"]).trim().to_owned();
    (folder, commit)
}

fn write_fake_codeint(folder: &Path, name: &str, report_json: &str) -> std::path::PathBuf {
    let path = folder.join(name);
    std::fs::write(&path, format!("#!/bin/sh\nprintf '%s' '{report_json}'\n"))
        .expect("write fake codeint");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = std::fs::metadata(&path).expect("meta").permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&path, permissions).expect("chmod");
    }
    path
}

fn run_eval(
    manifest: &Path,
    codeint: &Path,
    out: &Path,
    scratch: &Path,
    extra: &[&str],
) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_codeint-eval"))
        .args([
            "--manifest",
            manifest.to_str().expect("manifest str"),
            "--codeint",
            codeint.to_str().expect("codeint str"),
            "--output-dir",
            out.to_str().expect("out str"),
            "--scratch-root",
            scratch.to_str().expect("scratch str"),
            "--runs",
            "1",
        ])
        .args(extra)
        .output()
        .expect("run codeint-eval")
}

#[test]
fn parallel_jobs_preserves_manifest_order_and_apply_verdicts_rerenders_summary() {
    let work = TempDir::new().expect("work");
    let scratch = TempDir::new().expect("scratch");
    let (r1, c1) = init_repo(work.path(), "r1", "src/a.sql", "SELECT 1;\n");
    let (r2, c2) = init_repo(work.path(), "r2", "src/a.sql", "SELECT 2;\n");
    let (r3, c3) = init_repo(work.path(), "r3", "src/a.sql", "SELECT 3;\n");
    let manifest = work.path().join("manifest.json");
    let body = json!({
        "measured_on": "2026-11-26",
        "repositories": [
            {"name": "local/one", "language": "sql", "bucket": "small", "url": format!("file://{}", r1.display()), "commit": c1},
            {"name": "local/two", "language": "sql", "bucket": "small", "url": format!("file://{}", r2.display()), "commit": c2},
            {"name": "local/three", "language": "sql", "bucket": "small", "url": format!("file://{}", r3.display()), "commit": c3}
        ]
    });
    std::fs::write(&manifest, body.to_string()).expect("manifest");
    let fake_codeint = write_fake_codeint(
        work.path(),
        "fake-codeint.sh",
        r#"{"source_sets":{"production":{"source_lines":3000,"verbosity":{"findings":[]},"orphan_files":{"findings":[]}}}}"#,
    );
    let first_out = work.path().join("out1");

    let run = run_eval(
        &manifest,
        &fake_codeint,
        &first_out,
        scratch.path(),
        &["--jobs", "3"],
    );
    assert!(run.status.success(), "{run:?}");

    let summary_path = first_out.join("evaluation-summary.json");
    let summary: Value =
        serde_json::from_str(&std::fs::read_to_string(&summary_path).expect("summary"))
            .expect("json");
    let names: Vec<&str> = summary["repositories"]
        .as_array()
        .expect("repos")
        .iter()
        .filter_map(|item| item["name"].as_str())
        .collect();
    assert_eq!(names, ["local/one", "local/two", "local/three"]);
    assert!(summary["audit_review"].is_null());

    let sampled_item = summary["audit"][0]["items"][0]
        .as_str()
        .expect("sampled source set item");
    let verdicts_path = work.path().join("verdicts.json");
    let verdicts = json!({
        "reviewer": "agent",
        "verdicts": [{
            "category": "source_sets",
            "item": sampled_item,
            "verdict": "correct",
            "note": "matches source set"
        }]
    });
    std::fs::write(&verdicts_path, verdicts.to_string()).expect("verdicts");
    let second_out = work.path().join("out2");

    let rerender = Command::new(env!("CARGO_BIN_EXE_codeint-eval"))
        .args([
            "--apply-verdicts-to",
            summary_path.to_str().expect("summary str"),
            "--audit-verdicts",
            verdicts_path.to_str().expect("verdicts str"),
            "--output-dir",
            second_out.to_str().expect("out2 str"),
        ])
        .output()
        .expect("rerender codeint-eval");
    assert!(rerender.status.success(), "{rerender:?}");

    let updated: Value = serde_json::from_str(
        &std::fs::read_to_string(second_out.join("evaluation-summary.json")).expect("updated"),
    )
    .expect("json");
    assert_eq!(updated["audit_review"]["reviewer"], "agent");
    assert_eq!(updated["audit_review"]["categories"][0]["reviewed"], 1);
    let markdown = std::fs::read_to_string(second_out.join("evaluation.md")).expect("markdown");
    assert!(markdown.contains("agent-reviewed"), "{markdown}");
}

fn run_single_repo_eval(
    repo_dir: &str,
    relative_file: &str,
    file_body: &str,
    entry_name: &str,
    language: &str,
    fake_report: &str,
    expected_code: i32,
) -> (Value, String) {
    let work = TempDir::new().expect("work");
    let scratch = TempDir::new().expect("scratch");
    let (folder, commit) = init_repo(work.path(), repo_dir, relative_file, file_body);
    let fake_codeint = write_fake_codeint(work.path(), "fake-codeint.sh", fake_report);
    let manifest = work.path().join("manifest.json");
    let body = json!({
        "measured_on": "2026-11-27",
        "repositories": [{
            "name": entry_name,
            "language": language,
            "bucket": "small",
            "url": format!("file://{}", folder.display()),
            "commit": commit
        }]
    });
    std::fs::write(&manifest, body.to_string()).expect("manifest");
    let out = work.path().join("out");
    let run = run_eval(&manifest, &fake_codeint, &out, scratch.path(), &[]);
    assert_eq!(run.status.code(), Some(expected_code), "{run:?}");
    let summary: Value = serde_json::from_str(
        &std::fs::read_to_string(out.join("evaluation-summary.json")).expect("summary"),
    )
    .expect("json");
    let markdown = std::fs::read_to_string(out.join("evaluation.md")).expect("markdown");
    (summary, markdown)
}

#[test]
fn missing_declared_language_fails_repository_instead_of_falling_back() {
    let (summary, _) = run_single_repo_eval(
        "py_only",
        "src/app.py",
        "def f():\n    return 1\n",
        "local/misclassified-sql",
        "sql",
        r#"{"languages":{"python":{"source_sets":{"production":{"source_lines":3000,"import_cycles":{"ratio":0.0,"metric_version":1}}}}},"source_sets":{"production":{"source_lines":3000,"import_cycles":{"ratio":0.0,"metric_version":1},"verbosity":{"findings":[]},"orphan_files":{"findings":[]}}}}"#,
        1,
    );
    assert_eq!(summary["passed"], false);
    assert_eq!(summary["repositories"][0]["passed"], false);
    assert!(
        summary["repositories"][0]["error"]
            .as_str()
            .is_some_and(|message| message.contains("no measured files for declared language sql")),
        "{summary}"
    );
    assert!(
        summary["panels"][0]["metrics"]["import_cycles"].is_null(),
        "{summary}"
    );
}

#[test]
fn cognitive_complexity_is_collected_into_panel_distributions_and_markdown_tables() {
    let (summary, markdown) = run_single_repo_eval(
        "sql_repo",
        "src/a.sql",
        "SELECT 1;\n",
        "local/sql",
        "sql",
        r#"{"source_sets":{"production":{"source_lines":3000,"erosion":{"ratio":0.25,"metric_version":1},"cognitive_complexity":{"ratio":0.125,"metric_version":1},"verbosity":{"ratio":0.0,"metric_version":2,"findings":[]},"duplication":{"ratio":0.05,"metric_version":1},"orphan_files":{"findings":[]}}}}"#,
        0,
    );
    assert_eq!(
        summary["panels"][0]["metrics"]["cognitive_complexity"]["version"],
        1
    );
    assert_eq!(
        summary["panels"][0]["metrics"]["cognitive_complexity"]["distribution"]["p10"],
        0.125
    );
    assert!(
        markdown.contains("| Cognitive complexity | 1 |"),
        "{markdown}"
    );
}
