use std::num::NonZeroUsize;

use codeint_model::{
    CodeModel, FileModel, RepoPath, SourceFile, SourceSet, Token, TokenKind, token_text_hash,
};

use super::{CloneFamily, Duplication, Fragment, MinimumCloneTokens, MinimumDistinctTokens};

fn file(path: &str, lines: &[&str]) -> SourceFile {
    let tokens = lines
        .iter()
        .zip(1u32..)
        .flat_map(|(line, number)| {
            line.split_whitespace().map(move |text| Token {
                kind: kind_of(text),
                text_hash: token_text_hash(text),
                line: number,
            })
        })
        .collect();
    SourceFile {
        path: RepoPath::new(path).expect("a valid path"),
        model: FileModel {
            source_set: SourceSet::Production,
            source_lines: u32::try_from(lines.len()).expect("few lines"),
            functions: Vec::new(),
            tokens,
            flagged_lines: Vec::new(),
            runtime_imports: Vec::new(),
        },
    }
}

fn kind_of(text: &str) -> TokenKind {
    if text.starts_with('$') {
        TokenKind::Identifier
    } else if text.starts_with('"') {
        TokenKind::Literal
    } else if text.starts_with('@') {
        TokenKind::ImportOrExport
    } else {
        TokenKind::Other
    }
}

fn model(files: Vec<SourceFile>) -> CodeModel {
    CodeModel {
        files,
        ..CodeModel::default()
    }
}

fn minimum(tokens: usize) -> MinimumCloneTokens {
    MinimumCloneTokens::new(NonZeroUsize::new(tokens).expect("not zero"))
}

fn measure(model: &CodeModel, minimum: MinimumCloneTokens) -> Duplication {
    Duplication::measure(model, minimum, MinimumDistinctTokens::new(0))
}

fn groups(duplication: &Duplication) -> Vec<(usize, Vec<Fragment>)> {
    duplication
        .clone_groups
        .iter()
        .map(|group| (group.tokens, group.fragments.clone()))
        .collect()
}

fn fragment(path: &str, start_line: u32, end_line: u32) -> Fragment {
    Fragment {
        path: RepoPath::new(path).expect("a valid path"),
        start_line,
        end_line,
    }
}

#[test]
fn three_copies_form_one_clone_group_with_three_fragments() {
    let lines = ["a b c", "d e f"];
    let model = model(vec![
        file("one.ts", &lines),
        file("two.ts", &["x", "a b c", "d e f", "y"]),
        file("three.ts", &lines),
    ]);

    let duplication = measure(&model, minimum(4));

    assert_eq!(
        groups(&duplication),
        [(
            6,
            vec![
                fragment("one.ts", 1, 2),
                fragment("three.ts", 1, 2),
                fragment("two.ts", 2, 3),
            ]
        )]
    );
    assert_eq!(duplication.clone_lines.count(), 6);
    assert_eq!(duplication.source_lines, 8);
    assert_eq!(duplication.ratio(), 0.75);
}

#[test]
fn a_copy_inside_one_file_is_a_clone_group() {
    let model = model(vec![file("one.ts", &["a b c d", "x", "a b c d"])]);

    let duplication = measure(&model, minimum(4));

    assert_eq!(
        groups(&duplication),
        [(4, vec![fragment("one.ts", 1, 1), fragment("one.ts", 3, 3)])]
    );
    assert_eq!(duplication.clone_lines.count(), 2);
}

#[test]
fn a_repeated_token_run_does_not_form_a_clone_group_with_itself() {
    let model = model(vec![file("zeros.ts", &["0 0 0 0 0"])]);

    let duplication = measure(&model, minimum(4));

    assert_eq!(groups(&duplication), []);
    assert_eq!(duplication.clone_lines.count(), 0);
}

#[test]
fn fragments_that_share_fewer_tokens_than_the_minimum_are_not_clones() {
    let model = model(vec![
        file("one.ts", &["a b c x"]),
        file("two.ts", &["a b c y"]),
    ]);

    let duplication = measure(&model, minimum(4));

    assert_eq!(groups(&duplication), []);
}

#[test]
fn a_fragment_of_exactly_the_minimum_size_is_a_clone() {
    let model = model(vec![
        file("one.ts", &["a b c d"]),
        file("two.ts", &["a b c d"]),
    ]);

    let duplication = measure(&model, minimum(4));

    assert_eq!(duplication.clone_groups.len(), 1);
    assert_eq!(duplication.clone_groups[0].tokens, 4);
}

#[test]
fn a_model_without_source_lines_has_a_ratio_of_zero() {
    let duplication = measure(&CodeModel::default(), minimum(4));

    assert_eq!(duplication.ratio(), 0.0);
    assert_eq!(groups(&duplication), []);
}

#[test]
fn larger_clone_groups_come_first() {
    let model = model(vec![
        file("one.ts", &["a b c d", "p q r s t"]),
        file("two.ts", &["a b c d", "x", "p q r s t"]),
    ]);

    let duplication = measure(&model, minimum(4));

    assert_eq!(
        duplication
            .clone_groups
            .iter()
            .map(|group| group.tokens)
            .collect::<Vec<_>>(),
        [5, 4]
    );
}

#[test]
fn copies_with_other_identifiers_and_literals_form_one_clone_group() {
    let model = model(vec![
        file("one.ts", &["a $first = \"x\" ;"]),
        file("two.ts", &["a $second = \"y\" ;"]),
    ]);

    let duplication = measure(&model, minimum(5));

    assert_eq!(
        groups(&duplication),
        [(5, vec![fragment("one.ts", 1, 1), fragment("two.ts", 1, 1)])]
    );
}

#[test]
fn an_identifier_and_a_literal_in_the_same_place_are_not_a_clone() {
    let model = model(vec![
        file("one.ts", &["a $name = b ;"]),
        file("two.ts", &["a \"name\" = b ;"]),
    ]);

    let duplication = measure(&model, minimum(5));

    assert_eq!(groups(&duplication), []);
}

#[test]
fn copies_with_other_keywords_or_operators_are_not_a_clone() {
    let model = model(vec![
        file("one.ts", &["if $a < \"x\" ;"]),
        file("two.ts", &["while $a > \"x\" ;"]),
    ]);

    let duplication = measure(&model, minimum(3));

    assert_eq!(groups(&duplication), []);
}

#[test]
fn a_fragment_of_exactly_the_minimum_distinct_tokens_is_a_clone_and_one_fewer_is_not() {
    let fourteen = "$a = \"x\" ; $b + $c - ( $d * $e ) / $f < > ! ?";
    let thirteen = "$a = \"x\" ; $b + $c - ( $d * $e ) / $f < > ! !";
    let at = model(vec![
        file("one.ts", &[fourteen]),
        file("two.ts", &[fourteen]),
    ]);
    let below = model(vec![
        file("one.ts", &[thirteen]),
        file("two.ts", &[thirteen]),
    ]);

    let at = Duplication::measure(&at, minimum(4), MinimumDistinctTokens::DEFAULT);
    let below = Duplication::measure(&below, minimum(4), MinimumDistinctTokens::DEFAULT);

    assert_eq!(
        groups(&at),
        [(19, vec![fragment("one.ts", 1, 1), fragment("two.ts", 1, 1)])]
    );
    assert_eq!(groups(&below), []);
    assert_eq!(below.clone_lines.count(), 0);
}

#[test]
fn a_fragment_never_spans_an_import_or_export_token() {
    let model = model(vec![
        file("one.ts", &["a b c", "@import", "d e f"]),
        file("two.ts", &["a b c", "@import", "d e f"]),
    ]);

    let duplication = measure(&model, minimum(4));

    assert_eq!(groups(&duplication), []);
}

#[test]
fn import_or_export_tokens_never_form_a_clone_group() {
    let model = model(vec![
        file("one.ts", &["@a @b @c @d @e"]),
        file("two.ts", &["@a @b @c @d @e"]),
    ]);

    let duplication = measure(&model, minimum(4));

    assert_eq!(groups(&duplication), []);
}

#[test]
fn a_copy_after_import_or_export_tokens_keeps_its_place_in_the_file() {
    let model = model(vec![
        file("one.ts", &["@a @b", "a b c d"]),
        file("two.ts", &["@a", "@b @c", "a b c d"]),
    ]);

    let duplication = measure(&model, minimum(4));

    assert_eq!(
        groups(&duplication),
        [(4, vec![fragment("one.ts", 2, 2), fragment("two.ts", 3, 3)])]
    );
}

fn assert_single_family(
    duplication: &Duplication,
    expected_lines: u64,
    expected_groups: usize,
    expected_ranges: &[(&str, u32, u32)],
) {
    assert_eq!(duplication.clone_families.len(), 1);
    let family: &CloneFamily = &duplication.clone_families[0];
    let expected: Vec<Fragment> = expected_ranges
        .iter()
        .map(|&(path, start, end)| fragment(path, start, end))
        .collect();
    assert_eq!(family.duplicated_lines, expected_lines);
    assert_eq!(family.clone_group_ids.len(), expected_groups);
    assert_eq!(family.fragments, expected);
}

fn measure_files(specs: &[(&str, &[&str])], min_tokens: usize) -> Duplication {
    let files = specs
        .iter()
        .map(|&(path, lines)| file(path, lines))
        .collect();
    measure(&model(files), minimum(min_tokens))
}

#[test]
fn two_copies_alone_do_not_form_a_clone_family() {
    let body = ["a b c", "d e f"];
    let duplication = measure_files(&[("one.ts", &body), ("two.ts", &body)], 4);

    assert_eq!(duplication.clone_groups.len(), 1);
    assert!(duplication.clone_families.is_empty());
}

#[test]
fn three_copies_across_files_form_one_clone_family_in_canonical_order() {
    let body = ["a b c", "d e f"];
    let padded = ["x", "a b c", "d e f", "y"];
    let duplication = measure_files(
        &[("zeta.ts", &body), ("mid.ts", &padded), ("alpha.ts", &body)],
        4,
    );

    assert_single_family(
        &duplication,
        6,
        1,
        &[("alpha.ts", 1, 2), ("mid.ts", 2, 3), ("zeta.ts", 1, 2)],
    );
    assert_eq!(duplication.clone_lines.count(), 6);
    assert_eq!(duplication.ratio(), 0.75);
    assert_ne!(
        duplication.clone_families[0].id,
        duplication.clone_groups[0].id
    );
    assert_eq!(
        duplication.clone_families[0].clone_group_ids,
        vec![duplication.clone_groups[0].id]
    );
}

#[test]
fn transitively_overlapping_exact_clone_groups_cluster_into_one_family_without_double_counting() {
    let duplication = measure_files(
        &[
            ("a.ts", &["p1 p2", "q1 q2"]),
            ("b.ts", &["p1 p2", "q1 q2", "r1 r2"]),
            ("c.ts", &["q1 q2", "r1 r2"]),
        ],
        4,
    );

    assert_eq!(duplication.clone_groups.len(), 2);
    assert_single_family(
        &duplication,
        7,
        2,
        &[("a.ts", 1, 2), ("b.ts", 1, 3), ("c.ts", 1, 2)],
    );
    assert_eq!(duplication.clone_lines.count(), 7);
    assert_eq!(duplication.source_lines, 7);
    assert_eq!(duplication.ratio(), 1.0);
}

#[test]
fn clone_families_are_ordered_worst_first_by_total_duplicated_lines() {
    let model = model(vec![
        file("small_a.ts", &["s1 s2 s3 s4 s5 s6"]),
        file("small_b.ts", &["s1 s2 s3 s4 s5 s6"]),
        file("small_c.ts", &["s1 s2 s3 s4 s5 s6"]),
        file("tall_a.ts", &["t1", "t2", "t3", "t4"]),
        file("tall_b.ts", &["t1", "t2", "t3", "t4"]),
        file("tall_c.ts", &["t1", "t2", "t3", "t4"]),
    ]);

    let duplication = measure(&model, minimum(4));

    assert_eq!(
        duplication
            .clone_groups
            .iter()
            .map(|group| group.tokens)
            .collect::<Vec<_>>(),
        [6, 4]
    );
    assert_eq!(
        duplication
            .clone_families
            .iter()
            .map(|family| family.duplicated_lines)
            .collect::<Vec<_>>(),
        [12, 3]
    );
}

#[test]
fn clone_family_id_is_order_independent_and_stable_when_fragments_move() {
    let lines = ["a b c", "d e f"];
    let first = measure(
        &model(vec![
            file("c.ts", &lines),
            file("a.ts", &lines),
            file("b.ts", &lines),
        ]),
        minimum(4),
    );
    let shifted = measure(
        &model(vec![
            file("a.ts", &lines),
            file("b.ts", &["u", "v", "a b c", "d e f"]),
            file("c.ts", &lines),
        ]),
        minimum(4),
    );

    assert_eq!(first.clone_families.len(), 1);
    assert_eq!(shifted.clone_families.len(), 1);
    assert_eq!(first.clone_families[0].id, shifted.clone_families[0].id);
    assert_eq!(
        first.clone_families[0].fragments,
        vec![
            fragment("a.ts", 1, 2),
            fragment("b.ts", 1, 2),
            fragment("c.ts", 1, 2),
        ]
    );
    assert_eq!(
        shifted.clone_families[0].fragments,
        vec![
            fragment("a.ts", 1, 2),
            fragment("b.ts", 3, 4),
            fragment("c.ts", 1, 2),
        ]
    );
}
