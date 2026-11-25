use codeint_model::SourceSet;

use super::{
    DEFAULT_RATCHET_PATH, MeasuredRatchetMetric, RATCHET_SCHEMA_VERSION, Ratchet, RatchetConfig,
    RatchetConfigFile, RatchetFileError, round_ratchet_ratio,
};
use crate::ConfigurationFileError;

fn metric(
    source_set: Option<SourceSet>,
    name: &str,
    ratio: f64,
    metric_version: u32,
) -> MeasuredRatchetMetric {
    MeasuredRatchetMetric {
        source_set,
        metric: name.to_owned(),
        ratio,
        metric_version,
    }
}

#[test]
fn ratchet_config_defaults_and_rejects_invalid_paths() {
    assert_eq!(RatchetConfig::default().path, DEFAULT_RATCHET_PATH);

    let custom = RatchetConfig::from_file(RatchetConfigFile {
        path: Some("ci/ratchet.json".to_owned()),
    })
    .expect("valid relative path");
    assert_eq!(custom.path, "ci/ratchet.json");

    assert!(matches!(
        RatchetConfig::from_file(RatchetConfigFile {
            path: Some("   ".to_owned()),
        }),
        Err(ConfigurationFileError::EmptyRatchetPath { .. })
    ));
    assert!(matches!(
        RatchetConfig::from_file(RatchetConfigFile {
            path: Some("/tmp/ratchet.json".to_owned()),
        }),
        Err(ConfigurationFileError::AbsoluteRatchetPath { .. })
    ));
    assert!(matches!(
        RatchetConfig::from_file(RatchetConfigFile {
            path: Some("../ratchet.json".to_owned()),
        }),
        Err(ConfigurationFileError::ParentTraversalRatchetPath { .. })
    ));
}

#[test]
fn ratchet_update_handles_create_tighten_refuse_reset_and_version_refresh() {
    let three = |erosion: f64, verbosity: f64, verbosity_version: u32| {
        vec![
            metric(Some(SourceSet::Production), "erosion", erosion, 1),
            metric(
                Some(SourceSet::Production),
                "verbosity",
                verbosity,
                verbosity_version,
            ),
            metric(None, "unused_dependencies", 0.25, 1),
        ]
    };
    let prod_entry = |ratchet: &Ratchet, name: &str| {
        ratchet
            .entry(Some(SourceSet::Production), name)
            .unwrap_or_else(|| panic!("missing {name}"))
    };
    let initial_metrics = three(0.12346, 0.05, 1);
    let (created, summary) =
        Ratchet::update(None, &initial_metrics, false, DEFAULT_RATCHET_PATH).expect("create");
    assert!(summary.created);
    assert_eq!(summary.added, 3);
    assert_eq!(prod_entry(&created, "erosion").ceiling, 0.1235);

    let improved = three(0.1, 0.05, 1);
    let (tightened, tighten_summary) =
        Ratchet::update(Some(&created), &improved, false, DEFAULT_RATCHET_PATH).expect("tighten");
    assert_eq!(tighten_summary.tightened, 1);
    assert_eq!(tighten_summary.unchanged, 2);
    assert_eq!(prod_entry(&tightened, "erosion").ceiling, 0.1);

    let regressed = three(0.15, 0.05, 1);
    let refused = Ratchet::update(Some(&tightened), &regressed, false, DEFAULT_RATCHET_PATH)
        .expect_err("refuse regression without --reset");
    assert_eq!(refused.len(), 1);
    assert_eq!(refused[0].label(), "production.erosion");

    let (loosened, reset_summary) =
        Ratchet::update(Some(&tightened), &regressed, true, DEFAULT_RATCHET_PATH)
            .expect("allow regression with --reset");
    assert_eq!(reset_summary.loosened, 1);
    assert_eq!(prod_entry(&loosened, "erosion").ceiling, 0.15);

    let version_bumped = three(0.15, 0.2, 2);
    let (refreshed, refresh_summary) = Ratchet::update(
        Some(&loosened),
        &version_bumped,
        false,
        DEFAULT_RATCHET_PATH,
    )
    .expect("auto refresh changed version");
    assert_eq!(refresh_summary.refreshed, 1);
    let verbosity = prod_entry(&refreshed, "verbosity");
    assert_eq!(verbosity.metric_version, 2);
    assert_eq!(verbosity.ceiling, 0.2);
}

#[test]
fn ratchet_check_detects_violations_and_version_mismatches() {
    let pair = |erosion: f64, verbosity_ver: u32| {
        vec![
            metric(Some(SourceSet::Production), "erosion", erosion, 1),
            metric(
                Some(SourceSet::Production),
                "verbosity",
                0.05,
                verbosity_ver,
            ),
        ]
    };
    let initial = pair(0.1, 2);
    let (ratchet, _) = Ratchet::update(None, &initial, false, DEFAULT_RATCHET_PATH).unwrap();

    assert_eq!(ratchet.check(&initial).expect("equal passes").len(), 0);
    assert_eq!(round_ratchet_ratio(-0.0), 0.0);

    let violations = ratchet
        .check(&pair(0.1001, 2))
        .expect("no version mismatch");
    assert_eq!(violations.len(), 1);
    assert_eq!(violations[0].label(), "production.erosion");

    let mismatches = ratchet.check(&pair(0.1, 3)).expect_err("version mismatch");
    assert_eq!(mismatches.len(), 1);
    assert_eq!(mismatches[0].recorded_version, 2);
    assert_eq!(mismatches[0].current_version, 3);
}

#[test]
fn ratchet_parse_rejects_invalid_schema_ceiling_version_and_dependency_key() {
    assert!(matches!(
        Ratchet::parse(r#"{"schema_version": 99}"#, DEFAULT_RATCHET_PATH),
        Err(RatchetFileError::UnsupportedSchemaVersion { found: 99, .. })
    ));
    assert!(matches!(
        Ratchet::parse(
            &format!(
                r#"{{"schema_version": {RATCHET_SCHEMA_VERSION}, "source_sets": {{"production": {{"erosion": {{"ceiling": 1.5, "metric_version": 1}}}}}}}}"#
            ),
            DEFAULT_RATCHET_PATH,
        ),
        Err(RatchetFileError::InvalidCeiling { .. })
    ));
    assert!(matches!(
        Ratchet::parse(
            &format!(
                r#"{{"schema_version": {RATCHET_SCHEMA_VERSION}, "source_sets": {{"production": {{"erosion": {{"ceiling": 0.1, "metric_version": 0}}}}}}}}"#
            ),
            DEFAULT_RATCHET_PATH,
        ),
        Err(RatchetFileError::InvalidMetricVersion { .. })
    ));
    assert!(matches!(
        Ratchet::parse(
            &format!(
                r#"{{"schema_version": {RATCHET_SCHEMA_VERSION}, "dependencies": {{"bogus": {{"ceiling": 0.1, "metric_version": 1}}}}}}"#
            ),
            DEFAULT_RATCHET_PATH,
        ),
        Err(RatchetFileError::UnknownDependencyMetric { .. })
    ));
}
