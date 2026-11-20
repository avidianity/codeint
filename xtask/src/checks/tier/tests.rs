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
fn tool_configuration_anywhere_in_the_tree_is_tier_three() {
    let map = match TierMap::from_settings(&repository_settings()) {
        Ok(map) => map,
        Err(error) => panic!("tier map must build: {error}"),
    };
    for path in [
        "clippy.toml",
        "crates/a/clippy.toml",
        "lefthook-local.yml",
        "docs/.markdownlint.json",
        ".gitattributes",
        "crates/a/.gitattributes",
        "rustfmt.toml",
        ".gitleaksignore",
    ] {
        assert_eq!(map.classify(path).tier, 3, "{path}");
    }
}

#[test]
fn changing_the_expected_value_inside_a_multi_line_assertion_raises_the_tier() {
    let base = "#[test]\nfn adds() {\n    assert_eq!(\n        total(),\n        3\n    );\n}\n";
    let new =
        "#[test]\nfn adds() {\n    assert_eq!(\n        total(),\n        total()\n    );\n}\n";
    let diff = LineDiff {
        added: vec![(5, "        total()".to_string())],
        removed: vec![(5, "        3".to_string())],
    };
    let reasons = oracle_reasons(
        &change('M', "crates/a/src/tests.rs"),
        &diff,
        Some(new),
        Some(base),
        &tests_globs(),
    );
    assert_eq!(reasons, vec!["removes or changes an assertion".to_string()]);
}

#[test]
fn a_line_after_a_finished_assertion_is_not_part_of_it() {
    let base = "fn adds() {\n    assert!(ready());\n    let total = 3;\n}\n";
    let diff = LineDiff {
        added: vec![(3, "    let total = 4;".to_string())],
        removed: vec![(3, "    let total = 3;".to_string())],
    };
    let reasons = oracle_reasons(
        &change('M', "crates/a/src/tests.rs"),
        &diff,
        Some("fn adds() {\n    assert!(ready());\n    let total = 4;\n}\n"),
        Some(base),
        &tests_globs(),
    );
    assert!(reasons.is_empty(), "{reasons:?}");
}

#[test]
fn skipping_a_test_without_the_ignore_attribute_raises_the_tier() {
    let base = "#[test]\nfn adds() {}\n";
    let conditional = LineDiff {
        added: vec![(1, "#[cfg_attr(all(), ignore)]".to_string())],
        removed: Vec::new(),
    };
    assert_eq!(
        oracle_reasons(
            &change('M', "crates/a/tests/flow.rs"),
            &conditional,
            Some("#[cfg_attr(all(), ignore)]\n#[test]\nfn adds() {}\n"),
            Some(base),
            &tests_globs(),
        ),
        vec!["skips a test".to_string()]
    );
    let removed_attribute = LineDiff {
        added: Vec::new(),
        removed: vec![(1, "#[test]".to_string())],
    };
    assert_eq!(
        oracle_reasons(
            &change('M', "crates/a/tests/flow.rs"),
            &removed_attribute,
            Some("fn adds() {}\n"),
            Some(base),
            &tests_globs(),
        ),
        vec!["removes a test attribute".to_string()]
    );
}

#[test]
fn turning_off_a_test_module_raises_the_tier() {
    let diff = LineDiff {
        added: vec![(3, "#[cfg(any())]".to_string())],
        removed: vec![(3, "#[cfg(test)]".to_string())],
    };
    assert_eq!(
        oracle_reasons(
            &change('M', "crates/a/src/lib.rs"),
            &diff,
            Some("pub fn a() {}\n\n#[cfg(any())]\nmod tests;\n"),
            Some("pub fn a() {}\n\n#[cfg(test)]\nmod tests;\n"),
            &tests_globs(),
        ),
        vec!["turns off test code".to_string()]
    );
}

#[test]
fn skipping_a_script_test_raises_the_tier() {
    let diff = LineDiff {
        added: vec![(1, "it.skip(\"installs\", () => {});".to_string())],
        removed: Vec::new(),
    };
    assert_eq!(
        oracle_reasons(
            &change('M', "npm/install.test.mjs"),
            &diff,
            None,
            None,
            &tests_globs(),
        ),
        vec!["skips a test".to_string()]
    );
}

#[test]
fn changed_expected_value_and_skipped_test_raise_the_tier() {
    let changed_value = LineDiff {
        added: vec![(4, "    assert_eq!(total(), 99);".to_string())],
        removed: vec![(4, "    assert_eq!(total(), 100);".to_string())],
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
        removed: vec![(3, "    value".to_string())],
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
