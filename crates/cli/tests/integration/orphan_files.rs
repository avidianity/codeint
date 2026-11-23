use std::path::Path;
use std::process::Command;

use serde_json::{Value, json};
use tempfile::TempDir;

use crate::common::{init_git_repo, mcp_payload, mcp_tool_call, send_mcp_requests};

fn run_codeint(dir: &Path, cache_dir: Option<&Path>, args: &[&str], expected_code: i32) -> String {
    let mut command = Command::new(env!("CARGO_BIN_EXE_codeint"));
    command.args(args).current_dir(dir);
    if let Some(cache) = cache_dir {
        command.env("CODEINT_CACHE_DIR", cache);
    }
    let output = command.output().expect("run codeint");
    assert_eq!(
        output.status.code(),
        Some(expected_code),
        "codeint {args:?} exited {:?}; stderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("stdout utf8")
}

fn run_json(dir: &Path, cache_dir: Option<&Path>, args: &[&str], expected_code: i32) -> Value {
    let stdout = run_codeint(dir, cache_dir, args, expected_code);
    serde_json::from_str(&stdout).expect("valid JSON output")
}

fn write_file(dir: &Path, rel: &str, body: &str) {
    let path = dir.join(rel);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("create parent dirs");
    }
    std::fs::write(path, body).expect("write fixture file");
}

fn orphan_finding_paths(section: &Value) -> Vec<&str> {
    section["findings"]
        .as_array()
        .expect("orphan findings array")
        .iter()
        .map(|entry| entry["path"].as_str().expect("finding path"))
        .collect()
}

fn check_json(dir: &Path, expected_code: i32) -> Value {
    run_json(dir, None, &["check", "--format", "json"], expected_code)
}

fn production_orphans(report: &Value) -> &Value {
    &report["source_sets"]["production"]["orphan_files"]
}

fn assert_production_orphans(dir: &Path, expected_evaluated: usize, expected_paths: &[&str]) {
    let report = check_json(dir, 0);
    let orphan = production_orphans(&report);
    assert_eq!(orphan["evaluated_files"], expected_evaluated);
    assert_eq!(orphan["orphan_files"], expected_paths.len());
    assert_eq!(orphan_finding_paths(orphan), expected_paths);
}

fn diff_main_json(dir: &Path, cache: &Path, expected_code: i32) -> Value {
    run_json(
        dir,
        Some(cache),
        &["diff", "main", "--format", "json"],
        expected_code,
    )
}

#[test]
fn ts_check_reports_orphan_files_across_toon_json_sarif_and_markdown_excluding_generated_and_vendored()
 {
    let repo = init_git_repo(&[
        (
            "package.json",
            r#"{"name": "demo", "main": "./src/index.ts"}"#,
        ),
        (
            "src/index.ts",
            "import { live } from './live';\nexport const run = () => live();\n",
        ),
        ("src/live.ts", "export const live = () => 1;\n"),
        ("src/dead.ts", "export const dead = () => 2;\n"),
        ("src/helper.ts", "export const helper = () => 3;\n"),
        (
            "src/helper.test.ts",
            "import { helper } from './helper';\nexport const check = () => helper();\n",
        ),
        (
            "src/gen.ts",
            "// @generated\nexport const generated = () => 4;\n",
        ),
        ("vendor/third.ts", "export const vendored = () => 5;\n"),
    ]);

    let report = run_json(repo.path(), None, &["check", "--format", "json"], 0);
    assert!(report.get("score").is_none());
    let prod = &report["source_sets"]["production"];
    let orphan = &prod["orphan_files"];
    assert_eq!(orphan["orphan_files"], 2);
    assert_eq!(orphan["evaluated_files"], 4);
    assert_eq!(orphan["entry_points"], 1);
    assert_eq!(orphan["unreferenced"], 1);
    assert_eq!(orphan["test_only"], 1);
    assert_eq!(orphan["ratio"], 0.5);

    let findings = orphan["findings"].as_array().expect("orphan findings");
    assert_eq!(findings.len(), 2);
    assert_eq!(findings[0]["path"], "src/dead.ts");
    assert_eq!(findings[0]["kind"], "unreferenced");
    assert!(findings[0].get("test_importers").is_none());
    assert_eq!(findings[1]["path"], "src/helper.ts");
    assert_eq!(findings[1]["kind"], "test-only");
    assert_eq!(findings[1]["test_importers"], json!(["src/helper.test.ts"]));
    assert!(
        findings[1]["message"]
            .as_str()
            .unwrap()
            .contains("only imported by tests")
    );

    assert!(report["source_sets"]["test"].get("orphan_files").is_none());
    assert!(
        report["source_sets"]["generated"]
            .get("orphan_files")
            .is_none()
    );
    assert!(
        report["source_sets"]["vendored"]
            .get("orphan_files")
            .is_none()
    );

    let toon = run_codeint(repo.path(), None, &["check"], 0);
    assert!(toon.contains("orphan_files:"));
    assert!(toon.contains("src/dead.ts"));
    assert!(toon.contains("src/helper.ts"));
    assert!(toon.contains("test-only"));
    assert!(toon.contains("unreferenced"));

    let sarif = run_json(repo.path(), None, &["check", "--format", "sarif"], 0);
    let results: Vec<&Value> = sarif["runs"][0]["results"]
        .as_array()
        .expect("sarif results")
        .iter()
        .filter(|item| item["ruleId"] == "orphan_files")
        .collect();
    assert_eq!(results.len(), 2);
    assert_eq!(
        results[1]["relatedLocations"][0]["physicalLocation"]["artifactLocation"]["uri"],
        "src/helper.test.ts"
    );
    assert_eq!(
        results[0]["partialFingerprints"]["codeintFindingId"],
        findings[0]["id"]
    );

    let markdown = run_codeint(repo.path(), None, &["check", "--format", "markdown"], 0);
    assert!(markdown.contains("| Production | Orphan files | 0.5 |"));
    assert!(markdown.contains("### Orphan files"));
    assert!(markdown.contains("`src/dead.ts`"));
    assert!(markdown.contains("`src/helper.ts`"));
    assert!(markdown.contains("test-only"));
}

#[test]
fn silent_no_op_when_no_entry_points_and_activates_via_config_entry_points_globs() {
    let repo = init_git_repo(&[
        (
            "src/routes/home.ts",
            "import { shared } from '../shared';\nexport const home = () => shared();\n",
        ),
        ("src/shared.ts", "export const shared = () => 1;\n"),
        ("src/abandoned.ts", "export const abandoned = () => 2;\n"),
    ]);

    let silent = run_json(repo.path(), None, &["check", "--format", "json"], 0);
    assert!(
        silent["source_sets"]["production"]
            .get("orphan_files")
            .is_none()
    );

    write_file(
        repo.path(),
        "codeint.json",
        r#"{"entry_points": ["src/routes/**/*.ts"]}"#,
    );

    let active = run_json(repo.path(), None, &["check", "--format", "json"], 0);
    let orphan = &active["source_sets"]["production"]["orphan_files"];
    assert_eq!(orphan["evaluated_files"], 3);
    assert_eq!(orphan["entry_points"], 1);
    assert_eq!(orphan["orphan_files"], 1);
    assert_eq!(orphan["findings"][0]["path"], "src/abandoned.ts");
    assert_eq!(orphan["findings"][0]["kind"], "unreferenced");
}

#[test]
fn monorepo_cross_package_reachability_and_isolated_package_safety() {
    let repo = init_git_repo(&[
        (
            "package.json",
            r#"{"name": "mono", "private": true, "workspaces": ["packages/*"]}"#,
        ),
        (
            "packages/app/package.json",
            r#"{"name": "@mono/app", "main": "./src/index.ts"}"#,
        ),
        (
            "packages/app/src/index.ts",
            "import { used } from '@mono/lib';\nexport const start = () => used();\n",
        ),
        (
            "packages/lib/package.json",
            r#"{"name": "@mono/lib", "exports": {".": "./src/used.ts"}}"#,
        ),
        ("packages/lib/src/used.ts", "export const used = () => 1;\n"),
        (
            "packages/lib/src/unused.ts",
            "export const unused = () => 2;\n",
        ),
        ("packages/docs/package.json", r#"{"name": "@mono/docs"}"#),
        (
            "packages/docs/src/page.ts",
            "export const page = () => 3;\n",
        ),
    ]);

    assert_production_orphans(repo.path(), 3, &["packages/lib/src/unused.ts"]);
}

fn init_single_entry_repo(name: &str, extra: &[(&str, &str)]) -> (TempDir, TempDir) {
    let manifest = format!(r#"{{"name": "{name}", "main": "./src/index.ts"}}"#);
    let mut files: Vec<(&str, &str)> = vec![
        ("package.json", manifest.as_str()),
        ("src/index.ts", "export const main = () => 1;\n"),
    ];
    files.extend_from_slice(extra);
    (TempDir::new().expect("cache dir"), init_git_repo(&files))
}

#[test]
fn budget_and_diff_regression_gating_with_stable_finding_ids() {
    let (cache, repo) = init_single_entry_repo(
        "gate",
        &[("src/legacy_dead.ts", "export const legacy = () => 2;\n")],
    );

    write_file(
        repo.path(),
        "codeint.json",
        r#"{"budgets": {"production": {"orphan_files": 0}}}"#,
    );
    let over_budget = run_json(repo.path(), None, &["check", "--format", "json"], 1);
    assert_eq!(over_budget["over_budget"][0]["metric"], "orphan_files");

    write_file(repo.path(), "codeint.json", "{}");
    let unchanged_diff = diff_main_json(repo.path(), cache.path(), 0);
    let diff_orphans = &unchanged_diff["source_sets"]["production"]["orphan_files"];
    assert_eq!(diff_orphans["new"].as_array().unwrap().len(), 0);
    assert_eq!(diff_orphans["resolved"].as_array().unwrap().len(), 0);
    assert_eq!(diff_orphans["unchanged"], 1);

    write_file(
        repo.path(),
        "src/new_dead.ts",
        "export const added = () => 3;\n",
    );
    let regressed_diff = diff_main_json(repo.path(), cache.path(), 1);
    let regressed_orphans = &regressed_diff["source_sets"]["production"]["orphan_files"];
    assert_eq!(regressed_orphans["new"].as_array().unwrap().len(), 1);
    assert_eq!(regressed_orphans["new"][0]["path"], "src/new_dead.ts");
    assert_eq!(regressed_orphans["unchanged"], 1);
    assert_eq!(regressed_diff["regressions"][0]["metric"], "orphan_files");

    std::fs::remove_file(repo.path().join("src/new_dead.ts")).expect("remove new_dead.ts");
    write_file(
        repo.path(),
        "src/index.ts",
        "import { legacy } from './legacy_dead';\nexport const main = () => legacy();\n",
    );
    let resolved_diff = diff_main_json(repo.path(), cache.path(), 0);
    let resolved_orphans = &resolved_diff["source_sets"]["production"]["orphan_files"];
    assert_eq!(resolved_orphans["new"].as_array().unwrap().len(), 0);
    assert_eq!(resolved_orphans["resolved"].as_array().unwrap().len(), 1);
    assert_eq!(
        resolved_orphans["resolved"][0]["path"],
        "src/legacy_dead.ts"
    );
    assert_eq!(resolved_orphans["unchanged"], 0);
}

#[test]
fn multi_language_rust_and_python_check_and_mcp_expose_orphan_files() {
    let repo = init_git_repo(&[
        (
            "Cargo.toml",
            "[package]\nname = \"svc\"\nversion = \"0.1.0\"\n",
        ),
        (
            "src/lib.rs",
            "mod live;\nuse crate::live::value;\npub fn entry() -> i32 { value() }\n",
        ),
        ("src/live.rs", "pub fn value() -> i32 { 1 }\n"),
        ("src/dead.rs", "pub fn unused() -> i32 { 2 }\n"),
        (
            "pyproject.toml",
            "[project]\nname = \"pyapp\"\nversion = \"0.1.0\"\n[project.scripts]\npyapp = \"app.cli:main\"\n",
        ),
        ("app/__init__.py", ""),
        (
            "app/cli.py",
            "from app.used import run\ndef main():\n    return run()\n",
        ),
        ("app/used.py", "def run():\n    return 1\n"),
        ("app/unused.py", "def stale():\n    return 2\n"),
        ("app/helper_for_tests.py", "def helper():\n    return 3\n"),
        (
            "tests/test_helper.py",
            "from app.helper_for_tests import helper\ndef test_it():\n    assert helper() == 3\n",
        ),
    ]);

    let report = check_json(repo.path(), 0);
    let total_orphans = production_orphans(&report);
    let paths = orphan_finding_paths(total_orphans);
    assert!(paths.contains(&"src/dead.rs"), "got {paths:?}");
    assert!(paths.contains(&"app/unused.py"), "got {paths:?}");
    assert!(paths.contains(&"app/helper_for_tests.py"), "got {paths:?}");
    assert!(!paths.contains(&"src/live.rs"), "got {paths:?}");
    assert!(!paths.contains(&"app/used.py"), "got {paths:?}");

    let test_only_finding = total_orphans["findings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["path"] == "app/helper_for_tests.py")
        .expect("python test-only finding");
    assert_eq!(test_only_finding["kind"], "test-only");

    let rust_orphans = &report["languages"]["rust"]["source_sets"]["production"]["orphan_files"];
    assert!(orphan_finding_paths(rust_orphans).contains(&"src/dead.rs"));
    let py_orphans = &report["languages"]["python"]["source_sets"]["production"]["orphan_files"];
    assert!(orphan_finding_paths(py_orphans).contains(&"app/unused.py"));

    write_file(
        repo.path(),
        "app/extra_orphan.py",
        "def extra():\n    return 4\n",
    );
    let mcp_calls = [
        mcp_tool_call(1, "check", json!({})),
        mcp_tool_call(2, "diff", json!({"base_ref": "main"})),
    ];
    let responses = send_mcp_requests(repo.path(), &mcp_calls);
    let check_payload = mcp_payload(&responses[0]);
    let mcp_orphans = &check_payload["report"]["source_sets"]["production"]["orphan_files"];
    assert!(orphan_finding_paths(mcp_orphans).contains(&"app/extra_orphan.py"));
    let diff_payload = mcp_payload(&responses[1]);
    assert_eq!(diff_payload["passed"], false);
    assert_eq!(
        diff_payload["report"]["source_sets"]["production"]["orphan_files"]["new"][0]["path"],
        "app/extra_orphan.py"
    );
}

#[test]
fn orphan_files_attach_codeowners_and_filter_with_owner_in_check_and_diff() {
    let (cache, repo) = init_single_entry_repo(
        "owned",
        &[
            ("src/backend_dead.ts", "export const backend = () => 2;\n"),
            (
                ".github/CODEOWNERS",
                "src/backend_dead.ts @org/backend\nsrc/frontend_dead.ts @org/frontend\n",
            ),
        ],
    );

    let check_all = check_json(repo.path(), 0);
    let check_findings = production_orphans(&check_all)["findings"]
        .as_array()
        .expect("check orphan findings");
    assert_eq!(check_findings.len(), 1);
    assert_eq!(check_findings[0]["owners"], json!(["@org/backend"]));

    let check_md = run_codeint(repo.path(), None, &["check", "--format", "markdown"], 0);
    assert!(check_md.contains("| File | Kind | Details | Owners |"));
    assert!(check_md.contains("`@org/backend`"));

    write_file(
        repo.path(),
        "src/frontend_dead.ts",
        "export const frontend = () => 3;\n",
    );
    let check_filtered = run_json(
        repo.path(),
        None,
        &["check", "--format", "json", "--owner", "@org/frontend"],
        0,
    );
    assert_eq!(
        orphan_finding_paths(production_orphans(&check_filtered)),
        vec!["src/frontend_dead.ts"]
    );

    let diff_filtered = run_json(
        repo.path(),
        Some(cache.path()),
        &[
            "diff",
            "main",
            "--format",
            "json",
            "--owner",
            "@org/frontend",
        ],
        1,
    );
    let diff_new = diff_filtered["source_sets"]["production"]["orphan_files"]["new"]
        .as_array()
        .expect("diff new orphan findings");
    assert_eq!(diff_new.len(), 1);
    assert_eq!(diff_new[0]["path"], "src/frontend_dead.ts");
    assert_eq!(diff_new[0]["owners"], json!(["@org/frontend"]));
}

#[test]
fn non_runtime_tooling_config_script_and_example_files_are_not_flagged_as_orphans() {
    let (_cache, repo) = init_single_entry_repo(
        "tooling-orphans",
        &[
            ("src/dead.ts", "export const dead = () => 2;\n"),
            ("vite.config.ts", "export default { base: '/' };\n"),
            ("scripts/release.ts", "export const release = () => 3;\n"),
            ("examples/demo.ts", "export const demo = () => 4;\n"),
            (
                ".opencode/plugins/no-hook-bypass.js",
                "export const guard = () => 5;\n",
            ),
        ],
    );

    assert_production_orphans(repo.path(), 2, &["src/dead.ts"]);
}

#[test]
fn non_workspace_subdirectory_package_excludes_package_tooling_from_orphans() {
    let repo = init_git_repo(&[
        (
            "frontend/package.json",
            r#"{"name": "frontend", "main": "./src/index.ts"}"#,
        ),
        ("frontend/src/index.ts", "export const start = () => 1;\n"),
        ("frontend/src/dead.ts", "export const dead = () => 2;\n"),
        ("frontend/vite.config.ts", "export default { base: '/' };\n"),
        (
            "frontend/scripts/release.ts",
            "export const release = () => 3;\n",
        ),
    ]);

    assert_production_orphans(repo.path(), 2, &["frontend/src/dead.ts"]);
}
