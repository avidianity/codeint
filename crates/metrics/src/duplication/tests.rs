use std::num::NonZeroUsize;

use codeint_model::{CodeModel, FileModel, RepoPath, SourceFile, Token};

use super::{CloneGroup, Duplication, Fragment, MinimumCloneTokens};

fn file(path: &str, lines: &[&str]) -> SourceFile {
    let tokens = lines
        .iter()
        .zip(1u32..)
        .flat_map(|(line, number)| {
            line.split_whitespace().map(move |text| Token {
                text: text.to_owned(),
                line: number,
            })
        })
        .collect();
    SourceFile {
        path: RepoPath::new(path).expect("a valid path"),
        model: FileModel {
            source_lines: u32::try_from(lines.len()).expect("few lines"),
            functions: Vec::new(),
            tokens,
            flagged_lines: Vec::new(),
        },
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

    let duplication = Duplication::measure(&model, minimum(4));

    assert_eq!(
        duplication.clone_groups,
        [CloneGroup {
            tokens: 6,
            fragments: vec![
                fragment("one.ts", 1, 2),
                fragment("three.ts", 1, 2),
                fragment("two.ts", 2, 3),
            ],
        }]
    );
    assert_eq!(duplication.clone_lines, 6);
    assert_eq!(duplication.source_lines, 8);
    assert_eq!(duplication.ratio(), 0.75);
}

#[test]
fn a_copy_inside_one_file_is_a_clone_group() {
    let model = model(vec![file("one.ts", &["a b c d", "x", "a b c d"])]);

    let duplication = Duplication::measure(&model, minimum(4));

    assert_eq!(
        duplication.clone_groups,
        [CloneGroup {
            tokens: 4,
            fragments: vec![fragment("one.ts", 1, 1), fragment("one.ts", 3, 3)],
        }]
    );
    assert_eq!(duplication.clone_lines, 2);
}

#[test]
fn a_repeated_token_run_does_not_form_a_clone_group_with_itself() {
    let model = model(vec![file("zeros.ts", &["0 0 0 0 0"])]);

    let duplication = Duplication::measure(&model, minimum(4));

    assert_eq!(duplication.clone_groups, []);
    assert_eq!(duplication.clone_lines, 0);
}

#[test]
fn fragments_that_share_fewer_tokens_than_the_minimum_are_not_clones() {
    let model = model(vec![
        file("one.ts", &["a b c x"]),
        file("two.ts", &["a b c y"]),
    ]);

    let duplication = Duplication::measure(&model, minimum(4));

    assert_eq!(duplication.clone_groups, []);
}

#[test]
fn a_fragment_of_exactly_the_minimum_size_is_a_clone() {
    let model = model(vec![
        file("one.ts", &["a b c d"]),
        file("two.ts", &["a b c d"]),
    ]);

    let duplication = Duplication::measure(&model, minimum(4));

    assert_eq!(duplication.clone_groups.len(), 1);
    assert_eq!(duplication.clone_groups[0].tokens, 4);
}

#[test]
fn a_model_without_source_lines_has_a_ratio_of_zero() {
    let duplication = Duplication::measure(&CodeModel::default(), minimum(4));

    assert_eq!(duplication.ratio(), 0.0);
    assert_eq!(duplication.clone_groups, []);
}

#[test]
fn larger_clone_groups_come_first() {
    let model = model(vec![
        file("one.ts", &["a b c d", "p q r s t"]),
        file("two.ts", &["a b c d", "x", "p q r s t"]),
    ]);

    let duplication = Duplication::measure(&model, minimum(4));

    assert_eq!(
        duplication
            .clone_groups
            .iter()
            .map(|group| group.tokens)
            .collect::<Vec<_>>(),
        [5, 4]
    );
}
