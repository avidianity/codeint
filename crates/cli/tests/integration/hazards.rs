use std::fs;
use std::path::Path;

use codeint_cli::{DiffOptions, ReportFormat, diff};
use serde_json::{Value, json};
use tempfile::TempDir;

use crate::common::{
    init_git_repo, mcp_payload, mcp_tool_call, run_check_json as check_json, run_codeint_text,
    send_mcp_requests,
};
use crate::history_support::HistoryFixture;

fn run_stdout(dir: &Path, args: &[&str], expected_code: i32) -> String {
    run_codeint_text(dir, args, expected_code).0
}

fn cached_base_hazards_ratio(root: &Path, options: &DiffOptions, config_json: &str) -> Value {
    fs::write(root.join("codeint.json"), config_json).expect("write config");
    let outcome = diff(root, "HEAD~1", options).expect("diff with cache");
    let rendered = outcome.report.render(ReportFormat::Json).expect("render");
    let parsed: Value = serde_json::from_str(&rendered).expect("valid diff json");
    parsed["source_sets"]["production"]["hazards"]["base_ratio"].clone()
}

#[test]
fn hazards_absent_for_non_sql_and_isolated_in_multi_language_repos_across_formats_and_mcp() {
    let ts_only = init_git_repo(&[
        (
            "codeint.json",
            r#"{"budgets":{"production":{"hazards":0}}}"#,
        ),
        ("src/app.ts", "export const value = 1;\n"),
    ]);
    let ts_report = check_json(ts_only.path());
    assert!(
        ts_report["source_sets"]["production"]
            .get("hazards")
            .is_none(),
        "ts_report: {ts_report}"
    );
    assert!(ts_report.get("over_budget").is_none());

    let mixed = init_git_repo(&[
        (
            "src/app.ts",
            "export const a = 1;\nexport const b = 2;\nexport const c = 3;\nexport const d = 4;\n",
        ),
        (
            "queries/users.sql",
            "SELECT * FROM users WHERE id = 1;\nSELECT id FROM users WHERE id = 2;\n",
        ),
    ]);
    let report = check_json(mixed.path());
    let prod_hazards = &report["source_sets"]["production"]["hazards"];
    assert_eq!(prod_hazards["hazard_lines"], 1, "{report}");
    assert_eq!(prod_hazards["source_lines"], 2, "{report}");
    assert_eq!(prod_hazards["ratio"], 0.5, "{report}");
    assert_eq!(prod_hazards["findings"][0]["path"], "queries/users.sql");
    assert_eq!(prod_hazards["findings"][0]["line"], 1);
    assert_eq!(prod_hazards["findings"][0]["rule"], "select-star");

    assert!(
        report["languages"]["typescript-javascript"]["source_sets"]["production"]
            .get("hazards")
            .is_none()
    );
    assert_eq!(
        report["languages"]["sql"]["source_sets"]["production"]["hazards"]["ratio"],
        0.5
    );

    let toon = run_stdout(mixed.path(), &["check", "--format", "toon"], 0);
    assert!(toon.contains("hazards:"), "toon: {toon}");
    assert!(toon.contains("select-star"), "toon: {toon}");

    let md = run_stdout(mixed.path(), &["check", "--format", "markdown"], 0);
    assert!(md.contains("| Production | Hazards |"), "md: {md}");
    assert!(md.contains("select-star"), "md: {md}");

    let sarif: Value = serde_json::from_str(&run_stdout(
        mixed.path(),
        &["check", "--format", "sarif"],
        0,
    ))
    .expect("valid sarif");
    let sarif_rules: Vec<&str> = sarif["runs"][0]["results"]
        .as_array()
        .expect("sarif results")
        .iter()
        .filter_map(|r| r["ruleId"].as_str())
        .collect();
    assert!(
        sarif_rules
            .iter()
            .any(|id| id.contains("hazards") || id.contains("select-star")),
        "sarif: {sarif}"
    );

    let mcp = send_mcp_requests(mixed.path(), &[mcp_tool_call(1, "check", json!({}))]);
    let payload = mcp_payload(&mcp[0]);
    assert_eq!(payload["passed"], true);
    assert_eq!(
        payload["report"]["source_sets"]["production"]["hazards"]["ratio"],
        0.5
    );
    assert_eq!(
        payload["report"]["source_sets"]["production"]["hazards"]["findings"][0]["rule"],
        "select-star"
    );
}

#[test]
fn hazards_budgets_generated_vs_production_and_disabled_rules_errors() {
    let repo = init_git_repo(&[
        (
            "queries/users.sql",
            "SELECT * FROM users WHERE id = 1;\nSELECT id FROM users WHERE id = 2;\n",
        ),
        ("drizzle/0001_drop.sql", "DROP TABLE legacy_items;\n"),
    ]);

    let base = check_json(repo.path());
    assert_eq!(
        base["source_sets"]["production"]["hazards"]["ratio"], 0.5,
        "{base}"
    );
    assert_eq!(
        base["source_sets"]["generated"]["hazards"]["ratio"], 1.0,
        "{base}"
    );
    assert_eq!(
        base["source_sets"]["generated"]["hazards"]["findings"][0]["rule"],
        "drop"
    );

    fs::write(
        repo.path().join("codeint.json"),
        r#"{"budgets":{"production":{"hazards":0.5}}}"#,
    )
    .expect("write equal budget");
    run_stdout(repo.path(), &["check", "--format", "json"], 0);

    fs::write(
        repo.path().join("codeint.json"),
        r#"{"budgets":{"production":{"hazards":0.4}}}"#,
    )
    .expect("write strict budget");
    let over: Value =
        serde_json::from_str(&run_stdout(repo.path(), &["check", "--format", "json"], 1))
            .expect("valid json");
    assert_eq!(over["over_budget"][0]["metric"], "hazards");

    let mcp = send_mcp_requests(repo.path(), &[mcp_tool_call(1, "check", json!({}))]);
    let payload = mcp_payload(&mcp[0]);
    assert_eq!(payload["passed"], false);
    assert!(
        payload["failures"][0]
            .as_str()
            .unwrap_or_default()
            .contains("hazards"),
        "payload: {payload}"
    );

    fs::write(
        repo.path().join("codeint.json"),
        r#"{"hazards":{"disabled_rules":["no-such-hazard-rule"]}}"#,
    )
    .expect("write invalid rule");
    for args in [&["check"][..], &["diff", "main"][..], &["trend"][..]] {
        let (_, stderr) = run_codeint_text(repo.path(), args, 2);
        assert!(
            stderr.contains("hazards.disabled_rules") && stderr.contains("no-such-hazard-rule"),
            "args {args:?}: stderr: {stderr}"
        );
    }
}

#[test]
fn hazards_diff_regression_limits_diff_sarif_trend_and_result_cache_invalidation() {
    let fixture = HistoryFixture::init();
    fixture.save_and_commit(
        &[("src/app.ts", "export const x = 1;\n".to_owned())],
        "ts only",
        "2026-05-01T10:00:00Z",
    );
    fixture.save_and_commit(
        &[(
            "queries/a.sql",
            "SELECT * FROM users WHERE id = 1;\nSELECT id FROM users WHERE id = 2;\n".to_owned(),
        )],
        "add first sql",
        "2026-05-02T10:00:00Z",
    );
    fixture.save_and_commit(
        &[(
            "queries/b.sql",
            "DELETE FROM sessions;\nSELECT id FROM accounts WHERE id = 3;\n".to_owned(),
        )],
        "add second sql",
        "2026-05-03T10:00:00Z",
    );

    let trend = fixture.run_json("trend", &[]);
    let points = trend["source_sets"]["production"]
        .as_array()
        .expect("trend production points");
    assert_eq!(points.len(), 3, "trend: {trend}");
    assert!(
        points[0].get("hazards").is_none(),
        "pre-SQL commit must omit hazards: {trend}"
    );
    assert_eq!(points[1]["hazards"], 0.5, "trend: {trend}");
    assert_eq!(points[2]["hazards"], 0.5, "trend: {trend}");

    let diff_out = fixture.run("diff", &["HEAD~1", "--format", "json"]);
    assert_eq!(
        diff_out.status.code(),
        Some(1),
        "new hazard finding should regress by default: {}",
        String::from_utf8_lossy(&diff_out.stderr)
    );
    let diff_json: Value = serde_json::from_slice(&diff_out.stdout).expect("valid diff json");
    let new_hazards = diff_json["source_sets"]["production"]["hazards"]["new"]
        .as_array()
        .expect("new hazards");
    assert_eq!(new_hazards.len(), 1, "diff: {diff_json}");
    assert_eq!(new_hazards[0]["rule"], "delete-without-filter");

    let diff_sarif: Value = serde_json::from_str(&run_stdout(
        fixture.root(),
        &["diff", "HEAD~1", "--format", "sarif"],
        1,
    ))
    .expect("valid diff sarif");
    let sarif_results = diff_sarif["runs"][0]["results"]
        .as_array()
        .expect("sarif results");
    assert_eq!(sarif_results.len(), 1, "diff sarif: {diff_sarif}");
    assert_eq!(
        sarif_results[0]["locations"][0]["physicalLocation"]["artifactLocation"]["uri"],
        "queries/b.sql"
    );

    let root = fixture.root();
    for (config_json, expected_code) in [
        (
            r#"{"regression_limits":{"hazards":{"ratio_rise":1.0,"new_findings":"never"}}}"#,
            0,
        ),
        (
            r#"{"regression_limits":{"hazards":{"ratio_rise":1.0,"new_findings":"never"},"production":{"hazards":{"ratio_rise":1.0,"new_findings":"any_new_finding"}}}}"#,
            1,
        ),
        (
            r#"{"regression_limits":{"production":{"hazards":{"ratio_rise":1.0,"new_findings":"never"}}}}"#,
            0,
        ),
    ] {
        fs::write(root.join("codeint.json"), config_json).expect("write regression limit");
        run_codeint_text(root, &["diff", "HEAD~1", "--format", "json"], expected_code);
    }

    let cache_dir = TempDir::new().expect("cache dir");
    let cache_options = DiffOptions {
        result_cache: Some(cache_dir.path().to_path_buf()),
        ..DiffOptions::default()
    };
    assert_eq!(cached_base_hazards_ratio(root, &cache_options, "{}"), 0.5);
    assert_eq!(
        cached_base_hazards_ratio(
            root,
            &cache_options,
            r#"{"hazards":{"disabled_rules":["select-star"]}}"#,
        ),
        0.0
    );
    assert_eq!(
        cached_base_hazards_ratio(
            root,
            &cache_options,
            r#"{"sql":{"dialects":[{"dialect":"postgresql","include":["queries/**"]}]}}"#,
        ),
        0.5
    );
}

#[test]
fn secret_sentinel_never_leaks_in_any_check_diff_or_mcp_output() {
    let sentinel = "SENTINEL_TOP_SECRET_99481_XYZ";
    let repo = init_git_repo(&[("db/setup.sql", "SELECT id FROM accounts WHERE id = 1;\n")]);
    fs::write(
        repo.path().join("db/auth.sql"),
        format!("INSERT INTO credentials (user_id, api_key) VALUES (1, '{sentinel}');\n"),
    )
    .expect("write secret sql");

    for format in ["toon", "json", "markdown", "sarif"] {
        for (args, code) in [
            (vec!["check", "--format", format], 0),
            (vec!["diff", "main", "--format", format], 1),
        ] {
            let (stdout, stderr) = run_codeint_text(repo.path(), &args, code);
            assert!(
                !stdout.contains(sentinel) && !stderr.contains(sentinel),
                "{args:?} leaked secret"
            );
        }
    }

    let check_report = check_json(repo.path());
    let finding = &check_report["source_sets"]["production"]["hazards"]["findings"][0];
    assert_eq!(finding["path"], "db/auth.sql");
    assert_eq!(finding["line"], 1);
    assert_eq!(finding["rule"], "plain-text-secret");

    let mcp_responses = send_mcp_requests(
        repo.path(),
        &[
            mcp_tool_call(1, "check", json!({})),
            mcp_tool_call(2, "diff", json!({"base_ref": "main"})),
        ],
    );
    for response in &mcp_responses {
        let serialized = response.to_string();
        assert!(
            !serialized.contains(sentinel),
            "MCP response leaked secret: {serialized}"
        );
    }
    let mcp_check = mcp_payload(&mcp_responses[0]);
    let mcp_finding = &mcp_check["report"]["source_sets"]["production"]["hazards"]["findings"][0];
    assert_eq!(mcp_finding["path"], "db/auth.sql");
    assert_eq!(mcp_finding["line"], 1);
    assert_eq!(mcp_finding["rule"], "plain-text-secret");
}
