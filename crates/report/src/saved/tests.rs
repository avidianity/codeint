use super::{SavedReport, SavedReportError};

#[test]
fn rejects_non_object_json_values() {
    let err = SavedReport::from_json("[1, 2]").expect_err("array is not a saved report");
    assert!(matches!(err, SavedReportError::NotAnObject));
}

#[test]
fn accepts_explicit_schema_version_1() {
    let json = r#"{"schema_version": 1, "files": 0, "source_lines": 0, "source_sets": {}}"#;
    let parsed = SavedReport::from_json(json).expect("schema_version 1 is supported");
    assert!(matches!(parsed, SavedReport::Check(_)));
}

#[test]
fn rejects_unsupported_schema_version_values() {
    let err = SavedReport::from_json(
        r#"{"schema_version": "2.0", "files": 0, "source_lines": 0, "source_sets": {}}"#,
    )
    .expect_err("schema_version 2.0 is unsupported");
    assert!(matches!(
        err,
        SavedReportError::UnsupportedSchemaVersion { .. }
    ));
}

fn rerender_saved_json(raw: &str) -> (SavedReport, serde_json::Value) {
    let parsed = SavedReport::from_json(raw).expect("parse saved report");
    let rendered = parsed
        .render(codeint_config::ReportRenderFormat::Json)
        .expect("render saved json");
    let value: serde_json::Value = serde_json::from_str(&rendered).expect("valid json");
    (parsed, value)
}

#[test]
fn saved_check_and_diff_reports_preserve_metric_versions_across_json_and_toon_round_trips() {
    use codeint_config::{
        Budgets, FindingsShown, RegressionLimits, ReportFormat, ReportRenderFormat,
    };
    use codeint_model::{
        CodeModel, ExternalMetricMeasurement, FileModel, MetricPlugin, MetricPluginError,
        ReferenceRanges, RepoPath, SourceFile, SourceSet,
    };

    use crate::{BaseCommit, CommitMeasurement, DiffReport, Head, MeasuredLanguage, Report};

    struct VersionFourMetric;

    impl MetricPlugin for VersionFourMetric {
        fn key(&self) -> &str {
            "custom_v4"
        }

        fn name(&self) -> &str {
            "Custom v4"
        }

        fn description(&self) -> &str {
            "Test Metric plugin at version 4"
        }

        fn version(&self) -> u32 {
            4
        }

        fn measure(
            &self,
            _model: &CodeModel,
        ) -> Result<ExternalMetricMeasurement, MetricPluginError> {
            Ok(ExternalMetricMeasurement {
                ratio: 0.5,
                numerator: 1.0,
                denominator: 2.0,
                findings: Vec::new(),
            })
        }
    }

    let mut model = CodeModel::default();
    model.files.push(SourceFile {
        path: RepoPath::new("src/app.ts").expect("path"),
        model: FileModel {
            source_set: SourceSet::Production,
            source_lines: 4,
            functions: Vec::new(),
            tokens: Vec::new(),
            flagged_lines: Vec::new(),
            hazard_lines: Vec::new(),
            runtime_imports: Vec::new(),
            program_start: false,
            is_tooling_file: false,
            diagnostics: Vec::new(),
            scope_artifact: None,
        },
    });
    let plugin = VersionFourMetric;
    let plugins: [&dyn MetricPlugin; 1] = [&plugin];
    let budgets = Budgets::default();

    let check = Report::summarize_with_metrics(
        vec![MeasuredLanguage::new(
            "typescript-javascript",
            model.clone(),
            ReferenceRanges::default(),
            true,
        )],
        Vec::new(),
        FindingsShown::default(),
        &budgets,
        &plugins,
    )
    .expect("check report");
    let check_json = check.render(ReportFormat::Json).expect("check json");
    let (saved_check, rerendered_check_json) = rerender_saved_json(&check_json);
    assert_eq!(
        rerendered_check_json["source_sets"]["production"]["erosion"]["metric_version"],
        1
    );
    assert_eq!(
        rerendered_check_json["source_sets"]["production"]["verbosity"]["metric_version"],
        2
    );
    assert_eq!(
        rerendered_check_json["source_sets"]["production"]["custom_v4"]["metric_version"],
        4
    );
    let rerendered_check_toon = saved_check
        .render(ReportRenderFormat::Toon)
        .expect("render saved check toon");
    assert!(rerendered_check_toon.contains("metric_version: 2"));
    assert!(rerendered_check_toon.contains("metric_version: 4"));

    let mut legacy_check: serde_json::Value =
        serde_json::from_str(&check_json).expect("parse check json");
    for metric in [
        "erosion",
        "cognitive_complexity",
        "duplication",
        "verbosity",
        "import_cycles",
        "custom_v4",
    ] {
        legacy_check["source_sets"]["production"][metric]
            .as_object_mut()
            .expect("metric object")
            .remove("metric_version");
    }
    legacy_check["source_sets"]["production"]["verbosity"]
        .as_object_mut()
        .expect("verbosity object")
        .insert("clone_lines".to_owned(), serde_json::json!(5));
    let (_, legacy_rerendered) = rerender_saved_json(&legacy_check.to_string());
    assert_eq!(
        legacy_rerendered["source_sets"]["production"]["erosion"]["metric_version"],
        1
    );
    assert_eq!(
        legacy_rerendered["source_sets"]["production"]["verbosity"]["metric_version"],
        1
    );

    let base_measurement =
        CommitMeasurement::of_with_metrics(model.clone(), &plugins).expect("base measurement");
    let head_measurement =
        CommitMeasurement::of_with_metrics(model, &plugins).expect("head measurement");
    let diff = DiffReport::compare(
        BaseCommit {
            git_ref: "main".to_owned(),
            merge_base: "abc".to_owned(),
        },
        Head::WorkingTree,
        base_measurement,
        head_measurement,
        FindingsShown::default(),
        &budgets,
        &RegressionLimits::default(),
    );
    let diff_json = diff.render(ReportFormat::Json).expect("diff json");
    let (_, rerendered_diff) = rerender_saved_json(&diff_json);
    assert_eq!(
        rerendered_diff["source_sets"]["production"]["erosion"]["metric_version"],
        1
    );
    assert_eq!(
        rerendered_diff["source_sets"]["production"]["custom_v4"]["metric_version"],
        4
    );
}
