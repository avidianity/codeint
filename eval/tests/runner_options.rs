use std::path::Path;
use std::process::Command;

use serde_json::{Value, json};
use tempfile::TempDir;

fn git(folder: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(args)
        .current_dir(folder)
        .output()
        .expect("run git");
    assert!(output.status.success(), "{output:?}");
    String::from_utf8(output.stdout).expect("utf8")
}

fn init_repo(root: &Path, name: &str, body: &str) -> (std::path::PathBuf, String) {
    let folder = root.join(name);
    std::fs::create_dir_all(folder.join("src")).expect("mkdir");
    std::fs::write(folder.join("src/a.sql"), body).expect("write");
    git(&folder, &["init", "--initial-branch=main"]);
    git(&folder, &["config", "user.name", "Test"]);
    git(&folder, &["config", "user.email", "test@example.com"]);
    git(&folder, &["add", "."]);
    git(&folder, &["commit", "-m", "initial"]);
    let commit = git(&folder, &["rev-parse", "HEAD"]).trim().to_owned();
    (folder, commit)
}

fn write_fake_codeint(folder: &Path) -> std::path::PathBuf {
    let path = folder.join("fake-codeint.sh");
    std::fs::write(
        &path,
        "#!/bin/sh\nprintf '{\"source_sets\":{\"production\":{\"source_lines\":3000,\"verbosity\":{\"findings\":[]},\"orphan_files\":{\"findings\":[]}}}}'\n",
    )
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

#[test]
fn parallel_jobs_preserves_manifest_order_and_apply_verdicts_rerenders_summary() {
    let work = TempDir::new().expect("work");
    let scratch = TempDir::new().expect("scratch");
    let (r1, c1) = init_repo(work.path(), "r1", "SELECT 1;\n");
    let (r2, c2) = init_repo(work.path(), "r2", "SELECT 2;\n");
    let (r3, c3) = init_repo(work.path(), "r3", "SELECT 3;\n");
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
    let fake_codeint = write_fake_codeint(work.path());
    let first_out = work.path().join("out1");

    let run = Command::new(env!("CARGO_BIN_EXE_codeint-eval"))
        .args([
            "--manifest",
            manifest.to_str().expect("manifest str"),
            "--codeint",
            fake_codeint.to_str().expect("codeint str"),
            "--output-dir",
            first_out.to_str().expect("out1 str"),
            "--scratch-root",
            scratch.path().to_str().expect("scratch str"),
            "--runs",
            "1",
            "--jobs",
            "3",
        ])
        .output()
        .expect("run codeint-eval");
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
