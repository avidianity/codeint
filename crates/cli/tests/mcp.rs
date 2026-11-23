mod common;

use std::process::Command;

use common::{init_git_repo, mcp_payload, mcp_tool_call, send_mcp_requests};
use serde_json::{Value, json};
use tempfile::TempDir;

#[test]
fn mcp_initialize_list_and_call_tools() {
    let repo = init_git_repo(&[(
        "app.ts",
        "export function greet(x: number): number {\n  return x + 1;\n}\n",
    )]);
    let responses = send_mcp_requests(
        repo.path(),
        &[
            json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": "2024-11-05"}}),
            json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
            json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"}),
            mcp_tool_call(3, "check", json!({})),
            mcp_tool_call(4, "diff", json!({"base_ref": "main"})),
            mcp_tool_call(5, "trend", json!({})),
            mcp_tool_call(6, "impact", json!({"paths": ["app.ts"]})),
            json!({"jsonrpc": "2.0", "id": 7, "method": "unknown/method"}),
        ],
    );
    assert_eq!(responses.len(), 7);
    assert_eq!(responses[0]["result"]["serverInfo"]["name"], "codeint");
    let tools = responses[1]["result"]["tools"].as_array().unwrap();
    let names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
    assert_eq!(
        names,
        [
            "check", "diff", "trend", "guard", "impact", "trace", "churn", "coupling"
        ]
    );

    let check_payload = mcp_payload(&responses[2]);
    assert_eq!(check_payload["passed"], true);
    assert_eq!(check_payload["report"]["files"], 1);

    assert_eq!(responses[3]["result"]["isError"], false);
    assert_eq!(responses[4]["result"]["isError"], false);
    let impact_payload = mcp_payload(&responses[5]);
    assert_eq!(impact_payload["targets"][0]["path"], "app.ts");
    assert_eq!(impact_payload["summary"]["total_dependents"], 0);
    assert_eq!(responses[6]["error"]["code"], -32601);
}

#[test]
fn mcp_guard_tool_reports_existing_planned_unzoned_and_unconfigured_paths() {
    let repo = init_git_repo(&[(
        "app.ts",
        "export function greet(x: number): number {\n  return x + 1;\n}\n",
    )]);
    let unconfigured = send_mcp_requests(
        repo.path(),
        &[mcp_tool_call(1, "guard", json!({"file": "app.ts"}))],
    );
    let unconfigured_payload = mcp_payload(&unconfigured[0]);
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
            mcp_tool_call(1, "guard", json!({"file": "src/core/internal.ts"})),
            mcp_tool_call(2, "guard", json!({"file": "src/web/planned/view.ts"})),
            mcp_tool_call(3, "guard", json!({"file": "app.ts"})),
        ],
    );

    assert_eq!(responses.len(), 3);

    let existing = mcp_payload(&responses[0]);
    assert_eq!(existing["zone"], "core");
    assert_eq!(existing["exists"], true);
    assert_eq!(existing["is_public_file"], false);
    assert_eq!(existing["allowed_importers"], json!(["core"]));
    assert_eq!(existing["denied_targets"][0]["zone"], "adapter");
    assert_eq!(
        existing["denied_targets"][0]["reason"],
        "Core must stay independent of adapters"
    );

    let planned = mcp_payload(&responses[1]);
    assert_eq!(planned["zone"], "web");
    assert_eq!(planned["exists"], false);
    assert_eq!(planned["denied_targets"][0]["zone"], "adapter");

    let unzoned = mcp_payload(&responses[2]);
    assert_eq!(unzoned["configured"], true);
    assert!(unzoned.get("zone").is_none());
}

fn init_sample_repo() -> TempDir {
    init_git_repo(&[(
        "app.ts",
        "export function greet(x: number): number {\n  return x + 1;\n}\n",
    )])
}

fn assert_tool_err_contains(response: &Value, needle: &str) {
    assert_eq!(response["result"]["isError"], true);
    let err_text = response["result"]["content"][0]["text"].as_str().unwrap();
    assert!(err_text.contains(needle), "err: {err_text}");
}

#[test]
fn mcp_churn_tool_reports_file_churn_and_handles_non_git_directory() {
    let repo = init_sample_repo();
    let non_git = TempDir::new().unwrap();
    let responses = send_mcp_requests(
        repo.path(),
        &[
            mcp_tool_call(1, "churn", json!({"commits": 10})),
            mcp_tool_call(
                2,
                "churn",
                json!({"path": non_git.path().to_string_lossy()}),
            ),
        ],
    );
    assert_eq!(responses.len(), 2);
    let payload = mcp_payload(&responses[0]);
    assert_eq!(payload["commits_analyzed"], 1);
    assert_eq!(payload["shallow"], false);
    assert!(payload.get("score").is_none());
    assert_tool_err_contains(&responses[1], "not inside a Git repository");
}

#[test]
fn mcp_coupling_tool_reports_co_change_pairs_bulk_exclusion_and_errors() {
    let repo = init_sample_repo();
    let commit_pair = |step: usize, when: &str| {
        std::fs::create_dir_all(repo.path().join("src")).unwrap();
        std::fs::write(
            repo.path().join("src/a.ts"),
            format!("export const a = {step};\n"),
        )
        .unwrap();
        std::fs::write(
            repo.path().join("src/b.ts"),
            format!("export const b = {step};\n"),
        )
        .unwrap();
        for args in [vec!["add", "-A"], vec!["commit", "-q", "-m", "pair"]] {
            let status = Command::new("git")
                .args(&args)
                .current_dir(repo.path())
                .env("GIT_CONFIG_GLOBAL", "/dev/null")
                .env("GIT_CONFIG_NOSYSTEM", "1")
                .env("GIT_AUTHOR_NAME", "Test")
                .env("GIT_AUTHOR_EMAIL", "test@example.com")
                .env("GIT_COMMITTER_NAME", "Test")
                .env("GIT_COMMITTER_EMAIL", "test@example.com")
                .env("GIT_AUTHOR_DATE", when)
                .env("GIT_COMMITTER_DATE", when)
                .status()
                .unwrap();
            assert!(status.success(), "git {args:?} failed");
        }
    };
    commit_pair(1, "2026-05-01T10:00:00Z");
    commit_pair(2, "2026-05-02T10:00:00Z");
    commit_pair(3, "2026-05-03T10:00:00Z");

    let non_git = TempDir::new().unwrap();
    let responses = send_mcp_requests(
        repo.path(),
        &[
            mcp_tool_call(
                1,
                "coupling",
                json!({"commits": 10, "max_commit_files": 20, "min_co_changes": 3, "min_ratio": 0.5}),
            ),
            mcp_tool_call(
                2,
                "coupling",
                json!({"path": non_git.path().to_string_lossy()}),
            ),
        ],
    );
    assert_eq!(responses.len(), 2);
    let payload = mcp_payload(&responses[0]);
    assert_eq!(payload["commits_analyzed"], 4);
    assert_eq!(payload["pairs"][0]["file_a"], "src/a.ts");
    assert_eq!(payload["pairs"][0]["file_b"], "src/b.ts");
    assert_eq!(payload["pairs"][0]["co_change_count"], 3);
    assert_eq!(payload["pairs"][0]["coupling_ratio"], 1.0);
    assert_eq!(payload["pairs"][0]["has_direct_import"], false);
    assert!(payload.get("score").is_none());
    assert_tool_err_contains(&responses[1], "not inside a Git repository");
}
