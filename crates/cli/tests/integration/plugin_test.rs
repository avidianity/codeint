use std::path::PathBuf;
use std::process::{Command, Output};

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
    let sources = tempfile::tempdir().expect("tempdir");
    let hazard_source = sources.path().join("probe.uhr");
    std::fs::write(&hazard_source, "UNLISTED_HAZARD\n").expect("write source");
    let source_args: [&std::ffi::OsStr; 2] = ["--source".as_ref(), hazard_source.as_os_str()];
    for (kind, wasm, check_id) in cases {
        let extra: &[&std::ffi::OsStr] = if check_id == "lower" {
            &source_args
        } else {
            &[]
        };
        for format in ["text", "json", "toon"] {
            let output = run_plugin_test_with(kind, wasm, format, extra);
            assert_eq!(output.status.code(), Some(1), "{wasm} {format}: {output:?}");
            assert!(
                stdout(&output).contains(check_id),
                "{wasm} {format}: {}",
                stdout(&output)
            );
        }
        let (_, report) = json_report(kind, wasm, extra);
        assert_eq!(report["passed"], false, "{wasm}");
        let failed = failed_checks(&report);
        assert!(
            failed
                .iter()
                .any(|(id, hint)| id == check_id && !hint.is_empty()),
            "{wasm}: expected failed {check_id} with a hint, got {failed:?}"
        );
    }
}

#[test]
fn unreadable_plugin_file_is_an_operational_error() {
    let output = Command::new(env!("CARGO_BIN_EXE_codeint"))
        .args(["plugin", "test", "/nonexistent/plugin.wasm"])
        .output()
        .expect("run codeint plugin test");
    assert_eq!(output.status.code(), Some(2));
}
