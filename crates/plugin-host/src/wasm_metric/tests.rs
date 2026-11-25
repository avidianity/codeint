use codeint_model::{Metric, STRUCTURAL_REPORT_KEYS, SourceSet};

use super::validate_metric_key;
use crate::PluginHostError;

fn assert_reserved_metric_key(reserved: &str) {
    let error =
        validate_metric_key("custom-metric", reserved).expect_err("reserved key must be rejected");
    assert!(
        matches!(
            error,
            PluginHostError::InvalidMetricKey {
                ref name,
                ref key,
                ..
            } if name == "custom-metric" && key == reserved
        ),
        "unexpected error for {reserved}: {error}"
    );
}

#[test]
fn rejects_test_gap_and_other_built_in_metric_keys_as_reserved() {
    for reserved in [
        "test_gap",
        "hazards",
        "erosion",
        "duplication",
        "verbosity",
        "import_cycles",
        "cognitive_complexity",
        "boundaries",
        "orphan_files",
        "unused_dependencies",
        "unlisted_dependencies",
        "dev_in_production_dependencies",
        "files",
        "source_lines",
        "packages",
    ] {
        assert_reserved_metric_key(reserved);
    }
}

#[test]
fn accepts_valid_external_metric_keys() {
    for valid in ["long_functions", "custom-metric", "m1_check"] {
        validate_metric_key("custom-metric", valid)
            .unwrap_or_else(|error| panic!("expected {valid} to be accepted, got: {error}"));
    }
}

#[test]
fn rejects_trend_and_config_structural_keys_as_reserved() {
    for reserved in [
        "commit",
        "committed_on",
        "production",
        "test",
        "generated",
        "vendored",
        "dependencies",
    ] {
        assert_reserved_metric_key(reserved);
    }
}

#[test]
fn rejects_every_metric_registry_structural_and_source_set_key() {
    for reserved in Metric::ALL
        .into_iter()
        .map(Metric::key)
        .chain(STRUCTURAL_REPORT_KEYS)
        .chain(SourceSet::ALL.into_iter().map(SourceSet::key))
    {
        assert_reserved_metric_key(reserved);
    }
}
