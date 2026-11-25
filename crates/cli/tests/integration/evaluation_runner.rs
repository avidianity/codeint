use std::collections::BTreeSet;
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
while IFS= read -r path; do
  case "$path" in
    app/calc.py) echo '{"path":"app/calc.py","functions":[[4,8,2],[20,22,1]],"imports":["os"]}' ;;
    app/broken.py) echo '{"path":"app/broken.py","error":"invalid syntax"}' ;;
    *) echo "{\"path\":\"$path\",\"functions\":[],\"imports\":[]}" ;;
  esac
done < "$1"
"#;

fn source_repository(files: &[(&str, &str)]) -> (TempDir, String) {
    let repo = init_git_repo(files);
    run_git(
        repo.path(),
        &["config", "uploadpack.allowAnySHA1InWant", "true"],
    );
    let output = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(repo.path())
        .output()
        .expect("git rev-parse");
    let commit = String::from_utf8(output.stdout)
        .expect("utf8")
        .trim()
        .to_owned();
    (repo, commit)
}

fn file_url(path: &Path) -> String {
    format!("file://{}", path.display())
}

fn scratch_folders() -> BTreeSet<PathBuf> {
    std::fs::read_dir(std::env::temp_dir())
        .expect("read temp folder")
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("codeint-eval-"))
        })
        .collect()
}

#[cfg(unix)]
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

#[cfg(unix)]
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
    let before = scratch_folders();

    let summary = run_evaluation(&EvaluationConfig {
        manifest,
        codeint: PathBuf::from(env!("CARGO_BIN_EXE_codeint")),
        output_dir: output_dir.clone(),
        codeint_repository: work.path().to_path_buf(),
        languages: Vec::new(),
        repositories: Vec::new(),
        runs: 3,
        audit_sample_size: 100,
        oracle_overrides: [("python".to_owned(), write_fake_oracle(work.path()))]
            .into_iter()
            .collect(),
    })
    .expect("evaluation runs");

    assert!(
        scratch_folders().is_subset(&before),
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
