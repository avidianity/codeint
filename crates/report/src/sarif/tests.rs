use codeint_model::SourceSet;

use super::{ListedFindings, SarifLog, relative_uri};
use crate::{
    CloneGroupFinding, CloneLocation, CognitiveComplexityFinding, Decimal, ErodedFunctionFinding,
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
        end_line: 23,
        name: Some("big".to_owned()),
        cyclomatic_complexity: 12,
        cognitive_complexity: 14,
        source_lines: 20,
        function_mass: Decimal(53.6656),
        owners: Vec::new(),
    }];
    let complex = [CognitiveComplexityFinding {
        id: "5555555555555555".to_owned(),
        path: "src/deep.ts".to_owned(),
        line: 5,
        end_line: 30,
        name: Some("deep".to_owned()),
        cognitive_complexity: 21,
        cyclomatic_complexity: 7,
        source_lines: 25,
        owners: Vec::new(),
    }];
    let clones = [CloneGroupFinding {
        id: "2222222222222222".to_owned(),
        tokens: 80,
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
        owners: Vec::new(),
    }];
    let flagged = [FlaggedLineRow {
        id: "3333333333333333".to_owned(),
        path: "src/sign.ts".to_owned(),
        line: 4,
        rule: "else-after-return".to_owned(),
        owners: Vec::new(),
    }];
    let cycles = [ImportCycleFinding {
        id: "4444444444444444".to_owned(),
        modules: vec!["src/x.ts".to_owned(), "src/y.ts".to_owned()],
        owners: Vec::new(),
    }];
    let listed = ListedFindings {
        eroded_functions: &eroded,
        cognitive_complexity: &complex,
        clone_groups: &clones,
        flagged_lines: &flagged,
        import_cycles: &cycles,
        boundaries: &[],
        external_metrics: Vec::new(),
    };

    let log = serde_json::to_value(SarifLog::listing(
        std::iter::once((SourceSet::Production, listed)),
        super::ListedDependencies::EMPTY,
    ))
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
            ("cognitive_complexity", "5555555555555555"),
            ("duplication", "2222222222222222"),
            ("verbosity", "3333333333333333"),
            ("import_cycles", "4444444444444444"),
        ]
    );
}
