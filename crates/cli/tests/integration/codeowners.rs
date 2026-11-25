use std::fs;
use std::path::Path;
use std::process::{Command, Output};

use serde_json::Value;
use tempfile::TempDir;

use crate::common::run_git;

fn init_fixture(files: &[(&str, &str)]) -> TempDir {
    let dir = tempfile::tempdir().expect("create temporary folder");
    run_git(dir.path(), &["init", "--quiet", "--initial-branch", "main"]);
    for (relative, text) in files {
        write_fixture(dir.path(), relative, text);
    }
    dir
}

fn write_fixture(root: &Path, relative: &str, text: &str) {
    let full = root.join(relative);
    if let Some(parent) = full.parent() {
        fs::create_dir_all(parent).expect("create parent folder");
    }
    fs::write(full, text).expect("write file");
}

fn run_cli(root: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_codeint"))
        .args(args)
        .current_dir(root)
        .env("RUST_BACKTRACE", "1")
        .output()
        .expect("run codeint")
}

fn eroded_ts(name: &str) -> String {
    let branches: String = (1..=12)
        .map(|value| format!("  if (value === {value}) return {value};\n"))
        .collect();
    format!("export function {name}(value: number): number {{\n{branches}  return 0;\n}}\n")
}

fn run_json(root: &Path, args: &[&str], expected_exit: i32) -> Value {
    let output = run_cli(root, args);
    assert_eq!(
        output.status.code(),
        Some(expected_exit),
        "codeint {args:?} stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("parse JSON stdout")
}

fn erosion_paths(value: &Value, key: &str) -> Vec<String> {
    value["source_sets"]["production"]["erosion"][key]
        .as_array()
        .expect("erosion findings array")
        .iter()
        .map(|item| item["path"].as_str().expect("path string").to_owned())
        .collect()
}

#[test]
fn check_is_silent_when_no_codeowners_file_exists() {
    let dir = init_fixture(&[("src/backend.ts", &eroded_ts("compute"))]);
    for (format, forbidden) in [
        ("json", "\"owners\""),
        ("toon", "owners"),
        ("markdown", "Owners"),
    ] {
        let output = run_cli(dir.path(), &["check", "--format", format]);
        assert_eq!(output.status.code(), Some(0));
        let text = String::from_utf8(output.stdout).expect("UTF-8 stdout");
        assert!(
            !text.contains(forbidden),
            "unexpected {forbidden} in {format}: {text}"
        );
    }
}

#[test]
fn check_attaches_owners_in_json_toon_and_markdown_with_precedence() {
    let dir = init_fixture(&[
        ("src/backend.ts", &eroded_ts("computeBackend")),
        ("src/frontend.ts", &eroded_ts("renderFrontend")),
        ("CODEOWNERS", "* @fallback-team\n"),
        (
            ".github/CODEOWNERS",
            "* @org/platform\nsrc/backend.ts @org/backend @alice\nsrc/frontend.ts @org/frontend\n",
        ),
    ]);

    let value = run_json(dir.path(), &["check", "--format", "json"], 0);
    let findings = value["source_sets"]["production"]["erosion"]["findings"]
        .as_array()
        .expect("erosion findings array");
    assert_eq!(findings.len(), 2);
    let backend = findings
        .iter()
        .find(|f| f["path"] == "src/backend.ts")
        .expect("backend finding");
    assert_eq!(
        backend["owners"],
        serde_json::json!(["@org/backend", "@alice"])
    );
    let frontend = findings
        .iter()
        .find(|f| f["path"] == "src/frontend.ts")
        .expect("frontend finding");
    assert_eq!(frontend["owners"], serde_json::json!(["@org/frontend"]));

    for (format, required) in [
        ("toon", &["@org/backend", "@org/frontend"][..]),
        (
            "markdown",
            &["| Owners |", "@org/backend", "@org/frontend"][..],
        ),
    ] {
        let output = run_cli(dir.path(), &["check", "--format", format]);
        assert_eq!(output.status.code(), Some(0));
        let text = String::from_utf8(output.stdout).expect("UTF-8 stdout");
        for needle in required {
            assert!(text.contains(needle), "{format} missing {needle}: {text}");
        }
    }
}

#[test]
fn check_owner_flag_filters_findings() {
    let dir = init_fixture(&[
        ("src/backend.ts", &eroded_ts("computeBackend")),
        ("src/frontend.ts", &eroded_ts("renderFrontend")),
        (
            ".github/CODEOWNERS",
            "src/backend.ts @org/backend\nsrc/frontend.ts @org/frontend\n",
        ),
    ]);

    let exact = run_json(
        dir.path(),
        &["check", "--format", "json", "--owner", "@org/backend"],
        0,
    );
    assert_eq!(erosion_paths(&exact, "findings"), vec!["src/backend.ts"]);

    let wildcard = run_json(
        dir.path(),
        &["check", "--format", "json", "--owner", "@org/front*"],
        0,
    );
    assert_eq!(
        erosion_paths(&wildcard, "findings"),
        vec!["src/frontend.ts"]
    );
}

#[test]
fn diff_attaches_owners_and_filters_with_owner_flag() {
    let dir = init_fixture(&[
        (
            "src/clean.ts",
            "export function ok(): number { return 1; }\n",
        ),
        (
            "CODEOWNERS",
            "src/backend.ts @org/backend\nsrc/frontend.ts @org/frontend\n",
        ),
    ]);
    run_git(dir.path(), &["add", "--all"]);
    run_git(dir.path(), &["commit", "--quiet", "--message", "base"]);

    write_fixture(dir.path(), "src/backend.ts", &eroded_ts("computeBackend"));
    write_fixture(dir.path(), "src/frontend.ts", &eroded_ts("renderFrontend"));

    let diff_all = run_json(dir.path(), &["diff", "main", "--format", "json"], 1);
    let new_all = diff_all["source_sets"]["production"]["erosion"]["new"]
        .as_array()
        .expect("new findings array");
    assert_eq!(new_all.len(), 2);
    assert!(new_all.iter().any(|f| {
        f["path"] == "src/backend.ts" && f["owners"] == serde_json::json!(["@org/backend"])
    }));
    assert!(new_all.iter().any(|f| {
        f["path"] == "src/frontend.ts" && f["owners"] == serde_json::json!(["@org/frontend"])
    }));

    let diff_filtered = run_json(
        dir.path(),
        &[
            "diff",
            "main",
            "--format",
            "json",
            "--owner",
            "@org/backend",
        ],
        1,
    );
    assert_eq!(erosion_paths(&diff_filtered, "new"), vec!["src/backend.ts"]);
}

#[test]
fn check_attaches_owners_to_clone_families_and_filters_with_owner_flag() {
    let duplicated = [
        "export function tallyOrder(rows: { unit: number; tax: number; qty: number }[]): number {",
        "  let total = 0;",
        "  for (const row of rows.filter(Boolean)) {",
        "    if (row.qty > 0 && row.unit >= 0) {",
        "      total += (row.unit + row.tax) * row.qty;",
        "    }",
        "  }",
        "  return Math.floor(total * 100) / 100;",
        "}",
    ]
    .join("\n");
    let dir = init_fixture(&[
        ("src/backend/a.ts", &duplicated),
        ("src/backend/b.ts", &duplicated),
        ("src/shared/c.ts", &duplicated),
        (
            ".github/CODEOWNERS",
            "src/backend/** @org/backend\nsrc/shared/** @org/shared\n",
        ),
    ]);

    let all = run_json(dir.path(), &["check", "--format", "json"], 0);
    let families = all["source_sets"]["production"]["duplication"]["clone_families"]
        .as_array()
        .expect("clone_families array");
    assert_eq!(families.len(), 1);
    assert_eq!(
        families[0]["owners"],
        serde_json::json!(["@org/backend", "@org/shared"])
    );

    let filtered_out = run_json(
        dir.path(),
        &["check", "--format", "json", "--owner", "@org/frontend"],
        0,
    );
    assert!(
        filtered_out["source_sets"]["production"]["duplication"]
            .get("clone_families")
            .is_none()
    );
}
