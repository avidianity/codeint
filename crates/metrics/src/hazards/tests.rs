use std::collections::BTreeSet;

use codeint_model::{
    CodeModel, HazardLine, LanguageFacts, Package, SourceFile, SourceSet, Token, TokenKind,
    Workspace, token_text_hash,
};

use super::{HAZARDS_METRIC_KEY, HazardLines, Hazards};
use crate::test_support::{empty_source_file, repo_path};

fn language_facts(language: &str, has_hazard_rules: bool, has_imports: bool) -> LanguageFacts {
    LanguageFacts {
        language: language.to_owned(),
        has_coverage: false,
        has_hazard_rules,
        has_imports,
    }
}

fn sql_file(path: &str, source_lines: u32, hazards: &[(u32, &str)]) -> SourceFile {
    let mut file = empty_source_file(path, SourceSet::Production, source_lines);
    file.model.hazard_lines = hazards
        .iter()
        .map(|&(line, rule)| HazardLine {
            line,
            rule: rule.to_owned(),
        })
        .collect();
    file
}

fn with_tokens(mut file: SourceFile, pairs: &[(u32, &str)]) -> SourceFile {
    file.model.tokens = pairs
        .iter()
        .map(|&(line, text)| Token {
            kind: TokenKind::Other,
            text_hash: token_text_hash(text),
            line,
        })
        .collect();
    file
}

fn sql_model(files: Vec<SourceFile>, workspace: Workspace) -> CodeModel {
    CodeModel {
        files,
        workspace,
        languages: vec![language_facts("sql", true, false)],
        ..CodeModel::default()
    }
}

fn measure_sql(files: Vec<SourceFile>, disabled: &[&str]) -> Hazards {
    let disabled_set: BTreeSet<String> = disabled.iter().map(|rule| (*rule).to_owned()).collect();
    Hazards::measure(&sql_model(files, Workspace::default()), &disabled_set)
        .expect("Hazards must be present for SQL model")
}

fn finding_triples(hazards: &Hazards) -> Vec<(&str, u32, &str)> {
    hazards
        .findings()
        .iter()
        .map(|item| (item.path.as_str(), item.line, item.rule.as_str()))
        .collect()
}

#[test]
fn hazards_is_absent_when_no_language_has_hazard_rules() {
    assert_eq!(HAZARDS_METRIC_KEY, "hazards");
    let none_disabled = BTreeSet::new();
    assert_eq!(
        Hazards::measure(&CodeModel::default(), &none_disabled),
        None
    );

    let ts_model = CodeModel {
        files: vec![empty_source_file("src/app.ts", SourceSet::Production, 20)],
        languages: vec![language_facts("typescript-javascript", false, true)],
        ..CodeModel::default()
    };
    assert_eq!(Hazards::measure(&ts_model, &none_disabled), None);
}

#[test]
fn hazards_reports_zero_ratio_for_clean_or_empty_sql() {
    let clean = measure_sql(vec![sql_file("db/schema.sql", 12, &[])], &[]);
    assert_eq!(
        clean.totals(),
        HazardLines {
            hazard_lines: 0,
            source_lines: 12,
        }
    );
    assert_eq!(clean.ratio(), 0.0);
    assert!(clean.findings().is_empty());

    let empty = measure_sql(vec![sql_file("db/empty.sql", 0, &[])], &[]);
    assert_eq!(empty.ratio(), 0.0);
    assert_eq!(empty.hazard_lines(), 0);
    assert_eq!(empty.source_lines(), 0);
}

#[test]
fn a_line_matching_multiple_rules_counts_once_and_emits_sorted_distinct_findings() {
    let measured = measure_sql(
        vec![
            sql_file(
                "db/z.sql",
                6,
                &[
                    (4, "select-star"),
                    (4, "count-without-filter"),
                    (4, "select-star"),
                ],
            ),
            sql_file("db/a.sql", 4, &[(2, "delete-without-filter")]),
        ],
        &[],
    );

    assert_eq!(measured.hazard_lines(), 2);
    assert_eq!(measured.source_lines(), 10);
    assert_eq!(measured.ratio(), 0.2);
    assert_eq!(
        finding_triples(&measured),
        [
            ("db/a.sql", 2, "delete-without-filter"),
            ("db/z.sql", 4, "count-without-filter"),
            ("db/z.sql", 4, "select-star"),
        ]
    );
}

#[test]
fn disabled_rules_are_excluded_from_both_numerator_and_findings() {
    let files = vec![sql_file(
        "db/queries.sql",
        10,
        &[
            (2, "select-star"),
            (2, "select-without-filter-or-limit"),
            (5, "select-star"),
        ],
    )];

    let partial = measure_sql(files.clone(), &["select-star"]);
    assert_eq!(partial.hazard_lines(), 1);
    assert_eq!(partial.ratio(), 0.1);
    assert_eq!(
        finding_triples(&partial),
        [("db/queries.sql", 2, "select-without-filter-or-limit")]
    );

    let all_off = measure_sql(files, &["select-star", "select-without-filter-or-limit"]);
    assert_eq!(all_off.hazard_lines(), 0);
    assert_eq!(all_off.source_lines(), 10);
    assert!(all_off.findings().is_empty());
}

#[test]
fn by_package_aggregates_only_packages_with_hazard_capable_files() {
    let api_root = repo_path("packages/api");
    let web_root = repo_path("packages/web");
    let workspace = Workspace::new([api_root.clone(), web_root.clone()]);
    let model = sql_model(
        vec![
            sql_file("migrations/001.sql", 4, &[(1, "drop")]),
            sql_file(
                "packages/api/queries.sql",
                6,
                &[(2, "select-star"), (5, "truncate")],
            ),
        ],
        workspace,
    );

    let measured = Hazards::measure(&model, &BTreeSet::new()).expect("Hazards present");
    let packages = measured.by_package(&model);

    assert_eq!(
        packages.get(&Package::Root).copied(),
        Some(HazardLines {
            hazard_lines: 1,
            source_lines: 4,
        })
    );
    assert_eq!(
        packages.get(&Package::Member(&api_root)).copied(),
        Some(HazardLines {
            hazard_lines: 2,
            source_lines: 6,
        })
    );
    assert_eq!(packages.get(&Package::Member(&web_root)), None);

    let summed_hazards: u64 = packages.values().map(|entry| entry.hazard_lines).sum();
    let summed_source: u64 = packages.values().map(|entry| entry.source_lines).sum();
    assert_eq!(summed_hazards, measured.hazard_lines());
    assert_eq!(summed_source, measured.source_lines());
}

#[test]
fn finding_id_stays_stable_when_line_moves_and_distinguishes_duplicate_occurrences() {
    let pair_at = |line_a: u32, line_b: u32, total_lines: u32| {
        let file = with_tokens(
            sql_file(
                "db/queries.sql",
                total_lines,
                &[(line_a, "select-star"), (line_b, "select-star")],
            ),
            &[
                (line_a, "SELECT"),
                (line_a, "*"),
                (line_b, "SELECT"),
                (line_b, "*"),
            ],
        );
        measure_sql(vec![file], &[])
    };

    let original = pair_at(2, 6, 10);
    let shifted = pair_at(12, 16, 20);
    assert_eq!(original.findings().len(), 2);
    assert_ne!(original.findings()[0].id, original.findings()[1].id);
    assert_eq!(original.findings()[0].id, shifted.findings()[0].id);
    assert_eq!(original.findings()[1].id, shifted.findings()[1].id);
}

#[test]
fn finding_debug_output_contains_no_source_text() {
    let secret = "super-secret-password-value";
    let file = with_tokens(
        sql_file("db/secrets.sql", 3, &[(2, "plain-text-secret")]),
        &[(2, "PASSWORD"), (2, secret)],
    );

    let measured = measure_sql(vec![file], &[]);
    let rendered = format!("{:?}", measured.findings());
    assert!(!rendered.contains(secret), "{rendered}");
    assert!(rendered.contains("plain-text-secret"), "{rendered}");
    assert!(rendered.contains("db/secrets.sql"), "{rendered}");
}
