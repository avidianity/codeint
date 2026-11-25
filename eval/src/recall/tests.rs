use super::*;
use codeint_model::SourceSet;

fn measured(path: &str, functions: Vec<[u32; 3]>, imports: &[&str]) -> LoweredFile {
    LoweredFile {
        path: path.to_owned(),
        outcome: LoweredOutcome::Measured {
            source_set: SourceSet::Production,
            functions,
            imports: imports.iter().map(|import| (*import).to_owned()).collect(),
        },
    }
}

fn oracle_file(path: &str, functions: Vec<[u32; 3]>, imports: &[&str]) -> OracleFile {
    OracleFile {
        path: path.to_owned(),
        functions: Some(functions),
        imports: Some(imports.iter().map(|import| (*import).to_owned()).collect()),
        error: None,
    }
}

#[test]
fn recall_matches_functions_by_span_and_caps_decision_points_at_the_oracle_count() {
    let lowered = vec![
        measured("a.py", vec![[1, 4, 3], [6, 9, 1]], &["os", "sys"]),
        LoweredFile {
            path: "b.py".to_owned(),
            outcome: LoweredOutcome::Unparsable,
        },
    ];
    let oracle = vec![
        oracle_file(
            "a.py",
            vec![[1, 4, 2], [6, 9, 2], [11, 12, 1]],
            &["os", "json"],
        ),
        oracle_file("b.py", Vec::new(), &[]),
        OracleFile {
            path: "c.py".to_owned(),
            functions: None,
            imports: None,
            error: Some("invalid syntax".to_owned()),
        },
    ];
    let recall = LanguageRecall::compare(&oracle, &lowered);
    assert_eq!(recall.files_compared, 1);
    assert_eq!(
        recall.functions,
        RecallCount {
            matched: 2,
            total: 3
        }
    );
    assert_eq!(
        recall.decision_points,
        RecallCount {
            matched: 3,
            total: 5
        }
    );
    assert_eq!(
        recall.imports,
        RecallCount {
            matched: 1,
            total: 2
        }
    );
    assert_eq!(recall.oracle_unparsable_files, vec!["c.py".to_owned()]);
    assert_eq!(recall.unexpected_unparsable_files, vec!["b.py".to_owned()]);
    assert!(!recall.meets_bar());
}

#[test]
fn recall_meets_the_bar_at_ninety_eight_percent_with_no_unexpected_unparsable_files() {
    let mut recall = LanguageRecall {
        functions: RecallCount {
            matched: 98,
            total: 100,
        },
        ..LanguageRecall::default()
    };
    assert!(recall.meets_bar());
    recall.functions.matched = 97;
    assert!(!recall.meets_bar());
    assert_eq!(RecallCount::default().ratio(), 1.0);
}

#[test]
fn recall_counts_oracle_items_in_unmeasured_files_as_misses() {
    let lowered = vec![
        LoweredFile {
            path: "latin.php".to_owned(),
            outcome: LoweredOutcome::NotMeasured,
        },
        measured("a.php", vec![[1, 2, 1]], &[]),
    ];
    let oracle = vec![
        oracle_file("latin.php", vec![[3, 5, 2]], &["a"]),
        oracle_file("a.php", vec![[1, 2, 1]], &[]),
    ];
    let recall = LanguageRecall::compare(&oracle, &lowered);
    assert_eq!(recall.unmeasured_files, vec!["latin.php".to_owned()]);
    assert_eq!(
        recall.functions,
        RecallCount {
            matched: 1,
            total: 2
        }
    );
    assert_eq!(
        recall.decision_points,
        RecallCount {
            matched: 1,
            total: 3
        }
    );
    assert_eq!(
        recall.imports,
        RecallCount {
            matched: 0,
            total: 1
        }
    );
    assert!(!recall.meets_bar());
}

#[test]
fn functions_that_share_a_line_span_pair_without_losing_decision_points() {
    let shared = vec![[1, 1, 0], [1, 1, 1]];
    let lowered = vec![measured("a.py", shared.clone(), &[])];
    let oracle = vec![oracle_file("a.py", shared, &[])];

    let recall = LanguageRecall::compare(&oracle, &lowered);

    assert_eq!(recall.functions.matched, 2);
    assert_eq!(recall.functions.total, 2);
    assert_eq!(recall.decision_points.matched, 1);
    assert_eq!(recall.decision_points.total, 1);
}

#[test]
fn functions_that_share_a_line_span_never_earn_more_than_the_oracle_decision_points() {
    let lowered = vec![measured("a.py", vec![[1, 1, 3], [1, 1, 0]], &[])];
    let oracle = vec![oracle_file("a.py", vec![[1, 1, 1], [1, 1, 0]], &[])];

    let recall = LanguageRecall::compare(&oracle, &lowered);

    assert_eq!(recall.functions.matched, 2);
    assert_eq!(recall.decision_points.matched, 1);
    assert_eq!(recall.decision_points.total, 1);
}

#[test]
fn a_shared_span_with_fewer_codeint_functions_matches_only_what_codeint_has() {
    let lowered = vec![measured("a.py", vec![[1, 1, 1]], &[])];
    let oracle = vec![oracle_file("a.py", vec![[1, 1, 0], [1, 1, 1]], &[])];

    let recall = LanguageRecall::compare(&oracle, &lowered);

    assert_eq!(recall.functions.matched, 1);
    assert_eq!(recall.functions.total, 2);
    assert_eq!(recall.decision_points.matched, 1);
}
