#![forbid(unsafe_code)]

use crate::language::{
    FileModel, FrameworkProfileResult, FrameworkProfileSelection, LanguagePlugin, LowerError,
    ManifestAnalysis, ScopeAnalysis, SourceSet,
};
use crate::metric::{CodeModel, Finding, Measurement, MetricPlugin};
use crate::prelude::*;
use crate::{Fnv1a64, PLUGIN_INTERFACE_VERSION, fnv1a_64};

struct MinimalLanguage;

impl LanguagePlugin for MinimalLanguage {
    fn extensions() -> Vec<String> {
        vec!["tiny".to_string()]
    }

    fn lower(
        _path: String,
        text: String,
        _dialect: Option<String>,
    ) -> Result<FileModel, LowerError> {
        Ok(FileModel {
            source_lines: text.lines().count() as u32,
            ..FileModel::empty(SourceSet::Production)
        })
    }
}

struct MinimalMetric;

impl MetricPlugin for MinimalMetric {
    fn key() -> String {
        "tiny_metric".to_string()
    }

    fn name() -> String {
        "Tiny metric".to_string()
    }

    fn description() -> String {
        "Counts files in the Code model".to_string()
    }

    fn measure(model: CodeModel) -> Measurement {
        let count = model.files.len() as f64;
        Measurement::from_counts(count, count, Vec::new())
    }
}

#[test]
fn language_plugin_trait_provides_safe_sensible_defaults() {
    assert_eq!(MinimalLanguage::extensions(), vec!["tiny"]);
    assert!(MinimalLanguage::known_dialects().is_empty());
    assert!(MinimalLanguage::flagged_line_rules().is_empty());
    let lowered = MinimalLanguage::lower("src/a.tiny".to_string(), "one\ntwo\n".to_string(), None)
        .expect("lower succeeds");
    assert_eq!(lowered.source_set, SourceSet::Production);
    assert_eq!(lowered.source_lines, 2);
    assert!(lowered.functions.is_empty());
    assert!(lowered.tokens.is_empty());
    assert!(lowered.flagged_lines.is_empty());
    assert!(lowered.hazard_lines.is_empty());
    assert!(lowered.runtime_imports.is_empty());
    assert!(!lowered.program_start);
    assert!(!lowered.is_tooling_file);
    assert!(lowered.diagnostics.is_empty());
    assert!(lowered.scope_artifact.is_none());

    assert!(!MinimalLanguage::has_imports());
    assert!(MinimalLanguage::manifest_paths(vec!["a.toml".to_string()]).is_empty());
    assert_eq!(
        MinimalLanguage::analyze_manifests(Vec::new()),
        ManifestAnalysis::empty()
    );
    assert!(
        MinimalLanguage::resolve_imports(Vec::new(), Vec::new(), Vec::new(), Vec::new()).is_empty()
    );
    assert!(MinimalLanguage::entry_points(Vec::new(), Vec::new()).is_empty());
    assert!(MinimalLanguage::reference_ranges().is_empty());
    assert!(MinimalLanguage::known_framework_profiles().is_empty());
    assert_eq!(
        MinimalLanguage::resolve_framework_profiles(
            String::new(),
            Vec::new(),
            Vec::new(),
            FrameworkProfileSelection::Auto,
        ),
        Ok(FrameworkProfileResult::empty())
    );
    assert!(!MinimalLanguage::has_hazard_rules());
    assert!(MinimalLanguage::hazard_rules().is_empty());
    assert!(!MinimalLanguage::has_scope_analysis());
    assert_eq!(
        MinimalLanguage::analyze_scope(String::new(), None, Vec::new()),
        ScopeAnalysis::empty()
    );
}

#[test]
fn metric_plugin_trait_defaults_version_to_one_and_builds_measurements() {
    assert_eq!(PLUGIN_INTERFACE_VERSION, "0.5.0");
    assert_eq!(MinimalMetric::key(), "tiny_metric");
    assert_eq!(MinimalMetric::name(), "Tiny metric");
    assert_eq!(MinimalMetric::version(), 1);

    let empty = Measurement::empty();
    assert_eq!(empty.ratio, 0.0);
    assert_eq!(empty.numerator, 0.0);
    assert_eq!(empty.denominator, 0.0);
    assert!(empty.findings.is_empty());

    let zero_denom = Measurement::from_counts(0.0, 0.0, Vec::new());
    assert_eq!(zero_denom.ratio, 0.0);

    let findings = vec![Finding {
        id: "f1".to_string(),
        path: "src/a.tiny".to_string(),
        line: 1,
        end_line: Some(2),
        message: "example".to_string(),
    }];
    let measured = Measurement::from_counts(1.0, 4.0, findings);
    assert!((measured.ratio - 0.25).abs() < 1e-12);
    assert_eq!(measured.findings.len(), 1);
}

#[test]
fn fnv1a_64_matches_standard_test_vectors() {
    assert_eq!(fnv1a_64(""), 0xcbf2_9ce4_8422_2325);
    assert_eq!(fnv1a_64("a"), 0xaf63_dc4c_8601_ec8c);
    assert_eq!(fnv1a_64("foobar"), 0x8594_4171_f739_67e8);

    let mut incremental = Fnv1a64::default();
    incremental.update(b"foo");
    incremental.update(b"bar");
    assert_eq!(incremental.finish(), fnv1a_64("foobar"));

    let mut with_number = Fnv1a64::new();
    with_number.update_u64(42);
    assert_ne!(with_number.finish(), Fnv1a64::new().finish());
}
