mod common;

use common::{init_git_repo, mcp_payload, mcp_tool_call, send_mcp_requests};
use serde_json::{Value, json};
use tempfile::TempDir;

fn init_sample_repo() -> TempDir {
    init_git_repo(&[(
        "app.ts",
        "export function greet(x: number): number {\n  return x + 1;\n}\n",
    )])
}

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

const DUPLICATED_INVOICE_TS: &str = concat!(
    "export function computeInvoice(entries: { unit: number; tax: number; count: number }[]): number {\n",
    "  let accumulator = 0;\n",
    "  for (const entry of entries.filter(Boolean)) {\n",
    "    if (entry.count > 0 && entry.unit >= 0) {\n",
    "      accumulator += (entry.unit + entry.tax) * entry.count;\n",
    "    }\n",
    "  }\n",
    "  return Math.floor(accumulator * 100) / 100;\n",
    "}\n",
);

fn run_mcp_check_and_diff(repo: &std::path::Path) -> (Value, Value) {
    let responses = send_mcp_requests(
        repo,
        &[
            mcp_tool_call(1, "check", json!({})),
            mcp_tool_call(2, "diff", json!({"base_ref": "main"})),
        ],
    );
    assert_eq!(responses.len(), 2);
    (mcp_payload(&responses[0]), mcp_payload(&responses[1]))
}

#[test]
fn mcp_check_and_diff_expose_clone_families_for_three_or_more_copies() {
    let repo = init_sample_repo();
    std::fs::write(repo.path().join("c.ts"), DUPLICATED_INVOICE_TS).unwrap();
    std::fs::write(repo.path().join("a.ts"), DUPLICATED_INVOICE_TS).unwrap();
    std::fs::write(repo.path().join("b.ts"), DUPLICATED_INVOICE_TS).unwrap();

    let (check_payload, diff_payload) = run_mcp_check_and_diff(repo.path());

    let families =
        check_payload["report"]["source_sets"]["production"]["duplication"]["clone_families"]
            .as_array()
            .expect("check clone_families");
    assert_eq!(families.len(), 1);
    assert_eq!(families[0]["duplicated_lines"], 27);
    let paths: Vec<&str> = families[0]["locations"]
        .as_array()
        .unwrap()
        .iter()
        .map(|loc| loc["path"].as_str().unwrap())
        .collect();
    assert_eq!(paths, ["a.ts", "b.ts", "c.ts"]);

    let new_families =
        diff_payload["report"]["source_sets"]["production"]["duplication"]["clone_families"]["new"]
            .as_array()
            .expect("diff new clone_families");
    assert_eq!(new_families.len(), 1);
    assert_eq!(new_families[0]["id"], families[0]["id"]);
}

#[test]
fn mcp_check_and_diff_expose_near_clone_similarity_and_families() {
    let repo = init_sample_repo();
    std::fs::write(
        repo.path().join("codeint.json"),
        r#"{ "duplication": { "near_clones": true, "min_similarity": 0.85 } }"#,
    )
    .unwrap();
    let copy_one = DUPLICATED_INVOICE_TS.replacen(
        "  let accumulator = 0;\n",
        "  let accumulator = 0;\n  console.log(entries.length);\n",
        1,
    );
    let copy_two = DUPLICATED_INVOICE_TS.replacen(
        "      accumulator += (entry.unit + entry.tax) * entry.count;\n",
        "      accumulator += (entry.unit + entry.tax) * entry.count;\n      console.log(accumulator);\n",
        1,
    );
    std::fs::write(repo.path().join("one.ts"), DUPLICATED_INVOICE_TS).unwrap();
    std::fs::write(repo.path().join("two.ts"), copy_one).unwrap();
    std::fs::write(repo.path().join("three.ts"), copy_two).unwrap();

    let (check_payload, diff_payload) = run_mcp_check_and_diff(repo.path());

    let dup = &check_payload["report"]["source_sets"]["production"]["duplication"];
    assert_eq!(dup["near_clones"]["min_similarity"], 0.85);
    let groups = dup["clone_groups"].as_array().expect("clone_groups");
    assert_eq!(groups.len(), 1);
    assert!(groups[0]["similarity"].as_f64().unwrap() >= 0.85);
    let families = dup["clone_families"].as_array().expect("clone_families");
    assert_eq!(families.len(), 1);

    assert_eq!(diff_payload["passed"], false);
    let new_groups = diff_payload["report"]["source_sets"]["production"]["duplication"]["new"]
        .as_array()
        .expect("new near-clone groups");
    assert_eq!(new_groups.len(), 1);
    assert_eq!(new_groups[0]["id"], groups[0]["id"]);
    assert!(new_groups[0]["similarity"].as_f64().unwrap() >= 0.85);
}
