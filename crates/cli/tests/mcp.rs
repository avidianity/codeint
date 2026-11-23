mod common;

use common::{init_git_repo, mcp_payload, mcp_tool_call, send_mcp_requests};
use serde_json::json;

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
    assert_eq!(names, ["check", "diff", "trend", "impact", "trace"]);

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
