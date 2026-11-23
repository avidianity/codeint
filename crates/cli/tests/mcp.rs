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
    assert_eq!(
        names,
        ["check", "diff", "trend", "guard", "impact", "trace"]
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

#[test]
fn mcp_check_and_diff_surface_function_names_and_cognitive_complexity() {
    let source = concat!(
        "export function processOrder(a: boolean, b: boolean, c: boolean, d: boolean, e: boolean, f: boolean, x: number): number {\n",
        "  if (a) {\n",
        "    if (b) {\n",
        "      if (c) {\n",
        "        if (d) {\n",
        "          if (e) {\n",
        "            if (f) {\n",
        "              if (x === 1 || x === 2 || x === 3 || x === 4 || x === 5) {\n",
        "                return 1;\n",
        "              }\n",
        "            }\n",
        "          }\n",
        "        }\n",
        "      }\n",
        "    }\n",
        "  }\n",
        "  return 0;\n",
        "}\n",
    );
    let repo = init_git_repo(&[("src/order.ts", source)]);
    let responses = send_mcp_requests(
        repo.path(),
        &[
            mcp_tool_call(1, "check", json!({})),
            mcp_tool_call(2, "diff", json!({"base_ref": "main"})),
        ],
    );
    let check_payload = mcp_payload(&responses[0]);
    let prod = &check_payload["report"]["source_sets"]["production"];
    assert_eq!(prod["erosion"]["findings"][0]["name"], "processOrder");
    assert_eq!(prod["erosion"]["findings"][0]["line"], 1);
    assert_eq!(prod["erosion"]["findings"][0]["end_line"], 18);
    assert!(
        prod["erosion"]["findings"][0]["cognitive_complexity"]
            .as_u64()
            .unwrap()
            > 15
    );
    assert_eq!(
        prod["cognitive_complexity"]["findings"][0]["name"],
        "processOrder"
    );

    let diff_payload = mcp_payload(&responses[1]);
    assert_eq!(
        diff_payload["report"]["source_sets"]["production"]["cognitive_complexity"]["unchanged"],
        1
    );
}
