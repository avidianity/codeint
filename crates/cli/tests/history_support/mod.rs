use std::fs;
use std::path::Path;
use std::process::{Command, Output};

use serde_json::Value;
use tempfile::TempDir;

pub(crate) struct HistoryFixture(TempDir);

impl HistoryFixture {
    pub(crate) fn init() -> HistoryFixture {
        let dir = tempfile::tempdir().expect("tempdir");
        run_git(
            dir.path(),
            &["init", "-q", "-b", "main"],
            "2026-05-01T10:00:00Z",
        );
        HistoryFixture(dir)
    }

    fn root(&self) -> &Path {
        self.0.path()
    }

    pub(crate) fn save_and_commit(&self, files: &[(&str, String)], msg: &str, when: &str) {
        for (rel, content) in files {
            let dest = self.root().join(rel);
            fs::create_dir_all(dest.parent().expect("parent")).expect("mkdir");
            fs::write(dest, content).expect("write");
        }
        run_git(self.root(), &["add", "-A"], when);
        run_git(self.root(), &["commit", "-q", "-m", msg], when);
    }

    pub(crate) fn run(&self, subcommand: &str, extra: &[&str]) -> Output {
        exec_subcommand(self.root(), subcommand, extra)
    }

    pub(crate) fn run_json(&self, subcommand: &str, extra: &[&str]) -> Value {
        let args: Vec<&str> = extra.iter().copied().chain(["--format", "json"]).collect();
        let out = self.run(subcommand, &args);
        assert_eq!(
            out.status.code(),
            Some(0),
            "{subcommand} stderr: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        serde_json::from_slice(&out.stdout).expect("valid JSON report")
    }

    pub(crate) fn shallow_clone_of(&self, depth: &str) -> HistoryFixture {
        let shallow = HistoryFixture::init();
        let url = format!("file://{}", self.root().display());
        run_git(
            shallow.root(),
            &["fetch", "-q", "--depth", depth, &url, "main"],
            "2026-05-05T10:00:00Z",
        );
        run_git(
            shallow.root(),
            &["reset", "-q", "--hard", "FETCH_HEAD"],
            "2026-05-05T10:00:00Z",
        );
        shallow
    }
}

fn run_git(cwd: &Path, args: &[&str], when: &str) {
    let vars = [
        ("GIT_CONFIG_GLOBAL", "/dev/null"),
        ("GIT_CONFIG_NOSYSTEM", "1"),
        ("GIT_AUTHOR_NAME", "History"),
        ("GIT_AUTHOR_EMAIL", "history@example.invalid"),
        ("GIT_COMMITTER_NAME", "History"),
        ("GIT_COMMITTER_EMAIL", "history@example.invalid"),
        ("GIT_AUTHOR_DATE", when),
        ("GIT_COMMITTER_DATE", when),
    ];
    let out = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .envs(vars)
        .output()
        .expect("git");
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

fn exec_subcommand(cwd: &Path, subcommand: &str, extra: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_codeint"))
        .arg(subcommand)
        .args(extra)
        .current_dir(cwd)
        .output()
        .expect("run codeint")
}

pub(crate) fn assert_no_blended_score_keys(value: &Value) {
    let forbidden = ["score", "index", "grade", "rating", "hotspot_score"];
    match value {
        Value::Object(map) => {
            for (k, v) in map {
                let lower = k.to_ascii_lowercase();
                for word in forbidden {
                    assert!(
                        !lower.contains(word),
                        "ADR 0003 violation: unexpected blended key {k:?}"
                    );
                }
                assert_no_blended_score_keys(v);
            }
        }
        Value::Array(items) => items.iter().for_each(assert_no_blended_score_keys),
        _ => {}
    }
}

pub(crate) fn assert_format_contains(
    repo: &HistoryFixture,
    subcommand: &str,
    format: &str,
    needles: &[&str],
) {
    let out = repo.run(subcommand, &["--format", format]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "{subcommand} {format} stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    for needle in needles {
        assert!(
            stdout.contains(needle),
            "{subcommand} {format} missing {needle:?}: {stdout}"
        );
    }
}

pub(crate) fn assert_shallow_and_non_git(
    origin: &HistoryFixture,
    subcommand: &str,
    shallow_args: &[&str],
) {
    let shallow = origin.shallow_clone_of("2");
    let report = shallow.run_json(subcommand, shallow_args);
    assert_eq!(report["shallow"], true);
    assert_eq!(report["commits_analyzed"], 1);
    let warning = report["warning"].as_str().expect("shallow clone warning");
    assert!(warning.contains("shallow"), "warning: {warning}");
    assert!(warning.contains("--unshallow"), "warning: {warning}");

    let non_git = tempfile::tempdir().expect("non-git tempdir");
    let out = exec_subcommand(non_git.path(), subcommand, &[]);
    assert_eq!(out.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("not inside a Git repository"),
        "stderr: {stderr}"
    );
}

pub(crate) fn assert_unborn_repo_remedy(subcommand: &str) {
    let empty = HistoryFixture::init();
    let out = empty.run(subcommand, &[]);
    assert_eq!(out.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("Commit once, then run codeint again"),
        "stderr: {stderr}"
    );
    assert!(!stderr.contains("codeint trend"), "stderr: {stderr}");
}
