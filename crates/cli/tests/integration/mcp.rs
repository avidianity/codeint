use std::process::Command;

use serde_json::{Value, json};
use tempfile::TempDir;

use crate::common::{init_git_repo, mcp_payload, mcp_tool_call, send_mcp_requests};

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
    let (check_payload, diff_payload) = run_mcp_check_and_diff(repo.path());
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
    assert_eq!(
        diff_payload["report"]["source_sets"]["production"]["cognitive_complexity"]["unchanged"],
        1
    );
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

#[test]
fn mcp_churn_and_coupling_reject_non_object_arguments_and_wrong_types() {
    let repo = init_sample_repo();
    let responses = send_mcp_requests(
        repo.path(),
        &[
            mcp_tool_call(1, "churn", json!("not-an-object")),
            mcp_tool_call(2, "churn", json!([])),
            mcp_tool_call(3, "churn", json!(null)),
            mcp_tool_call(4, "churn", json!({"all": "true"})),
            mcp_tool_call(5, "churn", json!({"path": 42})),
            mcp_tool_call(6, "coupling", json!([1, 2])),
            mcp_tool_call(7, "coupling", json!({"include_tests": "true"})),
            mcp_tool_call(8, "coupling", json!({"all": 1})),
            mcp_tool_call(9, "coupling", json!({"path": true})),
        ],
    );
    assert_eq!(responses.len(), 9);
    assert_tool_err_contains(&responses[0], "JSON object");
    assert_tool_err_contains(&responses[1], "JSON object");
    assert_tool_err_contains(&responses[2], "JSON object");
    assert_tool_err_contains(&responses[3], "all");
    assert_tool_err_contains(&responses[4], "path");
    assert_tool_err_contains(&responses[5], "JSON object");
    assert_tool_err_contains(&responses[6], "include_tests");
    assert_tool_err_contains(&responses[7], "all");
    assert_tool_err_contains(&responses[8], "path");
}
