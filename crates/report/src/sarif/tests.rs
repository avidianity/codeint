use codeint_model::SourceSet;

use super::{ListedFindings, SarifLog, relative_uri};
use crate::{
    CloneFamilyFinding, CloneGroupFinding, CloneLocation, Decimal, ErodedFunctionFinding,
    FlaggedLineRow, ImportCycleFinding,
};

#[test]
fn a_path_of_letters_digits_dots_dashes_and_slashes_is_its_own_uri() {
    assert_eq!(
        relative_uri("packages/web-app/src/v2/index.test.ts"),
        "packages/web-app/src/v2/index.test.ts"
    );
}

#[test]
fn a_space_a_percent_sign_and_a_hash_are_percent_encoded() {
    assert_eq!(relative_uri("src/my file%#1.ts"), "src/my%20file%25%231.ts");
}

#[test]
fn a_colon_is_percent_encoded_so_the_first_segment_never_reads_as_a_uri_scheme() {
    assert_eq!(relative_uri("c:/notes.ts"), "c%3A/notes.ts");
}

#[test]
fn each_utf_8_byte_of_a_non_ascii_character_is_percent_encoded() {
    assert_eq!(relative_uri("src/café.ts"), "src/caf%C3%A9.ts");
}

#[test]
fn every_sarif_result_carries_its_finding_id_in_partial_fingerprints() {
    let eroded = [ErodedFunctionFinding {
        id: "1111111111111111".to_owned(),
        path: "src/big.ts".to_owned(),
        line: 3,
        cyclomatic_complexity: 12,
        source_lines: 20,
        function_mass: Decimal(53.6656),
    }];
    let clones = [CloneGroupFinding {
        id: "2222222222222222".to_owned(),
        tokens: 80,
        similarity: None,
        locations: vec![
            CloneLocation {
                path: "src/a.ts".to_owned(),
                start_line: 1,
                end_line: 10,
            },
            CloneLocation {
                path: "src/b.ts".to_owned(),
                start_line: 1,
                end_line: 10,
            },
        ],
    }];
    let flagged = [FlaggedLineRow {
        id: "3333333333333333".to_owned(),
        path: "src/sign.ts".to_owned(),
        line: 4,
        rule: "else-after-return".to_owned(),
    }];
    let cycles = [ImportCycleFinding {
        id: "4444444444444444".to_owned(),
        modules: vec!["src/x.ts".to_owned(), "src/y.ts".to_owned()],
    }];
    let listed = ListedFindings {
        eroded_functions: &eroded,
        clone_groups: &clones,
        clone_families: &[],
        flagged_lines: &flagged,
        import_cycles: &cycles,
        boundaries: &[],
        external_metrics: Vec::new(),
    };

    let log = serde_json::to_value(SarifLog::listing(std::iter::once((
        SourceSet::Production,
        listed,
    ))))
    .expect("SARIF serializes");

    let fingerprints: Vec<(&str, &str)> = log["runs"][0]["results"]
        .as_array()
        .expect("results array")
        .iter()
        .map(|result| {
            (
                result["ruleId"].as_str().expect("ruleId"),
                result["partialFingerprints"]["codeintFindingId"]
                    .as_str()
                    .expect("codeintFindingId"),
            )
        })
        .collect();
    assert_eq!(
        fingerprints,
        [
            ("erosion", "1111111111111111"),
            ("duplication", "2222222222222222"),
            ("verbosity", "3333333333333333"),
            ("import_cycles", "4444444444444444"),
        ]
    );
}

#[test]
fn clone_family_sarif_result_replaces_its_member_clone_groups_and_keeps_standalone_groups() {
    let clones = [
        CloneGroupFinding {
            id: "aaaa000000000001".to_owned(),
            tokens: 80,
            similarity: None,
            locations: vec![
                CloneLocation {
                    path: "src/a.ts".to_owned(),
                    start_line: 1,
                    end_line: 10,
                },
                CloneLocation {
                    path: "src/b.ts".to_owned(),
                    start_line: 1,
                    end_line: 10,
                },
                CloneLocation {
                    path: "src/c.ts".to_owned(),
                    start_line: 1,
                    end_line: 10,
                },
            ],
        },
        CloneGroupFinding {
            id: "bbbb000000000002".to_owned(),
            tokens: 75,
            similarity: Some(Decimal(0.92)),
            locations: vec![
                CloneLocation {
                    path: "src/x.ts".to_owned(),
                    start_line: 1,
                    end_line: 8,
                },
                CloneLocation {
                    path: "src/y.ts".to_owned(),
                    start_line: 1,
                    end_line: 8,
                },
            ],
        },
    ];
    let families = [CloneFamilyFinding {
        id: "ffff000000000009".to_owned(),
        duplicated_lines: 30,
        clone_group_ids: vec!["aaaa000000000001".to_owned()],
        locations: vec![
            CloneLocation {
                path: "src/a.ts".to_owned(),
                start_line: 1,
                end_line: 10,
            },
            CloneLocation {
                path: "src/b.ts".to_owned(),
                start_line: 1,
                end_line: 10,
            },
            CloneLocation {
                path: "src/c.ts".to_owned(),
                start_line: 1,
                end_line: 10,
            },
        ],
    }];
    let listed = ListedFindings {
        eroded_functions: &[],
        clone_groups: &clones,
        clone_families: &families,
        flagged_lines: &[],
        import_cycles: &[],
        boundaries: &[],
        external_metrics: Vec::new(),
    };

    let log = serde_json::to_value(SarifLog::listing(std::iter::once((
        SourceSet::Production,
        listed,
    ))))
    .expect("SARIF serializes");

    let results = log["runs"][0]["results"].as_array().expect("results");
    assert_eq!(results.len(), 2);
    assert_eq!(
        results[0]["partialFingerprints"]["codeintFindingId"],
        "ffff000000000009"
    );
    assert_eq!(
        results[0]["message"]["text"],
        "Clone family of 30 duplicated lines across 3 locations (1 Clone group); the other locations: src/b.ts lines 1-10, src/c.ts lines 1-10."
    );
    assert_eq!(
        results[0]["relatedLocations"].as_array().map(Vec::len),
        Some(2)
    );
    assert_eq!(
        results[1]["partialFingerprints"]["codeintFindingId"],
        "bbbb000000000002"
    );
    assert_eq!(
        results[1]["message"]["text"],
        "Near-clone group of 75 tokens (92% similar) in 2 fragments; the other fragments: src/y.ts lines 1-8."
    );
}
