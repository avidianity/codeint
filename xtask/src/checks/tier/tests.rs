use std::path::Path;

use super::{TierMap, oracle_reasons};
use crate::checks::glob_set;
use crate::config::load_settings;
use crate::git::{FileChange, LineDiff};

fn repository_settings() -> crate::config::Settings {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    match load_settings(&root) {
        Ok(settings) => settings,
        Err(error) => panic!("checks.toml must load: {error}"),
    }
}

fn tests_globs() -> globset::GlobSet {
    match glob_set(&repository_settings().behavior_tests) {
        Ok(set) => set,
        Err(error) => panic!("test globs must build: {error}"),
    }
}

fn change(status: char, path: &str) -> FileChange {
    FileChange {
        status,
        path: path.to_string(),
    }
}

#[test]
fn highest_matching_rule_wins_and_unlisted_paths_get_the_default() {
    let settings = repository_settings();
    let map = match TierMap::from_settings(&settings) {
        Ok(map) => map,
        Err(error) => panic!("tier map must build: {error}"),
    };
    assert_eq!(map.classify("README.md").tier, 0);
    assert_eq!(map.classify("docs/checks/no-comments.md").tier, 3);
    assert_eq!(map.classify(".github/workflows/ci.yml").tier, 3);
    assert_eq!(map.classify("crates/metrics/src/erosion.rs").tier, 2);
}

#[test]
fn changed_expected_value_and_skipped_test_raise_the_tier() {
    let changed_value = LineDiff {
        added: vec![(4, "    assert_eq!(total(), 99);".to_string())],
        removed: vec!["    assert_eq!(total(), 100);".to_string()],
    };
    let skipped = LineDiff {
        added: vec![(1, "#[ignore]".to_string())],
        removed: Vec::new(),
    };
    let tests = tests_globs();
    let first = oracle_reasons(
        &change('M', "crates/a/src/b/tests.rs"),
        &changed_value,
        None,
        None,
        &tests,
    );
    let second = oracle_reasons(
        &change('M', "crates/a/tests/flow.rs"),
        &skipped,
        None,
        None,
        &tests,
    );
    assert_eq!(first, vec!["removes or changes an assertion".to_string()]);
    assert_eq!(second, vec!["skips a test".to_string()]);
}

#[test]
fn deleting_a_test_file_raises_the_tier() {
    let reasons = oracle_reasons(
        &change('D', "crates/a/tests/flow.rs"),
        &LineDiff::default(),
        None,
        None,
        &tests_globs(),
    );
    assert_eq!(reasons, vec!["deletes a test file".to_string()]);
}

#[test]
fn adding_a_suppression_raises_the_tier() {
    let text = "#[allow(dead_code)]\nfn a() {}\n";
    let diff = LineDiff {
        added: vec![(1, "#[allow(dead_code)]".to_string())],
        removed: Vec::new(),
    };
    let reasons = oracle_reasons(
        &change('M', "crates/a/src/lib.rs"),
        &diff,
        Some(text),
        Some("fn a() {}\n"),
        &tests_globs(),
    );
    assert_eq!(reasons, vec!["adds a lint or type suppression".to_string()]);
}

#[test]
fn new_crate_that_inherits_workspace_lints_is_not_a_lint_change() {
    let manifest = "[package]\nname = \"a\"\n\n[lints]\nworkspace = true\n";
    let diff = LineDiff::default();
    assert!(
        oracle_reasons(
            &change('A', "crates/a/Cargo.toml"),
            &diff,
            Some(manifest),
            None,
            &tests_globs()
        )
        .is_empty()
    );
    let weakened = "[package]\nname = \"a\"\n\n[lints.rust]\nunused = \"allow\"\n";
    assert_eq!(
        oracle_reasons(
            &change('M', "crates/a/Cargo.toml"),
            &diff,
            Some(weakened),
            Some(manifest),
            &tests_globs()
        ),
        vec!["changes lint configuration".to_string()]
    );
}

#[test]
fn ordinary_source_change_has_no_oracle_reason() {
    let diff = LineDiff {
        added: vec![(3, "    value + 1".to_string())],
        removed: vec!["    value".to_string()],
    };
    assert!(
        oracle_reasons(
            &change('M', "crates/a/src/lib.rs"),
            &diff,
            Some("fn a() {}\n"),
            None,
            &tests_globs()
        )
        .is_empty()
    );
}
