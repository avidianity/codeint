use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::OnceLock;

use codeint_plugin_host::{sha256, sha256_hex};
use serde_json::{Value, json};

use crate::common::{init_git_repo, mcp_payload, mcp_tool_call, send_mcp_requests};

static FOLDER_IMPORT_WASM: OnceLock<PathBuf> = OnceLock::new();

fn folder_import_wasm_path() -> &'static Path {
    FOLDER_IMPORT_WASM.get_or_init(|| {
        let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let manifest =
            workspace_root.join("crates/plugin-host/tests/fixtures/sandbox-probes/Cargo.toml");
        let target_dir = workspace_root.join("target/wasm-fixtures");
        let status = Command::new("cargo")
            .args([
                "build",
                "-p",
                "folder-import-language-plugin",
                "--locked",
                "--offline",
                "--target",
                "wasm32-wasip2",
            ])
            .arg("--manifest-path")
            .arg(&manifest)
            .arg("--target-dir")
            .arg(&target_dir)
            .status()
            .expect("build folder-import-language-plugin wasm fixture");
        assert!(
            status.success(),
            "folder-import-language-plugin build failed: {status}"
        );
        target_dir.join("wasm32-wasip2/debug/folder_import_language_plugin.wasm")
    })
}

fn run_trace(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_codeint"))
        .arg("trace")
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap()
}

fn trace_json(dir: &Path, args: &[&str]) -> Value {
    let mut full = args.to_vec();
    full.extend(["--format", "json"]);
    let output = run_trace(dir, &full);
    assert!(
        output.status.success(),
        "trace {full:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

fn trace_text(dir: &Path, args: &[&str]) -> String {
    let mut full = args.to_vec();
    full.extend(["--format", "text"]);
    let output = run_trace(dir, &full);
    assert_eq!(output.status.code(), Some(0));
    String::from_utf8(output.stdout).unwrap().trim().to_owned()
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

    let parsed = trace_json(repo.path(), &["src/a.ts", "src/target.ts"]);
    assert_eq!(parsed["from"], "src/a.ts");
    assert_eq!(parsed["to"], "src/target.ts");
    assert_eq!(parsed["connected"], true);
    assert_eq!(parsed["shortest_distance"], 2);
    assert_eq!(parsed["truncated"], false);
    assert!(parsed.get("multi_file_imports").is_none());
    assert_eq!(
        parsed["paths"],
        json!([["src/a.ts", "src/d.ts", "src/target.ts"]])
    );

    assert_eq!(
        trace_text(repo.path(), &["src/a.ts", "src/target.ts"]),
        "src/a.ts -> src/d.ts -> src/target.ts"
    );

    let toon_output = run_trace(repo.path(), &["src/a.ts", "src/target.ts"]);
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

    let parsed = trace_json(repo.path(), &["src/entry.ts", "src/leaf.ts", "--all"]);
    assert_eq!(parsed["shortest_distance"], 2);
    assert_eq!(parsed["truncated"], false);
    assert!(parsed.get("multi_file_imports").is_none());
    assert_eq!(
        parsed["paths"],
        json!([
            ["src/entry.ts", "src/m1.ts", "src/leaf.ts"],
            ["src/entry.ts", "src/m2.ts", "src/leaf.ts"],
            ["src/entry.ts", "src/m3.ts", "src/leaf.ts"],
        ])
    );

    let capped = trace_json(
        repo.path(),
        &["src/entry.ts", "src/leaf.ts", "--all", "--max-paths", "2"],
    );
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

    let cyclic = trace_json(repo.path(), &["src/a.ts", "src/c.ts", "--all"]);
    assert_eq!(cyclic["connected"], true);
    assert_eq!(cyclic["shortest_distance"], 2);
    assert_eq!(
        cyclic["paths"],
        json!([["src/a.ts", "src/b.ts", "src/c.ts"]])
    );

    let disconnected = trace_json(repo.path(), &["src/a.ts", "src/isolated.ts"]);
    assert_eq!(disconnected["connected"], false);
    assert_eq!(disconnected["shortest_distance"], Value::Null);
    assert_eq!(disconnected["paths"], json!([]));
    assert_eq!(disconnected["truncated"], false);

    assert_eq!(
        trace_text(repo.path(), &["src/a.ts", "src/isolated.ts"]),
        "No runtime import path from src/a.ts to src/isolated.ts"
    );

    let missing_output = run_trace(repo.path(), &["src/a.ts", "src/does-not-exist.ts"]);
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
            mcp_tool_call(3, "trace", json!({"from": "src/a.ts", "to": "src/c.ts"})),
            mcp_tool_call(
                4,
                "trace",
                json!({"from": "src/a.ts", "to": "src/other.ts"}),
            ),
            mcp_tool_call(
                5,
                "trace",
                json!({"from": "src/a.ts", "to": "src/missing.ts"}),
            ),
        ],
    );

    assert_eq!(responses.len(), 5);
    let tools = responses[1]["result"]["tools"].as_array().unwrap();
    assert!(
        tools.iter().any(|tool| tool["name"] == "trace"),
        "expected trace tool in tools/list: {tools:?}"
    );

    let found = mcp_payload(&responses[2]);
    assert_eq!(found["connected"], true);
    assert_eq!(found["shortest_distance"], 2);
    assert_eq!(
        found["paths"],
        json!([["src/a.ts", "src/b.ts", "src/c.ts"]])
    );

    let disconnected = mcp_payload(&responses[3]);
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

    let parsed = trace_json(repo.path(), &["src/app.test.ts", "src/core.ts"]);
    assert_eq!(parsed["connected"], true);
    assert_eq!(
        parsed["paths"],
        json!([["src/app.test.ts", "src/helper.ts", "src/core.ts"]])
    );

    assert_eq!(
        trace_text(repo.path(), &["py/main.py", "py/util.py"]),
        "py/main.py -> py/util.py"
    );

    let zero_cap = run_trace(
        repo.path(),
        &[
            "src/app.test.ts",
            "src/core.ts",
            "--all",
            "--max-paths",
            "0",
        ],
    );
    assert_eq!(zero_cap.status.code(), Some(2));
}

#[test]
fn trace_lists_every_file_of_a_folder_import_and_explains_reachability() {
    let wasm_bytes = fs::read(folder_import_wasm_path()).expect("read folder import wasm");
    let wasm_hash = sha256_hex(&sha256(&wasm_bytes));
    let config = format!(
        r#"{{"plugins":[{{"name":"folder-import-lang","path":"plugins/folder_import_language_plugin.wasm","sha256":"{wasm_hash}"}}]}}"#
    );
    let repo = init_git_repo(&[
        ("codeint.json", &config),
        (
            "src/app.fld",
            "program main\nuse pkg/mid\nuse pkg/unused\nfn main()\n",
        ),
        ("pkg/mid/a.fld", "use pkg/leaf\nfn alpha()\n"),
        ("pkg/mid/b.fld", "use pkg/leaf\nfn beta()\n"),
        ("pkg/mid/sibling.fld", "fn sibling()\n"),
        ("pkg/leaf/end.fld", "fn end()\n"),
        ("pkg/leaf/other.fld", "fn other()\n"),
        ("pkg/unused/u1.fld", "fn u1()\n"),
        ("pkg/unused/u2.fld", "fn u2()\n"),
    ]);
    let plugin_dest = repo
        .path()
        .join("plugins/folder_import_language_plugin.wasm");
    fs::create_dir_all(plugin_dest.parent().unwrap()).unwrap();
    fs::write(&plugin_dest, &wasm_bytes).unwrap();

    let all_json = trace_json(repo.path(), &["src/app.fld", "pkg/leaf/end.fld", "--all"]);
    assert_eq!(all_json["from"], "src/app.fld");
    assert_eq!(all_json["to"], "pkg/leaf/end.fld");
    assert_eq!(all_json["from_entry_point"], "program-start mark");
    assert_eq!(all_json["connected"], true);
    assert_eq!(all_json["shortest_distance"], 2);
    assert_eq!(
        all_json["paths"],
        json!([
            ["src/app.fld", "pkg/mid/a.fld", "pkg/leaf/end.fld"],
            ["src/app.fld", "pkg/mid/b.fld", "pkg/leaf/end.fld"],
        ])
    );
    assert_eq!(
        all_json["multi_file_imports"],
        json!([
            {
                "importer": "src/app.fld",
                "specifier": "pkg/mid",
                "targets": ["pkg/mid/a.fld", "pkg/mid/b.fld", "pkg/mid/sibling.fld"],
            },
            {
                "importer": "pkg/mid/a.fld",
                "specifier": "pkg/leaf",
                "targets": ["pkg/leaf/end.fld", "pkg/leaf/other.fld"],
            },
            {
                "importer": "pkg/mid/b.fld",
                "specifier": "pkg/leaf",
                "targets": ["pkg/leaf/end.fld", "pkg/leaf/other.fld"],
            },
        ])
    );

    let all_text = trace_text(repo.path(), &["src/app.fld", "pkg/leaf/end.fld", "--all"]);
    assert_eq!(
        all_text,
        [
            "Entry point: src/app.fld (program-start mark)",
            "src/app.fld -> pkg/mid/a.fld -> pkg/leaf/end.fld",
            "src/app.fld -> pkg/mid/b.fld -> pkg/leaf/end.fld",
            "Import pkg/mid in src/app.fld leads to: pkg/mid/a.fld, pkg/mid/b.fld, pkg/mid/sibling.fld",
            "Import pkg/leaf in pkg/mid/a.fld leads to: pkg/leaf/end.fld, pkg/leaf/other.fld",
            "Import pkg/leaf in pkg/mid/b.fld leads to: pkg/leaf/end.fld, pkg/leaf/other.fld",
        ]
        .join("\n")
    );

    let toon_output = run_trace(repo.path(), &["src/app.fld", "pkg/leaf/end.fld", "--all"]);
    assert!(
        toon_output.status.success(),
        "trace toon failed: {}",
        String::from_utf8_lossy(&toon_output.stderr)
    );
    let toon = String::from_utf8(toon_output.stdout).unwrap();
    assert!(toon.contains("multi_file_imports"), "toon: {toon}");
    assert!(toon.contains("pkg/mid/sibling.fld"), "toon: {toon}");
    assert!(toon.contains("pkg/leaf/other.fld"), "toon: {toon}");

    let direct_json = trace_json(repo.path(), &["src/app.fld", "pkg/mid/sibling.fld"]);
    assert_eq!(direct_json["connected"], true);
    assert_eq!(direct_json["shortest_distance"], 1);
    assert_eq!(
        direct_json["paths"],
        json!([["src/app.fld", "pkg/mid/sibling.fld"]])
    );
    assert_eq!(
        direct_json["multi_file_imports"],
        json!([
            {
                "importer": "src/app.fld",
                "specifier": "pkg/mid",
                "targets": ["pkg/mid/a.fld", "pkg/mid/b.fld", "pkg/mid/sibling.fld"],
            }
        ])
    );
    assert_eq!(
        trace_text(repo.path(), &["src/app.fld", "pkg/mid/sibling.fld"]),
        [
            "Entry point: src/app.fld (program-start mark)",
            "src/app.fld -> pkg/mid/sibling.fld",
            "Import pkg/mid in src/app.fld leads to: pkg/mid/a.fld, pkg/mid/b.fld, pkg/mid/sibling.fld",
        ]
        .join("\n")
    );
}
