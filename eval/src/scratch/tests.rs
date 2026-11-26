use std::process::Command;
use std::time::{Duration, Instant};

use super::*;

fn dead_process_id() -> u32 {
    let mut child = Command::new("git")
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .spawn()
        .expect("spawn a short process");
    let id = child.id();
    child.wait().expect("wait for the process");
    id
}

#[test]
fn a_scratch_folder_names_its_owner_process() {
    let root = tempfile::TempDir::new().expect("scratch root");

    let scratch = ScratchFolder::outside(&[], Some(root.path())).expect("scratch folder");

    let name = scratch
        .path()
        .file_name()
        .and_then(|name| name.to_str())
        .expect("folder name")
        .to_owned();
    assert!(
        name.starts_with(&format!("codeint-eval-{}-", std::process::id())),
        "{name}"
    );
}

#[test]
fn the_sweep_removes_scratch_folders_of_dead_processes_and_keeps_live_and_foreign_ones() {
    let root = tempfile::TempDir::new().expect("scratch root");
    let dead = root
        .path()
        .join(format!("codeint-eval-{}-abc123", dead_process_id()));
    let live = root
        .path()
        .join(format!("codeint-eval-{}-abc123", std::process::id()));
    let legacy = root.path().join("codeint-eval-abc123");
    let unrelated = root.path().join("my-folder");
    for folder in [&dead, &live, &legacy, &unrelated] {
        std::fs::create_dir_all(folder.join("checkout")).expect("create folder");
    }

    let sweep = sweep_stale_scratch_folders(Some(root.path()));

    assert_eq!(
        sweep,
        StaleSweep {
            removed: 1,
            failures: Vec::new()
        }
    );
    assert!(!dead.exists());
    assert!(live.exists());
    assert!(legacy.exists());
    assert!(unrelated.exists());
}

#[test]
fn the_sweep_of_a_missing_scratch_root_removes_nothing() {
    let root = tempfile::TempDir::new().expect("scratch root");

    let sweep = sweep_stale_scratch_folders(Some(&root.path().join("missing")));

    assert_eq!(sweep, StaleSweep::default());
}

#[test]
fn a_process_is_alive_when_the_listing_names_it() {
    assert!(listing_names_process(
        "\"eval.exe\",\"4242\",\"Console\",\"1\",\"9,000 K\"\r\n",
        4242
    ));
    assert!(!listing_names_process(
        "INFO: No tasks are running which match the specified criteria.\r\n",
        4242
    ));
    assert!(!listing_names_process(
        "\"eval.exe\",\"14242\",\"Console\",\"1\",\"9,000 K\"\r\n",
        4242
    ));
}

#[test]
fn local_file_fetch_fails_without_retry_backoff() {
    let origin = tempfile::TempDir::new().expect("origin folder");
    let init = Command::new("git")
        .args(["init", "--quiet", "--initial-branch", "main"])
        .current_dir(origin.path())
        .status()
        .expect("git init");
    assert!(init.success());
    let root = tempfile::TempDir::new().expect("scratch root");
    let scratch = ScratchFolder::outside(&[], Some(root.path())).expect("scratch folder");
    let url = format!("file://{}", origin.path().display());

    let started = Instant::now();
    let result = clone_at_commit(&scratch, &url, "0123456789abcdef0123456789abcdef01234567");
    let elapsed = started.elapsed();

    assert!(result.is_err(), "missing commit must fail");
    assert!(
        elapsed < Duration::from_secs(2),
        "local file:// fetch must fail without waiting for remote retry backoff, took {elapsed:?}"
    );
}

#[test]
fn remote_urls_retry_and_local_file_urls_try_once() {
    assert_eq!(fetch_attempts_for("file:///tmp/repo"), 1);
    assert_eq!(
        fetch_attempts_for("https://github.com/example/repo.git"),
        FETCH_ATTEMPTS
    );
}
