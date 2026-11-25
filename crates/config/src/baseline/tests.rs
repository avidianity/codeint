use std::collections::BTreeSet;

use codeint_model::SourceSet;

use super::{BASELINE_SCHEMA_VERSION, Baseline, BaselineFileError, MeasuredBaselineMetric};

fn measured(metric: &str, metric_version: u32, ids: &[&str]) -> MeasuredBaselineMetric {
    MeasuredBaselineMetric {
        source_set: Some(SourceSet::Production),
        metric: metric.to_owned(),
        metric_version,
        findings: ids.iter().map(|id| (*id).to_owned()).collect(),
    }
}

fn stored_ids(baseline: &Baseline, metric: &str) -> Vec<String> {
    baseline
        .entry(Some(SourceSet::Production), metric)
        .expect("stored entry")
        .findings
        .iter()
        .cloned()
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
fn parse_rejects_unknown_schema_versions_and_zero_metric_versions() {
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
    let parsed = Baseline::parse(
        r#"{"schema_version": 1, "source_sets": {"production": {"erosion": {"metric_version": 1, "findings": ["x"]}}}}"#,
    )
    .expect("valid Baseline");
    assert!(parsed.is_baselined(Some(SourceSet::Production), "erosion", "x"));
    assert_eq!(
        parsed
            .entry(Some(SourceSet::Production), "erosion")
            .map(|entry| entry.findings.clone()),
        Some(BTreeSet::from(["x".to_owned()]))
    );
}
