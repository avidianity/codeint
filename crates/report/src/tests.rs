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
            hazard_lines: Vec::new(),
            runtime_imports: Vec::new(),
            program_start: false,
        },
    });
    let report = Report::summarize_with_config(
        vec![MeasuredLanguage::new(
            "typescript-javascript",
            model,
            ReferenceRanges::default(),
            true,
        )],
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

#[test]
fn parallel_measured_report_is_byte_identical_across_repeated_runs() {
    let mut model = CodeModel::default();
    for idx in 0..4usize {
        model.files.push(SourceFile {
            path: RepoPath::new(format!("src/mod_{idx}.ts")).expect("valid path"),
            model: FileModel {
                source_set: SourceSet::Production,
                source_lines: 10,
                functions: Vec::new(),
                tokens: Vec::new(),
                flagged_lines: Vec::new(),
                hazard_lines: Vec::new(),
                runtime_imports: Vec::new(),
                program_start: false,
            },
        });
    }
    let build_json = || {
        let report = Report::summarize_with_config(
            vec![MeasuredLanguage::new(
                "typescript-javascript",
                model.clone(),
                ReferenceRanges::default(),
                true,
            )],
            Vec::new(),
            FindingsShown::default(),
            &Budgets::default(),
            &[],
            None,
            DuplicationConfig::default(),
        )
        .expect("summarize");
        report.render(ReportFormat::Json).expect("json")
    };
    let expected = build_json();
    for _ in 0..8 {
        assert_eq!(build_json(), expected);
    }
}

fn token_stream_file(path: &str, lines: u32, tokens_per_line: usize) -> SourceFile {
    let mut tokens = Vec::new();
    for line in 1..=lines {
        for col in 0..tokens_per_line {
            let kind = match col % 3 {
                0 => codeint_model::TokenKind::Identifier,
                1 => codeint_model::TokenKind::Literal,
                _ => codeint_model::TokenKind::Other,
            };
            let text_hash = codeint_model::token_text_hash(&format!("op_{line}_{col}"));
            tokens.push(codeint_model::Token {
                line,
                kind,
                text_hash,
            });
        }
    }
    SourceFile {
        path: RepoPath::new(path).expect("valid path"),
        model: FileModel {
            source_set: SourceSet::Production,
            source_lines: lines,
            functions: Vec::new(),
            tokens,
            flagged_lines: Vec::new(),
            hazard_lines: Vec::new(),
            runtime_imports: Vec::new(),
            program_start: false,
        },
    }
}

fn summarize_polyglot_json(rust_model: &CodeModel, py_model: &CodeModel) -> serde_json::Value {
    let report = Report::summarize(
        vec![
            MeasuredLanguage::new("rust", rust_model.clone(), ReferenceRanges::default(), true),
            MeasuredLanguage::new("python", py_model.clone(), ReferenceRanges::default(), true),
        ],
        Vec::new(),
        FindingsShown::default(),
        &Budgets::default(),
    );
    serde_json::from_str(&report.render(ReportFormat::Json).expect("json")).expect("parse")
}

fn measure_polyglot_commit(rust_model: CodeModel, py_model: CodeModel) -> CommitMeasurement {
    CommitMeasurement::of_languages_with_options(
        vec![rust_model, py_model],
        &[],
        None,
        &codeint_config::DependenciesConfig::default(),
        DuplicationConfig::default(),
        None,
        None,
    )
    .expect("commit measurement")
}

#[test]
fn polyglot_duplication_isolates_languages_and_aggregates_per_language_totals() {
    let mut rust_model = CodeModel::default();
    rust_model
        .files
        .push(token_stream_file("src/only_rust.rs", 20, 5));

    let mut py_model = CodeModel::default();
    py_model
        .files
        .push(token_stream_file("src/same_as_rust.py", 20, 5));

    let isolated_json = summarize_polyglot_json(&rust_model, &py_model);
    let isolated_dup = &isolated_json["source_sets"]["production"]["duplication"];
    assert_eq!(
        isolated_dup["clone_lines"], 0,
        "identical normalized tokens across distinct Language plugins must not match: {isolated_dup}"
    );
    assert_eq!(
        isolated_dup["clone_groups"]
            .as_array()
            .expect("array")
            .len(),
        0
    );

    let isolated_prod =
        &measure_polyglot_commit(rust_model.clone(), py_model.clone()).into_source_sets()[0].1;
    assert_eq!(
        isolated_prod.duplication.numerator, 0.0,
        "CommitMeasurement must not match cross-language clones"
    );

    rust_model
        .files
        .push(token_stream_file("src/rust_copy.rs", 20, 5));
    let agg_json = summarize_polyglot_json(&rust_model, &py_model);
    let agg_dup = &agg_json["source_sets"]["production"]["duplication"];
    assert_eq!(agg_dup["metric_version"], 2);
    assert_eq!(agg_dup["clone_lines"], 40);
    assert_eq!(agg_dup["clone_groups"].as_array().expect("array").len(), 1);
    assert_eq!(agg_json["source_sets"]["production"]["source_lines"], 60);
    assert_eq!(
        agg_json["languages"]["rust"]["source_sets"]["production"]["duplication"]["clone_lines"],
        40
    );
    assert_eq!(
        agg_json["languages"]["python"]["source_sets"]["production"]["duplication"]["clone_lines"],
        0
    );

    let agg_prod = &measure_polyglot_commit(rust_model, py_model).into_source_sets()[0].1;
    assert_eq!(agg_prod.duplication.numerator, 40.0);
    assert_eq!(agg_prod.duplication.findings_worst_first.len(), 1);
}
