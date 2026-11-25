use std::fs;
use std::path::Path;

use serde_json::{Value, json};

use crate::common::{eroded_ts, init_git_repo, run_codeint_text, write_repo_files};

const BASELINE_FILE: &str = ".codeint/baseline.json";
const STRICT_EROSION_BUDGET: &str = r#"{ "budgets": { "production": { "erosion": 0 } } }"#;

fn tidy_ts(name: &str) -> String {
    format!("export function {name}(input: number): number {{\n  return input * 2;\n}}\n")
}

fn check_json(root: &Path, extra: &[&str], expected_code: i32) -> Value {
    let mut args = vec!["check", "--format", "json"];
    args.extend_from_slice(extra);
    let (stdout, _) = run_codeint_text(root, &args, expected_code);
    serde_json::from_str(&stdout).expect("check prints JSON")
}

fn listed_erosion_paths(report: &Value) -> Vec<String> {
    report["source_sets"]["production"]["erosion"]["findings"]
        .as_array()
        .expect("erosion findings array")
        .iter()
        .map(|finding| finding["path"].as_str().expect("path").to_owned())
        .collect()
}

fn baseline_file(root: &Path) -> Value {
    let text = fs::read_to_string(root.join(BASELINE_FILE)).expect("read Baseline file");
    serde_json::from_str(&text).expect("Baseline file is JSON")
}

fn baselined_erosion_ids(root: &Path) -> Vec<Value> {
    baseline_file(root)["source_sets"]["production"]["erosion"]["findings"]
        .as_array()
        .expect("erosion Baseline findings")
        .clone()
}

fn legacy_repository() -> tempfile::TempDir {
    init_git_repo(&[
        ("codeint.json", STRICT_EROSION_BUDGET),
        ("src/legacy.ts", &eroded_ts("legacy", 12)),
        ("src/clean.ts", &tidy_ts("clean")),
    ])
}

#[test]
fn check_hides_baselined_findings_and_evaluates_budgets_on_new_findings_only() {
    let repo = legacy_repository();
    let before = check_json(repo.path(), &[], 1);
    assert_eq!(listed_erosion_paths(&before), vec!["src/legacy.ts"]);
    assert_eq!(before.get("baseline"), None);

    let (created, _) = run_codeint_text(repo.path(), &["baseline", "update"], 0);
    assert!(
        created.contains(BASELINE_FILE),
        "expected the create summary to name the Baseline file, got: {created}"
    );
    let stored = baseline_file(repo.path());
    assert_eq!(stored["schema_version"], 1);
    assert_eq!(
        stored["source_sets"]["production"]["erosion"]["metric_version"],
        1
    );
    assert_eq!(baselined_erosion_ids(repo.path()).len(), 1);
    assert_eq!(
        stored["source_sets"]["production"]["verbosity"]["metric_version"],
        2
    );

    let hidden = check_json(repo.path(), &[], 0);
    assert!(listed_erosion_paths(&hidden).is_empty());
    assert_eq!(
        hidden["source_sets"]["production"]["erosion"]["baselined_findings"],
        1
    );
    assert_eq!(hidden["baseline"]["path"], BASELINE_FILE);
    assert_eq!(hidden["baseline"]["hidden_findings"], 1);
    let measured_ratio = hidden["source_sets"]["production"]["erosion"]["ratio"]
        .as_f64()
        .expect("erosion ratio");
    assert!(measured_ratio > 0.0, "the measured ratio stays unchanged");

    let (markdown, _) = run_codeint_text(repo.path(), &["check", "--format", "markdown"], 0);
    assert!(
        markdown.contains("1 Finding hidden by the Baseline in .codeint/baseline.json"),
        "expected the Markdown summary to show the hidden count, got:\n{markdown}"
    );

    let (sarif, _) = run_codeint_text(repo.path(), &["check", "--format", "sarif"], 0);
    let sarif: Value = serde_json::from_str(&sarif).expect("check prints SARIF");
    assert_eq!(sarif["runs"][0]["properties"]["baselineHiddenFindings"], 1);

    write_repo_files(repo.path(), &[("src/fresh.ts", &eroded_ts("fresh", 11))]);
    let (failing, stderr) = run_codeint_text(repo.path(), &["check", "--format", "json"], 1);
    let failing: Value = serde_json::from_str(&failing).expect("check prints JSON");
    assert_eq!(listed_erosion_paths(&failing), vec!["src/fresh.ts"]);
    assert!(
        stderr.contains("non-baselined production erosion ratio"),
        "expected the Budget failure to name the non-baselined ratio, got: {stderr}"
    );

    let everything = check_json(repo.path(), &["--ignore-baseline"], 1);
    let mut all_paths = listed_erosion_paths(&everything);
    all_paths.sort();
    assert_eq!(all_paths, vec!["src/fresh.ts", "src/legacy.ts"]);
    assert_eq!(everything.get("baseline"), None);
    assert_eq!(
        everything["source_sets"]["production"]["erosion"].get("baselined_findings"),
        None
    );
}

#[test]
fn baseline_update_shrinks_monotonically_and_adds_findings_only_with_reset() {
    let repo = legacy_repository();
    run_codeint_text(repo.path(), &["baseline", "update"], 0);
    assert_eq!(baselined_erosion_ids(repo.path()).len(), 1);

    write_repo_files(repo.path(), &[("src/legacy.ts", &tidy_ts("legacy"))]);
    let (shrunk, _) = run_codeint_text(repo.path(), &["baseline", "update"], 0);
    assert!(
        shrunk.contains("1 removed"),
        "expected the update summary to count the fixed Finding, got: {shrunk}"
    );
    assert!(baselined_erosion_ids(repo.path()).is_empty());

    write_repo_files(repo.path(), &[("src/legacy.ts", &eroded_ts("legacy", 12))]);
    let (skipped, _) = run_codeint_text(repo.path(), &["baseline", "update"], 0);
    assert!(
        skipped.contains("1 new Finding not added") && skipped.contains("--reset"),
        "expected the update summary to name the skipped Finding and --reset, got: {skipped}"
    );
    assert!(baselined_erosion_ids(repo.path()).is_empty());
    check_json(repo.path(), &[], 1);

    let (reset, _) = run_codeint_text(
        repo.path(),
        &["baseline", "update", "--reset", "--format", "json"],
        0,
    );
    let summary: Value = serde_json::from_str(&reset).expect("update prints JSON");
    assert_eq!(summary["reset"], true);
    assert_eq!(summary["added"], 1);
    assert_eq!(baselined_erosion_ids(repo.path()).len(), 1);
    check_json(repo.path(), &[], 0);
}

#[test]
fn stale_metric_versions_fail_check_loudly_and_baseline_update_refreshes_them() {
    let repo = legacy_repository();
    run_codeint_text(repo.path(), &["baseline", "update"], 0);

    let mut stale = baseline_file(repo.path());
    stale["source_sets"]["production"]["erosion"] = json!({
        "metric_version": 99,
        "findings": []
    });
    fs::write(
        repo.path().join(BASELINE_FILE),
        serde_json::to_string_pretty(&stale).expect("serialize stale Baseline"),
    )
    .expect("write stale Baseline");

    let (_, stderr) = run_codeint_text(repo.path(), &["check"], 2);
    assert!(
        stderr
            .contains("production.erosion (baseline Metric version 99, current Metric version 1)")
            && stderr.contains("codeint baseline update"),
        "expected a loud mismatch naming the re-baseline command, got: {stderr}"
    );
    check_json(repo.path(), &["--ignore-baseline"], 1);

    let (refreshed, _) = run_codeint_text(repo.path(), &["baseline", "update"], 0);
    assert!(
        refreshed.contains("1 refreshed"),
        "expected the update summary to count the refreshed Metric, got: {refreshed}"
    );
    let stored = baseline_file(repo.path());
    assert_eq!(
        stored["source_sets"]["production"]["erosion"]["metric_version"],
        1
    );
    assert_eq!(baselined_erosion_ids(repo.path()).len(), 1);
    check_json(repo.path(), &[], 0);
}

#[test]
fn repository_without_baseline_file_reports_exactly_as_before() {
    let repo = legacy_repository();
    let (plain, _) = run_codeint_text(repo.path(), &["check", "--format", "json"], 1);
    let (ignoring, _) = run_codeint_text(
        repo.path(),
        &["check", "--format", "json", "--ignore-baseline"],
        1,
    );
    assert_eq!(plain, ignoring);
    assert!(!plain.contains("baseline"));
}

fn assert_baseline_reset_regenerates(corrupt: &str, needle: &str) {
    let repo = legacy_repository();
    write_repo_files(repo.path(), &[(BASELINE_FILE, corrupt)]);

    let (_, stderr) = run_codeint_text(repo.path(), &["baseline", "update"], 2);
    assert!(
        stderr.contains(needle) && stderr.contains("codeint baseline update --reset"),
        "expected the error to name the fault and --reset, got: {stderr}"
    );
    assert_eq!(
        fs::read_to_string(repo.path().join(BASELINE_FILE)).expect("read"),
        corrupt
    );

    let (stdout, _) = run_codeint_text(repo.path(), &["baseline", "update", "--reset"], 0);
    assert!(
        stdout.contains("Replaced invalid .codeint/baseline.json"),
        "expected the summary to say the invalid file was replaced, got: {stdout}"
    );
    assert_eq!(baselined_erosion_ids(repo.path()).len(), 1);
    check_json(repo.path(), &[], 0);
}

#[test]
fn reset_regenerates_a_baseline_file_that_is_not_json() {
    assert_baseline_reset_regenerates("{", "not valid JSON");
}

#[test]
fn reset_regenerates_a_baseline_file_with_an_unsupported_schema_version() {
    assert_baseline_reset_regenerates(r#"{"schema_version":2}"#, "schema_version");
}

#[test]
fn reset_regenerates_a_baseline_file_with_a_zero_metric_version() {
    assert_baseline_reset_regenerates(
        r#"{"schema_version":1,"source_sets":{"production":{"erosion":{"metric_version":0}}}}"#,
        "metric_version 0",
    );
}

#[test]
fn baseline_with_an_unknown_dependency_metric_key_is_rejected() {
    let repo = legacy_repository();
    write_repo_files(
        repo.path(),
        &[(
            BASELINE_FILE,
            r#"{"schema_version":1,"dependencies":{"mystery":{"metric_version":1}}}"#,
        )],
    );
    let (_, stderr) = run_codeint_text(repo.path(), &["check"], 2);
    assert!(
        stderr.contains("unknown dependency Metric \"mystery\"")
            && stderr.contains("unused_dependencies"),
        "expected the unknown key and the allowed keys, got: {stderr}"
    );
}

#[test]
fn a_metric_first_measured_after_the_baseline_exists_records_its_current_findings() {
    let lcov =
        "SF:src/legacy.ts\nDA:2,0\nDA:3,0\nend_of_record\nSF:src/clean.ts\nDA:2,1\nend_of_record\n";
    let repo = init_git_repo(&[
        (
            "codeint.json",
            r#"{ "budgets": { "production": { "test_gap": 0 } } }"#,
        ),
        ("src/legacy.ts", &tidy_ts("legacy")),
        ("src/clean.ts", &tidy_ts("clean")),
        ("lcov.info", lcov),
    ]);
    run_codeint_text(repo.path(), &["baseline", "update"], 0);
    assert!(
        baseline_file(repo.path())["source_sets"]["production"]
            .get("test_gap")
            .is_none()
    );

    let (updated, _) = run_codeint_text(
        repo.path(),
        &["baseline", "update", "--coverage-report", "lcov.info"],
        0,
    );
    assert!(
        !updated.contains("not added"),
        "expected no skipped Findings for a first-measured Metric, got: {updated}"
    );
    let stored = &baseline_file(repo.path())["source_sets"]["production"]["test_gap"];
    assert_eq!(stored["findings"].as_array().expect("findings").len(), 1);
    check_json(repo.path(), &["--coverage-report", "lcov.info"], 0);
}
