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
            json!({"jsonrpc": "2.0", "id": 6, "method": "unknown/method"}),
        ],
    );
    assert_eq!(responses.len(), 6);
    assert_eq!(responses[0]["result"]["serverInfo"]["name"], "codeint");
    let tools = responses[1]["result"]["tools"].as_array().unwrap();
    let names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
    assert_eq!(names, ["check", "diff", "trend", "guard", "churn"]);

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
    assert_eq!(responses[5]["error"]["code"], -32601);
}

#[test]
fn mcp_guard_tool_reports_existing_planned_unzoned_and_unconfigured_paths() {
    let repo = init_sample_repo();
    let unconfigured = send_mcp_requests(
        repo.path(),
        &[json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": {"name": "guard", "arguments": {"file": "app.ts"}}
        })],
    );
    assert_eq!(unconfigured[0]["result"]["isError"], false);
    let unconfigured_payload: Value = serde_json::from_str(
        unconfigured[0]["result"]["content"][0]["text"]
            .as_str()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(unconfigured_payload["configured"], false);
    assert_eq!(unconfigured_payload["exists"], true);

    std::fs::write(
        repo.path().join("codeint.json"),
        json!({
            "boundaries": {
                "zones": {
                    "core": { "patterns": ["src/core/**"], "public_files": ["src/core/api.ts"] },
                    "adapter": { "patterns": ["src/adapter/**"] },
                    "web": { "patterns": ["src/web/**"] }
                },
                "rules": [
                    { "from": "core", "deny": ["adapter"], "reason": "Core must stay independent of adapters" },
                    { "from": "web", "allow": ["core"], "reason": "Web may only import core" }
                ]
            }
        })
        .to_string(),
    )
    .unwrap();
    std::fs::create_dir_all(repo.path().join("src/core")).unwrap();
    std::fs::write(
        repo.path().join("src/core/internal.ts"),
        "export const internal = 1;\n",
    )
    .unwrap();

    let responses = send_mcp_requests(
        repo.path(),
        &[
            json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "tools/call",
                "params": {"name": "guard", "arguments": {"file": "src/core/internal.ts"}}
            }),
            json!({
                "jsonrpc": "2.0",
                "id": 2,
                "method": "tools/call",
                "params": {"name": "guard", "arguments": {"file": "src/web/planned/view.ts"}}
            }),
            json!({
                "jsonrpc": "2.0",
                "id": 3,
                "method": "tools/call",
                "params": {"name": "guard", "arguments": {"file": "app.ts"}}
            }),
        ],
    );

    assert_eq!(responses.len(), 3);
    for response in &responses {
        assert_eq!(response["result"]["isError"], false);
    }

    let existing: Value = serde_json::from_str(
        responses[0]["result"]["content"][0]["text"]
            .as_str()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(existing["zone"], "core");
    assert_eq!(existing["exists"], true);
    assert_eq!(existing["is_public_file"], false);
    assert_eq!(existing["allowed_importers"], json!(["core"]));
    assert_eq!(existing["denied_targets"][0]["zone"], "adapter");
    assert_eq!(
        existing["denied_targets"][0]["reason"],
        "Core must stay independent of adapters"
    );

    let planned: Value = serde_json::from_str(
        responses[1]["result"]["content"][0]["text"]
            .as_str()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(planned["zone"], "web");
    assert_eq!(planned["exists"], false);
    assert_eq!(planned["denied_targets"][0]["zone"], "adapter");

    let unzoned: Value = serde_json::from_str(
        responses[2]["result"]["content"][0]["text"]
            .as_str()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(unzoned["configured"], true);
    assert!(unzoned.get("zone").is_none());
}

fn tool_call(id: u64, name: &str, arguments: Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": "tools/call",
        "params": { "name": name, "arguments": arguments }
    })
}

#[test]
fn mcp_churn_tool_reports_file_churn_and_handles_non_git_directory() {
    let repo = init_sample_repo();
    let non_git = TempDir::new().unwrap();
    let responses = send_mcp_requests(
        repo.path(),
        &[
            tool_call(1, "churn", json!({"commits": 10})),
            tool_call(
                2,
                "churn",
                json!({"path": non_git.path().to_string_lossy()}),
            ),
        ],
    );
    assert_eq!(responses.len(), 2);
    assert_eq!(responses[0]["result"]["isError"], false);
    let payload: Value = serde_json::from_str(
        responses[0]["result"]["content"][0]["text"]
            .as_str()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(payload["commits_analyzed"], 1);
    assert_eq!(payload["shallow"], false);
    assert!(payload.get("score").is_none());

    assert_eq!(responses[1]["result"]["isError"], true);
    let err_text = responses[1]["result"]["content"][0]["text"]
        .as_str()
        .unwrap();
    assert!(
        err_text.contains("not inside a Git repository"),
        "err: {err_text}"
    );
}
