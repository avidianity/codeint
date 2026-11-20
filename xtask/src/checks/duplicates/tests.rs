use super::reportable_clones;
use crate::checks::glob_set;

fn report(pairs: &[(&str, &str)]) -> String {
    let duplicates: Vec<serde_json::Value> = pairs
        .iter()
        .map(|(first, second)| {
            serde_json::json!({
                "format": "typescript",
                "lines": 15,
                "tokens": 375,
                "firstFile": { "name": first, "start": 1, "end": 15 },
                "secondFile": { "name": second, "start": 3, "end": 17 }
            })
        })
        .collect();
    serde_json::json!({ "duplicates": duplicates, "statistics": {} }).to_string()
}

fn clones(pairs: &[(&str, &str)]) -> Vec<String> {
    let test_input = match glob_set(&["crates/*/tests/fixtures/**".to_string()]) {
        Ok(set) => set,
        Err(error) => panic!("{error}"),
    };
    match reportable_clones(&report(pairs), &test_input) {
        Ok(found) => found,
        Err(error) => panic!("{error}"),
    }
}

#[test]
fn a_clone_between_two_test_input_files_is_not_reported() {
    assert!(
        clones(&[(
            "crates/lang-ts/tests/fixtures/one.ts",
            "crates/lang-ts/tests/fixtures/deep/two.ts"
        )])
        .is_empty()
    );
}

#[test]
fn a_clone_with_one_side_outside_test_input_is_reported_with_both_locations() {
    assert_eq!(
        clones(&[(
            "crates/lang-ts/src/two.ts",
            "crates/lang-ts/tests/fixtures/one.ts"
        )]),
        vec![
            "crates/lang-ts/src/two.ts:1-15 duplicates crates/lang-ts/tests/fixtures/one.ts:3-17. Extract the shared code into one place; see docs/checks/no-duplicate-code.md."
                .to_string()
        ]
    );
}

#[test]
fn fixtures_folders_outside_a_crate_tests_folder_are_not_test_input() {
    for (first, second) in [
        (
            "crates/a/src/tests/fixtures/one.ts",
            "crates/a/src/tests/fixtures/two.ts",
        ),
        (
            "crates/a/b/tests/fixtures/one.ts",
            "crates/a/b/tests/fixtures/two.ts",
        ),
        (
            "x/crates/a/tests/fixtures/one.ts",
            "x/crates/a/tests/fixtures/two.ts",
        ),
        ("crates/a/src/one.ts", "crates/a/src/two.ts"),
    ] {
        assert_eq!(clones(&[(first, second)]).len(), 1, "{first} and {second}");
    }
}

#[test]
fn a_report_without_a_duplicates_list_is_an_error() {
    let test_input = match glob_set(&[]) {
        Ok(set) => set,
        Err(error) => panic!("{error}"),
    };
    assert!(reportable_clones("{\"statistics\":{}}", &test_input).is_err());
    assert!(reportable_clones("not json", &test_input).is_err());
}
