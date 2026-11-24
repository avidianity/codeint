use super::validate_metric_key;
use crate::PluginHostError;

#[test]
fn rejects_test_gap_and_other_built_in_metric_keys_as_reserved() {
    for reserved in [
        "test_gap",
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
        let error = validate_metric_key("custom-metric", reserved)
            .expect_err("reserved key must be rejected");
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
}

#[test]
fn accepts_valid_external_metric_keys() {
    for valid in ["long_functions", "custom-metric", "m1_check"] {
        validate_metric_key("custom-metric", valid)
            .unwrap_or_else(|error| panic!("expected {valid} to be accepted, got: {error}"));
    }
}
