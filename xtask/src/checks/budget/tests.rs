use super::{
    Verdict, ci_gate_seconds, gating_run_ids, parse_timestamp, pull_request_head_sha, verdict,
};

#[test]
fn eleven_minutes_fails_a_ten_minute_gate() {
    assert_eq!(verdict(660.0, 600.0, 0.8), Verdict::OverBudget);
}

#[test]
fn seven_minutes_passes_and_eight_and_a_half_warns() {
    assert_eq!(verdict(420.0, 600.0, 0.8), Verdict::Within);
    assert_eq!(verdict(510.0, 600.0, 0.8), Verdict::NearBudget);
}

#[test]
fn timestamps_parse_to_unix_seconds() {
    assert_eq!(parse_timestamp("1970-01-01T00:00:00Z"), Some(0));
    assert_eq!(parse_timestamp("2026-11-21T00:00:10Z"), Some(1_795_219_210));
}

#[test]
fn ci_gate_spans_first_start_to_last_end_without_the_excluded_job() {
    let jobs = serde_json::json!({"jobs": [
        {"name": "test", "started_at": "2026-11-21T00:00:00Z", "completed_at": "2026-11-21T00:05:00Z"},
        {"name": "lint", "started_at": "2026-11-21T00:01:00Z", "completed_at": "2026-11-21T00:07:00Z"},
        {"name": "budget", "started_at": "2026-11-21T00:07:10Z", "completed_at": null}
    ]});
    assert_eq!(ci_gate_seconds(&[jobs], "budget"), Some(420.0));
}

#[test]
fn ci_gate_spans_earliest_start_when_pull_request_workflow_starts_first() {
    let ci_jobs = serde_json::json!({"jobs": [
        {"name": "rust", "started_at": "2026-11-21T00:01:00Z", "completed_at": "2026-11-21T00:06:00Z"},
        {"name": "budget", "started_at": "2026-11-21T00:06:05Z", "completed_at": "2026-11-21T00:09:00Z"}
    ]});
    let pull_request_jobs = serde_json::json!({"jobs": [
        {"name": "body", "started_at": "2026-11-21T00:00:00Z", "completed_at": "2026-11-21T00:00:45Z"}
    ]});
    assert_eq!(
        ci_gate_seconds(&[ci_jobs, pull_request_jobs], "budget"),
        Some(360.0)
    );
}

#[test]
fn ci_gate_spans_latest_end_when_pull_request_workflow_finishes_last() {
    let ci_jobs = serde_json::json!({"jobs": [
        {"name": "rust", "started_at": "2026-11-21T00:00:00Z", "completed_at": "2026-11-21T00:05:00Z"},
        {"name": "budget", "started_at": "2026-11-21T00:05:10Z", "completed_at": null}
    ]});
    let pull_request_jobs = serde_json::json!({"jobs": [
        {"name": "body", "started_at": "2026-11-21T00:02:00Z", "completed_at": "2026-11-21T00:08:00Z"}
    ]});
    assert_eq!(
        ci_gate_seconds(&[ci_jobs, pull_request_jobs], "budget"),
        Some(480.0)
    );
}

#[test]
fn gating_run_ids_includes_pull_request_workflow_and_ignores_other_events_and_cancelled_runs() {
    let primary = serde_json::json!({
        "id": 200,
        "workflow_id": 10,
        "name": "ci",
        "event": "pull_request",
        "head_sha": "abc123"
    });
    let sha_runs = serde_json::json!({"workflow_runs": [
        {"id": 200, "workflow_id": 10, "name": "ci", "event": "pull_request", "conclusion": null},
        {"id": 150, "workflow_id": 10, "name": "ci", "event": "pull_request", "conclusion": "failure"},
        {"id": 201, "workflow_id": 20, "name": "pull-request", "event": "pull_request", "conclusion": "cancelled"},
        {"id": 202, "workflow_id": 20, "name": "pull-request", "event": "pull_request", "conclusion": "success"},
        {"id": 205, "workflow_id": 20, "name": "pull-request", "event": "pull_request", "conclusion": "success"},
        {"id": 206, "workflow_id": 30, "name": "label", "event": "workflow_run", "conclusion": "success"}
    ]});
    assert_eq!(pull_request_head_sha(&primary), Some("abc123"));
    assert_eq!(
        gating_run_ids(&primary, Some(&sha_runs), "200"),
        vec!["200".to_string(), "205".to_string()]
    );
}

#[test]
fn gating_run_ids_times_only_ci_when_run_is_a_push_to_main() {
    let primary = serde_json::json!({
        "id": 300,
        "workflow_id": 10,
        "name": "ci",
        "event": "push",
        "head_sha": "def456"
    });
    assert_eq!(pull_request_head_sha(&primary), None);
    assert_eq!(
        gating_run_ids(&primary, None, "300"),
        vec!["300".to_string()]
    );
}

#[test]
fn gating_run_ids_handles_pull_request_with_only_ci_run() {
    let primary = serde_json::json!({
        "id": 400,
        "workflow_id": 10,
        "name": "ci",
        "event": "pull_request",
        "head_sha": "789abc"
    });
    let sha_runs = serde_json::json!({"workflow_runs": [
        {"id": 400, "workflow_id": 10, "name": "ci", "event": "pull_request", "conclusion": null}
    ]});
    assert_eq!(
        gating_run_ids(&primary, Some(&sha_runs), "400"),
        vec!["400".to_string()]
    );
}
