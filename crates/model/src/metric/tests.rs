use std::collections::BTreeSet;

use super::{
    Metric, MetricCapability, MetricKind, MetricScope, NewFindingsRule, STRUCTURAL_REPORT_KEYS,
    is_reserved_report_key,
};
use crate::SourceSet;

#[test]
fn metric_registry_covers_all_twelve_built_in_metrics_in_order() {
    assert_eq!(Metric::ALL.len(), 12);
    assert_eq!(Metric::DESCRIPTORS.len(), 12);
    assert_eq!(Metric::BUILT_IN_KEYS.len(), 12);
    assert_eq!(
        Metric::BUILT_IN_KEYS,
        [
            "erosion",
            "cognitive_complexity",
            "duplication",
            "verbosity",
            "import_cycles",
            "boundaries",
            "orphan_files",
            "test_gap",
            "hazards",
            "unused_dependencies",
            "unlisted_dependencies",
            "dev_in_production_dependencies",
        ]
    );

    let unique_keys: BTreeSet<&str> = Metric::BUILT_IN_KEYS.into_iter().collect();
    assert_eq!(unique_keys.len(), Metric::ALL.len());

    for (index, metric) in Metric::ALL.into_iter().enumerate() {
        let (table_metric, descriptor) = Metric::DESCRIPTORS[index];
        assert_eq!(table_metric, metric);
        assert_eq!(metric.index(), index);
        assert_eq!(metric.descriptor(), &descriptor);
        assert_eq!(Metric::BUILT_IN_KEYS[index], descriptor.key);
        assert_eq!(Metric::from_key(descriptor.key), Some(metric));
    }

    assert_eq!(Metric::from_key("unknown_metric"), None);
    for structural_key in STRUCTURAL_REPORT_KEYS {
        assert_eq!(Metric::from_key(structural_key), None);
    }
}

#[test]
fn every_metric_descriptor_has_expected_version_and_metadata() {
    let expected = [
        (
            Metric::Erosion,
            "erosion",
            "Erosion",
            1,
            MetricKind::SoftwareEntropy,
            MetricScope::AllSourceSets,
            true,
            NewFindingsRule::NumeratorRise,
            MetricCapability::None,
            "eroded_function_mass",
        ),
        (
            Metric::CognitiveComplexity,
            "cognitive_complexity",
            "Cognitive complexity",
            1,
            MetricKind::SoftwareEntropy,
            MetricScope::AllSourceSets,
            true,
            NewFindingsRule::NumeratorRise,
            MetricCapability::None,
            "complex_cognitive_complexity",
        ),
        (
            Metric::Duplication,
            "duplication",
            "Duplication",
            2,
            MetricKind::SoftwareEntropy,
            MetricScope::AllSourceSets,
            true,
            NewFindingsRule::NumeratorRise,
            MetricCapability::None,
            "clone_lines",
        ),
        (
            Metric::Verbosity,
            "verbosity",
            "Verbosity",
            2,
            MetricKind::SoftwareEntropy,
            MetricScope::AllSourceSets,
            true,
            NewFindingsRule::NumeratorRise,
            MetricCapability::None,
            "verbose_lines",
        ),
        (
            Metric::ImportCycles,
            "import_cycles",
            "Import cycles",
            1,
            MetricKind::SoftwareEntropy,
            MetricScope::AllSourceSets,
            true,
            NewFindingsRule::AnyNewFinding,
            MetricCapability::Imports,
            "modules_in_import_cycles",
        ),
        (
            Metric::Boundaries,
            "boundaries",
            "Import boundaries",
            2,
            MetricKind::SoftwareEntropy,
            MetricScope::AllSourceSets,
            true,
            NewFindingsRule::AnyNewFinding,
            MetricCapability::Imports,
            "boundary_findings",
        ),
        (
            Metric::OrphanFiles,
            "orphan_files",
            "Orphan files",
            1,
            MetricKind::SoftwareEntropy,
            MetricScope::ProductionOnly,
            true,
            NewFindingsRule::AnyNewFinding,
            MetricCapability::Imports,
            "orphan_files",
        ),
        (
            Metric::TestGap,
            "test_gap",
            "Test gap",
            1,
            MetricKind::SoftwareEntropy,
            MetricScope::ProductionOnly,
            false,
            NewFindingsRule::NumeratorRise,
            MetricCapability::CoverageReport,
            "untested_function_mass",
        ),
        (
            Metric::Hazards,
            "hazards",
            "Hazards",
            1,
            MetricKind::Hazards,
            MetricScope::AllSourceSets,
            true,
            NewFindingsRule::AnyNewFinding,
            MetricCapability::HazardLines,
            "hazard_lines",
        ),
        (
            Metric::UnusedDependencies,
            "unused_dependencies",
            "Unused dependencies",
            1,
            MetricKind::SoftwareEntropy,
            MetricScope::RepositoryWide,
            true,
            NewFindingsRule::AnyNewFinding,
            MetricCapability::Imports,
            "unused",
        ),
        (
            Metric::UnlistedDependencies,
            "unlisted_dependencies",
            "Unlisted dependencies",
            1,
            MetricKind::SoftwareEntropy,
            MetricScope::RepositoryWide,
            true,
            NewFindingsRule::AnyNewFinding,
            MetricCapability::Imports,
            "unlisted",
        ),
        (
            Metric::DevInProductionDependencies,
            "dev_in_production_dependencies",
            "Dev dependencies in production",
            1,
            MetricKind::SoftwareEntropy,
            MetricScope::RepositoryWide,
            true,
            NewFindingsRule::AnyNewFinding,
            MetricCapability::Imports,
            "dev_in_production",
        ),
    ];

    assert_eq!(expected.len(), Metric::ALL.len());
    for (
        index,
        (
            metric,
            key,
            display_name,
            version,
            kind,
            scope,
            supports_diff_regression,
            default_new_findings_rule,
            required_capability,
            numerator_field,
        ),
    ) in expected.into_iter().enumerate()
    {
        assert_eq!(Metric::ALL[index], metric);
        let descriptor = metric.descriptor();
        assert_eq!(descriptor.key, key);
        assert_eq!(descriptor.display_name, display_name);
        assert_eq!(descriptor.version, version);
        assert_eq!(descriptor.kind, kind);
        assert_eq!(descriptor.scope, scope);
        assert_eq!(
            descriptor.supports_diff_regression,
            supports_diff_regression
        );
        assert_eq!(
            descriptor.default_new_findings_rule,
            default_new_findings_rule
        );
        assert_eq!(descriptor.required_capability, required_capability);
        assert_eq!(descriptor.numerator_field, numerator_field);
        assert_eq!(metric.key(), key);
        assert_eq!(metric.display_name(), display_name);
        assert_eq!(metric.version(), version);
        assert_eq!(metric.kind(), kind);
        assert_eq!(metric.scope(), scope);
        assert_eq!(metric.supports_diff_regression(), supports_diff_regression);
        assert_eq!(
            metric.default_new_findings_rule(),
            default_new_findings_rule
        );
        assert_eq!(metric.required_capability(), required_capability);
        assert_eq!(metric.numerator_field(), numerator_field);
    }
}

#[test]
fn structural_report_keys_and_reserved_key_checks_cover_built_ins_and_structure_only() {
    assert_eq!(
        STRUCTURAL_REPORT_KEYS,
        ["files", "source_lines", "packages"]
    );
    for key in Metric::BUILT_IN_KEYS {
        assert!(
            is_reserved_report_key(key),
            "built-in key {key} must be reserved"
        );
    }
    for key in STRUCTURAL_REPORT_KEYS {
        assert!(
            is_reserved_report_key(key),
            "structural key {key} must be reserved"
        );
    }
    assert!(!is_reserved_report_key("custom_metric"));
    assert!(!is_reserved_report_key("long_functions"));
}

#[test]
fn metric_scope_applies_to_expected_source_sets() {
    for source_set in SourceSet::ALL {
        assert!(MetricScope::AllSourceSets.applies_to_source_set(source_set));
        assert_eq!(
            MetricScope::ProductionOnly.applies_to_source_set(source_set),
            source_set == SourceSet::Production
        );
        assert!(!MetricScope::RepositoryWide.applies_to_source_set(source_set));
    }
}

struct StubMetricPlugin {
    key: &'static str,
    version: u32,
}

impl super::MetricPlugin for StubMetricPlugin {
    fn key(&self) -> &str {
        self.key
    }

    fn name(&self) -> &str {
        self.key
    }

    fn description(&self) -> &str {
        self.key
    }

    fn version(&self) -> u32 {
        self.version
    }

    fn measure(
        &self,
        _model: &crate::CodeModel,
    ) -> Result<super::ExternalMetricMeasurement, super::MetricPluginError> {
        Ok(super::ExternalMetricMeasurement {
            ratio: 0.0,
            numerator: 0.0,
            denominator: 0.0,
            findings: Vec::new(),
        })
    }
}

#[test]
fn metric_version_helpers_return_sorted_positive_versions_for_built_ins_and_plugins() {
    let built_in = Metric::built_in_versions();
    assert_eq!(built_in.len(), Metric::ALL.len());
    for metric in Metric::ALL {
        let version = built_in.get(metric.key()).copied();
        assert_eq!(version, Some(metric.version()));
        assert!(version.is_some_and(|value| value >= 1));
    }

    let zeta = StubMetricPlugin {
        key: "zeta_metric",
        version: 4,
    };
    let alpha = StubMetricPlugin {
        key: "alpha_metric",
        version: 2,
    };
    let plugins: [&dyn super::MetricPlugin; 2] = [&zeta, &alpha];
    let combined = Metric::versions_with_plugins(&plugins);
    assert_eq!(combined.len(), Metric::ALL.len() + 2);
    assert_eq!(combined.get("alpha_metric"), Some(&2));
    assert_eq!(combined.get("zeta_metric"), Some(&4));
    for metric in Metric::ALL {
        assert_eq!(combined.get(metric.key()), Some(&metric.version()));
    }
    let keys: Vec<&String> = combined.keys().collect();
    let mut sorted_keys = keys.clone();
    sorted_keys.sort();
    assert_eq!(keys, sorted_keys);
}

#[test]
fn duplication_and_boundaries_declare_metric_version_2() {
    assert_eq!(Metric::Duplication.version(), 2);
    assert_eq!(Metric::Boundaries.version(), 2);
}
