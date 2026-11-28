use std::num::NonZeroUsize;

use codeint_model::{
    FileModel, RepoPath, SourceFile, SourceSet, SourceSetModel, Token, TokenKind,
    language::Function, token_text_hash,
};

use super::{
    CloneFamily, Duplication, Fragment, MinimumCloneTokens, MinimumDistinctTokens, NearCloneConfig,
};

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
            hazard_lines: Vec::new(),
            runtime_imports: Vec::new(),
            program_start: false,
            is_tooling_file: false,
            diagnostics: Vec::new(),
            scope_artifact: None,
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

fn model(files: Vec<SourceFile>) -> SourceSetModel {
    SourceSetModel {
        files,
        ..SourceSetModel::default()
    }
}

fn minimum(tokens: usize) -> MinimumCloneTokens {
    MinimumCloneTokens::new(NonZeroUsize::new(tokens).expect("not zero"))
}

fn measure(model: &SourceSetModel, minimum: MinimumCloneTokens) -> Duplication {
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
    let duplication = measure(&SourceSetModel::default(), minimum(4));

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

fn near_clone_lines() -> Vec<&'static str> {
    vec![
        "fn $f ( $a , $b , $c ) {",
        "let $x = $a + $b * \"2\" ;",
        "let $y = $b - $c / \"3\" ;",
        "if ( $x > $y ) {",
        "$x += ( $a - $b ) * $c ;",
        "}",
        "for ( let $i of $a ) {",
        "if ( $i >= \"0\" ) {",
        "$y += $i % \"5\" ;",
        "}",
        "}",
        "return $x + $y - \"1\" ;",
        "}",
    ]
}

fn file_with_functions(path: &str, lines: &[&str], functions: Vec<Function>) -> SourceFile {
    let mut source = file(path, lines);
    source.model.functions = functions;
    source
}

fn whole_function(line_count: usize) -> Vec<Function> {
    vec![Function {
        line: 1,
        end_line: u32::try_from(line_count).expect("few lines"),
        decision_points: 2,
        source_lines: u32::try_from(line_count).expect("few lines"),
        name: None,
        cognitive_complexity: 0,
        coverage: None,
    }]
}

fn function_file(path: &str, lines: &[&str]) -> SourceFile {
    file_with_functions(path, lines, whole_function(lines.len()))
}

fn measure_near_pair(left: &[&str], right: &[&str], threshold: Option<f64>) -> Duplication {
    let code = model(vec![
        function_file("one.ts", left),
        function_file("two.ts", right),
    ]);
    let config = threshold.map(|value| NearCloneConfig::new(value).expect("valid threshold"));
    Duplication::measure_with_near_clones(
        &code,
        minimum(55),
        MinimumDistinctTokens::new(10),
        config,
    )
}

#[test]
fn near_clones_off_by_default_leaves_edited_copy_undetected() {
    let base = near_clone_lines();
    let mut edited = base.clone();
    edited.insert(5, "$log ( $x ) ;");

    let duplication = measure_near_pair(&base, &edited, None);

    assert!(duplication.clone_groups.is_empty());
    assert_eq!(duplication.clone_lines.count(), 0);
    assert_eq!(duplication.exact_clone_lines.count(), 0);
}

#[test]
fn near_clones_enabled_catches_one_line_edit_with_similarity_and_keeps_exact_lines_separate() {
    let base = near_clone_lines();
    let mut edited = base.clone();
    edited.insert(5, "$log ( $x ) ;");

    let duplication = measure_near_pair(&base, &edited, Some(0.85));

    assert_eq!(duplication.clone_groups.len(), 1);
    let group = &duplication.clone_groups[0];
    let similarity = group.similarity.expect("near-clone reports similarity");
    assert!(
        (0.85..1.0).contains(&similarity),
        "unexpected similarity {similarity}"
    );
    assert_eq!(
        group.fragments,
        vec![fragment("one.ts", 1, 13), fragment("two.ts", 1, 14)]
    );
    assert_eq!(duplication.clone_lines.count(), 27);
    assert_eq!(duplication.exact_clone_lines.count(), 0);
}

#[test]
fn near_clones_ignore_blocks_below_min_similarity() {
    let base = near_clone_lines();
    let different = [
        "class $cls {",
        "static $m ( $p ) {",
        "switch ( $p ) {",
        "case \"a\" : break ;",
        "case \"b\" : continue ;",
        "default : throw $p ;",
        "}",
        "while ( $p != \"0\" ) {",
        "$p -= \"1\" ;",
        "$p ^= \"3\" ;",
        "$p |= \"7\" ;",
        "yield $p ;",
        "}",
        "}",
        "}",
    ];

    let duplication = measure_near_pair(&base, &different, Some(0.85));

    assert!(duplication.clone_groups.is_empty());
}

#[test]
fn near_clones_exclude_import_or_export_tokens_and_respect_token_guards() {
    let imports_only = [
        "@import @a @b @c @d @e @f @g @h @i @j",
        "@import @k @l @m @n @o @p @q @r @s @t",
        "@export @u @v @w @x @y @z @a1 @b1 @c1 @d1",
    ];
    let imports_edited = [
        "@import @a @b @c @d @e @f @g @h @i @j",
        "@import @k @l @m @n @o @p @q @r @s @t",
        "@export @u @v @w @x @y @z @a1 @b1 @c1 @d1 @e1",
    ];
    let low_distinct = [
        "{ $a : \"1\" , $b : \"2\" , $c : \"3\" , $d : \"4\" ,",
        "$e : \"5\" , $f : \"6\" , $g : \"7\" , $h : \"8\" ,",
        "$i : \"9\" , $j : \"10\" , $k : \"11\" , $l : \"12\" }",
    ];
    let low_distinct_edited = [
        "{ $a : \"1\" , $b : \"2\" , $c : \"3\" , $d : \"4\" ,",
        "$e : \"5\" , $f : \"6\" , $g : \"7\" , $h : \"8\" ,",
        "$i : \"9\" , $j : \"10\" , $k : \"11\" , $l : \"12\" , $m : \"13\" }",
    ];
    let config = NearCloneConfig::new(0.80).expect("valid threshold");

    let imports_dup = Duplication::measure_with_near_clones(
        &model(vec![
            file("a.ts", &imports_only),
            file("b.ts", &imports_edited),
        ]),
        minimum(10),
        MinimumDistinctTokens::new(0),
        Some(config),
    );
    let boilerplate_dup = Duplication::measure_with_near_clones(
        &model(vec![
            file("a.ts", &low_distinct),
            file("b.ts", &low_distinct_edited),
        ]),
        minimum(20),
        MinimumDistinctTokens::DEFAULT,
        Some(config),
    );

    assert!(imports_dup.clone_groups.is_empty());
    assert!(boilerplate_dup.clone_groups.is_empty());
}

#[test]
fn exact_clones_are_not_duplicated_as_near_clones() {
    let base = near_clone_lines();

    let duplication = measure_near_pair(&base, &base, Some(0.85));

    assert_eq!(duplication.clone_groups.len(), 1);
    assert_eq!(duplication.clone_groups[0].similarity, None);
    assert_eq!(duplication.clone_lines, duplication.exact_clone_lines);
}

#[test]
fn three_near_clones_cluster_into_one_clone_family_with_stable_id_across_line_shifts() {
    let base = near_clone_lines();
    let mut second = base.clone();
    second.insert(5, "$log ( $x ) ;");
    let mut third = base.clone();
    third.insert(10, "$trace ( $y ) ;");
    let config = NearCloneConfig::new(0.85).expect("valid threshold");

    let first_run = Duplication::measure_with_near_clones(
        &model(vec![
            file_with_functions("c.ts", &third, whole_function(third.len())),
            file_with_functions("a.ts", &base, whole_function(base.len())),
            file_with_functions("b.ts", &second, whole_function(second.len())),
        ]),
        minimum(55),
        MinimumDistinctTokens::new(10),
        Some(config),
    );

    let mut shifted_second = vec!["const $pad1 = \"1\" ;", "const $pad2 = \"2\" ;"];
    shifted_second.extend_from_slice(&second);
    let shifted_fn = vec![Function {
        line: 3,
        end_line: u32::try_from(shifted_second.len()).expect("few lines"),
        decision_points: 2,
        source_lines: u32::try_from(second.len()).expect("few lines"),
        name: None,
        cognitive_complexity: 0,
        coverage: None,
    }];
    let shifted_run = Duplication::measure_with_near_clones(
        &model(vec![
            file_with_functions("a.ts", &base, whole_function(base.len())),
            file_with_functions("b.ts", &shifted_second, shifted_fn),
            file_with_functions("c.ts", &third, whole_function(third.len())),
        ]),
        minimum(55),
        MinimumDistinctTokens::new(10),
        Some(config),
    );

    assert_eq!(first_run.clone_families.len(), 1);
    assert_eq!(shifted_run.clone_families.len(), 1);
    assert_eq!(
        first_run.clone_families[0].id,
        shifted_run.clone_families[0].id
    );
    assert_eq!(first_run.clone_groups[0].id, shifted_run.clone_groups[0].id);
    assert_eq!(
        first_run.clone_families[0].fragments,
        vec![
            fragment("a.ts", 1, 13),
            fragment("b.ts", 1, 14),
            fragment("c.ts", 1, 14),
        ]
    );
    assert_eq!(first_run.clone_families[0].duplicated_lines, 41);
}

#[test]
fn bounded_near_clone_detection_scales_across_hundreds_of_distinct_functions() {
    let mut files = Vec::new();
    for idx in 0..240usize {
        let ops = [
            format!("op_{idx}_a"),
            format!("op_{idx}_b"),
            format!("op_{idx}_c"),
            format!("op_{idx}_d"),
            format!("op_{idx}_e"),
            format!("op_{idx}_f"),
        ];
        let lines: Vec<String> = (0..12)
            .map(|row| {
                format!(
                    "{} $v {} \"0\" {} k_{idx}_{row} ;",
                    ops[row % 6],
                    ops[(row + 1) % 6],
                    ops[(row + 2) % 6],
                )
            })
            .collect();
        let refs: Vec<&str> = lines.iter().map(String::as_str).collect();
        files.push(file_with_functions(
            &format!("src/f{idx:03}.ts"),
            &refs,
            whole_function(refs.len()),
        ));
    }
    let planted_a = near_clone_lines();
    let mut planted_b = planted_a.clone();
    planted_b.insert(5, "$log ( $x ) ;");
    files.push(file_with_functions(
        "src/planted_a.ts",
        &planted_a,
        whole_function(planted_a.len()),
    ));
    files.push(file_with_functions(
        "src/planted_b.ts",
        &planted_b,
        whole_function(planted_b.len()),
    ));
    let config = NearCloneConfig::new(0.85).expect("valid threshold");

    let duplication = Duplication::measure_with_near_clones(
        &model(files),
        minimum(55),
        MinimumDistinctTokens::new(10),
        Some(config),
    );

    assert_eq!(duplication.clone_groups.len(), 1);
    assert_eq!(
        duplication.clone_groups[0].fragments,
        vec![
            fragment("src/planted_a.ts", 1, 13),
            fragment("src/planted_b.ts", 1, 14),
        ]
    );
}

#[test]
fn near_clone_subsumption_scales_across_many_distinct_file_pairs() {
    let mut files = Vec::new();
    for idx in 0..120usize {
        let mut base = Vec::new();
        for row in 0..12usize {
            base.push(format!(
                "kw_{idx}_{} $v op_{idx}_{} \"1\" ;",
                row % 5,
                (row + 1) % 5,
            ));
        }
        let mut edited = base.clone();
        edited.insert(6, format!("extra_{idx} ( $v ) ;"));
        let base_refs: Vec<&str> = base.iter().map(String::as_str).collect();
        let edited_refs: Vec<&str> = edited.iter().map(String::as_str).collect();
        files.push(file_with_functions(
            &format!("src/pair_{idx:03}_a.ts"),
            &base_refs,
            whole_function(base_refs.len()),
        ));
        files.push(file_with_functions(
            &format!("src/pair_{idx:03}_b.ts"),
            &edited_refs,
            whole_function(edited_refs.len()),
        ));
    }
    let config = NearCloneConfig::new(0.80).expect("valid threshold");

    let duplication = Duplication::measure_with_near_clones(
        &model(files),
        minimum(55),
        MinimumDistinctTokens::new(8),
        Some(config),
    );

    assert_eq!(duplication.clone_groups.len(), 120);
    assert!(
        duplication
            .clone_groups
            .iter()
            .all(|group| group.similarity.is_some())
    );
}

#[test]
fn three_consecutive_identical_functions_in_one_file_remain_a_clone_family_with_bridging_spans() {
    let unit = [
        "fn $a ( $x ) {",
        "let $r = $x + \"1\" ;",
        "return $r ;",
        "}",
    ];
    let mut three_in_one = Vec::new();
    three_in_one.extend_from_slice(&unit);
    three_in_one.extend_from_slice(&unit);
    three_in_one.extend_from_slice(&unit);
    let mut two_in_other = Vec::new();
    two_in_other.extend_from_slice(&unit);
    two_in_other.extend_from_slice(&unit);

    let duplication = measure_files(&[("one.ts", &three_in_one), ("two.ts", &two_in_other)], 12);

    assert_eq!(duplication.clone_families.len(), 1);
    let family = &duplication.clone_families[0];
    assert_eq!(family.fragments.len(), 5);
    assert_eq!(family.duplicated_lines, 20);
    assert_eq!(duplication.clone_lines.count(), 20);
}

#[test]
fn exact_three_copy_family_with_enclosing_near_clone_remains_a_valid_family() {
    let unit = near_clone_lines();
    let mut alpha_lines = Vec::new();
    alpha_lines.extend_from_slice(&unit);
    alpha_lines.push("const $sep1 = \"10\" ;");
    alpha_lines.extend_from_slice(&unit);
    alpha_lines.push("const $sep2 = \"20\" ;");
    alpha_lines.extend_from_slice(&unit);
    let mut beta_lines = alpha_lines.clone();
    beta_lines.insert(5, "$audit ( $x ) ;");

    let duplication = measure_near_pair(&alpha_lines, &beta_lines, Some(0.85));

    assert_eq!(duplication.clone_families.len(), 1);
    let family = &duplication.clone_families[0];
    assert!(
        family.fragments.len() >= 3,
        "expected >= 3 fragments, got {:?}",
        family.fragments
    );
    assert_eq!(
        family.duplicated_lines,
        duplication.clone_lines.count(),
        "duplicated_lines must match unique line union without double counting"
    );
    assert!(family.duplicated_lines <= (alpha_lines.len() + beta_lines.len()) as u64);
}

#[test]
fn prepending_top_level_statement_preserves_downstream_near_clone_id_in_large_gap() {
    let prefix: Vec<String> = (0..45)
        .map(|i| format!("let $p = p_{i} + q_{i} * \"3\" ;"))
        .collect();
    let shared_block: Vec<String> = (0..35)
        .map(|i| format!("let $s = s_{i} - t_{i} + \"7\" ;"))
        .collect();
    let suffix: Vec<String> = (0..25)
        .map(|i| format!("let $z = z_{i} / w_{i} + \"9\" ;"))
        .collect();

    let mut left_lines = prefix.clone();
    left_lines.extend(shared_block.clone());
    left_lines.extend(suffix.clone());

    let mut right_lines: Vec<String> = (0..45)
        .map(|i| format!("const $r = r_{i} ^ u_{i} ;"))
        .collect();
    let mut edited_block = shared_block;
    edited_block.insert(17, "$trace_gap ( $s ) ;".to_owned());
    right_lines.extend(edited_block);
    right_lines.extend((0..25).map(|i| format!("const $e = e_{i} | f_{i} ;")));

    let mut shifted_left = vec!["const $added_header = header_tag + \"42\" ;".to_owned()];
    shifted_left.extend(left_lines.clone());

    let left_refs: Vec<&str> = left_lines.iter().map(String::as_str).collect();
    let right_refs: Vec<&str> = right_lines.iter().map(String::as_str).collect();
    let shifted_refs: Vec<&str> = shifted_left.iter().map(String::as_str).collect();
    let config = NearCloneConfig::new(0.80).expect("valid threshold");

    let before = Duplication::measure_with_near_clones(
        &model(vec![file("a.ts", &left_refs), file("b.ts", &right_refs)]),
        minimum(55),
        MinimumDistinctTokens::new(10),
        Some(config),
    );
    let after = Duplication::measure_with_near_clones(
        &model(vec![file("a.ts", &shifted_refs), file("b.ts", &right_refs)]),
        minimum(55),
        MinimumDistinctTokens::new(10),
        Some(config),
    );

    assert!(
        !before.clone_groups.is_empty(),
        "expected near-clone in large top-level gap"
    );
    let before_ids: Vec<_> = before.clone_groups.iter().map(|g| g.id).collect();
    let after_ids: Vec<_> = after.clone_groups.iter().map(|g| g.id).collect();
    assert_eq!(before_ids, after_ids);
}

#[test]
fn parallel_duplication_and_compact_clone_lines_are_deterministic_across_repeated_runs() {
    let base = near_clone_lines();
    let mut edited = base.clone();
    edited.insert(5, "$audit ( $x ) ;");
    let mut files = Vec::new();
    for idx in 0..8usize {
        files.push(file_with_functions(
            &format!("src/exact_{idx:02}.ts"),
            &base,
            whole_function(base.len()),
        ));
        files.push(file_with_functions(
            &format!("src/near_{idx:02}.ts"),
            &edited,
            whole_function(edited.len()),
        ));
    }
    let test_model = model(files);
    let config = NearCloneConfig::new(0.85).expect("valid threshold");

    let baseline = Duplication::measure_with_near_clones(
        &test_model,
        minimum(55),
        MinimumDistinctTokens::new(10),
        Some(config),
    );
    assert!(!baseline.clone_groups.is_empty());
    assert!(!baseline.clone_families.is_empty());
    assert!(
        baseline
            .clone_lines
            .contains(&RepoPath::new("src/exact_00.ts").expect("path"), 1)
    );
    assert!(
        !baseline
            .clone_lines
            .contains(&RepoPath::new("src/exact_00.ts").expect("path"), 999)
    );

    for _ in 0..8 {
        let repeated = Duplication::measure_with_near_clones(
            &test_model,
            minimum(55),
            MinimumDistinctTokens::new(10),
            Some(config),
        );
        assert_eq!(repeated, baseline);
    }
}

fn chain_variants(length: usize, edits: &[&[(usize, &str)]]) -> Vec<Vec<String>> {
    let base: Vec<String> = (0..length)
        .map(|idx| format!("let $v = k_{idx} + p_{idx} ;"))
        .collect();
    let mut variants = vec![base];
    for step in edits {
        let mut next = variants.last().expect("at least base").clone();
        for &(slot, replacement) in *step {
            next[slot] = replacement.to_owned();
        }
        variants.push(next);
    }
    variants
}

fn measure_identical_copies_with_two_variants(copy_count: usize) -> Duplication {
    let base: Vec<String> = (0..20usize)
        .map(|idx| format!("let $v = copy_{idx} + factor_{idx} ;"))
        .collect();
    let mut variant_zero = base.clone();
    variant_zero[3] = "let $v = variant_zero_3 - offset_3 ;".to_owned();
    let mut variant_one = base.clone();
    variant_one[14] = "let $v = variant_one_14 - offset_14 ;".to_owned();

    let mut files = Vec::new();
    let base_refs: Vec<&str> = base.iter().map(String::as_str).collect();
    for idx in 0..copy_count {
        files.push(file_with_functions(
            &format!("src/c{idx:02}.ts"),
            &base_refs,
            whole_function(base_refs.len()),
        ));
    }
    for (name, lines) in [
        ("src/zz_v0.ts", &variant_zero),
        ("src/zz_v1.ts", &variant_one),
    ] {
        let refs: Vec<&str> = lines.iter().map(String::as_str).collect();
        files.push(file_with_functions(name, &refs, whole_function(refs.len())));
    }
    Duplication::measure_with_near_clones(
        &model(files),
        minimum(55),
        MinimumDistinctTokens::new(10),
        NearCloneConfig::new(0.80),
    )
}

fn measure_near_clone_files(entries: &[(&str, &Vec<String>)], min_similarity: f64) -> Duplication {
    let files: Vec<SourceFile> = entries
        .iter()
        .map(|&(path, owned)| {
            let refs: Vec<&str> = owned.iter().map(String::as_str).collect();
            file_with_functions(path, &refs, whole_function(refs.len()))
        })
        .collect();
    Duplication::measure_with_near_clones(
        &model(files),
        minimum(55),
        MinimumDistinctTokens::new(10),
        NearCloneConfig::new(min_similarity),
    )
}

#[test]
fn near_clone_anchor_clustering_does_not_chain_dissimilar_endpoints() {
    let variants = chain_variants(
        20,
        &[
            &[
                (2, "const $x = b_2 - q_2 ;"),
                (6, "const $y = b_6 - q_6 ;"),
                (12, "const $z = b_12 - q_12 ;"),
            ],
            &[
                (4, "var $p = c_4 ^ r_4 ;"),
                (10, "var $q = c_10 ^ r_10 ;"),
                (16, "var $r = c_16 ^ r_16 ;"),
            ],
        ],
    );
    let duplication = measure_near_clone_files(
        &[
            ("src/a.ts", &variants[0]),
            ("src/b.ts", &variants[1]),
            ("src/c.ts", &variants[2]),
        ],
        0.70,
    );

    let near_groups: Vec<_> = duplication
        .clone_groups
        .iter()
        .filter(|group| group.similarity.is_some())
        .collect();
    assert_eq!(near_groups.len(), 2, "got: {near_groups:?}");
    for group in near_groups {
        let paths: Vec<&str> = group.fragments.iter().map(|f| f.path.as_str()).collect();
        assert!(
            !(paths.contains(&"src/a.ts") && paths.contains(&"src/c.ts")),
            "dissimilar endpoints must not chain: {paths:?}"
        );
    }
}

#[test]
fn oversized_lsh_bucket_with_many_identical_copies_still_detects_near_clone_variants() {
    let duplication = measure_identical_copies_with_two_variants(40);
    let near_groups: Vec<_> = duplication
        .clone_groups
        .iter()
        .filter(|group| group.similarity.is_some())
        .collect();
    assert!(
        !near_groups.is_empty(),
        "oversized bucket of 40 identical copies must not starve near-clone variants"
    );
    let paths: Vec<&str> = near_groups
        .iter()
        .flat_map(|group| {
            group
                .fragments
                .iter()
                .map(|fragment| fragment.path.as_str())
        })
        .collect();
    assert!(paths.contains(&"src/zz_v0.ts"), "missing zz_v0: {paths:?}");
    assert!(paths.contains(&"src/zz_v1.ts"), "missing zz_v1: {paths:?}");
}

#[test]
fn identical_copies_with_near_clone_variants_emit_single_near_clone_group() {
    let duplication = measure_identical_copies_with_two_variants(20);
    let near_groups: Vec<_> = duplication
        .clone_groups
        .iter()
        .filter(|group| group.similarity.is_some())
        .collect();
    assert_eq!(
        near_groups.len(),
        1,
        "expected 1 near-clone group instead of one per identical copy, got {}",
        near_groups.len()
    );
    assert_eq!(near_groups[0].fragments.len(), 22);
}

#[test]
fn near_clone_chain_clustering_is_rename_invariant_and_keeps_every_pair_above_min_similarity() {
    let variants = chain_variants(
        24,
        &[
            &[
                (1, "const $x = b_1 - q_1 ;"),
                (5, "const $y = b_5 - q_5 ;"),
                (9, "const $z = b_9 - q_9 ;"),
            ],
            &[
                (3, "var $p = c_3 ^ r_3 ;"),
                (11, "var $q = c_11 ^ r_11 ;"),
                (17, "var $r = c_17 ^ r_17 ;"),
            ],
            &[
                (7, "let $m = d_7 * s_7 ;"),
                (15, "let $n = d_15 * s_15 ;"),
                (21, "let $o = d_21 * s_21 ;"),
            ],
        ],
    );

    let measure_labeled = |entries: [(&str, &str, &Vec<String>); 4]| -> Vec<Vec<String>> {
        let path_to_label: std::collections::BTreeMap<&str, &str> = entries
            .iter()
            .map(|&(path, label, _)| (path, label))
            .collect();
        let file_entries: Vec<(&str, &Vec<String>)> = entries
            .into_iter()
            .map(|(path, _, owned)| (path, owned))
            .collect();
        let duplication = measure_near_clone_files(&file_entries, 0.70);
        let mut clusters: Vec<Vec<String>> = duplication
            .clone_groups
            .iter()
            .filter(|group| group.similarity.is_some())
            .map(|group| {
                let mut labels: Vec<String> = group
                    .fragments
                    .iter()
                    .map(|fragment| path_to_label[fragment.path.as_str()].to_owned())
                    .collect();
                labels.sort();
                labels
            })
            .collect();
        clusters.sort();
        clusters
    };

    let canonical = measure_labeled([
        ("src/a.ts", "A", &variants[0]),
        ("src/b.ts", "B", &variants[1]),
        ("src/c.ts", "C", &variants[2]),
        ("src/d.ts", "D", &variants[3]),
    ]);
    let renamed = measure_labeled([
        ("src/b.ts", "A", &variants[0]),
        ("src/a.ts", "B", &variants[1]),
        ("src/c.ts", "C", &variants[2]),
        ("src/d.ts", "D", &variants[3]),
    ]);

    assert_eq!(canonical, renamed);
    assert!(!canonical.is_empty());
    let disallowed_pairs = [("A", "C"), ("A", "D"), ("B", "D")];
    for cluster in &canonical {
        for &(left, right) in &disallowed_pairs {
            assert!(
                !(cluster.iter().any(|item| item == left)
                    && cluster.iter().any(|item| item == right)),
                "cluster {cluster:?} contains pair ({left}, {right}) below min_similarity"
            );
        }
    }
}
