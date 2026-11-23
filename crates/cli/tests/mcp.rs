use std::io::Write;
use std::process::{Command, Stdio};

use serde_json::{Value, json};
use tempfile::TempDir;

fn init_sample_repo() -> TempDir {
    let dir = TempDir::new().unwrap();
    let run_git = |args: &[&str]| {
        let status = Command::new("git")
            .args(args)
            .current_dir(dir.path())
            .env("GIT_AUTHOR_NAME", "Test")
            .env("GIT_AUTHOR_EMAIL", "test@example.com")
            .env("GIT_COMMITTER_NAME", "Test")
            .env("GIT_COMMITTER_EMAIL", "test@example.com")
            .status()
            .unwrap();
        assert!(status.success(), "git {args:?} failed");
    };
    run_git(&["init", "-q", "-b", "main"]);
    std::fs::write(
        dir.path().join("app.ts"),
        "export function greet(x: number): number {\n  return x + 1;\n}\n",
    )
    .unwrap();
    run_git(&["add", "."]);
    run_git(&["commit", "-q", "-m", "initial"]);
    dir
}

fn send_mcp_requests(dir: &std::path::Path, requests: &[Value]) -> Vec<Value> {
    let mut child = Command::new(env!("CARGO_BIN_EXE_codeint"))
        .arg("mcp")
        .current_dir(dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    {
        let mut stdin = child.stdin.take().unwrap();
        for req in requests {
            writeln!(stdin, "{req}").unwrap();
        }
    }
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "codeint mcp failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}

#[test]
fn mcp_initialize_list_and_call_tools() {
    let repo = init_sample_repo();
    let responses = send_mcp_requests(
        repo.path(),
        &[
            json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": "2024-11-05"}}),
            json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
            json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"}),
            json!({"jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": {"name": "check", "arguments": {}}}),
            json!({"jsonrpc": "2.0", "id": 4, "method": "tools/call", "params": {"name": "diff", "arguments": {"base_ref": "main"}}}),
            json!({"jsonrpc": "2.0", "id": 5, "method": "tools/call", "params": {"name": "trend", "arguments": {}}}),
            json!({"jsonrpc": "2.0", "id": 6, "method": "tools/call", "params": {"name": "impact", "arguments": {"paths": ["app.ts"]}}}),
            json!({"jsonrpc": "2.0", "id": 7, "method": "unknown/method"}),
        ],
    );
    assert_eq!(responses.len(), 7);
    assert_eq!(responses[0]["result"]["serverInfo"]["name"], "codeint");
    let tools = responses[1]["result"]["tools"].as_array().unwrap();
    let names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
    assert_eq!(names, ["check", "diff", "trend", "impact"]);

    assert_eq!(responses[2]["result"]["isError"], false);
    let check_payload: Value = serde_json::from_str(
        responses[2]["result"]["content"][0]["text"]
            .as_str()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(check_payload["passed"], true);
    assert_eq!(check_payload["report"]["files"], 1);

    assert_eq!(responses[3]["result"]["isError"], false);
    assert_eq!(responses[4]["result"]["isError"], false);
    assert_eq!(responses[5]["result"]["isError"], false);
    let impact_payload: Value = serde_json::from_str(
        responses[5]["result"]["content"][0]["text"]
            .as_str()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(impact_payload["targets"][0]["path"], "app.ts");
    assert_eq!(impact_payload["summary"]["total_dependents"], 0);
    assert_eq!(responses[6]["error"]["code"], -32601);
}

#[test]
fn mcp_impact_tool_reports_blast_radius_depth_and_errors() {
    let repo = init_sample_repo();
    std::fs::write(
        repo.path().join("consumer.ts"),
        "import { greet } from './app';\nexport const run = () => greet(1);\n",
    )
    .unwrap();
    std::fs::write(
        repo.path().join("app.test.ts"),
        "import { greet } from './app';\nexport const testGreet = greet(2);\n",
    )
    .unwrap();
    let responses = send_mcp_requests(
        repo.path(),
        &[
            json!({"jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": {"name": "impact", "arguments": {"paths": ["app.ts"], "depth": 1}}}),
            json!({"jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": {"name": "impact", "arguments": {"changed": "main"}}}),
            json!({"jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": {"name": "impact", "arguments": {"paths": ["missing.ts"]}}}),
        ],
    );
    assert_eq!(responses.len(), 3);
    assert_eq!(responses[0]["result"]["isError"], false);
    let direct: Value = serde_json::from_str(
        responses[0]["result"]["content"][0]["text"]
            .as_str()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(direct["summary"]["direct_dependents"], 2);
    assert_eq!(direct["production"]["direct"][0]["path"], "consumer.ts");
    assert_eq!(direct["test"]["direct"][0]["path"], "app.test.ts");

    assert_eq!(responses[1]["result"]["isError"], false);
    let changed: Value = serde_json::from_str(
        responses[1]["result"]["content"][0]["text"]
            .as_str()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(changed["changed_base"]["ref"], "main");
    assert_eq!(changed["targets"].as_array().unwrap().len(), 2);

    assert_eq!(responses[2]["result"]["isError"], true);
    assert!(
        responses[2]["result"]["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("missing.ts is not in the measured Code model")
    );
}
