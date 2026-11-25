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

fn codeint_checkout_scratch_root() -> TempDir {
    let target = Path::new(env!("CARGO_MANIFEST_DIR")).join("../target");
    std::fs::create_dir_all(&target).expect("target folder");
    tempfile::Builder::new()
        .prefix("guard-")
        .tempdir_in(&target)
        .expect("scratch root inside the codeint checkout")
}

fn assert_guard_refuses(folder: &Path, manifest_repository: &Path, scratch_root: &Path) {
    let output = run_from(folder, manifest_repository, scratch_root);

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(2), "{stderr}");
    assert!(stderr.contains(GUARD_MESSAGE), "{stderr}");
}

#[test]
fn a_scratch_root_inside_the_codeint_checkout_exits_2_when_the_manifest_is_in_another_repository() {
    let other_repository = repository_with_manifest();
    let scratch_root = codeint_checkout_scratch_root();
    let elsewhere = TempDir::new().expect("other folder");

    assert_guard_refuses(
        elsewhere.path(),
        other_repository.path(),
        scratch_root.path(),
    );
}

#[test]
fn a_scratch_root_inside_the_codeint_checkout_exits_2_when_the_manifest_is_outside_any_repository()
{
    let manifest_folder = TempDir::new().expect("manifest folder");
    let corpus = serde_json::json!({
        "measured_on": "2026-11-25",
        "repositories": [
            {"name": "local/python", "language": "python", "bucket": "small", "url": "file:///nonexistent", "commit": "0123456789abcdef0123456789abcdef01234567"}
        ]
    });
    std::fs::create_dir_all(manifest_folder.path().join("eval")).expect("eval folder");
    std::fs::write(
        manifest_folder.path().join("eval/corpus.json"),
        corpus.to_string(),
    )
    .expect("write manifest");
    let scratch_root = codeint_checkout_scratch_root();
    let elsewhere = TempDir::new().expect("other folder");

    assert_guard_refuses(
        elsewhere.path(),
        manifest_folder.path(),
        scratch_root.path(),
    );
}

#[test]
fn a_scratch_root_inside_the_current_folders_repository_exits_2() {
    let manifest_folder = repository_with_manifest();
    let current_repository = repository_with_manifest();
    let scratch_root = current_repository.path().join("eval/scratch");

    assert_guard_refuses(
        current_repository.path(),
        manifest_folder.path(),
        &scratch_root,
    );
}
