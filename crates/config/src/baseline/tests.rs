use codeint_model::SourceSet;

use super::{BASELINE_SCHEMA_VERSION, Baseline, BaselineFileError, MeasuredBaselineMetric};
use crate::FindingAnchor;

fn measured(metric: &str, metric_version: u32, ids: &[&str]) -> MeasuredBaselineMetric {
    MeasuredBaselineMetric {
        source_set: Some(SourceSet::Production),
        metric: metric.to_owned(),
        metric_version,
        findings: ids.iter().map(|id| FindingAnchor::id_only(*id)).collect(),
    }
}

fn stored_ids(baseline: &Baseline, metric: &str) -> Vec<String> {
    baseline
        .entry(Some(SourceSet::Production), metric)
        .expect("stored entry")
        .findings
        .iter()
        .map(|finding| finding.id.clone())
        .collect()
}

#[test]
fn first_update_records_every_current_finding_and_metric_version() {
    let (created, summary) = Baseline::update(
        None,
        &[
            measured("erosion", 1, &["a", "b"]),
            measured("hazards", 1, &[]),
        ],
        false,
    );
    assert!(summary.created);
    assert_eq!(created.schema_version, BASELINE_SCHEMA_VERSION);
    assert_eq!(summary.added, 2);
    assert_eq!(summary.total_findings, 2);
    assert_eq!(stored_ids(&created, "erosion"), vec!["a", "b"]);
    assert!(stored_ids(&created, "hazards").is_empty());
}

#[test]
fn later_updates_only_remove_fixed_findings_unless_reset() {
    let (first, _) = Baseline::update(None, &[measured("erosion", 1, &["a", "b"])], false);
    let current = [measured("erosion", 1, &["b", "c"])];

    let (shrunk, summary) = Baseline::update(Some(&first), &current, false);
    assert_eq!(stored_ids(&shrunk, "erosion"), vec!["b"]);
    assert_eq!(
        (summary.removed, summary.skipped_new, summary.added),
        (1, 1, 0)
    );

    let (reset, summary) = Baseline::update(Some(&first), &current, true);
    assert_eq!(stored_ids(&reset, "erosion"), vec!["b", "c"]);
    assert_eq!((summary.removed, summary.added), (1, 1));
}

#[test]
fn version_change_refreshes_only_that_metric() {
    let (first, _) = Baseline::update(
        None,
        &[
            measured("erosion", 1, &["a"]),
            measured("verbosity", 1, &["v"]),
        ],
        false,
    );
    let (next, summary) = Baseline::update(
        Some(&first),
        &[
            measured("erosion", 1, &["a", "new"]),
            measured("verbosity", 2, &["w"]),
        ],
        false,
    );
    assert_eq!(summary.refreshed, 1);
    assert_eq!(summary.skipped_new, 1);
    assert_eq!(stored_ids(&next, "erosion"), vec!["a"]);
    assert_eq!(stored_ids(&next, "verbosity"), vec!["w"]);
    let mismatches = first.version_mismatches(&[measured("verbosity", 2, &[])]);
    assert_eq!(
        mismatches[0].to_string(),
        "production.verbosity (baseline Metric version 1, current Metric version 2)"
    );
}

#[test]
fn unmeasured_metrics_keep_their_entries() {
    let (first, _) = Baseline::update(None, &[measured("test_gap", 1, &["t"])], false);
    let (next, _) = Baseline::update(Some(&first), &[measured("erosion", 1, &[])], false);
    assert_eq!(stored_ids(&next, "test_gap"), vec!["t"]);
}

#[test]
fn parse_supports_schema_v1_and_v2_and_rejects_unknown_schema_and_zero_metric_versions() {
    let future = format!(
        "{{\"schema_version\": {}, \"source_sets\": {{}}}}",
        BASELINE_SCHEMA_VERSION + 1
    );
    assert!(matches!(
        Baseline::parse(&future),
        Err(BaselineFileError::UnsupportedSchemaVersion { .. })
    ));
    let zero =
        r#"{"schema_version": 1, "dependencies": {"unused_dependencies": {"metric_version": 0}}}"#;
    assert!(matches!(
        Baseline::parse(zero),
        Err(BaselineFileError::InvalidMetricVersion { .. })
    ));
    let parsed_v1 = Baseline::parse(
        r#"{"schema_version": 1, "source_sets": {"production": {"erosion": {"metric_version": 1, "findings": ["x"]}}}}"#,
    )
    .expect("valid v1 Baseline");
    assert_eq!(
        parsed_v1
            .match_findings(
                Some(SourceSet::Production),
                "erosion",
                &[FindingAnchor::id_only("x")]
            )
            .matched_pairs,
        vec![(0, 0)]
    );
    assert_eq!(
        parsed_v1
            .entry(Some(SourceSet::Production), "erosion")
            .map(|entry| entry.findings.clone()),
        Some(vec![FindingAnchor::id_only("x")])
    );

    let parsed_v2 = Baseline::parse(
        r#"{"schema_version": 2, "source_sets": {"production": {"hazards": {"metric_version": 1, "findings": [{"id": "h1", "content": "c1", "before": "b1", "line": 2}]}}}}"#,
    )
    .expect("valid v2 Baseline");
    assert_eq!(parsed_v2.schema_version, 2);
    assert_eq!(
        parsed_v2
            .entry(Some(SourceSet::Production), "hazards")
            .expect("hazards entry")
            .findings,
        vec![FindingAnchor {
            id: "h1".to_owned(),
            content: Some("c1".to_owned()),
            before: Some("b1".to_owned()),
            after: None,
            line: Some(2),
        }]
    );
}

#[test]
fn a_metric_without_an_entry_records_current_findings_in_a_later_update() {
    let (first, _) = Baseline::update(None, &[measured("erosion", 1, &["a"])], false);
    let (second, summary) = Baseline::update(
        Some(&first),
        &[
            measured("erosion", 1, &["a", "b"]),
            measured("test_gap", 1, &["t"]),
        ],
        false,
    );
    assert_eq!(stored_ids(&second, "test_gap"), vec!["t"]);
    assert_eq!(stored_ids(&second, "erosion"), vec!["a"]);
    assert_eq!((summary.added, summary.skipped_new), (1, 1));
}
