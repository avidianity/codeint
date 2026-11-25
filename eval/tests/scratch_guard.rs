use std::path::Path;
use std::process::{Command, Output};

use tempfile::TempDir;

const GUARD_MESSAGE: &str = "inside the codeint repository";

fn repository_with_manifest() -> TempDir {
    let repository = TempDir::new().expect("repository folder");
    let init = Command::new("git")
        .args(["init", "--quiet"])
        .arg(repository.path())
        .output()
        .expect("run git init");
    assert!(init.status.success(), "{init:?}");
    std::fs::create_dir_all(repository.path().join("eval/scratch")).expect("scratch folder");
    let corpus = serde_json::json!({
        "measured_on": "2026-11-25",
        "repositories": [
            {"name": "local/python", "language": "python", "bucket": "small", "url": "file:///nonexistent", "commit": "0123456789abcdef0123456789abcdef01234567"}
        ]
    });
    std::fs::write(
        repository.path().join("eval/corpus.json"),
        corpus.to_string(),
    )
    .expect("write manifest");
    repository
}

fn run_from(folder: &Path, repository: &Path, scratch_root: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_codeint-eval"))
        .current_dir(folder)
        .arg("--manifest")
        .arg(repository.join("eval/corpus.json"))
        .arg("--codeint")
        .arg(std::env::current_exe().expect("test executable"))
        .arg("--scratch-root")
        .arg(scratch_root)
        .arg("--output-dir")
        .arg(folder.join("out"))
        .output()
        .expect("run codeint-eval")
}

#[test]
fn a_scratch_root_inside_the_repository_exits_2_from_any_folder() {
    let repository = repository_with_manifest();
    let scratch_root = repository.path().join("eval/scratch");
    let elsewhere = TempDir::new().expect("other folder");
    let subfolder = repository.path().join("eval");

    for folder in [elsewhere.path(), subfolder.as_path()] {
        let output = run_from(folder, repository.path(), &scratch_root);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.code(), Some(2), "{stderr}");
        assert!(stderr.contains(GUARD_MESSAGE), "{stderr}");
    }
}

#[test]
fn a_scratch_root_outside_the_repository_passes_the_guard() {
    let repository = repository_with_manifest();
    let scratch_root = TempDir::new().expect("scratch root");
    let elsewhere = TempDir::new().expect("other folder");

    let output = run_from(elsewhere.path(), repository.path(), scratch_root.path());

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!stderr.contains(GUARD_MESSAGE), "{stderr}");
}
