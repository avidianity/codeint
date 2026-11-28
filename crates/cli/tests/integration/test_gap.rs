use std::fs;
use std::path::Path;
use std::process::{Command, Output};

use serde_json::{Value, json};
use tempfile::TempDir;

use crate::common::{
    init_git_repo, mcp_payload, mcp_tool_call, send_mcp_requests, write_repo_files,
};

fn write_file(root: &Path, relative: &str, content: &str) {
    write_repo_files(root, &[(relative, content)]);
}

fn write_config(root: &Path, content: &str) {
    write_file(root, "codeint.json", content);
}

fn write_coverage_config(root: &Path, reports: &[&str], budget: Option<f64>) {
    let mut doc = json!({ "coverage_reports": reports });
    if let Some(limit) = budget {
        doc["budgets"] = json!({ "production": { "test_gap": limit } });
    }
    write_config(root, &doc.to_string());
}

fn has_no_prod_test_gap(section: &Value) -> bool {
    section["source_sets"]["production"]
        .get("test_gap")
        .is_none()
}

fn run_codeint(dir: &Path, args: &[&str], cache: Option<&Path>) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_codeint"));
    command.args(args).current_dir(dir);
    if let Some(cache_dir) = cache {
        command.env("CODEINT_CACHE_DIR", cache_dir);
    }
    command.output().expect("run codeint")
}

fn run_stdout(dir: &Path, args: &[&str], cache: Option<&Path>, expected_code: i32) -> String {
    let output = run_codeint(dir, args, cache);
    assert_eq!(
        output.status.code(),
        Some(expected_code),
        "codeint {args:?} stderr:\n{}",
        String::from_utf8_lossy(&output.stderr),
    );
    String::from_utf8(output.stdout).expect("UTF-8 stdout")
}

fn run_json(
    dir: &Path,
    args: &[&str],
    cache: Option<&Path>,
    expected_code: i32,
) -> (Value, String) {
    let output = run_codeint(dir, args, cache);
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    assert_eq!(
        output.status.code(),
        Some(expected_code),
        "codeint {args:?} stdout:\n{}\nstderr:\n{stderr}",
        String::from_utf8_lossy(&output.stdout),
    );
    let parsed: Value = serde_json::from_slice(&output.stdout).expect("valid JSON output");
    (parsed, stderr)
}

fn check_json(dir: &Path, extra_args: &[&str], expected_code: i32) -> (Value, String) {
    let mut args = vec!["check", "--format", "json"];
    args.extend_from_slice(extra_args);
    run_json(dir, &args, None, expected_code)
}

fn prod_test_gap(report: &Value) -> &Value {
    &report["source_sets"]["production"]["test_gap"]
}

fn assert_prod_gap_ratio(report: &Value, expected: f64) {
    assert_eq!(prod_test_gap(report)["ratio"], json!(expected));
}

fn assert_lang_gap_ratio(report: &Value, language: &str, expected: f64) {
    assert_eq!(
        report["languages"][language]["source_sets"]["production"]["test_gap"]["ratio"],
        json!(expected)
    );
}

fn sarif_test_gap_results(dir: &Path) -> Vec<Value> {
    let text = run_stdout(dir, &["check", "--format", "sarif"], None, 0);
    let sarif: Value = serde_json::from_str(&text).expect("valid SARIF");
    sarif["runs"][0]["results"]
        .as_array()
        .expect("sarif results")
        .iter()
        .filter(|result| result["ruleId"] == "test_gap")
        .cloned()
        .collect()
}

fn assert_exit_2_contains(dir: &Path, args: &[&str], expected_substrings: &[&str]) {
    let output = run_codeint(dir, args, None);
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    assert_eq!(
        output.status.code(),
        Some(2),
        "expected exit 2 for {args:?}, got {:?}\nstdout:\n{}\nstderr:\n{stderr}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
    );
    for needle in expected_substrings {
        assert!(
            stderr.contains(needle),
            "expected {needle:?} in stderr for {args:?}, got:\n{stderr}"
        );
    }
}

fn run_trend_and_assert_cache(dir: &Path, cache: &Path, expected_entries: usize) {
    let _ = run_json(dir, &["trend", "--format", "json"], Some(cache), 0);
    let count = fs::read_dir(cache).expect("read cache dir").count();
    assert_eq!(count, expected_entries);
}

fn mcp_check(dir: &Path, id: u64) -> Value {
    let responses = send_mcp_requests(dir, &[mcp_tool_call(id, "check", json!({}))]);
    responses.into_iter().next().expect("single MCP response")
}

const SAMPLE_TWO_FUNCTIONS_TS: &str = concat!(
    "export function light(x: number): number {\n",
    "  return x + 1;\n",
    "}\n",
    "\n",
    "export function heavy(a: number, b: number): number {\n",
    "  if (a > 0) {\n",
    "    if (b > 0) {\n",
    "      return a + b;\n",
    "    }\n",
    "    return a;\n",
    "  }\n",
    "  return b;\n",
    "}\n",
);

const SAMPLE_NO_EXECUTABLE_LINES_TS: &str = concat!(
    "export function emptyBody(): void {\n",
    "}\n",
    "\n",
    "export function covered(v: number): number {\n",
    "  return v * 2;\n",
    "}\n",
);

#[test]
fn lcov_end_to_end_measures_production_only_orders_heaviest_first_and_handles_uncovered_and_unlisted_functions()
 {
    let repo = init_git_repo(&[
        ("src/main.ts", SAMPLE_TWO_FUNCTIONS_TS),
        ("src/empty.ts", SAMPLE_NO_EXECUTABLE_LINES_TS),
        (
            "src/unloaded.ts",
            "export function neverLoaded(x: number): number {\n  if (x > 1) {\n    return x;\n  }\n  return 0;\n}\n",
        ),
        (
            "src/main.test.ts",
            "export function testHelper(): number {\n  return 42;\n}\n",
        ),
        (
            "coverage/lcov.info",
            concat!(
                "TN:unit\n",
                "SF:src/main.ts\n",
                "FN:1,light\n",
                "FNDA:1,light\n",
                "DA:2,1\n",
                "DA:6,1\n",
                "DA:7,0\n",
                "DA:8,0\n",
                "DA:10,0\n",
                "DA:12,1\n",
                "BRDA:6,0,0,1\n",
                "end_of_record\n",
                "SF:src/empty.ts\n",
                "DA:5,1\n",
                "end_of_record\n",
                "SF:src/main.test.ts\n",
                "DA:2,0\n",
                "end_of_record\n",
            ),
        ),
        (
            "codeint.jsonc",
            "{\n  // LCOV coverage report\n  \"coverage_reports\": [\"coverage/lcov.info\"],\n}\n",
        ),
    ]);

    let (report, _) = check_json(repo.path(), &[], 0);
    assert!(report.get("test_gap_note").is_none());
    assert!(report.get("coverage_unmatched_note").is_none());
    assert!(report["source_sets"]["test"].get("test_gap").is_none());

    let gap = prod_test_gap(&report);
    assert_eq!(gap["measured_functions"], 4);
    let ratio = gap["ratio"].as_f64().expect("ratio number");
    assert!(ratio > 0.5 && ratio < 1.0, "unexpected ratio {ratio}");

    let findings = gap["findings"].as_array().expect("findings array");
    assert_eq!(findings.len(), 2);
    assert_eq!(findings[0]["name"], "heavy");
    assert_eq!(findings[0]["path"], "src/main.ts");
    assert_eq!(findings[0]["line"], 5);
    assert_eq!(findings[0]["untested_share"], 0.6);
    assert_eq!(findings[1]["name"], "neverLoaded");
    assert_eq!(findings[1]["path"], "src/unloaded.ts");
    assert_eq!(findings[1]["line"], 1);
    assert_eq!(findings[1]["untested_share"], 1.0);
    let weight = |row: &Value| {
        row["function_mass"].as_f64().unwrap() * row["untested_share"].as_f64().unwrap()
    };
    assert!(weight(&findings[0]) >= weight(&findings[1]));

    let md = run_stdout(repo.path(), &["check", "--format", "markdown"], None, 0);
    assert!(md.contains("| Production | Test gap |"), "{md}");
    assert!(md.contains("### Test gap"), "{md}");
    assert!(
        md.contains("| Function | Function mass | Untested share |"),
        "{md}"
    );
    let heavy_pos = md.find("`heavy` (`src/main.ts:5`)").expect("heavy row");
    let unloaded_pos = md
        .find("`neverLoaded` (`src/unloaded.ts:1`)")
        .expect("neverLoaded row");
    assert!(heavy_pos < unloaded_pos);

    let gap_results = sarif_test_gap_results(repo.path());
    assert_eq!(gap_results.len(), 2);
    assert_eq!(
        gap_results[0]["locations"][0]["physicalLocation"]["region"]["startLine"],
        5
    );
    let msg = gap_results[0]["message"]["text"].as_str().unwrap();
    assert!(
        msg.contains("Function mass") && msg.contains("untested share 0.6"),
        "{msg}"
    );

    let toon = run_stdout(repo.path(), &["check", "--format", "toon"], None, 0);
    assert!(toon.contains("test_gap:"), "{toon}");

    let saved_path = repo.path().join("saved-check.json");
    fs::write(&saved_path, serde_json::to_vec_pretty(&report).unwrap()).unwrap();
    for format in ["json", "toon", "markdown", "sarif"] {
        let _ = run_stdout(
            repo.path(),
            &["report", "saved-check.json", "--format", format],
            None,
            0,
        );
    }
}

#[test]
fn cli_coverage_report_replaces_config_list_and_merges_multiple_reports() {
    let repo = init_git_repo(&[
        ("src/main.ts", SAMPLE_TWO_FUNCTIONS_TS),
        (
            "codeint.json",
            r#"{ "coverage_reports": ["coverage/missing-config-only.info"] }"#,
        ),
        (
            "reports/unit.info",
            "SF:src/main.ts\nDA:2,1\nDA:6,1\nDA:7,0\nDA:8,0\nDA:10,0\nDA:12,0\nend_of_record\n",
        ),
        (
            "reports/e2e.info",
            "SF:src/main.ts\nDA:2,0\nDA:6,0\nDA:7,1\nDA:8,1\nDA:10,1\nDA:12,1\nend_of_record\n",
        ),
    ]);

    assert_exit_2_contains(
        repo.path(),
        &["check"],
        &["coverage/missing-config-only.info", "does not exist"],
    );

    let (report, _) = check_json(
        repo.path(),
        &[
            "--coverage-report",
            "reports/unit.info",
            "--coverage-report",
            "reports/e2e.info",
        ],
        0,
    );
    assert_prod_gap_ratio(&report, 0.0);
    assert_eq!(prod_test_gap(&report)["findings"], json!([]));
}

#[test]
fn absent_test_gap_emits_one_line_reason_and_zero_sarif_test_gap_results() {
    let repo = init_git_repo(&[("src/main.ts", SAMPLE_TWO_FUNCTIONS_TS)]);

    let (report, _) = check_json(repo.path(), &[], 0);
    assert!(has_no_prod_test_gap(&report));
    let note = report["test_gap_note"].as_str().expect("test_gap_note");
    assert!(!note.contains('\n') && note.contains("no Coverage report"));
    assert!(sarif_test_gap_results(repo.path()).is_empty());
}

#[test]
fn multi_language_and_multi_package_repo_reports_test_gap_per_language_and_per_package() {
    let repo = init_git_repo(&[
        (
            "package.json",
            r#"{ "private": true, "workspaces": ["packages/*"] }"#,
        ),
        (
            "packages/web/package.json",
            r#"{ "name": "web", "version": "1.0.0" }"#,
        ),
        (
            "packages/web/src/view.ts",
            "export function render(x: number): number {\n  return x + 1;\n}\n",
        ),
        (
            "packages/api/package.json",
            r#"{ "name": "api", "version": "1.0.0" }"#,
        ),
        (
            "packages/api/src/handler.ts",
            "export function handle(x: number): number {\n  return x + 2;\n}\n",
        ),
        (
            "Cargo.toml",
            "[workspace]\nmembers = [\"crates/core\"]\nresolver = \"2\"\n",
        ),
        (
            "crates/core/Cargo.toml",
            "[package]\nname = \"core-lib\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
        ),
        (
            "crates/core/src/lib.rs",
            "pub fn compute(x: i32) -> i32 {\n    x + 1\n}\n",
        ),
        (
            "coverage/ts.info",
            "SF:packages/web/src/view.ts\nDA:2,1\nend_of_record\nSF:packages/api/src/handler.ts\nDA:2,0\nend_of_record\n",
        ),
    ]);
    write_coverage_config(repo.path(), &["coverage/ts.info"], None);

    let (report, _) = check_json(repo.path(), &[], 0);
    assert!(report.get("test_gap_note").is_none());
    assert_prod_gap_ratio(&report, 0.5);
    assert_lang_gap_ratio(&report, "typescript-javascript", 0.5);
    assert!(has_no_prod_test_gap(&report["languages"]["rust"]));
    assert!(
        report["languages"]["rust"]["test_gap_note"]
            .as_str()
            .unwrap()
            .contains("no named Coverage report covers this language")
    );

    let packages = report["source_sets"]["production"]["packages"]
        .as_array()
        .expect("packages");
    let find_pkg = |path: &str| {
        packages
            .iter()
            .find(|pkg| pkg["path"] == path)
            .unwrap_or_else(|| panic!("missing package {path}"))
    };
    assert_eq!(find_pkg("packages/web")["test_gap"]["ratio"], 0.0);
    assert_eq!(find_pkg("packages/api")["test_gap"]["ratio"], 1.0);
    assert!(find_pkg("crates/core").get("test_gap").is_none());

    let md = run_stdout(repo.path(), &["check", "--format", "markdown"], None, 0);
    assert!(md.contains("| Test gap |"), "{md}");
    assert!(
        md.contains(
            "Test gap is not measured because no named Coverage report covers this language."
        ),
        "{md}"
    );
}

#[test]
fn path_matching_resolves_absolute_relative_and_package_relative_and_reports_unmatched_and_ambiguous_paths()
 {
    let repo = init_git_repo(&[
        (
            "packages/a/src/util.ts",
            "export function fromA(): number {\n  return 1;\n}\n",
        ),
        (
            "packages/b/src/util.ts",
            "export function fromB(): number {\n  return 2;\n}\n",
        ),
        (
            "packages/a/src/unique.ts",
            "export function uniqueFn(): number {\n  return 3;\n}\n",
        ),
    ]);
    let abs_a_util = repo.path().join("packages/a/src/util.ts");
    write_file(
        repo.path(),
        "coverage/lcov.info",
        &format!(
            concat!(
                "SF:{}\n",
                "DA:2,1\n",
                "end_of_record\n",
                "SF:src/unique.ts\n",
                "DA:2,1\n",
                "end_of_record\n",
                "SF:src/util.ts\n",
                "DA:2,1\n",
                "end_of_record\n",
                "SF:src/does-not-exist.ts\n",
                "DA:1,1\n",
                "end_of_record\n",
            ),
            abs_a_util.display()
        ),
    );

    let (report, _) = check_json(repo.path(), &["--coverage-report", "coverage/lcov.info"], 0);
    let note = report["coverage_unmatched_note"]
        .as_str()
        .expect("unmatched note");
    assert!(note.contains("src/does-not-exist.ts"), "{note}");
    assert!(note.contains("src/util.ts"), "{note}");
    assert!(!note.contains("unique.ts"), "{note}");
    let findings = prod_test_gap(&report)["findings"].as_array().unwrap();
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0]["path"], "packages/b/src/util.ts");

    let md = run_stdout(
        repo.path(),
        &[
            "check",
            "--format",
            "markdown",
            "--coverage-report",
            "coverage/lcov.info",
        ],
        None,
        0,
    );
    assert!(md.contains("src/does-not-exist.ts") && md.contains("src/util.ts"));
}

#[test]
fn line_beyond_eof_exits_2_naming_report_file_and_line_while_last_line_is_accepted() {
    let repo = init_git_repo(&[(
        "src/app.ts",
        "export function alpha(x: number): number {\n  return x + 1;\n}",
    )]);
    write_file(
        repo.path(),
        "coverage/ok.info",
        "SF:src/app.ts\nDA:3,1\nend_of_record\n",
    );
    let (ok_report, _) = check_json(repo.path(), &["--coverage-report", "coverage/ok.info"], 0);
    assert_prod_gap_ratio(&ok_report, 0.0);

    write_file(
        repo.path(),
        "coverage/stale.info",
        "SF:src/app.ts\nDA:4,1\nend_of_record\n",
    );
    assert_exit_2_contains(
        repo.path(),
        &["check", "--coverage-report", "coverage/stale.info"],
        &["coverage/stale.info", "src/app.ts", "4"],
    );
}

#[test]
fn cobertura_xml_detection_by_content_source_roots_and_mixing_with_lcov() {
    let repo = init_git_repo(&[
        (
            "services/billing/src/calc.py",
            "def total(x):\n    if x > 0:\n        return x * 2\n    return 0\n",
        ),
        (
            "services/auth/src/check.php",
            "<?php\nfunction allow($ok) {\n    return $ok ? 1 : 0;\n}\n",
        ),
        (
            "reports/cobertura.dat",
            r#"<?xml version="1.0"?>
<coverage version="1.0">
  <sources>
    <source>/external/outside/repo</source>
    <source>services/billing</source>
    <source>services/auth</source>
  </sources>
  <packages>
    <package name="all">
      <classes>
        <class name="calc" filename="src/calc.py">
          <lines>
            <line number="2" hits="1"/>
            <line number="3" hits="0"/>
            <line number="4" hits="0"/>
          </lines>
        </class>
        <class name="check" filename="src/check.php">
          <lines>
            <line number="3" hits="1"/>
          </lines>
        </class>
      </classes>
    </package>
  </packages>
</coverage>
"#,
        ),
        (
            "reports/extra.txt",
            "SF:services/billing/src/calc.py\nDA:3,1\nDA:4,1\nend_of_record\n",
        ),
    ]);
    write_coverage_config(
        repo.path(),
        &["reports/cobertura.dat", "reports/extra.txt"],
        None,
    );

    let (report, _) = check_json(repo.path(), &[], 0);
    assert!(report.get("coverage_unmatched_note").is_none());
    assert_prod_gap_ratio(&report, 0.0);
    assert_lang_gap_ratio(&report, "python", 0.0);
    assert_lang_gap_ratio(&report, "php", 0.0);
}

#[test]
fn invalid_unrecognized_and_hostile_coverage_reports_exit_2_naming_the_report() {
    let repo = init_git_repo(&[("src/app.ts", SAMPLE_TWO_FUNCTIONS_TS)]);
    let mut deep = String::from("<coverage>");
    deep.push_str(&"<p>".repeat(70));
    deep.push_str(&"</p>".repeat(70));
    deep.push_str("</coverage>");

    let cases: [(&str, &str, &[&str]); 6] = [
        (
            "reports/unknown.cov",
            "hello world\n",
            &["reports/unknown.cov", "LCOV", "Cobertura XML"],
        ),
        (
            "reports/bad.lcov",
            "SF:src/app.ts\nBADREC:1\n",
            &["reports/bad.lcov"],
        ),
        (
            "reports/broken.xml",
            "<coverage><packages>",
            &["reports/broken.xml"],
        ),
        (
            "reports/xxe.xml",
            "<?xml version=\"1.0\"?><!DOCTYPE coverage [<!ENTITY xxe SYSTEM \"file:///etc/passwd\">]><coverage>&xxe;</coverage>",
            &["reports/xxe.xml"],
        ),
        (
            "reports/dtd.xml",
            "<?xml version=\"1.0\"?><!DOCTYPE coverage SYSTEM \"http://127.0.0.1:9/cobertura.dtd\"><coverage/>",
            &["reports/dtd.xml"],
        ),
        ("reports/deep.xml", &deep, &["reports/deep.xml"]),
    ];
    for (report_path, body, expected) in cases {
        write_file(repo.path(), report_path, body);
        assert_exit_2_contains(
            repo.path(),
            &["check", "--coverage-report", report_path],
            expected,
        );
    }

    let oversized = repo.path().join("reports/huge.xml");
    let file = fs::File::create(&oversized).expect("create huge file");
    file.set_len(64 * 1024 * 1024 + 1)
        .expect("set oversized sparse length");
    drop(file);
    assert_exit_2_contains(
        repo.path(),
        &["check", "--coverage-report", "reports/huge.xml"],
        &["reports/huge.xml"],
    );
}

#[test]
fn test_gap_budget_passes_under_and_equal_fails_over_with_full_report_and_rejects_invalid_configs()
{
    let repo = init_git_repo(&[
        (
            "src/app.ts",
            "export function a(x: number): number {\n  return x + 1;\n}\nexport function b(x: number): number {\n  return x + 2;\n}\n",
        ),
        (
            "coverage/lcov.info",
            "SF:src/app.ts\nDA:2,1\nDA:5,0\nend_of_record\n",
        ),
    ]);

    for budget in [0.6, 0.5] {
        write_coverage_config(repo.path(), &["coverage/lcov.info"], Some(budget));
        let (passed, _) = check_json(repo.path(), &[], 0);
        assert_prod_gap_ratio(&passed, 0.5);
    }

    write_coverage_config(repo.path(), &["coverage/lcov.info"], Some(0.4));
    let (over, stderr) = check_json(repo.path(), &[], 1);
    assert_eq!(over["over_budget"][0]["source_set"], "production");
    assert_eq!(over["over_budget"][0]["metric"], "test_gap");
    assert_eq!(over["over_budget"][0]["ratio"], 0.5);
    assert_eq!(over["over_budget"][0]["budget"], 0.4);
    assert_eq!(
        prod_test_gap(&over)["findings"].as_array().map(Vec::len),
        Some(1)
    );
    assert!(
        stderr.contains("production test_gap ratio 0.5 is over its Budget 0.4"),
        "{stderr}"
    );

    let invalid_configs: [(&str, &[&str]); 6] = [
        (
            r#"{ "budgets": { "production": { "test_gap": 0.4 } } }"#,
            &["budgets.production.test_gap", "needs a Coverage report"],
        ),
        (
            r#"{ "budgets": { "test": { "test_gap": 0.2 } } }"#,
            &["budgets.test.test_gap"],
        ),
        (
            r#"{ "budgets": { "generated": { "test_gap": 0.2 } } }"#,
            &["budgets.generated.test_gap"],
        ),
        (
            r#"{ "budgets": { "vendored": { "test_gap": 0.2 } } }"#,
            &["budgets.vendored.test_gap"],
        ),
        (
            r#"{ "regression_limits": { "test_gap": { "ratio_rise": 0.05 } } }"#,
            &["regression_limits.test_gap"],
        ),
        (
            r#"{ "regression_limits": { "production": { "test_gap": { "ratio_rise": 0.05 } } } }"#,
            &["regression_limits.production.test_gap"],
        ),
    ];
    for (config, expected) in invalid_configs {
        write_config(repo.path(), config);
        assert_exit_2_contains(repo.path(), &["check"], expected);
    }
}

#[test]
fn diff_trend_and_result_cache_handle_coverage_reports_and_emit_one_line_note_only_when_named() {
    let repo = init_git_repo(&[
        (
            "src/app.ts",
            "export function alpha(x: number): number {\n  return x + 1;\n}\n",
        ),
        (
            "coverage/lcov.info",
            "SF:src/app.ts\nDA:2,0\nend_of_record\n",
        ),
    ]);
    let base_cache = TempDir::new().expect("cache dir");
    for args in [
        &["diff", "main", "--format", "json"][..],
        &["trend", "--format", "json"][..],
    ] {
        let (doc, _) = run_json(repo.path(), args, Some(base_cache.path()), 0);
        assert!(doc.get("test_gap_note").is_none());
    }

    write_config(
        repo.path(),
        r#"{ "coverage_reports": ["coverage/lcov.info"] }"#,
    );
    let cache = TempDir::new().expect("coverage cache dir");
    for (json_args, md_args) in [
        (
            &["diff", "main", "--format", "json"][..],
            &["diff", "main", "--format", "markdown"][..],
        ),
        (
            &["trend", "--format", "json"][..],
            &["trend", "--format", "markdown"][..],
        ),
    ] {
        let (doc, _) = run_json(repo.path(), json_args, Some(cache.path()), 0);
        let note = doc["test_gap_note"].as_str().expect("test_gap_note");
        assert!(note.contains("left out") && note.contains("Coverage report"));
        let md = run_stdout(repo.path(), md_args, Some(cache.path()), 0);
        assert!(md.contains(note), "{md}");
    }
    run_trend_and_assert_cache(repo.path(), cache.path(), 1);

    write_file(
        repo.path(),
        "coverage/lcov.info",
        "SF:src/app.ts\nDA:2,1\nend_of_record\n",
    );
    run_trend_and_assert_cache(repo.path(), cache.path(), 1);

    let (check_after, _) = check_json(repo.path(), &[], 0);
    assert_prod_gap_ratio(&check_after, 0.0);

    write_repo_files(
        repo.path(),
        &[
            (
                "coverage/lcov2.info",
                "SF:src/app.ts\nDA:2,1\nend_of_record\n",
            ),
            (
                "codeint.json",
                r#"{ "coverage_reports": ["coverage/lcov.info", "coverage/lcov2.info"] }"#,
            ),
        ],
    );
    run_trend_and_assert_cache(repo.path(), cache.path(), 2);
}

#[test]
fn mcp_check_returns_test_gap_findings_budget_failures_and_missing_coverage_budget_error() {
    let repo = init_git_repo(&[
        ("src/main.ts", SAMPLE_TWO_FUNCTIONS_TS),
        (
            "coverage/lcov.info",
            "SF:src/main.ts\nDA:2,1\nDA:6,1\nDA:7,0\nDA:8,0\nDA:10,0\nDA:12,1\nend_of_record\n",
        ),
    ]);

    let absent = mcp_payload(&mcp_check(repo.path(), 1));
    assert_eq!(absent["passed"], true);
    assert!(has_no_prod_test_gap(&absent["report"]));
    assert!(absent["report"]["test_gap_note"].is_string());

    write_coverage_config(repo.path(), &["coverage/lcov.info"], Some(0.2));
    let over = mcp_payload(&mcp_check(repo.path(), 2));
    assert_eq!(over["passed"], false);
    assert!(
        over["failures"]
            .as_array()
            .unwrap()
            .iter()
            .any(|msg| msg.as_str().unwrap().contains("test_gap"))
    );
    let gap = prod_test_gap(&over["report"]);
    assert_eq!(gap["findings"][0]["name"], "heavy");
    assert_eq!(gap["findings"][0]["untested_share"], 0.6);

    write_config(
        repo.path(),
        r#"{ "budgets": { "production": { "test_gap": 0.2 } } }"#,
    );
    let err_resp = mcp_check(repo.path(), 3);
    assert_eq!(err_resp["result"]["isError"], true);
    let err_text = err_resp["result"]["content"][0]["text"].as_str().unwrap();
    assert!(
        err_text.contains("budgets.production.test_gap")
            && err_text.contains("needs a Coverage report"),
        "{err_text}"
    );
}

#[test]
fn coverage_reports_outside_repository_and_mcp_cwd_isolation_are_enforced() {
    let outer = tempfile::tempdir().expect("tempdir");
    let outside_secret = outer.path().join("secret.info");
    std::fs::write(&outside_secret, "TN:probe\nSECRET_VALUE=TOP_SECRET_12345\n")
        .expect("write outside secret");
    let outside_lcov = outer.path().join("outside.info");
    std::fs::write(&outside_lcov, "SF:src/main.ts\nDA:2,1\nend_of_record\n")
        .expect("write outside lcov");

    let repo_a = init_git_repo(&[
        ("src/a.ts", "export function a() { return 1; }\n"),
        ("cov/lcov.info", "SF:src/a.ts\nDA:1,1\nend_of_record\n"),
    ]);
    let repo_b = init_git_repo(&[
        ("src/b.ts", "export function b() { return 2; }\n"),
        ("cov/lcov.info", "SF:src/b.ts\nDA:1,0\nend_of_record\n"),
    ]);

    let abs_out = run_codeint(
        repo_a.path(),
        &[
            "check",
            "--format",
            "json",
            "--coverage-report",
            outside_secret.to_str().unwrap(),
        ],
        None,
    );
    assert_eq!(abs_out.status.code(), Some(2));
    let abs_err = String::from_utf8_lossy(&abs_out.stderr);
    assert!(abs_err.contains("outside the Git repository"), "{abs_err}");
    assert!(!abs_err.contains("TOP_SECRET_12345"), "{abs_err}");

    #[cfg(unix)]
    {
        let link = repo_a.path().join("cov/escape.info");
        std::os::unix::fs::symlink(&outside_lcov, &link).expect("symlink");
        write_coverage_config(repo_a.path(), &["cov/escape.info"], None);
        let sym_out = run_codeint(repo_a.path(), &["check", "--format", "json"], None);
        assert_eq!(sym_out.status.code(), Some(2));
        let sym_err = String::from_utf8_lossy(&sym_out.stderr);
        assert!(sym_err.contains("outside the Git repository"), "{sym_err}");
        let _ = std::fs::remove_file(&link);
        write_config(repo_a.path(), "{}");
    }

    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 9,
        "method": "tools/call",
        "params": {
            "name": "check",
            "arguments": {
                "path": repo_b.path().to_str().unwrap(),
                "coverage_reports": ["cov/lcov.info"]
            }
        }
    });
    let mut child = std::process::Command::new(env!("CARGO_BIN_EXE_codeint"))
        .arg("mcp")
        .current_dir(repo_a.path())
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("spawn mcp");
    use std::io::Write;
    writeln!(child.stdin.as_mut().unwrap(), "{request}").expect("write mcp");
    drop(child.stdin.take());
    let output = child.wait_with_output().expect("mcp output");
    assert!(output.status.success());
    let line = String::from_utf8(output.stdout).expect("utf8");
    let resp: serde_json::Value = serde_json::from_str(line.trim()).expect("mcp json");
    let payload = mcp_payload(&resp);
    assert_eq!(prod_test_gap(&payload["report"])["ratio"], 1.0);
    assert!(payload["report"].get("coverage_unmatched_note").is_none());
}

fn nested_outer_inner_repo() -> TempDir {
    let repo = init_git_repo(&[
        (
            "src/nested.ts",
            concat!(
                "export function outer(x: number): number {\n",
                "  const base = x + 1;\n",
                "  function inner(y: number): number {\n",
                "    const doubled = y * 2;\n",
                "    return doubled;\n",
                "  }\n",
                "  return base;\n",
                "}\n",
            ),
        ),
        (
            "coverage/lcov.info",
            "SF:src/nested.ts\nDA:2,1\nDA:4,0\nDA:5,0\nDA:7,1\nend_of_record\n",
        ),
    ]);
    write_coverage_config(repo.path(), &["coverage/lcov.info"], None);
    repo
}

fn assert_v1_state_mismatch(
    repo: &Path,
    state_rel_path: &str,
    update_args: &[&str],
    expected_substrings: &[&str],
) {
    let _ = run_stdout(repo, update_args, None, 0);
    let state_path = repo.join(state_rel_path);
    let mut doc: Value =
        serde_json::from_str(&fs::read_to_string(&state_path).expect("read state"))
            .expect("valid state json");
    let test_gap_entry = &mut doc["source_sets"]["production"]["test_gap"];
    assert_eq!(test_gap_entry["metric_version"], 2);
    test_gap_entry["metric_version"] = json!(1);
    fs::write(
        &state_path,
        format!("{}\n", serde_json::to_string_pretty(&doc).unwrap()),
    )
    .expect("write v1 state");
    assert_exit_2_contains(repo, &["check"], expected_substrings);
    fs::remove_file(&state_path).expect("remove state");
}

#[test]
fn innermost_attribution_excludes_nested_functions_and_emits_metric_version_2() {
    let repo = nested_outer_inner_repo();

    let (report, _) = check_json(repo.path(), &[], 0);
    let gap = prod_test_gap(&report);
    assert_eq!(gap["metric_version"], 2);
    assert_eq!(gap["measured_functions"], 2);
    assert_eq!(gap["ratio"], 0.5);

    let findings = gap["findings"].as_array().expect("findings array");
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0]["name"], "inner");
    assert_eq!(findings[0]["line"], 3);
    assert_eq!(findings[0]["untested_share"], 1.0);
}

#[test]
fn innermost_attribution_handles_closures_deep_nesting_same_line_zero_own_lines_and_shell_scripts()
{
    let repo = init_git_repo(&[
        (
            "src/complex.ts",
            concat!(
                "export function wrapper(x: number): () => number {\n",
                "  return () => {\n",
                "    const y = x + 1;\n",
                "    const deepest = () => {\n",
                "      return y * 2;\n",
                "    };\n",
                "    return deepest();\n",
                "  };\n",
                "}\n",
                "\n",
                "export const pair = () => () => 1;\n",
                "\n",
                "export function containerOnly() {\n",
                "  function child(v: number): number {\n",
                "    return v + 1;\n",
                "  }\n",
                "}\n",
            ),
        ),
        (
            "src/omitted.ts",
            "export function omittedFn(x: number): number {\n  return x;\n}\n",
        ),
        (
            "scripts/run.sh",
            concat!(
                "#!/bin/sh\n",
                "helper() {\n",
                "  echo \"helper\"\n",
                "}\n",
                "echo \"top\"\n",
            ),
        ),
        (
            "coverage/unit.info",
            concat!(
                "SF:src/complex.ts\n",
                "DA:2,1\n",
                "DA:3,1\n",
                "DA:5,0\n",
                "DA:7,1\n",
                "DA:11,0\n",
                "DA:15,1\n",
                "end_of_record\n",
                "SF:scripts/run.sh\n",
                "DA:3,0\n",
                "DA:5,1\n",
                "end_of_record\n",
            ),
        ),
        (
            "coverage/extra.xml",
            "<coverage><packages><package><classes><class filename=\"src/complex.ts\"><lines><line number=\"3\" hits=\"1\"/></lines></class></classes></package></packages></coverage>\n",
        ),
    ]);
    write_coverage_config(
        repo.path(),
        &["coverage/unit.info", "coverage/extra.xml"],
        None,
    );

    let (report, _) = check_json(repo.path(), &[], 0);
    let ts_gap =
        &report["languages"]["typescript-javascript"]["source_sets"]["production"]["test_gap"];
    assert_eq!(ts_gap["metric_version"], 2);
    let ts_findings = ts_gap["findings"].as_array().expect("ts findings");
    let ts_finding_lines: Vec<u64> = ts_findings
        .iter()
        .filter(|row| row["path"] == "src/complex.ts")
        .map(|row| row["line"].as_u64().expect("line"))
        .collect();
    assert!(
        !ts_finding_lines.contains(&1),
        "wrapper should be fully tested, got findings: {ts_findings:?}"
    );
    assert!(
        !ts_finding_lines.contains(&2),
        "middle closure should be fully tested, got findings: {ts_findings:?}"
    );
    assert!(
        ts_finding_lines.contains(&4),
        "deepest closure should be untested, got findings: {ts_findings:?}"
    );
    assert_eq!(
        ts_finding_lines.iter().filter(|&&line| line == 11).count(),
        2,
        "both identical-span same-line functions on line 11 should be untested: {ts_findings:?}"
    );
    assert!(
        !ts_finding_lines.contains(&13),
        "containerOnly has zero own listed lines and must be omitted: {ts_findings:?}"
    );
    assert!(
        ts_findings
            .iter()
            .any(|row| row["path"] == "src/omitted.ts" && row["untested_share"] == 1.0),
        "omitted file must stay fully untested: {ts_findings:?}"
    );

    let sh_gap = &report["languages"]["shell"]["source_sets"]["production"]["test_gap"];
    assert_eq!(sh_gap["metric_version"], 2);
    let sh_findings = sh_gap["findings"].as_array().expect("shell findings");
    assert_eq!(sh_findings.len(), 1);
    assert_eq!(sh_findings[0]["name"], "helper");
    assert_eq!(sh_findings[0]["untested_share"], 1.0);
}

#[test]
fn test_gap_metric_version_2_detects_v1_ratchet_and_baseline_mismatches() {
    let repo = nested_outer_inner_repo();

    assert_v1_state_mismatch(
        repo.path(),
        ".codeint/ratchet.json",
        &["ratchet", "update"],
        &[
            "production.test_gap",
            "Metric version 1",
            "current Metric version 2",
            "codeint ratchet update",
        ],
    );
    assert_v1_state_mismatch(
        repo.path(),
        ".codeint/baseline.json",
        &["baseline", "update"],
        &[
            "production.test_gap",
            "baseline Metric version 1",
            "current Metric version 2",
            "codeint baseline update",
        ],
    );
}
