use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use codeint_eval::{
    AuditCategory, EvaluationConfig, EvaluationSummary, OracleStatus, RepositoryResult,
    run_evaluation,
};
use tempfile::TempDir;

use crate::common::{init_git_repo, run_git};

const PYTHON_CALC: &str = r#"import os


def classify(value):
    if value > 0 and value < 10:
        return "small"
    else:
        return os.sep
"#;

const PYTHON_BROKEN: &str = "def broken(:\n    return 1\n";

const PYTHON_UNCLOSED: &str = "def unclosed(value):\n    return [value,\n";

const SQL_REPORT: &str = "SELECT r.id FROM reports AS r WHERE r.id = 1 LIMIT 5;\n";

const FAKE_PYTHON_ORACLE: &str = r#"#!/bin/sh
tr '\0' '\n' < "$1" | while IFS= read -r path; do
  case "$path" in
    app/calc.py) echo '{"path":"app/calc.py","functions":[[4,8,2],[20,22,1]],"imports":["os"]}' ;;
    app/broken.py) echo '{"path":"app/broken.py","error":"invalid syntax"}' ;;
    *) echo "{\"path\":\"$path\",\"functions\":[],\"imports\":[]}" ;;
  esac
done
"#;

fn source_repository(files: &[(&str, &str)]) -> (TempDir, String) {
    let repo = init_git_repo(files);
    run_git(
        repo.path(),
        &["config", "uploadpack.allowAnySHA1InWant", "true"],
    );
    let commit = head_commit(repo.path());
    (repo, commit)
}

fn file_url(path: &Path) -> String {
    format!("file://{}", path.display())
}

fn write_fake_oracle(folder: &Path) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;

    let path = folder.join("fake-python-oracle.sh");
    std::fs::write(&path, FAKE_PYTHON_ORACLE).expect("write fake oracle");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
        .expect("make fake oracle executable");
    path
}

fn repository<'summary>(
    summary: &'summary EvaluationSummary,
    name: &str,
) -> &'summary RepositoryResult {
    summary
        .repositories
        .iter()
        .find(|result| result.name == name)
        .expect("repository result")
}

#[test]
fn evaluation_runner_measures_local_repositories_in_scratch_folders_and_writes_reports() {
    let (python_repo, python_commit) = source_repository(&[
        ("app/calc.py", PYTHON_CALC),
        ("app/broken.py", PYTHON_BROKEN),
        ("app/unclosed.py", PYTHON_UNCLOSED),
    ]);
    let (sql_repo, sql_commit) = source_repository(&[("db/report.sql", SQL_REPORT)]);
    let work = TempDir::new().expect("work folder");
    let manifest = work.path().join("corpus.json");
    let corpus = serde_json::json!({
        "measured_on": "2026-11-25",
        "repositories": [
            {"name": "local/python", "language": "python", "bucket": "small", "url": file_url(python_repo.path()), "commit": python_commit},
            {"name": "local/sql", "language": "sql", "bucket": "small", "url": file_url(sql_repo.path()), "commit": sql_commit},
            {"name": "local/missing", "language": "python", "bucket": "small", "url": file_url(python_repo.path()), "commit": "0123456789abcdef0123456789abcdef01234567"}
        ]
    });
    std::fs::write(&manifest, corpus.to_string()).expect("write manifest");
    let output_dir = work.path().join("docs");
    let scratch_root = TempDir::new().expect("scratch root");

    let summary = run_evaluation(&EvaluationConfig {
        manifest,
        codeint: PathBuf::from(env!("CARGO_BIN_EXE_codeint")),
        output_dir: output_dir.clone(),
        protected_repositories: vec![work.path().to_path_buf()],
        scratch_root: Some(scratch_root.path().to_path_buf()),
        languages: Vec::new(),
        repositories: Vec::new(),
        runs: 3,
        audit_sample_size: 100,
        oracle_overrides: [("python".to_owned(), write_fake_oracle(work.path()))]
            .into_iter()
            .collect(),
    })
    .expect("evaluation runs");

    assert_eq!(
        std::fs::read_dir(scratch_root.path())
            .expect("read scratch root")
            .count(),
        0,
        "scratch folders are removed"
    );
    assert!(!summary.passed, "{summary:?}");

    let python = repository(&summary, "local/python");
    assert_eq!(python.commit, python_commit);
    assert_eq!(python.exit_code, Some(0), "{python:?}");
    assert_eq!(python.wall_seconds.len(), 3);
    assert!(
        python
            .median_wall_seconds
            .is_some_and(|median| median > 0.0)
    );
    assert_eq!(python.production_lines, Some(6));
    assert_eq!(python.measured_bucket, None);
    assert_eq!(
        python.peak_rss_bytes.is_some(),
        Path::new("/usr/bin/time").exists(),
        "{python:?}"
    );
    let recall = python.oracle.recall.as_ref().expect("python recall");
    assert_eq!(python.oracle.status, OracleStatus::Measured);
    assert_eq!((recall.functions.matched, recall.functions.total), (1, 2));
    assert_eq!(
        (recall.decision_points.matched, recall.decision_points.total),
        (2, 3)
    );
    assert_eq!((recall.imports.matched, recall.imports.total), (1, 1));
    assert_eq!(
        recall.oracle_unparsable_files,
        vec!["app/broken.py".to_owned()]
    );
    assert_eq!(
        recall.unexpected_unparsable_files,
        vec!["app/unclosed.py".to_owned()]
    );
    assert!(!python.passed);

    let sql = repository(&summary, "local/sql");
    assert_eq!(sql.oracle.status, OracleStatus::NotApplicable);
    assert!(sql.passed, "{sql:?}");

    let missing = repository(&summary, "local/missing");
    assert!(
        missing
            .error
            .as_deref()
            .is_some_and(|error| error.contains("fetch")),
        "{missing:?}"
    );
    assert!(!missing.passed);

    let flagged = summary
        .audit
        .iter()
        .find(|sample| sample.category == AuditCategory::FlaggedLines)
        .expect("flagged line sample");
    let mut flagged_items = flagged.items.clone();
    flagged_items.sort();
    assert_eq!(
        flagged_items,
        vec![
            "local/python app/calc.py:7 else-after-return".to_owned(),
            "local/python app/calc.py:8 else-after-return".to_owned(),
        ]
    );
    assert_eq!(flagged.population, 2);
    assert_eq!(summary.audit.len(), 3);

    let written: EvaluationSummary = serde_json::from_str(
        &std::fs::read_to_string(output_dir.join("evaluation-summary.json")).expect("summary file"),
    )
    .expect("summary parses");
    assert_eq!(written, summary);
    let markdown =
        std::fs::read_to_string(output_dir.join("evaluation.md")).expect("markdown report");
    assert!(markdown.starts_with("# codeint Evaluation\n"), "{markdown}");
    assert!(!markdown.contains("\n\n\n"), "{markdown}");
    assert!(
        markdown.ends_with('\n') && !markdown.ends_with("\n\n"),
        "{markdown}"
    );
    for name in ["local/python", "local/sql", "local/missing"] {
        assert!(markdown.contains(name), "{markdown}");
    }
}

const FAKE_PHP_ORACLE: &str = r#"#!/bin/sh
tr '\0' '\n' < "$1" | while IFS= read -r path; do
  echo "{\"path\":\"$path\",\"functions\":[[3,5,1]],\"imports\":[]}"
done
"#;

#[test]
fn evaluation_runner_counts_files_codeint_did_not_measure_as_recall_misses() {
    use std::os::unix::fs::PermissionsExt;

    let repo = init_git_repo(&[("README.md", "# latin\n")]);
    let latin = b"<?php\n\nfunction caf\xe9($value) {\n    return $value;\n}\n";
    std::fs::write(repo.path().join("Latin.php"), latin).expect("write latin-1 file");
    let commit = commit_all(repo.path());
    let work = TempDir::new().expect("work folder");
    let oracle = work.path().join("fake-php-oracle.sh");
    std::fs::write(&oracle, FAKE_PHP_ORACLE).expect("write fake oracle");
    std::fs::set_permissions(&oracle, std::fs::Permissions::from_mode(0o755))
        .expect("make fake oracle executable");

    let latin = evaluate_one(
        repo.path(),
        &commit,
        "php",
        [("php".to_owned(), oracle)].into_iter().collect(),
    );

    let recall = latin.oracle.recall.as_ref().expect("php recall");
    assert_eq!((recall.functions.matched, recall.functions.total), (0, 1));
    assert_eq!(recall.unmeasured_files, vec!["Latin.php".to_owned()]);
    assert!(!latin.passed, "{latin:?}");
}

fn python_is_new_enough() -> bool {
    Command::new("python3")
        .args(["-c", "import sys; assert sys.version_info >= (3, 12)"])
        .output()
        .is_ok_and(|output| output.status.success())
}

fn head_commit(repo: &Path) -> String {
    let output = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(repo)
        .output()
        .expect("git rev-parse");
    String::from_utf8(output.stdout)
        .expect("utf8")
        .trim()
        .to_owned()
}

fn commit_all(repo: &Path) -> String {
    run_git(repo, &["add", "."]);
    run_git(
        repo,
        &[
            "commit",
            "--quiet",
            "--allow-empty",
            "-m",
            "add special files",
        ],
    );
    run_git(repo, &["config", "uploadpack.allowAnySHA1InWant", "true"]);
    head_commit(repo)
}

fn evaluate_one(
    repo: &Path,
    commit: &str,
    language: &str,
    oracle_overrides: BTreeMap<String, PathBuf>,
) -> RepositoryResult {
    let work = TempDir::new().expect("work folder");
    let manifest = work.path().join("corpus.json");
    let corpus = serde_json::json!({
        "measured_on": "2026-11-25",
        "repositories": [
            {"name": "local/special", "language": language, "bucket": "small", "url": file_url(repo), "commit": commit}
        ]
    });
    std::fs::write(&manifest, corpus.to_string()).expect("write manifest");
    let summary = run_evaluation(&EvaluationConfig {
        manifest,
        codeint: PathBuf::from(env!("CARGO_BIN_EXE_codeint")),
        output_dir: work.path().join("docs"),
        protected_repositories: vec![work.path().to_path_buf()],
        scratch_root: None,
        languages: Vec::new(),
        repositories: Vec::new(),
        runs: 1,
        audit_sample_size: 10,
        oracle_overrides,
    })
    .expect("evaluation runs");
    repository(&summary, "local/special").clone()
}

fn evaluate_python_files(
    files: &[(&str, &[u8])],
    links: &[(&str, &str)],
) -> Option<RepositoryResult> {
    if !python_is_new_enough() {
        eprintln!("SKIPPED: python3 3.12 or newer is not available");
        return None;
    }
    let repo = init_git_repo(&[("README.md", "# special files\n")]);
    for (path, bytes) in files {
        std::fs::write(repo.path().join(path), bytes).expect("write file");
    }
    for (link, target) in links {
        std::os::unix::fs::symlink(target, repo.path().join(link)).expect("symlink");
    }
    let commit = commit_all(repo.path());
    Some(evaluate_one(
        repo.path(),
        &commit,
        "python",
        BTreeMap::new(),
    ))
}

const PYTHON_ID: &[u8] = b"def f(x):\n    return x\n";

#[test]
fn evaluation_runner_counts_a_tracked_symlink_as_unmeasured_and_keeps_it_out_of_the_audit_sample() {
    let Some(result) = evaluate_python_files(&[("app.py", PYTHON_ID)], &[("link.py", "app.py")])
    else {
        return;
    };

    let recall = result.oracle.recall.as_ref().expect("python recall");
    assert_eq!(recall.unmeasured_files, vec!["link.py".to_owned()]);
    assert_eq!((recall.functions.matched, recall.functions.total), (1, 2));
    assert!(!result.passed, "{result:?}");
}

#[test]
fn evaluation_runner_counts_a_non_utf8_python_file_as_unmeasured_not_oracle_unparsable() {
    let latin: &[u8] = b"# -*- coding: latin-1 -*-\n\n\ndef g():\n    return \"caf\xe9\"\n";
    let Some(result) = evaluate_python_files(&[("ok.py", PYTHON_ID), ("latin.py", latin)], &[])
    else {
        return;
    };

    let recall = result.oracle.recall.as_ref().expect("python recall");
    assert!(recall.oracle_unparsable_files.is_empty(), "{recall:?}");
    assert_eq!(recall.unmeasured_files, vec!["latin.py".to_owned()]);
    assert_eq!((recall.functions.matched, recall.functions.total), (1, 2));
}

#[test]
fn evaluation_runner_compares_a_tracked_file_whose_name_contains_a_newline() {
    let Some(result) =
        evaluate_python_files(&[("ok.py", PYTHON_ID), ("odd\nname.py", PYTHON_ID)], &[])
    else {
        return;
    };

    let recall = result.oracle.recall.as_ref().expect("python recall");
    assert_eq!(recall.functions.total, 2, "{recall:?}");
    assert_eq!(
        recall.files_compared + recall.unmeasured_files.len() as u64,
        2,
        "{recall:?}"
    );
}

#[test]
fn evaluation_runner_scores_identical_functions_that_share_a_line_span_at_full_recall() {
    let shared: &[u8] = b"def outer(x): return lambda y: y if y else x\n";
    let Some(result) = evaluate_python_files(&[("shared.py", shared)], &[]) else {
        return;
    };

    let recall = result.oracle.recall.as_ref().expect("python recall");
    assert_eq!((recall.functions.matched, recall.functions.total), (2, 2));
    assert_eq!(
        (recall.decision_points.matched, recall.decision_points.total),
        (1, 1)
    );
    assert!(result.passed, "{result:?}");
}
