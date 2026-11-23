use codeint_config::{Budgets, DuplicationConfig, FindingsShown, MinSimilarity, ReportFormat};
use codeint_model::{CodeModel, FileModel, ReferenceRanges, RepoPath, SourceFile, SourceSet};

use super::{CommitMeasurement, Decimal, MeasuredLanguage, Report};

#[test]
fn a_negative_zero_from_an_empty_sum_is_reported_as_zero() {
    let empty_sum: f64 = std::iter::empty::<f64>().sum();

    let rendered = serde_json::to_string(&Decimal::rounded(empty_sum)).expect("serialize");

    assert_eq!(rendered, "0.0");
}

#[test]
fn a_tiny_negative_value_that_rounds_to_zero_is_reported_as_zero() {
    let rendered = serde_json::to_string(&Decimal::rounded(-0.00001)).expect("serialize");

    assert_eq!(rendered, "0.0");
}

#[test]
fn tiny_positive_min_similarity_preserves_nonzero_json_toon_and_markdown_display() {
    let mut model = CodeModel::default();
    model.files.push(SourceFile {
        path: RepoPath::new("src/app.ts").expect("valid path"),
        model: FileModel {
            source_set: SourceSet::Production,
            source_lines: 3,
            functions: Vec::new(),
            tokens: Vec::new(),
            flagged_lines: Vec::new(),
            runtime_imports: Vec::new(),
        },
    });
    let report = Report::summarize_with_config(
        vec![MeasuredLanguage {
            plugin: "typescript-javascript".to_owned(),
            model,
            reference_ranges: ReferenceRanges::default(),
        }],
        Vec::new(),
        FindingsShown::default(),
        &Budgets::default(),
        &[],
        None,
        DuplicationConfig {
            near_clones: true,
            min_similarity: MinSimilarity::new(0.00001).expect("positive min_similarity"),
        },
    )
    .expect("summarize");

    let json: serde_json::Value =
        serde_json::from_str(&report.render(ReportFormat::Json).expect("json")).expect("parse");
    let min_sim = json["source_sets"]["production"]["duplication"]["near_clones"]["min_similarity"]
        .as_f64()
        .expect("min_similarity f64");
    assert_eq!(min_sim, 0.00001);

    let toon = report.render(ReportFormat::Toon).expect("toon");
    assert!(
        !toon.contains("min_similarity: 0\n"),
        "TOON rounded tiny min_similarity to 0: {toon}"
    );

    let markdown = report.render(ReportFormat::Markdown).expect("markdown");
    assert!(
        markdown.contains("(near-clones >= <1%)"),
        "expected <1% in markdown: {markdown}"
    );
    assert!(
        !markdown.contains("(near-clones >= 0%)"),
        "unexpected 0% in markdown: {markdown}"
    );
}

#[test]
fn legacy_commit_measurement_missing_clone_families_fails_deserialization() {
    let measurement = CommitMeasurement::of(CodeModel::default());
    let mut encoded = serde_json::to_value(&measurement).expect("serialize measurement");
    encoded["production"]
        .as_object_mut()
        .expect("production object")
        .remove("clone_families");

    let decoded = serde_json::from_value::<CommitMeasurement>(encoded);
    assert!(decoded.is_err());
}
