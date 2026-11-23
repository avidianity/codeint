use std::fs;
use std::path::Path;
use std::process::{Command, Output};

use serde_json::{Value, json};

use crate::common::{init_git_repo, mcp_payload, mcp_tool_call, send_mcp_requests};

fn run_impact(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_codeint"))
        .arg("impact")
        .args(args)
        .current_dir(dir)
        .output()
        .expect("run codeint impact")
}

fn impact_json(dir: &Path, args: &[&str]) -> Value {
    let mut full_args = args.to_vec();
    full_args.extend(["--format", "json"]);
    let output = run_impact(dir, &full_args);
    assert_eq!(
        output.status.code(),
        Some(0),
        "codeint impact failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("valid JSON")
}

fn git_stdout(dir: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(args)
        .current_dir(dir)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_AUTHOR_NAME", "ImpactTest")
        .env("GIT_AUTHOR_EMAIL", "impact@example.invalid")
        .env("GIT_COMMITTER_NAME", "ImpactTest")
        .env("GIT_COMMITTER_EMAIL", "impact@example.invalid")
        .output()
        .expect("run git");
    assert!(
        output.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("utf8")
}

fn dependent_paths_and_distances(list: &Value) -> Vec<(String, u64)> {
    list.as_array()
        .expect("array")
        .iter()
        .map(|item| {
            (
                item["path"].as_str().expect("path").to_owned(),
                item["distance"].as_u64().expect("distance"),
            )
        })
        .collect()
}

#[test]
fn impact_reports_direct_and_transitive_dependents_separating_production_and_test() {
    let repo = init_git_repo(&[
        ("src/leaf.ts", "export const leaf = 1;\n"),
        (
            "src/mid.ts",
            "import { leaf } from './leaf';\nexport const mid = leaf + 1;\n",
        ),
        (
            "src/app.ts",
            "import { mid } from './mid';\nexport const app = mid + 1;\n",
        ),
        ("src/unrelated.ts", "export const unrelated = 0;\n"),
        (
            "src/leaf.test.ts",
            "import { leaf } from './leaf';\nexport const testLeaf = leaf;\n",
        ),
        (
            "src/mid.test.ts",
            "import { mid } from './mid';\nexport const testMid = mid;\n",
        ),
    ]);

    let report = impact_json(repo.path(), &["src/leaf.ts"]);

    assert_eq!(report["targets"].as_array().expect("targets").len(), 1);
    let target = &report["targets"][0];
    assert_eq!(target["path"], "src/leaf.ts");
    assert_eq!(target["source_set"], "production");
    assert_eq!(target["fan_in"], 2);
    assert_eq!(target["fan_out"], 0);
    assert_eq!(target["transitive_import_weight"], 4);

    assert_eq!(report["summary"]["direct_dependents"], 2);
    assert_eq!(report["summary"]["transitive_dependents"], 2);
    assert_eq!(report["summary"]["total_dependents"], 4);
    assert_eq!(report["summary"]["fan_in"], 2);
    assert_eq!(report["summary"]["fan_out"], 0);
    assert_eq!(report["summary"]["transitive_import_weight"], 4);

    assert_eq!(
        dependent_paths_and_distances(&report["production"]["direct"]),
        vec![("src/mid.ts".to_owned(), 1)]
    );
    assert_eq!(
        dependent_paths_and_distances(&report["production"]["transitive"]),
        vec![("src/app.ts".to_owned(), 2)]
    );
    assert_eq!(
        dependent_paths_and_distances(&report["test"]["direct"]),
        vec![("src/leaf.test.ts".to_owned(), 1)]
    );
    assert_eq!(
        dependent_paths_and_distances(&report["test"]["transitive"]),
        vec![("src/mid.test.ts".to_owned(), 2)]
    );
}

#[test]
fn impact_supports_depth_cap_and_handles_import_cycles_without_looping() {
    let repo = init_git_repo(&[
        (
            "src/a.ts",
            "import { c } from './c';\nexport const a = () => c;\n",
        ),
        (
            "src/b.ts",
            "import { a } from './a';\nexport const b = () => a;\n",
        ),
        (
            "src/c.ts",
            "import { b } from './b';\nexport const c = () => b;\n",
        ),
        (
            "src/entry.ts",
            "import { c } from './c';\nexport const entry = () => c;\n",
        ),
    ]);

    let full = impact_json(repo.path(), &["src/a.ts"]);
    assert_eq!(
        dependent_paths_and_distances(&full["production"]["direct"]),
        vec![("src/b.ts".to_owned(), 1)]
    );
    assert_eq!(
        dependent_paths_and_distances(&full["production"]["transitive"]),
        vec![("src/c.ts".to_owned(), 2), ("src/entry.ts".to_owned(), 3)]
    );
    assert_eq!(full["targets"][0]["fan_in"], 1);
    assert_eq!(full["targets"][0]["fan_out"], 1);
    assert_eq!(full["targets"][0]["transitive_import_weight"], 3);

    let capped = impact_json(repo.path(), &["src/a.ts", "--depth", "1"]);
    assert_eq!(capped["depth_limit"], 1);
    assert_eq!(
        dependent_paths_and_distances(&capped["production"]["direct"]),
        vec![("src/b.ts".to_owned(), 1)]
    );
    assert_eq!(
        dependent_paths_and_distances(&capped["production"]["transitive"]),
        Vec::<(String, u64)>::new()
    );
    assert_eq!(capped["summary"]["total_dependents"], 1);
}

#[test]
fn impact_supports_multiple_target_paths_with_deterministic_ordering() {
    let repo = init_git_repo(&[
        ("src/a.ts", "export const a = 1;\n"),
        ("src/b.ts", "export const b = 2;\n"),
        (
            "src/z_direct.ts",
            "import { a } from './a';\nexport const z = a;\n",
        ),
        (
            "src/c_direct.ts",
            "import { b } from './b';\nexport const c = b;\n",
        ),
        (
            "src/top.ts",
            "import { c } from './c_direct';\nimport { z } from './z_direct';\nexport const top = c + z;\n",
        ),
    ]);

    let report = impact_json(repo.path(), &["src/b.ts", "src/a.ts"]);
    let target_paths: Vec<&str> = report["targets"]
        .as_array()
        .expect("targets")
        .iter()
        .map(|t| t["path"].as_str().expect("path"))
        .collect();
    assert_eq!(target_paths, vec!["src/a.ts", "src/b.ts"]);
    assert_eq!(
        dependent_paths_and_distances(&report["production"]["direct"]),
        vec![
            ("src/c_direct.ts".to_owned(), 1),
            ("src/z_direct.ts".to_owned(), 1),
        ]
    );
    assert_eq!(
        dependent_paths_and_distances(&report["production"]["transitive"]),
        vec![("src/top.ts".to_owned(), 2)]
    );
}

#[test]
fn impact_supports_changed_base_using_git_merge_base() {
    let repo = init_git_repo(&[
        ("src/core.ts", "export const core = 1;\n"),
        (
            "src/service.ts",
            "import { core } from './core';\nexport const service = core + 1;\n",
        ),
        ("src/other.ts", "export const other = 1;\n"),
    ]);
    let root = repo.path();
    let merge_base = git_stdout(root, &["rev-parse", "HEAD"]).trim().to_owned();

    git_stdout(root, &["switch", "--quiet", "--create", "feature"]);
    fs::write(root.join("src/core.ts"), "export const core = 42;\n").expect("write");
    fs::write(root.join("README.md"), "# docs change\n").expect("write");
    git_stdout(root, &["add", "--all"]);
    git_stdout(root, &["commit", "--quiet", "--message", "feature change"]);

    git_stdout(root, &["switch", "--quiet", "main"]);
    fs::write(root.join("src/other.ts"), "export const other = 99;\n").expect("write");
    git_stdout(root, &["add", "--all"]);
    git_stdout(root, &["commit", "--quiet", "--message", "main advance"]);

    git_stdout(root, &["switch", "--quiet", "feature"]);

    let report = impact_json(root, &["--changed", "main"]);
    assert_eq!(report["changed_base"]["ref"], "main");
    assert_eq!(report["changed_base"]["merge_base"], merge_base);
    let target_paths: Vec<&str> = report["targets"]
        .as_array()
        .expect("targets")
        .iter()
        .map(|t| t["path"].as_str().expect("path"))
        .collect();
    assert_eq!(target_paths, vec!["src/core.ts"]);
    assert_eq!(
        dependent_paths_and_distances(&report["production"]["direct"]),
        vec![("src/service.ts".to_owned(), 1)]
    );
}

#[test]
fn impact_lists_affected_workspace_packages_in_monorepo() {
    let mut owned: Vec<(String, String)> = vec![(
        "package.json".to_owned(),
        r#"{ "name": "mono", "private": true, "workspaces": ["packages/*"] }"#.to_owned(),
    )];
    for (pkg, src) in [
        ("core", "export const coreVal = 1;\n"),
        (
            "api",
            "import { coreVal } from '../../core/src/index';\nexport const apiVal = coreVal + 1;\n",
        ),
        (
            "web",
            "import { apiVal } from '../../api/src/index';\nexport const webVal = apiVal + 1;\n",
        ),
    ] {
        owned.push((
            format!("packages/{pkg}/package.json"),
            format!(r#"{{ "name": "@mono/{pkg}", "main": "src/index.ts" }}"#),
        ));
        owned.push((format!("packages/{pkg}/src/index.ts"), src.to_owned()));
    }
    let refs: Vec<(&str, &str)> = owned
        .iter()
        .map(|(path, content)| (path.as_str(), content.as_str()))
        .collect();
    let repo = init_git_repo(&refs);

    let report = impact_json(repo.path(), &["packages/core/src/index.ts"]);
    let affected: Vec<&str> = report["affected_packages"]
        .as_array()
        .expect("affected_packages")
        .iter()
        .map(|v| v.as_str().expect("pkg"))
        .collect();
    assert_eq!(affected, vec!["packages/api", "packages/web"]);
    assert_eq!(report["targets"][0]["package"], "packages/core");
}

#[test]
fn impact_renders_text_and_toon_formats() {
    let repo = init_git_repo(&[
        ("src/util.ts", "export const util = 1;\n"),
        (
            "src/app.ts",
            "import { util } from './util';\nexport const app = util;\n",
        ),
        (
            "src/util.test.ts",
            "import { util } from './util';\nexport const t = util;\n",
        ),
    ]);

    let text_out = run_impact(repo.path(), &["src/util.ts", "--format", "text"]);
    assert_eq!(text_out.status.code(), Some(0));
    let text = String::from_utf8(text_out.stdout).expect("utf8");
    assert!(text.contains("Targets (1):"), "missing targets: {text}");
    assert!(text.contains("src/util.ts"), "missing target path: {text}");
    assert!(
        text.contains("Production dependents (1):"),
        "missing production section: {text}"
    );
    assert!(text.contains("src/app.ts"), "missing prod dep: {text}");
    assert!(
        text.contains("Test dependents (1):"),
        "missing test section: {text}"
    );
    assert!(
        text.contains("src/util.test.ts"),
        "missing test dep: {text}"
    );

    let toon_out = run_impact(repo.path(), &["src/util.ts", "--format", "toon"]);
    assert_eq!(toon_out.status.code(), Some(0));
    let toon_str = String::from_utf8(toon_out.stdout).expect("utf8");
    let decoded: Value = toon_format::decode_default(&toon_str).expect("valid TOON");
    assert_eq!(decoded["summary"]["total_dependents"], 2);
}

#[test]
fn impact_errors_when_target_is_not_in_measured_code_model() {
    let repo = init_git_repo(&[("src/real.ts", "export const real = 1;\n")]);

    let missing = run_impact(repo.path(), &["src/missing.ts"]);
    assert_eq!(missing.status.code(), Some(2));
    assert!(missing.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&missing.stderr);
    assert!(
        stderr.contains("src/missing.ts is not in the measured Code model"),
        "unexpected stderr: {stderr}"
    );

    let outside = run_impact(repo.path(), &["../outside.ts"]);
    assert_eq!(outside.status.code(), Some(2));
    let outside_err = String::from_utf8_lossy(&outside.stderr);
    assert!(
        outside_err.contains("outside the Git repository"),
        "unexpected stderr: {outside_err}"
    );
}

#[test]
fn mcp_impact_tool_reports_blast_radius_depth_and_errors() {
    let repo = init_git_repo(&[(
        "app.ts",
        "export function greet(x: number): number {\n  return x + 1;\n}\n",
    )]);
    fs::write(
        repo.path().join("consumer.ts"),
        "import { greet } from './app';\nexport const run = () => greet(1);\n",
    )
    .unwrap();
    fs::write(
        repo.path().join("app.test.ts"),
        "import { greet } from './app';\nexport const testGreet = greet(2);\n",
    )
    .unwrap();
    let responses = send_mcp_requests(
        repo.path(),
        &[
            mcp_tool_call(1, "impact", json!({"paths": ["app.ts"], "depth": 1})),
            mcp_tool_call(2, "impact", json!({"changed": "main"})),
            mcp_tool_call(3, "impact", json!({"paths": ["missing.ts"]})),
        ],
    );
    assert_eq!(responses.len(), 3);
    let direct = mcp_payload(&responses[0]);
    assert_eq!(direct["summary"]["direct_dependents"], 2);
    assert_eq!(direct["production"]["direct"][0]["path"], "consumer.ts");
    assert_eq!(direct["test"]["direct"][0]["path"], "app.test.ts");

    let changed = mcp_payload(&responses[1]);
    assert_eq!(changed["changed_base"]["ref"], "main");
    assert_eq!(changed["targets"].as_array().unwrap().len(), 2);

    assert_mcp_error_contains(
        &responses[2],
        "missing.ts is not in the measured Code model",
    );
}

fn assert_mcp_error_contains(response: &Value, expected: &str) {
    assert_eq!(response["result"]["isError"], true);
    let text = response["result"]["content"][0]["text"]
        .as_str()
        .expect("error text");
    assert!(text.contains(expected), "unexpected MCP error: {text}");
}
