use std::path::PathBuf;
use std::process::{Command, Output};

use codeint_plugin_host::{LANGUAGE_CHECKS, METRIC_CHECKS};
use serde_json::Value;

fn fixture(file_name: &str) -> PathBuf {
    crate::wasm_fixtures::wasm_fixture_path(file_name)
}

fn run_plugin_test(kind: &str, wasm: &str, format: &str) -> Output {
    run_plugin_test_with(kind, wasm, format, &[])
}

fn run_plugin_test_with(
    kind: &str,
    wasm: &str,
    format: &str,
    extra: &[&std::ffi::OsStr],
) -> Output {
    Command::new(env!("CARGO_BIN_EXE_codeint"))
        .args(["plugin", "test"])
        .arg(fixture(wasm))
        .args(["--kind", kind, "--format", format])
        .args(extra)
        .output()
        .expect("run codeint plugin test")
}

fn stdout(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).expect("utf8 stdout")
}

fn json_report(kind: &str, wasm: &str, extra: &[&std::ffi::OsStr]) -> (Output, Value) {
    let output = run_plugin_test_with(kind, wasm, "json", extra);
    let value: Value = serde_json::from_str(&stdout(&output)).expect("json report");
    (output, value)
}

fn failed_checks(report: &Value) -> Vec<(String, String)> {
    report["checks"]
        .as_array()
        .expect("checks array")
        .iter()
        .filter(|check| check["status"] == "fail")
        .map(|check| {
            (
                check["id"].as_str().expect("id").to_owned(),
                check["hint"].as_str().unwrap_or_default().to_owned(),
            )
        })
        .collect()
}

#[test]
fn valid_language_plugin_passes_in_every_format() {
    for format in ["text", "json", "toon"] {
        let output = run_plugin_test("language", "sample_mini_plugin.wasm", format);
        assert_eq!(output.status.code(), Some(0), "{format}: {output:?}");
    }
    let (_, report) = json_report("language", "sample_mini_plugin.wasm", &[]);
    assert_eq!(report["passed"], true);
    assert!(failed_checks(&report).is_empty());
}

#[test]
fn valid_metric_plugin_passes_in_every_format() {
    for format in ["text", "json", "toon"] {
        let output = run_plugin_test("metric", "sample_long_functions_plugin.wasm", format);
        assert_eq!(output.status.code(), Some(0), "{format}: {output:?}");
    }
}

#[test]
fn broken_plugins_fail_with_a_fix_hint() {
    let cases = [
        (
            "language",
            "legacy_v0_3_0_language_plugin.wasm",
            "wit-contract",
        ),
        ("metric", "legacy_v0_3_0_metric_plugin.wasm", "wit-contract"),
        ("language", "clock_reader_plugin.wasm", "zero-imports"),
        (
            "metric",
            "zero_version_metric_plugin.wasm",
            "metric-version",
        ),
        ("metric", "reserved_key_metric_plugin.wasm", "metric-key"),
        (
            "metric",
            "out_of_range_ratio_metric_plugin.wasm",
            "measurement-bounds",
        ),
        (
            "metric",
            "invalid_finding_path_metric_plugin.wasm",
            "finding-paths",
        ),
        (
            "language",
            "unlisted_hazard_rule_language_plugin.wasm",
            "lower",
        ),
    ];
    let (_sources, hazard_source) = write_source("probe.uhr", "UNLISTED_HAZARD\n");
    let source_args: [&std::ffi::OsStr; 2] = ["--source".as_ref(), hazard_source.as_os_str()];
    for (kind, wasm, check_id) in cases {
        let extra: &[&std::ffi::OsStr] = if check_id == "lower" {
            &source_args
        } else {
            &[]
        };
        let (output, report) = json_report(kind, wasm, extra);
        assert_eq!(output.status.code(), Some(1), "{wasm}: {output:?}");
        assert_eq!(report["passed"], false, "{wasm}");
        let failed = failed_checks(&report);
        let hint = failed
            .iter()
            .find(|(id, hint)| id == check_id && !hint.is_empty())
            .map(|(_, hint)| hint.clone())
            .unwrap_or_else(|| {
                panic!("{wasm}: expected failed {check_id} with a hint, got {failed:?}")
            });
        for format in ["text", "toon"] {
            let output = run_plugin_test_with(kind, wasm, format, extra);
            assert_eq!(output.status.code(), Some(1), "{wasm} {format}: {output:?}");
            let text = stdout(&output);
            if format == "text" {
                assert!(
                    text.contains(&format!("[FAIL] {check_id}:")),
                    "{wasm} text: {text}"
                );
                assert!(
                    text.contains(&format!("fix: {hint}")),
                    "{wasm} text: {text}"
                );
            } else {
                assert!(text.contains(check_id), "{wasm} toon: {text}");
                assert!(text.contains("fail"), "{wasm} toon: {text}");
                assert!(text.contains(&hint), "{wasm} toon: {text}");
            }
        }
    }
}

fn write_source(file_name: &str, text: &str) -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join(file_name);
    std::fs::write(&path, text).expect("write source");
    (dir, path)
}

fn check_ids(report: &Value) -> Vec<String> {
    report["checks"]
        .as_array()
        .expect("checks array")
        .iter()
        .map(|check| check["id"].as_str().expect("id").to_owned())
        .collect()
}

fn check_status(report: &Value, id: &str) -> String {
    report["checks"]
        .as_array()
        .expect("checks array")
        .iter()
        .find(|check| check["id"] == id)
        .and_then(|check| check["status"].as_str())
        .unwrap_or_else(|| panic!("no check {id}"))
        .to_owned()
}

fn assert_fails(kind: &str, wasm: &str, extra: &[&std::ffi::OsStr], check_id: &str) -> Value {
    let (output, report) = json_report(kind, wasm, extra);
    assert_eq!(output.status.code(), Some(1), "{wasm}: {output:?}");
    let failed = failed_checks(&report);
    assert!(
        failed
            .iter()
            .any(|(id, hint)| id == check_id && !hint.is_empty()),
        "{wasm}: expected failed {check_id} with a hint, got {failed:?}"
    );
    report
}

#[test]
fn workspace_packages_failures_fail_the_check() {
    for wasm in [
        "invalid_workspace_root_language_plugin.wasm",
        "workspace_trap_language_plugin.wasm",
    ] {
        assert_fails("language", wasm, &[], "workspace-packages");
    }
}

#[test]
fn a_plugin_that_chooses_no_manifests_is_never_sent_a_foreign_manifest() {
    let (output, report) =
        json_report("language", "cargo_only_workspace_language_plugin.wasm", &[]);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert_eq!(check_status(&report, "workspace-packages"), "pass");
}

#[test]
fn a_workspace_error_on_a_valid_manifest_fails_the_check() {
    assert_fails(
        "language",
        "workspace_always_fails_language_plugin.wasm",
        &[],
        "workspace-packages",
    );
}

#[test]
fn a_plugin_without_framework_profiles_skips_the_framework_profile_check() {
    let (output, report) = json_report("language", "no_imports_language_plugin.wasm", &[]);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert_eq!(check_status(&report, "framework-profiles"), "skipped");
}

#[test]
fn a_manifest_paths_trap_fails_the_check() {
    assert_fails(
        "language",
        "manifest_paths_trap_language_plugin.wasm",
        &[],
        "manifest-paths",
    );
}

#[test]
fn a_selection_a_plugin_cannot_answer_fails_framework_profiles() {
    for wasm in [
        "exclude_trap_language_plugin.wasm",
        "single_profile_only_language_plugin.wasm",
    ] {
        assert_fails("language", wasm, &[], "framework-profiles");
    }
}

#[test]
fn a_plugin_that_rejects_only_manifests_it_did_not_choose_passes() {
    let (output, report) = json_report(
        "language",
        "manifest_strict_profiles_language_plugin.wasm",
        &[],
    );
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert_eq!(check_status(&report, "framework-profiles"), "pass");
}

#[test]
fn a_plugin_with_an_empty_workspace_answer_is_sent_every_manifest_in_one_root_call() {
    let (output, report) = json_report(
        "language",
        "empty_workspace_multi_manifest_language_plugin.wasm",
        &[],
    );
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert!(
        failed_checks(&report)
            .iter()
            .any(|(id, _)| id == "framework-profiles"),
        "{report}"
    );
}

#[test]
fn a_chosen_source_file_is_sent_its_real_content_not_a_labelled_garbage_manifest() {
    for wasm in [
        "source_manifest_workspace_language_plugin.wasm",
        "source_manifest_profiles_language_plugin.wasm",
    ] {
        let (output, report) = json_report("language", wasm, &[]);
        assert_eq!(output.status.code(), Some(0), "{wasm}: {output:?}");
        assert!(failed_checks(&report).is_empty(), "{wasm}");
    }
}

#[test]
fn a_metric_that_traps_on_function_coverage_fails_measure() {
    let report = assert_fails(
        "metric",
        "coverage_trap_metric_plugin.wasm",
        &[],
        "measurement-bounds",
    );
    assert_eq!(check_status(&report, "measurement-bounds"), "fail");
}

#[test]
fn a_plugin_that_rejects_a_multi_profile_only_selection_fails_framework_profiles() {
    assert_fails(
        "language",
        "multi_profile_only_language_plugin.wasm",
        &[],
        "framework-profiles",
    );
}

#[test]
fn a_framework_profile_entry_point_outside_the_repository_fails_framework_profiles() {
    assert_fails(
        "language",
        "framework_entry_language_plugin.wasm",
        &[],
        "framework-profiles",
    );
}

#[test]
fn an_import_resolved_outside_the_repository_under_an_active_profile_fails_resolve_imports() {
    assert_fails(
        "language",
        "profile_imports_language_plugin.wasm",
        &[],
        "resolve-imports",
    );
}

#[test]
fn per_function_findings_with_an_empty_id_fail_the_check() {
    assert_fails(
        "metric",
        "empty_finding_id_metric_plugin.wasm",
        &[],
        "finding-ids",
    );
}

#[test]
fn a_memory_cap_hit_in_lower_fails_limits() {
    let (_sources, source) = write_source("big.hog", "HOG_MEMORY\n");
    let extra: [&std::ffi::OsStr; 4] = [
        "--source".as_ref(),
        source.as_os_str(),
        "--plugin-memory-mb".as_ref(),
        "16".as_ref(),
    ];
    let report = assert_fails(
        "language",
        "memory_hog_language_plugin.wasm",
        &extra,
        "lower",
    );
    assert_eq!(check_status(&report, "limits"), "fail");
}

#[test]
fn a_plugin_without_imports_skips_every_import_check_and_passes() {
    let (output, report) = json_report("language", "no_imports_language_plugin.wasm", &[]);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    for id in [
        "manifest-paths",
        "workspace-packages",
        "resolve-imports",
        "declared-dependencies",
        "entry-points",
    ] {
        assert_eq!(check_status(&report, id), "skipped", "{id}");
    }
}

#[test]
fn a_source_file_name_with_a_backslash_does_not_crash_the_runner() {
    let (_sources, source) = write_source("a\\b.noimp", "fn alpha\n");
    let extra: [&std::ffi::OsStr; 2] = ["--source".as_ref(), source.as_os_str()];
    let (output, report) = json_report("language", "no_imports_language_plugin.wasm", &extra);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert_eq!(check_status(&report, "lower"), "pass");
}

#[test]
fn an_extension_with_a_backslash_does_not_crash_the_runner() {
    let (output, report) = json_report("language", "backslash_extension_language_plugin.wasm", &[]);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert_eq!(check_status(&report, "extensions"), "pass");
}

#[test]
fn a_hazard_line_out_of_range_for_a_later_claimed_extension_fails_lower() {
    let (output, report) = json_report("language", "second_extension_language_plugin.wasm", &[]);
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert_eq!(check_status(&report, "lower"), "fail");
    let detail = report["checks"]
        .as_array()
        .expect("checks array")
        .iter()
        .find(|check| check["id"] == "lower")
        .expect("lower check")["message"]
        .to_string();
    assert!(detail.contains(".bbb"), "{detail}");
}

#[test]
fn a_plugin_that_miscounts_lines_on_crlf_input_fails_lower() {
    let (output, report) = json_report("language", "crlf_miscount_language_plugin.wasm", &[]);
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert_eq!(check_status(&report, "lower"), "fail");
    let detail = report["checks"]
        .as_array()
        .expect("checks array")
        .iter()
        .find(|check| check["id"] == "lower")
        .expect("lower check")["message"]
        .to_string();
    assert!(detail.contains("CRLF"), "{detail}");
}

#[test]
fn duplicate_extensions_and_hazard_rules_pass() {
    let (output, report) = json_report("language", "duplicate_extension_language_plugin.wasm", &[]);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert_eq!(check_status(&report, "extensions"), "pass");
    assert_eq!(check_status(&report, "hazard-rules"), "pass");
}

#[test]
fn every_report_lists_each_check_once_in_order() {
    let language = [
        "legacy_v0_3_0_language_plugin.wasm",
        "clock_reader_plugin.wasm",
        "invalid_workspace_root_language_plugin.wasm",
        "workspace_trap_language_plugin.wasm",
        "workspace_always_fails_language_plugin.wasm",
        "cargo_only_workspace_language_plugin.wasm",
        "manifest_paths_trap_language_plugin.wasm",
        "exclude_trap_language_plugin.wasm",
        "no_imports_language_plugin.wasm",
        "backslash_extension_language_plugin.wasm",
        "second_extension_language_plugin.wasm",
        "duplicate_extension_language_plugin.wasm",
        "unlisted_hazard_rule_language_plugin.wasm",
        "sample_mini_plugin.wasm",
    ];
    let metric = [
        "legacy_v0_3_0_metric_plugin.wasm",
        "zero_version_metric_plugin.wasm",
        "reserved_key_metric_plugin.wasm",
        "out_of_range_ratio_metric_plugin.wasm",
        "invalid_finding_path_metric_plugin.wasm",
        "empty_finding_id_metric_plugin.wasm",
        "sample_long_functions_plugin.wasm",
    ];
    for (kind, wasms, expected) in [
        ("language", &language[..], &LANGUAGE_CHECKS[..]),
        ("metric", &metric[..], &METRIC_CHECKS[..]),
    ] {
        for wasm in wasms {
            let (_, report) = json_report(kind, wasm, &[]);
            assert_eq!(check_ids(&report), expected, "{wasm}");
        }
    }
}

#[test]
fn a_non_limit_measure_error_still_lists_limits() {
    let report = assert_fails(
        "metric",
        "out_of_range_ratio_metric_plugin.wasm",
        &[],
        "measurement-bounds",
    );
    assert_eq!(check_status(&report, "limits"), "pass");
    assert_eq!(check_status(&report, "finding-ids"), "skipped");
}

#[test]
fn load_failures_report_earlier_checks_as_passed_and_later_checks_as_skipped() {
    let (_, report) = json_report("language", "clock_reader_plugin.wasm", &[]);
    assert_eq!(check_status(&report, "component"), "pass");
    assert_eq!(check_status(&report, "wit-contract"), "pass");
    assert_eq!(check_status(&report, "zero-imports"), "fail");
    assert_eq!(check_status(&report, "extensions"), "skipped");
    let (_, report) = json_report("language", "legacy_v0_3_0_language_plugin.wasm", &[]);
    assert_eq!(check_status(&report, "component"), "pass");
    assert_eq!(check_status(&report, "wit-contract"), "fail");
    assert_eq!(check_status(&report, "zero-imports"), "skipped");
}

#[test]
fn unreadable_plugin_file_is_an_operational_error() {
    let output = Command::new(env!("CARGO_BIN_EXE_codeint"))
        .args(["plugin", "test", "/nonexistent/plugin.wasm"])
        .output()
        .expect("run codeint plugin test");
    assert_eq!(output.status.code(), Some(2));
}

#[test]
fn stale_reference_ranges_fail_reference_ranges_with_a_recalibrate_hint_in_every_format() {
    let wasm = "stale_reference_range_language_plugin.wasm";
    let expected = [
        "erosion has version 2, current version is 1",
        "verbosity has version 0, current version is 2",
    ];
    let (output, report) = json_report("language", wasm, &[]);
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert_eq!(check_status(&report, "reference-ranges"), "fail");
    let hint = failed_checks(&report)
        .into_iter()
        .find(|(id, _)| id == "reference-ranges")
        .map(|(_, hint)| hint)
        .expect("reference-ranges failure");
    for needle in expected {
        assert!(hint.contains(needle), "{needle:?} missing from {hint:?}");
    }
    assert!(hint.contains("recalibrate"), "{hint:?}");
    assert!(!hint.contains("duplication"), "{hint:?}");

    let text = stdout(&run_plugin_test("language", wasm, "text"));
    assert!(text.contains("[FAIL] reference-ranges:"), "{text}");
    assert!(text.contains(&format!("fix: {hint}")), "{text}");

    let toon = stdout(&run_plugin_test("language", wasm, "toon"));
    assert!(toon.contains("reference-ranges"), "{toon}");
    assert!(toon.contains(&hint), "{toon}");
}

#[test]
fn current_reference_ranges_still_pass() {
    let (_, report) = json_report("language", "sample_mini_plugin.wasm", &[]);
    assert_eq!(check_status(&report, "reference-ranges"), "pass");
}
