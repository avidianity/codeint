use std::process::Command;

use serde_json::{Value, json};
use tempfile::TempDir;

use crate::common::{
    eroded_ts, init_git_repo, mcp_payload, mcp_tool_call, run_codeint_text, send_mcp_requests,
};

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

#[test]
fn mcp_check_tool_bypasses_the_baseline_with_ignore_baseline() {
    let repo = init_git_repo(&[
        (
            "codeint.json",
            r#"{ "budgets": { "production": { "erosion": 0 } } }"#,
        ),
        ("src/legacy.ts", &eroded_ts("legacy", 12)),
    ]);
    run_codeint_text(repo.path(), &["baseline", "update"], 0);

    let baseline_path = repo.path().join(".codeint/baseline.json");
    let mut stale: Value =
        serde_json::from_str(&std::fs::read_to_string(&baseline_path).unwrap()).unwrap();
    stale["source_sets"]["production"]["erosion"]["metric_version"] = json!(99);
    std::fs::write(&baseline_path, stale.to_string()).unwrap();

    let responses = send_mcp_requests(
        repo.path(),
        &[
            mcp_tool_call(1, "check", json!({})),
            mcp_tool_call(2, "check", json!({"ignore_baseline": true})),
            mcp_tool_call(3, "check", json!({"ignore_baseline": "yes"})),
        ],
    );
    assert_tool_err_contains(&responses[0], "ignore_baseline");
    let stale_text = responses[0]["result"]["content"][0]["text"]
        .as_str()
        .unwrap();
    assert!(
        !stale_text.contains("--ignore-baseline"),
        "err: {stale_text}"
    );

    let bypassed = mcp_payload(&responses[1]);
    assert_eq!(bypassed["passed"], false);
    assert!(bypassed["report"].get("baseline").is_none());
    assert_tool_err_contains(&responses[2], "ignore_baseline");

    let tools = send_mcp_requests(
        repo.path(),
        &[json!({"jsonrpc": "2.0", "id": 1, "method": "tools/list"})],
    );
    let check = &tools[0]["result"]["tools"][0];
    assert_eq!(
        check["inputSchema"]["properties"]["ignore_baseline"]["type"],
        "boolean"
    );
}

#[test]
fn mcp_all_eight_tools_default_to_toon_and_support_json_format() {
    let repo = init_git_repo(&[
        ("codeint.json", r#"{ "format": "text" }"#),
        (
            "src/a.ts",
            "import { b } from './b';\nexport function a(x: number): number { return b(x) + 1; }\n",
        ),
        (
            "src/b.ts",
            "export function b(x: number): number { return x * 2; }\n",
        ),
    ]);

    let list_responses = send_mcp_requests(
        repo.path(),
        &[json!({"jsonrpc": "2.0", "id": 1, "method": "tools/list"})],
    );
    let tools = list_responses[0]["result"]["tools"].as_array().unwrap();
    assert_eq!(tools.len(), 8);
    for tool in tools {
        let name = tool["name"].as_str().unwrap();
        let format_prop = &tool["inputSchema"]["properties"]["format"];
        assert_eq!(format_prop["type"], "string", "tool {name} format type");
        assert_eq!(
            format_prop["enum"],
            json!(["toon", "json"]),
            "tool {name} format enum"
        );
        assert_eq!(format_prop["default"], "toon", "tool {name} format default");
    }

    let cases: Vec<(&str, Value)> = vec![
        ("check", json!({})),
        ("diff", json!({"base_ref": "main"})),
        ("trend", json!({})),
        ("guard", json!({"file": "src/a.ts"})),
        ("impact", json!({"paths": ["src/b.ts"]})),
        ("trace", json!({"from": "src/a.ts", "to": "src/b.ts"})),
        ("churn", json!({"commits": 5})),
        ("coupling", json!({"commits": 5})),
    ];

    let mut requests = Vec::new();
    let mut id = 1u64;
    for (name, base_args) in &cases {
        requests.push(mcp_tool_call(id, name, base_args.clone()));
        id += 1;
        let mut toon_args = base_args.clone();
        toon_args["format"] = json!("toon");
        requests.push(mcp_tool_call(id, name, toon_args));
        id += 1;
        let mut json_args = base_args.clone();
        json_args["format"] = json!("json");
        requests.push(mcp_tool_call(id, name, json_args));
        id += 1;
    }

    let responses = send_mcp_requests(repo.path(), &requests);
    assert_eq!(responses.len(), cases.len() * 3);

    for (idx, (name, _)) in cases.iter().enumerate() {
        let default_resp = &responses[idx * 3];
        let toon_resp = &responses[idx * 3 + 1];
        let json_resp = &responses[idx * 3 + 2];

        assert_eq!(default_resp["result"]["isError"], false, "{name} default");
        assert_eq!(toon_resp["result"]["isError"], false, "{name} toon");
        assert_eq!(json_resp["result"]["isError"], false, "{name} json");

        let default_text = default_resp["result"]["content"][0]["text"]
            .as_str()
            .unwrap();
        let toon_text = toon_resp["result"]["content"][0]["text"].as_str().unwrap();
        let json_text = json_resp["result"]["content"][0]["text"].as_str().unwrap();

        assert!(
            !default_text.trim_start().starts_with('{'),
            "{name} default output must be TOON, not JSON: {default_text}"
        );
        assert_eq!(
            default_text, toon_text,
            "{name} default output must match explicit format=toon"
        );

        let mut decoded_toon: Value = toon_format::decode_default(default_text)
            .unwrap_or_else(|err| panic!("{name} invalid TOON: {err}\n{default_text}"));
        let mut parsed_json: Value = serde_json::from_str(json_text)
            .unwrap_or_else(|err| panic!("{name} invalid JSON: {err}\n{json_text}"));
        normalize_json_numbers(&mut decoded_toon);
        normalize_json_numbers(&mut parsed_json);
        assert_eq!(
            decoded_toon, parsed_json,
            "{name} TOON and JSON payloads must match"
        );
    }
}

fn normalize_json_numbers(value: &mut Value) {
    match value {
        Value::Number(number) => {
            if let Some(float) = number.as_f64()
                && let Some(normalized) = serde_json::Number::from_f64(float)
            {
                *number = normalized;
            }
        }
        Value::Array(items) => {
            for item in items {
                normalize_json_numbers(item);
            }
        }
        Value::Object(map) => {
            for entry in map.values_mut() {
                normalize_json_numbers(entry);
            }
        }
        _ => {}
    }
}

#[test]
fn mcp_tools_reject_invalid_format_values() {
    let repo = init_sample_repo();
    let responses = send_mcp_requests(
        repo.path(),
        &[
            mcp_tool_call(1, "check", json!({"format": "text"})),
            mcp_tool_call(2, "diff", json!({"base_ref": "main", "format": "sarif"})),
            mcp_tool_call(3, "trend", json!({"format": "markdown"})),
            mcp_tool_call(4, "guard", json!({"file": "app.ts", "format": "yaml"})),
            mcp_tool_call(5, "impact", json!({"paths": ["app.ts"], "format": 42})),
            mcp_tool_call(
                6,
                "trace",
                json!({"from": "app.ts", "to": "app.ts", "format": true}),
            ),
            mcp_tool_call(7, "churn", json!({"format": ""})),
            mcp_tool_call(8, "coupling", json!({"format": ["json"]})),
        ],
    );
    assert_eq!(responses.len(), 8);
    for (idx, resp) in responses.iter().enumerate() {
        assert_tool_err_contains(resp, "format");
        let text = resp["result"]["content"][0]["text"].as_str().unwrap();
        assert!(
            text.contains("toon") && text.contains("json"),
            "response {idx} must name allowed formats: {text}"
        );
    }
}

#[test]
fn mcp_all_eight_tools_return_unified_mcp_error_remedies() {
    let repo = init_git_repo(&[
        (
            "codeint.json",
            r#"{ "budgets": { "production": { "test_gap": 0.5 } } }"#,
        ),
        ("app.ts", "export const x = 1;\n"),
    ]);
    let non_git = TempDir::new().unwrap();
    let non_git_path = non_git.path().to_string_lossy().into_owned();

    let responses = send_mcp_requests(
        repo.path(),
        &[
            mcp_tool_call(1, "check", json!({})),
            mcp_tool_call(
                2,
                "diff",
                json!({"base_ref": "main", "package": "packages/missing"}),
            ),
            mcp_tool_call(3, "trend", json!({"path": non_git_path})),
            mcp_tool_call(4, "guard", json!({"file": "app.ts", "path": non_git_path})),
            mcp_tool_call(5, "impact", json!({})),
            mcp_tool_call(
                6,
                "trace",
                json!({"from": "app.ts", "to": "app.ts", "package": "packages/missing"}),
            ),
            mcp_tool_call(7, "churn", json!({"path": non_git_path})),
            mcp_tool_call(8, "coupling", json!({"path": non_git_path})),
        ],
    );
    assert_eq!(responses.len(), 8);

    let forbidden_cli_hints = [
        "--coverage-report",
        "--package",
        "--changed",
        "--head",
        "--max-paths",
        "--ignore-baseline",
        "codeint check <path>",
    ];
    for (idx, resp) in responses.iter().enumerate() {
        assert_eq!(resp["result"]["isError"], true, "case {idx} must error");
        let text = resp["result"]["content"][0]["text"].as_str().unwrap();
        for hint in forbidden_cli_hints {
            assert!(
                !text.contains(hint),
                "case {idx} leaked CLI hint {hint:?}: {text}"
            );
        }
    }

    assert_tool_err_contains(&responses[0], "coverage_reports");
    assert_tool_err_contains(&responses[1], "package");
    assert_tool_err_contains(&responses[4], "changed");
    assert_tool_err_contains(&responses[5], "package");
}

#[test]
fn mcp_trace_rejects_zero_max_paths_at_argument_validation() {
    let non_git = TempDir::new().unwrap();
    let responses = send_mcp_requests(
        non_git.path(),
        &[
            mcp_tool_call(
                1,
                "trace",
                json!({"from": "src/a.ts", "to": "src/b.ts", "max_paths": 0}),
            ),
            mcp_tool_call(
                2,
                "trace",
                json!({"from": "src/a.ts", "to": "src/b.ts", "max_paths": -1}),
            ),
            mcp_tool_call(
                3,
                "trace",
                json!({"from": "src/a.ts", "to": "src/b.ts", "max_paths": "2"}),
            ),
        ],
    );
    assert_eq!(responses.len(), 3);
    for (idx, resp) in responses.iter().enumerate() {
        assert_tool_err_contains(resp, "max_paths");
        let text = resp["result"]["content"][0]["text"].as_str().unwrap();
        assert!(
            !text.contains("--max-paths") && !text.contains("not inside a Git repository"),
            "case {idx} must fail at argument validation without CLI flag hints: {text}"
        );
    }
}

#[test]
fn mcp_git_remedies_cover_impact_no_merge_base_and_unborn_head() {
    let repo = init_git_repo(&[("app.ts", "export const x = 1;\n")]);
    let run_git = |args: &[&str]| {
        let output = Command::new("git")
            .args(args)
            .current_dir(repo.path())
            .envs([
                ("GIT_CONFIG_GLOBAL", "/dev/null"),
                ("GIT_CONFIG_NOSYSTEM", "1"),
                ("GIT_AUTHOR_NAME", "Test"),
                ("GIT_AUTHOR_EMAIL", "test@example.invalid"),
                ("GIT_COMMITTER_NAME", "Test"),
                ("GIT_COMMITTER_EMAIL", "test@example.invalid"),
            ])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    };
    run_git(&["checkout", "--orphan", "unborn"]);
    run_git(&["rm", "-rf", "."]);

    let unborn_responses = send_mcp_requests(
        repo.path(),
        &[
            mcp_tool_call(1, "diff", json!({"base_ref": "main"})),
            mcp_tool_call(2, "impact", json!({"changed": "main"})),
        ],
    );
    assert_eq!(unborn_responses.len(), 2);
    for (idx, resp) in unborn_responses.iter().enumerate() {
        assert_eq!(
            resp["result"]["isError"], true,
            "unborn case {idx} must error"
        );
        let text = resp["result"]["content"][0]["text"].as_str().unwrap();
        assert!(
            text.contains("MCP tool"),
            "unborn case {idx} must return MCP guidance: {text}"
        );
        assert!(
            !text.contains("codeint diff"),
            "unborn case {idx} must not leak CLI command: {text}"
        );
    }

    std::fs::write(repo.path().join("other.ts"), "export const y = 2;\n").unwrap();
    run_git(&["add", "other.ts"]);
    run_git(&["commit", "-m", "unrelated root"]);

    let no_merge_base_responses = send_mcp_requests(
        repo.path(),
        &[mcp_tool_call(3, "impact", json!({"changed": "main"}))],
    );
    assert_eq!(no_merge_base_responses.len(), 1);
    let impact_resp = &no_merge_base_responses[0];
    assert_eq!(impact_resp["result"]["isError"], true);
    let impact_text = impact_resp["result"]["content"][0]["text"]
        .as_str()
        .unwrap();
    assert!(
        impact_text.contains("changed"),
        "impact no-merge-base remedy must name changed: {impact_text}"
    );
    assert!(
        !impact_text.contains("base_ref") && !impact_text.contains("head_ref"),
        "impact no-merge-base remedy must not name diff arguments: {impact_text}"
    );
}
