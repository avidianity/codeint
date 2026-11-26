use super::*;
use crate::corpus::{CorpusEntry, SizeBucket};
use crate::summary::OracleReport;

fn result(wall: Option<f64>, rss: Option<u64>, oracle: OracleReport) -> RepositoryResult {
    let mut result = RepositoryResult::pending(&CorpusEntry {
        name: "owner/name".to_owned(),
        language: "python".to_owned(),
        panel: None,
        bucket: SizeBucket::Small,
        url: "https://example.com/owner/name".to_owned(),
        commit: "0".repeat(40),
    });
    result.median_wall_seconds = wall;
    result.peak_rss_bytes = rss;
    result.oracle = oracle;
    result
}

fn measured() -> OracleReport {
    OracleReport {
        status: OracleStatus::Measured,
        reason: None,
        recall: None,
    }
}

fn skipped() -> OracleReport {
    OracleReport {
        status: OracleStatus::Skipped,
        reason: Some("python3 is missing".to_owned()),
        recall: None,
    }
}

#[test]
fn no_ceiling_and_no_requirement_report_nothing() {
    let result = result(Some(99.0), Some(u64::MAX), skipped());
    assert!(ceiling_violations(&result, &Ceilings::default(), false).is_empty());
}

#[test]
fn a_ceiling_reports_only_a_measurement_above_it() {
    let ceilings = Ceilings {
        max_wall_seconds: Some(10.0),
        max_peak_rss_bytes: Some(100 * 1024 * 1024),
    };
    let within = result(Some(10.0), Some(100 * 1024 * 1024), measured());
    assert!(ceiling_violations(&within, &ceilings, true).is_empty());
    let over = result(Some(10.5), Some(101 * 1024 * 1024), measured());
    assert_eq!(
        ceiling_violations(&over, &ceilings, true),
        [
            "median wall time 10.50s exceeds the ceiling of 10s",
            "peak memory 101 MiB exceeds the ceiling of 100 MiB"
        ]
    );
}

#[test]
fn a_ceiling_without_its_measurement_is_a_violation() {
    let ceilings = Ceilings {
        max_wall_seconds: Some(10.0),
        max_peak_rss_bytes: Some(1024),
    };
    let violations = ceiling_violations(&result(None, None, measured()), &ceilings, false);
    assert_eq!(violations.len(), 2, "{violations:?}");
}

#[test]
fn a_skipped_oracle_is_a_violation_only_when_oracles_are_required() {
    let skipped = result(Some(1.0), None, skipped());
    assert!(ceiling_violations(&skipped, &Ceilings::default(), false).is_empty());
    assert_eq!(
        ceiling_violations(&skipped, &Ceilings::default(), true),
        ["oracle skipped: python3 is missing"]
    );
}
