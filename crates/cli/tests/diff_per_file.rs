use std::fs;
use std::path::Path;
use std::process::{Command, Output};

use serde_json::{Value, json};
use tempfile::TempDir;

struct PerFileWorkspace {
    worktree: TempDir,
    result_cache: TempDir,
}

impl PerFileWorkspace {
    fn from_base(entries: &[(&str, &str)]) -> PerFileWorkspace {
        let worktree = TempDir::new().expect("create worktree");
        let result_cache = TempDir::new().expect("create cache");
        git_exec(worktree.path(), &["init", "-q", "-b", "main"]);
        let workspace = PerFileWorkspace {
            worktree,
            result_cache,
        };
        entries.iter().for_each(|&(path, body)| {
            workspace.put(path, body);
        });
        workspace.commit_all("base");
        workspace
    }

    fn put(&self, relative: &str, body: &str) {
        let target = self.worktree.path().join(relative);
        target
            .parent()
            .into_iter()
            .for_each(|parent| fs::create_dir_all(parent).expect("mkdir"));
        fs::write(target, body).expect("write");
    }

    fn commit_all(&self, message: &str) {
        git_exec(self.worktree.path(), &["add", "--all"]);
        git_exec(self.worktree.path(), &["commit", "-q", "-m", message]);
    }

    fn invoke(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_codeint"))
            .args(args)
            .current_dir(self.worktree.path())
            .env("CODEINT_CACHE_DIR", self.result_cache.path())
            .output()
            .expect("invoke codeint")
    }

    fn diff_expect(&self, args: &[&str], expected_code: i32) -> (String, String) {
        let mut argv = Vec::with_capacity(args.len() + 1);
        argv.push("diff");
        argv.extend_from_slice(args);
        let output = self.invoke(&argv);
        let stdout = String::from_utf8(output.stdout).expect("utf8 stdout");
        let stderr = String::from_utf8(output.stderr).expect("utf8 stderr");
        assert_eq!(
            output.status.code(),
            Some(expected_code),
            "codeint {argv:?} exited {:?}\nstdout: {stdout}\nstderr: {stderr}",
            output.status.code()
        );
        (stdout, stderr)
    }

    fn assert_passing_diff(&self) {
        let (stdout, _) = self.diff_expect(&["main", "--format", "json"], 0);
        let report: Value = serde_json::from_str(&stdout).expect("valid diff json");
        assert_eq!(report.get("regressions"), None);
    }

    fn assert_resolved_setting(&self, key: &str, expected_value: Value, expected_source: &str) {
        let output = self.invoke(&["config", "--resolved", "--format", "json"]);
        assert_eq!(output.status.code(), Some(0));
        let payload: Value = serde_json::from_slice(&output.stdout).expect("valid config json");
        let setting = payload["settings"]
            .as_array()
            .expect("settings array")
            .iter()
            .find(|entry| entry["key"] == key)
            .unwrap_or_else(|| panic!("missing setting {key}"));
        assert_eq!(setting["value"], expected_value);
        assert_eq!(setting["source"], json!(expected_source));
    }
}

fn git_exec(cwd: &Path, args: &[&str]) {
    let env_pairs = [
        ("GIT_CONFIG_GLOBAL", "/dev/null"),
        ("GIT_CONFIG_NOSYSTEM", "1"),
        ("GIT_AUTHOR_NAME", "PerFile"),
        ("GIT_AUTHOR_EMAIL", "perfile@example.invalid"),
        ("GIT_COMMITTER_NAME", "PerFile"),
        ("GIT_COMMITTER_EMAIL", "perfile@example.invalid"),
    ];
    let ok = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .envs(env_pairs)
        .status()
        .expect("spawn git")
        .success();
    assert!(ok, "git {args:?} failed");
}

fn branching_ts(name: &str, branches: usize) -> String {
    let mut source = format!("export function {name}(v: number): number {{\n");
    for idx in 1..=branches {
        source.push_str(&format!("  if (v === {idx}) return {idx};\n"));
    }
    source.push_str("  return 0;\n}\n");
    source
}

fn simple_ts(name: &str) -> String {
    format!("export function {name}(v: number): number {{ return v + 1; }}\n")
}

fn per_file_entry<'a>(report: &'a Value, file: &str, metric: &str) -> &'a Value {
    report["regressions"]
        .as_array()
        .expect("regressions array")
        .iter()
        .find(|entry| entry["file"] == file && entry["metric"] == metric)
        .unwrap_or_else(|| panic!("missing per-file regression ({file}, {metric}) in {report}"))
}

#[test]
fn per_file_defaults_to_false_and_config_reports_provenance() {
    let ws = PerFileWorkspace::from_base(&[("src/app.ts", &simple_ts("app"))]);
    ws.assert_resolved_setting("regression_limits.per_file", json!(false), "default");

    ws.put(
        "codeint.json",
        r#"{ "regression_limits": { "per_file": true } }"#,
    );
    ws.assert_resolved_setting("regression_limits.per_file", json!(true), "file");

    ws.put(
        "codeint.json",
        r#"{ "regression_limits": { "per_file": "yes" } }"#,
    );
    let invalid = ws.invoke(&["config", "--resolved"]);
    assert_eq!(invalid.status.code(), Some(2));
    let stderr = String::from_utf8(invalid.stderr).expect("utf8 stderr");
    assert!(stderr.contains("regression_limits.per_file"), "{stderr}");
}

#[test]
fn default_per_file_false_allows_moving_unchanged_function_and_cross_file_offset() {
    let moved = branching_ts("compute", 14);
    let ws = PerFileWorkspace::from_base(&[("src/a.ts", &moved), ("src/b.ts", &simple_ts("h"))]);

    ws.put("src/a.ts", &simple_ts("compute"));
    ws.put("src/b.ts", &format!("{}\n{moved}", simple_ts("h")));
    ws.assert_passing_diff();

    ws.put("src/a.ts", &simple_ts("compute"));
    ws.put("src/b.ts", &branching_ts("smaller", 11));
    ws.assert_passing_diff();
}

#[test]
fn per_file_true_fails_worsened_modified_file_even_when_another_file_shrinks_across_all_formats() {
    let ws = PerFileWorkspace::from_base(&[
        ("src/legacy.ts", &branching_ts("legacy", 18)),
        ("src/service.ts", &simple_ts("service")),
        (
            "codeint.json",
            r#"{ "regression_limits": { "per_file": true } }"#,
        ),
    ]);
    ws.put("src/legacy.ts", &simple_ts("legacy"));
    ws.put("src/service.ts", &branching_ts("service", 12));

    let (json_stdout, stderr) = ws.diff_expect(&["main", "--format", "json"], 1);
    let report: Value = serde_json::from_str(&json_stdout).expect("valid json");
    let entry = per_file_entry(&report, "src/service.ts", "erosion");
    assert_eq!(entry["source_set"], json!("production"));
    assert_eq!(entry["base"].as_f64(), Some(0.0));
    let head_value = entry["head"].as_f64().expect("head number");
    assert!(head_value > 0.0);
    let head_str = head_value.to_string();
    assert!(
        report["regressions"]
            .as_array()
            .expect("regressions")
            .iter()
            .all(|item| item["file"] != "src/legacy.ts")
    );
    assert!(
        stderr.contains("src/service.ts")
            && stderr.contains("erosion")
            && stderr.contains("0")
            && stderr.contains(&head_str),
        "stderr missing details: {stderr}"
    );

    let (toon_stdout, _) = ws.diff_expect(&["main", "--format", "toon"], 1);
    let decoded_toon: Value = toon_format::decode_default(&toon_stdout).expect("valid toon");
    let toon_entry = per_file_entry(&decoded_toon, "src/service.ts", "erosion");
    assert_eq!(toon_entry["base"].as_f64(), Some(0.0));
    assert_eq!(toon_entry["head"].as_f64(), Some(head_value));

    let (md_stdout, _) = ws.diff_expect(&["main", "--format", "markdown"], 1);
    assert!(
        md_stdout.contains("## Regressions")
            && md_stdout.contains("`src/service.ts`")
            && md_stdout.contains("erosion")
            && md_stdout.contains("0")
            && md_stdout.contains(&head_str),
        "markdown missing details: {md_stdout}"
    );

    let (sarif_stdout, _) = ws.diff_expect(&["main", "--format", "sarif"], 1);
    let sarif: Value = serde_json::from_str(&sarif_stdout).expect("valid sarif");
    let sarif_entry = sarif["runs"][0]["properties"]["perFileRegressions"]
        .as_array()
        .expect("sarif perFileRegressions")
        .iter()
        .find(|item| item["file"] == "src/service.ts" && item["metric"] == "erosion")
        .unwrap_or_else(|| panic!("missing sarif entry: {sarif}"));
    assert_eq!(sarif_entry["sourceSet"], json!("production"));
    assert_eq!(sarif_entry["base"].as_f64(), Some(0.0));
    assert_eq!(sarif_entry["head"].as_f64(), Some(head_value));
    assert!(
        sarif_entry["message"]
            .as_str()
            .expect("sarif message")
            .contains(&head_str)
    );

    let saved = ws.worktree.path().join("saved-diff.json");
    fs::write(&saved, &json_stdout).expect("write saved diff");
    let saved_str = saved.to_str().expect("utf8 path");
    for (fmt, expected_exact) in [
        ("text", None),
        ("markdown", Some(md_stdout.as_str())),
        ("sarif", Some(sarif_stdout.as_str())),
    ] {
        let out = ws.invoke(&["report", saved_str, "--format", fmt]);
        assert_eq!(out.status.code(), Some(0), "format={fmt}");
        let rendered = String::from_utf8(out.stdout).expect("utf8");
        match expected_exact {
            Some(expected) => assert_eq!(rendered, expected, "format={fmt}"),
            None => assert!(
                rendered.contains("src/service.ts")
                    && rendered.contains("erosion")
                    && rendered.contains(&head_str),
                "text rerender missing details: {rendered}"
            ),
        }
    }
}

#[test]
fn per_file_true_passes_unmodified_improved_and_equal_modified_files() {
    let steady = branching_ts("steady", 12);
    let ws = PerFileWorkspace::from_base(&[
        ("src/untouched.ts", &branching_ts("untouched", 14)),
        ("src/improved.ts", &branching_ts("improved", 16)),
        ("src/steady.ts", &steady),
        ("src/clean.ts", &simple_ts("clean")),
        (
            "codeint.json",
            r#"{ "regression_limits": { "per_file": true } }"#,
        ),
    ]);

    ws.put("src/improved.ts", &branching_ts("improved", 12));
    ws.put(
        "src/steady.ts",
        &format!("{steady}\n{}", simple_ts("extra")),
    );
    ws.put(
        "src/clean.ts",
        &format!("{}\n{}", simple_ts("clean"), simple_ts("more")),
    );
    ws.assert_passing_diff();
}

#[test]
fn per_file_true_catches_new_finding_on_any_new_finding_metric_and_works_on_head_ref() {
    let ws = PerFileWorkspace::from_base(&[
        (
            "queries/a.sql",
            "SELECT * FROM accounts;\nSELECT * FROM invoices;\n",
        ),
        ("queries/b.sql", "SELECT * FROM orders;\n"),
        (
            "codeint.json",
            r#"{ "regression_limits": { "per_file": true } }"#,
        ),
    ]);

    ws.put("queries/a.sql", "SELECT id FROM accounts;\n");
    ws.put("queries/b.sql", "DELETE FROM orders;\n");
    git_exec(ws.worktree.path(), &["checkout", "-q", "-b", "feature"]);
    ws.commit_all("replace hazard in b and clear a");

    let (stdout, stderr) = ws.diff_expect(&["main", "--head", "HEAD", "--format", "json"], 1);
    let report: Value = serde_json::from_str(&stdout).expect("valid json");
    let entry = per_file_entry(&report, "queries/b.sql", "hazards");
    assert_eq!(entry["base"].as_f64(), Some(1.0));
    assert_eq!(entry["head"].as_f64(), Some(1.0));
    assert_eq!(entry["new_findings"].as_u64(), Some(1));
    assert!(stderr.contains("queries/b.sql") && stderr.contains("hazards"));
}

#[test]
fn per_file_true_respects_never_rule_on_source_set_metric() {
    let ws = PerFileWorkspace::from_base(&[
        ("src/legacy.ts", &branching_ts("legacy", 16)),
        ("src/gen/out.ts", &simple_ts("generated")),
        (
            "codeint.json",
            &json!({
                "source_sets": { "generated": ["src/gen/**"] },
                "regression_limits": {
                    "per_file": true,
                    "generated": {
                        "erosion": { "ratio_rise": 1, "new_findings": "never" },
                        "cognitive_complexity": { "ratio_rise": 1, "new_findings": "never" }
                    }
                }
            })
            .to_string(),
        ),
    ]);

    ws.put("src/legacy.ts", &simple_ts("legacy"));
    ws.put("src/gen/out.ts", &branching_ts("generated", 12));
    ws.assert_passing_diff();
}

fn diff_json_expect_fail(ws: &PerFileWorkspace, args: &[&str]) -> Value {
    let (stdout, _) = ws.diff_expect(args, 1);
    serde_json::from_str(&stdout).expect("valid json")
}

#[test]
fn per_file_and_new_file_budgets_work_together_on_working_tree_and_head_ref() {
    let config = json!({
        "regression_limits": { "per_file": true },
        "new_file_budgets": { "production": { "erosion": 0.0 } }
    })
    .to_string();
    let ws = PerFileWorkspace::from_base(&[
        ("codeint.json", &config),
        ("src/service.ts", &simple_ts("service")),
        ("src/legacy.ts", &branching_ts("legacy", 18)),
    ]);

    ws.put("src/service.ts", &branching_ts("service", 12));
    ws.put("src/legacy.ts", &simple_ts("legacy"));
    let edit_only = diff_json_expect_fail(&ws, &["main", "--format", "json"]);
    let edit_entry = per_file_entry(&edit_only, "src/service.ts", "erosion");
    assert_eq!(edit_entry["base"].as_f64(), Some(0.0));
    assert_eq!(edit_only.get("over_new_file_budget"), None);

    ws.put("src/brand_new.ts", &branching_ts("brandNew", 12));
    let working_report = diff_json_expect_fail(&ws, &["main", "--format", "json"]);
    let working_entry = per_file_entry(&working_report, "src/service.ts", "erosion");
    assert_eq!(working_entry["head"], edit_entry["head"]);
    let working_new_budgets = working_report["over_new_file_budget"]
        .as_array()
        .expect("over_new_file_budget array");
    assert_eq!(working_new_budgets.len(), 1);
    assert_eq!(working_new_budgets[0]["metric"], json!("erosion"));

    git_exec(ws.worktree.path(), &["checkout", "-q", "-b", "feature"]);
    ws.commit_all("worsen service and add complex new file");
    let head_report = diff_json_expect_fail(&ws, &["main", "--head", "HEAD", "--format", "json"]);
    assert_eq!(head_report["regressions"], working_report["regressions"]);
    assert_eq!(
        head_report["over_new_file_budget"],
        working_report["over_new_file_budget"]
    );
}
