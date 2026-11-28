use codeint_config::PluginLimits;
use codeint_model::{Metric, MetricPluginError, STRUCTURAL_REPORT_KEYS, SourceSet};

use super::{
    enforce_metric_lifted_budget, guest_metric, validate_metric_key, validate_metric_version,
};
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

#[test]
fn validates_positive_integer_metric_versions() {
    let error =
        validate_metric_version("custom-metric", 0).expect_err("Metric version 0 must be rejected");
    assert!(
        matches!(
            error,
            PluginHostError::InvalidMetricVersion {
                ref name,
                version: 0,
            } if name == "custom-metric"
        ),
        "unexpected error: {error}"
    );
    for version in [1, 2, 42] {
        validate_metric_version("custom-metric", version)
            .unwrap_or_else(|err| panic!("expected version {version} to be accepted: {err}"));
    }
}

#[test]
fn metric_lifted_budget_rejects_oversized_findings_payload() {
    let limits = PluginLimits {
        memory_mb: 1,
        timeout_ms: 5_000,
    };
    let oversized_message = guest_metric::Measurement {
        ratio: 0.5,
        numerator: 1.0,
        denominator: 2.0,
        findings: vec![guest_metric::Finding {
            id: "f1".to_owned(),
            path: "src/a.ts".to_owned(),
            line: 1,
            end_line: None,
            message: "x".repeat(limits.memory_bytes() + 1),
        }],
    };
    let error = enforce_metric_lifted_budget("oversized-metric", limits, &oversized_message)
        .expect_err("measurement exceeding lifted byte budget must fail");
    assert!(
        matches!(
            error,
            MetricPluginError::MemoryLimitExceeded {
                ref plugin,
                memory_mb: 1,
            } if plugin == "oversized-metric"
        ),
        "unexpected error: {error}"
    );
}
