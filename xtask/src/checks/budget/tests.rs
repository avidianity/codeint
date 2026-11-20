use super::{Verdict, ci_gate_seconds, parse_timestamp, verdict};

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
    assert_eq!(ci_gate_seconds(&jobs, "budget"), Some(420.0));
}
