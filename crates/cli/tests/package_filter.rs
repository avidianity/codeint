mod common;

use std::fs;
use std::path::Path;
use std::process::{Command, Output};

use common::{init_git_repo, send_mcp_requests};
use serde_json::{Value, json};
use tempfile::TempDir;

const WEB_IMPORTS_API: &str =
    "import { apiVal } from '@mono/api';\nexport const webVal = apiVal + 1;\n";

fn run_codeint(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_codeint"))
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap()
}

fn run_json(dir: &Path, args: &[&str], expected_code: i32) -> Value {
    let mut full = args.to_vec();
    full.extend(["--format", "json"]);
    let output = run_codeint(dir, &full);
    assert_eq!(
        output.status.code(),
        Some(expected_code),
        "unexpected exit for {full:?}, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

fn mcp_payload(response: &Value) -> Value {
    assert_eq!(response["result"]["isError"], false);
    serde_json::from_str(response["result"]["content"][0]["text"].as_str().unwrap()).unwrap()
}

fn eroded_function(name: &str) -> String {
    format!(
        "export function {name}(x: number): number {{\n\
         if (x === 1) return 1;\n\
         if (x === 2) return 2;\n\
         if (x === 3) return 3;\n\
         if (x === 4) return 4;\n\
         if (x === 5) return 5;\n\
         if (x === 6) return 6;\n\
         if (x === 7) return 7;\n\
         if (x === 8) return 8;\n\
         if (x === 9) return 9;\n\
         if (x === 10) return 10;\n\
         if (x === 11) return 11;\n\
         return 0;\n\
         }}\n"
    )
}

fn init_ts_monorepo(web_src: &str, codeint_json: Option<&str>) -> TempDir {
    let mut files: Vec<(&str, &str)> = vec![
        (
            "package.json",
            r#"{ "name": "mono", "private": true, "workspaces": ["packages/*"] }"#,
        ),
        (
            "packages/core/package.json",
            r#"{ "name": "@mono/core", "main": "src/index.ts" }"#,
        ),
        ("packages/core/src/index.ts", "export const coreVal = 1;\n"),
        (
            "packages/api/package.json",
            r#"{ "name": "@mono/api", "main": "src/index.ts" }"#,
        ),
        (
            "packages/api/src/helper.ts",
            "import { coreVal } from '@mono/core';\nexport const helperVal = coreVal + 1;\n",
        ),
        (
            "packages/api/src/index.ts",
            "import { helperVal } from './helper';\nexport const apiVal = helperVal + 1;\n",
        ),
        (
            "packages/web/package.json",
            r#"{ "name": "@mono/web", "main": "src/index.ts" }"#,
        ),
        ("packages/web/src/index.ts", web_src),
    ];
    if let Some(config) = codeint_json {
        files.push(("codeint.json", config));
    }
    init_git_repo(&files)
}

#[test]
fn check_package_scopes_metrics_findings_budgets_and_preserves_cross_package_imports() {
    let bad_web = eroded_function("erodedWeb");
    let repo = init_ts_monorepo(
        &bad_web,
        Some(r#"{ "budgets": { "production": { "erosion": 0.1 } } }"#),
    );

    let _whole = run_json(repo.path(), &["check"], 1);
    let parsed = run_json(repo.path(), &["check", "--package", "packages/api/"], 0);
    assert_eq!(parsed["files"], 2);
    assert_eq!(parsed["source_sets"]["production"]["files"], 2);
    assert_eq!(parsed["source_sets"]["production"]["erosion"]["ratio"], 0.0);
    assert_eq!(
        parsed["source_sets"]["production"]["import_cycles"]["unresolved_imports"]["count"],
        0
    );

    let web_parsed = run_json(repo.path(), &["check", "--package", "packages/web"], 1);
    assert_eq!(web_parsed["files"], 1);
    assert_eq!(
        web_parsed["source_sets"]["production"]["erosion"]["findings"][0]["path"],
        "packages/web/src/index.ts"
    );
}

#[test]
fn diff_package_scopes_findings_budgets_and_regressions() {
    let repo = init_ts_monorepo(WEB_IMPORTS_API, None);
    fs::write(
        repo.path().join("packages/web/src/index.ts"),
        eroded_function("erodedWeb"),
    )
    .unwrap();

    let _whole = run_json(repo.path(), &["diff", "main"], 1);
    let api_parsed = run_json(
        repo.path(),
        &["diff", "main", "--package", "packages/api"],
        0,
    );
    assert_eq!(
        api_parsed["source_sets"]["production"]["erosion"]["new"],
        json!([])
    );

    let web_parsed = run_json(
        repo.path(),
        &["diff", "main", "--package", "packages/web"],
        1,
    );
    assert_eq!(
        web_parsed["source_sets"]["production"]["erosion"]["new"][0]["path"],
        "packages/web/src/index.ts"
    );
}

fn trace_args<'a>(from: &'a str, to: &'a str, pkg: &'a str) -> [&'a str; 5] {
    ["trace", from, to, "--package", pkg]
}

fn impact_args<'a>(target: &'a str, pkg: &'a str) -> [&'a str; 4] {
    ["impact", target, "--package", pkg]
}

#[test]
fn trace_package_scopes_graph_to_selected_package() {
    let repo = init_ts_monorepo(WEB_IMPORTS_API, None);

    let parsed = run_json(
        repo.path(),
        &trace_args(
            "packages/api/src/index.ts",
            "packages/api/src/helper.ts",
            "packages/api",
        ),
        0,
    );
    assert_eq!(parsed["connected"], true);
    assert_eq!(
        parsed["paths"],
        json!([["packages/api/src/index.ts", "packages/api/src/helper.ts"]])
    );

    let outside = run_codeint(
        repo.path(),
        &trace_args(
            "packages/web/src/index.ts",
            "packages/api/src/index.ts",
            "packages/api",
        ),
    );
    assert_eq!(outside.status.code(), Some(2));
}

fn assert_impact_summary(parsed: &Value, direct: u64, transitive: u64, total: u64, pkg: &str) {
    assert_eq!(parsed["summary"]["direct_dependents"], direct);
    assert_eq!(parsed["summary"]["transitive_dependents"], transitive);
    assert_eq!(parsed["summary"]["total_dependents"], total);
    assert_eq!(parsed["affected_packages"], json!([pkg]));
}

fn impact_package_json(dir: &Path, target: &str, pkg: &str) -> Value {
    run_json(dir, &impact_args(target, pkg), 0)
}

#[test]
fn impact_package_scopes_dependents_while_preserving_cross_package_transitivity() {
    let repo = init_ts_monorepo(WEB_IMPORTS_API, None);

    let web_parsed = impact_package_json(repo.path(), "packages/core/src/index.ts", "packages/web");
    assert_impact_summary(&web_parsed, 0, 1, 1, "packages/web");
    assert_eq!(
        web_parsed["production"]["transitive"][0]["path"],
        "packages/web/src/index.ts"
    );
    assert_eq!(web_parsed["production"]["transitive"][0]["distance"], 3);

    let api_parsed = impact_package_json(repo.path(), "packages/core/src/index.ts", "packages/api");
    assert_impact_summary(&api_parsed, 1, 1, 2, "packages/api");
}

#[test]
fn unknown_package_exits_2_and_lists_discovered_workspace_packages() {
    let repo = init_ts_monorepo("export const webVal = 1;\n", None);
    let trace_missing = trace_args(
        "packages/api/src/index.ts",
        "packages/api/src/helper.ts",
        "packages/missing",
    );
    let impact_missing = impact_args("packages/core/src/index.ts", "packages/missing");
    let invocations: [&[&str]; 4] = [
        &["check", "--package", "packages/missing"],
        &["diff", "main", "--package", "packages/missing"],
        &trace_missing,
        &impact_missing,
    ];

    for args in invocations {
        let output = run_codeint(repo.path(), args);
        assert_eq!(
            output.status.code(),
            Some(2),
            "expected exit 2 for {args:?}, got {:?}",
            output.status.code()
        );
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains("packages/missing"),
            "stderr missing requested package for {args:?}: {stderr}"
        );
        for expected in ["packages/api", "packages/core", "packages/web"] {
            assert!(
                stderr.contains(expected),
                "stderr missing discovered package {expected} for {args:?}: {stderr}"
            );
        }
    }
}

#[test]
fn rust_cargo_workspace_supports_package_filter() {
    let repo = init_git_repo(&[
        (
            "Cargo.toml",
            "[workspace]\nmembers = [\"crates/alpha\", \"crates/beta\"]\nresolver = \"2\"\n",
        ),
        (
            "crates/alpha/Cargo.toml",
            "[package]\nname = \"alpha\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
        ),
        ("crates/alpha/src/lib.rs", "pub fn value() -> i32 { 1 }\n"),
        (
            "crates/beta/Cargo.toml",
            "[package]\nname = \"beta\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
        ),
        (
            "crates/beta/src/helper.rs",
            "pub fn helper() -> i32 { 2 }\n",
        ),
        (
            "crates/beta/src/lib.rs",
            "mod helper;\nuse crate::helper::helper;\npub fn doubled() -> i32 { helper() * 2 }\n",
        ),
    ]);

    let parsed = run_json(repo.path(), &["check", "--package", "crates/beta"], 0);
    assert_eq!(parsed["files"], 2);

    let impact_parsed = run_json(
        repo.path(),
        &[
            "impact",
            "crates/beta/src/helper.rs",
            "--package",
            "crates/beta",
        ],
        0,
    );
    assert_eq!(impact_parsed["affected_packages"], json!(["crates/beta"]));
    assert_eq!(
        impact_parsed["production"]["direct"][0]["path"],
        "crates/beta/src/lib.rs"
    );

    let unknown_rust = run_codeint(repo.path(), &["check", "--package", "crates/gamma"]);
    assert_eq!(unknown_rust.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&unknown_rust.stderr);
    assert!(stderr.contains("crates/alpha"));
    assert!(stderr.contains("crates/beta"));
}

#[test]
fn mcp_tools_support_package_filter() {
    let repo = init_ts_monorepo(WEB_IMPORTS_API, None);

    let responses = send_mcp_requests(
        repo.path(),
        &[
            json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": "2024-11-05"}}),
            json!({"jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": {"name": "check", "arguments": {"package": "packages/api"}}}),
            json!({"jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": {"name": "diff", "arguments": {"base_ref": "main", "package": "packages/api"}}}),
            json!({"jsonrpc": "2.0", "id": 4, "method": "tools/call", "params": {"name": "trace", "arguments": {"from": "packages/api/src/index.ts", "to": "packages/api/src/helper.ts", "package": "packages/api"}}}),
            json!({"jsonrpc": "2.0", "id": 5, "method": "tools/call", "params": {"name": "impact", "arguments": {"paths": ["packages/core/src/index.ts"], "package": "packages/web"}}}),
            json!({"jsonrpc": "2.0", "id": 6, "method": "tools/call", "params": {"name": "check", "arguments": {"package": "packages/missing"}}}),
        ],
    );

    assert_eq!(mcp_payload(&responses[1])["report"]["files"], 2);
    assert_eq!(mcp_payload(&responses[2])["passed"], true);
    assert_eq!(mcp_payload(&responses[3])["connected"], true);
    assert_eq!(
        mcp_payload(&responses[4])["affected_packages"],
        json!(["packages/web"])
    );

    assert_eq!(responses[5]["result"]["isError"], true);
    let err_text = responses[5]["result"]["content"][0]["text"]
        .as_str()
        .unwrap();
    assert!(
        err_text.contains("packages/api"),
        "expected discovered packages in MCP error: {err_text}"
    );
}
