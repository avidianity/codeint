mod common;

use std::path::Path;
use std::process::{Command, Output};

use common::{init_git_repo, send_mcp_requests};
use serde_json::{Value, json};

fn run_codeint(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_codeint"))
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap()
}

#[test]
fn trace_finds_shortest_runtime_import_path_in_cli_json_text_and_toon() {
    let repo = init_git_repo(&[
        (
            "src/a.ts",
            "import { b } from './b';\nimport { d } from './d';\nexport const a = b + d;\n",
        ),
        (
            "src/b.ts",
            "import { c } from './c';\nexport const b = c + 1;\n",
        ),
        (
            "src/c.ts",
            "import { target } from './target';\nexport const c = target + 1;\n",
        ),
        (
            "src/d.ts",
            "import { target } from './target';\nexport const d = target + 2;\n",
        ),
        ("src/target.ts", "export const target = 42;\n"),
    ]);

    let json_output = run_codeint(
        repo.path(),
        &["trace", "src/a.ts", "src/target.ts", "--format", "json"],
    );
    assert!(
        json_output.status.success(),
        "trace json failed: {}",
        String::from_utf8_lossy(&json_output.stderr)
    );
    let parsed: Value = serde_json::from_slice(&json_output.stdout).unwrap();
    assert_eq!(parsed["from"], "src/a.ts");
    assert_eq!(parsed["to"], "src/target.ts");
    assert_eq!(parsed["connected"], true);
    assert_eq!(parsed["shortest_distance"], 2);
    assert_eq!(parsed["truncated"], false);
    assert_eq!(
        parsed["paths"],
        json!([["src/a.ts", "src/d.ts", "src/target.ts"]])
    );

    let text_output = run_codeint(
        repo.path(),
        &["trace", "src/a.ts", "src/target.ts", "--format", "text"],
    );
    assert!(text_output.status.success());
    let text = String::from_utf8(text_output.stdout).unwrap();
    assert_eq!(text.trim(), "src/a.ts -> src/d.ts -> src/target.ts");

    let toon_output = run_codeint(repo.path(), &["trace", "src/a.ts", "src/target.ts"]);
    assert!(toon_output.status.success());
    let toon = String::from_utf8(toon_output.stdout).unwrap();
    assert!(toon.contains("from: src/a.ts"), "toon output: {toon}");
    assert!(toon.contains("to: src/target.ts"), "toon output: {toon}");
    assert!(toon.contains("connected: true"), "toon output: {toon}");
}

#[test]
fn trace_all_returns_multiple_shortest_paths_deterministically_with_cap() {
    let repo = init_git_repo(&[
        (
            "src/entry.ts",
            "import { m1 } from './m1';\nimport { m2 } from './m2';\nimport { m3 } from './m3';\nexport const e = m1 + m2 + m3;\n",
        ),
        (
            "src/m1.ts",
            "import { leaf } from './leaf';\nexport const m1 = leaf;\n",
        ),
        (
            "src/m2.ts",
            "import { leaf } from './leaf';\nexport const m2 = leaf;\n",
        ),
        (
            "src/m3.ts",
            "import { leaf } from './leaf';\nexport const m3 = leaf;\n",
        ),
        ("src/leaf.ts", "export const leaf = 1;\n"),
    ]);

    let all_output = run_codeint(
        repo.path(),
        &[
            "trace",
            "src/entry.ts",
            "src/leaf.ts",
            "--all",
            "--format",
            "json",
        ],
    );
    assert!(all_output.status.success());
    let parsed: Value = serde_json::from_slice(&all_output.stdout).unwrap();
    assert_eq!(parsed["shortest_distance"], 2);
    assert_eq!(parsed["truncated"], false);
    assert_eq!(
        parsed["paths"],
        json!([
            ["src/entry.ts", "src/m1.ts", "src/leaf.ts"],
            ["src/entry.ts", "src/m2.ts", "src/leaf.ts"],
            ["src/entry.ts", "src/m3.ts", "src/leaf.ts"],
        ])
    );

    let capped_output = run_codeint(
        repo.path(),
        &[
            "trace",
            "src/entry.ts",
            "src/leaf.ts",
            "--all",
            "--max-paths",
            "2",
            "--format",
            "json",
        ],
    );
    assert!(capped_output.status.success());
    let capped: Value = serde_json::from_slice(&capped_output.stdout).unwrap();
    assert_eq!(capped["truncated"], true);
    assert_eq!(
        capped["paths"],
        json!([
            ["src/entry.ts", "src/m1.ts", "src/leaf.ts"],
            ["src/entry.ts", "src/m2.ts", "src/leaf.ts"],
        ])
    );
}

#[test]
fn trace_handles_cycles_disconnected_modules_and_missing_paths() {
    let repo = init_git_repo(&[
        (
            "src/a.ts",
            "import { b } from './b';\nexport const a = b;\n",
        ),
        (
            "src/b.ts",
            "import { a } from './a';\nimport { c } from './c';\nexport const b = a + c;\n",
        ),
        (
            "src/c.ts",
            "import { a } from './a';\nexport const c = a;\n",
        ),
        ("src/isolated.ts", "export const isolated = 0;\n"),
    ]);

    let cyclic_output = run_codeint(
        repo.path(),
        &["trace", "src/a.ts", "src/c.ts", "--all", "--format", "json"],
    );
    assert!(cyclic_output.status.success());
    let cyclic: Value = serde_json::from_slice(&cyclic_output.stdout).unwrap();
    assert_eq!(cyclic["connected"], true);
    assert_eq!(cyclic["shortest_distance"], 2);
    assert_eq!(
        cyclic["paths"],
        json!([["src/a.ts", "src/b.ts", "src/c.ts"]])
    );

    let disconnected_output = run_codeint(
        repo.path(),
        &["trace", "src/a.ts", "src/isolated.ts", "--format", "json"],
    );
    assert_eq!(disconnected_output.status.code(), Some(0));
    let disconnected: Value = serde_json::from_slice(&disconnected_output.stdout).unwrap();
    assert_eq!(disconnected["connected"], false);
    assert_eq!(disconnected["shortest_distance"], Value::Null);
    assert_eq!(disconnected["paths"], json!([]));
    assert_eq!(disconnected["truncated"], false);

    let disconnected_text = run_codeint(
        repo.path(),
        &["trace", "src/a.ts", "src/isolated.ts", "--format", "text"],
    );
    assert_eq!(disconnected_text.status.code(), Some(0));
    assert_eq!(
        String::from_utf8(disconnected_text.stdout).unwrap().trim(),
        "No runtime import path from src/a.ts to src/isolated.ts"
    );

    let missing_output = run_codeint(repo.path(), &["trace", "src/a.ts", "src/does-not-exist.ts"]);
    assert_eq!(missing_output.status.code(), Some(2));
    let stderr = String::from_utf8(missing_output.stderr).unwrap();
    assert!(
        stderr.contains("src/does-not-exist.ts"),
        "expected missing path in stderr, got: {stderr}"
    );
}

#[test]
fn mcp_trace_tool_returns_paths_and_errors_cleanly() {
    let repo = init_git_repo(&[
        (
            "src/a.ts",
            "import { b } from './b';\nexport const a = b;\n",
        ),
        (
            "src/b.ts",
            "import { a } from './a';\nimport { c } from './c';\nexport const b = a + c;\n",
        ),
        ("src/c.ts", "export const c = 1;\n"),
        ("src/other.ts", "export const other = 2;\n"),
    ]);

    let responses = send_mcp_requests(
        repo.path(),
        &[
            json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": "2024-11-05"}}),
            json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"}),
            json!({"jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": {"name": "trace", "arguments": {"from": "src/a.ts", "to": "src/c.ts"}}}),
            json!({"jsonrpc": "2.0", "id": 4, "method": "tools/call", "params": {"name": "trace", "arguments": {"from": "src/a.ts", "to": "src/other.ts"}}}),
            json!({"jsonrpc": "2.0", "id": 5, "method": "tools/call", "params": {"name": "trace", "arguments": {"from": "src/a.ts", "to": "src/missing.ts"}}}),
        ],
    );

    assert_eq!(responses.len(), 5);
    let tools = responses[1]["result"]["tools"].as_array().unwrap();
    assert!(
        tools.iter().any(|tool| tool["name"] == "trace"),
        "expected trace tool in tools/list: {tools:?}"
    );

    assert_eq!(responses[2]["result"]["isError"], false);
    let found: Value = serde_json::from_str(
        responses[2]["result"]["content"][0]["text"]
            .as_str()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(found["connected"], true);
    assert_eq!(found["shortest_distance"], 2);
    assert_eq!(
        found["paths"],
        json!([["src/a.ts", "src/b.ts", "src/c.ts"]])
    );

    assert_eq!(responses[3]["result"]["isError"], false);
    let disconnected: Value = serde_json::from_str(
        responses[3]["result"]["content"][0]["text"]
            .as_str()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(disconnected["connected"], false);
    assert_eq!(disconnected["paths"], json!([]));

    assert_eq!(responses[4]["result"]["isError"], true);
    let err_text = responses[4]["result"]["content"][0]["text"]
        .as_str()
        .unwrap();
    assert!(
        err_text.contains("src/missing.ts"),
        "expected missing path in MCP error: {err_text}"
    );
}

#[test]
fn trace_works_across_source_sets_and_python_and_rejects_zero_max_paths() {
    let repo = init_git_repo(&[
        (
            "src/app.test.ts",
            "import { helper } from './helper';\nexport const tested = helper();\n",
        ),
        (
            "src/helper.ts",
            "import { core } from './core';\nexport function helper(): number { return core; }\n",
        ),
        ("src/core.ts", "export const core = 7;\n"),
        (
            "py/main.py",
            "from . import util\n\ndef run():\n    return util.value()\n",
        ),
        ("py/util.py", "def value():\n    return 1\n"),
        ("py/__init__.py", ""),
    ]);

    let cross_set = run_codeint(
        repo.path(),
        &[
            "trace",
            "src/app.test.ts",
            "src/core.ts",
            "--format",
            "json",
        ],
    );
    assert!(cross_set.status.success());
    let parsed: Value = serde_json::from_slice(&cross_set.stdout).unwrap();
    assert_eq!(parsed["connected"], true);
    assert_eq!(
        parsed["paths"],
        json!([["src/app.test.ts", "src/helper.ts", "src/core.ts"]])
    );

    let py_trace = run_codeint(
        repo.path(),
        &["trace", "py/main.py", "py/util.py", "--format", "text"],
    );
    assert!(py_trace.status.success());
    assert_eq!(
        String::from_utf8(py_trace.stdout).unwrap().trim(),
        "py/main.py -> py/util.py"
    );

    let zero_cap = run_codeint(
        repo.path(),
        &[
            "trace",
            "src/app.test.ts",
            "src/core.ts",
            "--all",
            "--max-paths",
            "0",
        ],
    );
    assert_eq!(zero_cap.status.code(), Some(2));
}
