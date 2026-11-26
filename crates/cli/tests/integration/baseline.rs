use std::fs;
use std::path::Path;

use serde_json::{Value, json};

use crate::common::{eroded_ts, finding_lines, init_git_repo, run_codeint_text, write_repo_files};

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
    assert_eq!(stored["schema_version"], 2);
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
    assert_baseline_reset_regenerates(r#"{"schema_version":3}"#, "schema_version");
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

const STRICT_HAZARDS_BUDGET: &str = r#"{ "budgets": { "production": { "hazards": 0 } } }"#;

fn assert_metric_baseline_state(
    report: &Value,
    metric: &str,
    expected_hidden: u64,
    expected_lines: &[u64],
) {
    let section = &report["source_sets"]["production"][metric];
    assert_eq!(section["baselined_findings"], expected_hidden);
    assert_eq!(finding_lines(&section["findings"]), expected_lines);
}

fn baselined_sql_repo(initial_sql: &str) -> tempfile::TempDir {
    let repo = init_git_repo(&[
        ("codeint.json", STRICT_HAZARDS_BUDGET),
        ("migrations/001.sql", initial_sql),
    ]);
    run_codeint_text(repo.path(), &["baseline", "update"], 0);
    repo
}

fn check_sql_edit(repo: &Path, updated_sql: &str, expected_code: i32, expected_lines: &[u64]) {
    write_repo_files(repo, &[("migrations/001.sql", updated_sql)]);
    let report = check_json(repo, &[], expected_code);
    assert_metric_baseline_state(&report, "hazards", 1, expected_lines);
}

fn eroded_anonymous_callback_ts(seed: usize) -> String {
    let branches: String = (0..11)
        .map(|index| format!("  if (value === {index}) {{ acc += {index}; }}\n"))
        .collect();
    format!(
        "handlers.push((value: number): number => {{\n  let acc = {seed};\n{branches}  return acc;\n}});\n"
    )
}

#[test]
fn inserting_an_identical_hazard_line_above_reports_the_new_line_and_hides_the_legacy_line() {
    let repo = baselined_sql_repo("SELECT 1;\nDELETE FROM users;\n");

    let raw_baseline = fs::read_to_string(repo.path().join(BASELINE_FILE)).expect("read Baseline");
    for forbidden in ["DELETE", "users", "SELECT", "migrations/001.sql"] {
        assert!(
            !raw_baseline.contains(forbidden),
            "Baseline v2 must not store raw source text {forbidden:?}:\n{raw_baseline}"
        );
    }
    assert_eq!(baseline_file(repo.path())["schema_version"], 2);

    check_sql_edit(
        repo.path(),
        "DELETE FROM users;\nSELECT 1;\nDELETE FROM users;\n",
        1,
        &[1],
    );

    let (updated, _) = run_codeint_text(repo.path(), &["baseline", "update"], 0);
    assert!(
        updated.contains("1 new Finding not added"),
        "expected update to skip the new line 1 Finding, got: {updated}"
    );
    let after_update = check_json(repo.path(), &[], 1);
    assert_metric_baseline_state(&after_update, "hazards", 1, &[1]);

    check_sql_edit(repo.path(), "SELECT 1;\nDELETE FROM users;\n", 0, &[]);
}

#[test]
fn inserting_an_adjacent_duplicate_hazard_line_hides_one_and_reports_one_new() {
    let repo = baselined_sql_repo("SELECT 1;\nDELETE FROM users;\n");
    check_sql_edit(
        repo.path(),
        "SELECT 1;\nDELETE FROM users;\nDELETE FROM users;\n",
        1,
        &[3],
    );
}

#[test]
fn editing_neighboring_lines_next_to_a_single_baselined_finding_still_hides_it() {
    let repo = baselined_sql_repo("SELECT 1;\nDELETE FROM users;\nSELECT 2;\n");
    check_sql_edit(
        repo.path(),
        "SELECT 100;\nDELETE FROM users;\nSELECT 200;\n",
        0,
        &[],
    );
}

#[test]
fn schema_version_1_baseline_still_hides_findings_and_upgrades_to_v2_on_update() {
    let repo = init_git_repo(&[
        ("codeint.json", STRICT_HAZARDS_BUDGET),
        ("migrations/001.sql", "SELECT 1;\nDELETE FROM users;\n"),
    ]);
    let initial = check_json(repo.path(), &[], 1);
    let legacy_id = initial["source_sets"]["production"]["hazards"]["findings"][0]["id"]
        .as_str()
        .expect("hazard finding id")
        .to_owned();

    let v1_baseline = json!({
        "schema_version": 1,
        "source_sets": {
            "production": {
                "hazards": {
                    "metric_version": 1,
                    "findings": [legacy_id]
                }
            }
        }
    });
    write_repo_files(
        repo.path(),
        &[(
            BASELINE_FILE,
            &serde_json::to_string_pretty(&v1_baseline).expect("serialize v1 Baseline"),
        )],
    );

    let with_v1 = check_json(repo.path(), &[], 0);
    assert_metric_baseline_state(&with_v1, "hazards", 1, &[]);

    run_codeint_text(repo.path(), &["baseline", "update"], 0);
    let upgraded = baseline_file(repo.path());
    assert_eq!(upgraded["schema_version"], 2);
    let upgraded_entry = &upgraded["source_sets"]["production"]["hazards"]["findings"][0];
    assert_eq!(upgraded_entry["id"], legacy_id);
    assert!(upgraded_entry["content"].is_string());
    assert!(upgraded_entry["before"].is_string());
    assert_eq!(upgraded_entry["line"], 2);

    check_sql_edit(
        repo.path(),
        "DELETE FROM users;\nSELECT 1;\nDELETE FROM users;\n",
        1,
        &[1],
    );
}

#[test]
fn inserting_an_identical_eroded_function_and_flagged_line_above_reports_the_new_lines() {
    let callback = eroded_anonymous_callback_ts(0);
    let legacy_verbose = "export function pickLegacy(flag: boolean): number {\n  if (flag) { return 1; } else { return 2; }\n}\n";
    let fresh_verbose = "export function pickFresh(flag: boolean): number {\n  if (flag) { return 1; } else { return 2; }\n}\n";
    let initial_erosion = format!(
        "const handlers: Array<(v: number) => number> = [];\nconst marker = 'separator';\n{callback}"
    );
    let repo = init_git_repo(&[
        (
            "codeint.json",
            r#"{ "budgets": { "production": { "erosion": 0, "verbosity": 0 } } }"#,
        ),
        ("src/eroded.ts", &initial_erosion),
        ("src/verbose.ts", legacy_verbose),
    ]);
    run_codeint_text(repo.path(), &["baseline", "update"], 0);
    check_json(repo.path(), &[], 0);

    let updated_erosion = format!(
        "const handlers: Array<(v: number) => number> = [];\n{callback}const marker = 'separator';\n{callback}"
    );
    let updated_verbosity = format!("{fresh_verbose}{legacy_verbose}");
    write_repo_files(
        repo.path(),
        &[
            ("src/eroded.ts", &updated_erosion),
            ("src/verbose.ts", &updated_verbosity),
        ],
    );
    let report = check_json(repo.path(), &[], 1);
    assert_metric_baseline_state(&report, "erosion", 1, &[2]);
    assert_metric_baseline_state(&report, "verbosity", 1, &[2]);
}
