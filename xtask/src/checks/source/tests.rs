use super::{comment_lines_added, config_reads_in, suppressions_in};
use crate::lexer::Language;

#[test]
fn added_comment_that_says_what_the_next_line_does_fails() {
    let text = "fn total() -> u32 {\n    // add the two numbers\n    1 + 2\n}\n";
    assert_eq!(comment_lines_added(Language::Rust, text, &[2, 3]), vec![2]);
}

#[test]
fn commented_out_code_and_todo_markers_fail() {
    let text = "fn a() {}\n// fn b() {}\n/* TODO: later */\n";
    assert_eq!(
        comment_lines_added(Language::Rust, text, &[2, 3]),
        vec![2, 3]
    );
}

#[test]
fn renamed_function_without_comment_passes() {
    let text = "fn sum_of_two_and_one() -> u32 {\n    1 + 2\n}\n";
    assert!(comment_lines_added(Language::Rust, text, &[1, 2, 3]).is_empty());
}

#[test]
fn existing_comment_on_an_unchanged_line_passes() {
    let text = "// old comment\nfn a() {}\n";
    assert!(comment_lines_added(Language::Rust, text, &[2]).is_empty());
}

#[test]
fn shebang_and_wit_doc_comments_are_allowed() {
    assert!(
        comment_lines_added(Language::Shell, "#!/usr/bin/env bash\necho hi\n", &[1, 2]).is_empty()
    );
    assert!(
        comment_lines_added(Language::Wit, "/// A file.\nrecord file {}\n", &[1, 2]).is_empty()
    );
    assert_eq!(
        comment_lines_added(Language::Wit, "// plain\nrecord file {}\n", &[1]),
        vec![1]
    );
}

#[test]
fn rust_doc_comments_are_not_allowed_because_no_crate_is_published() {
    assert_eq!(
        comment_lines_added(Language::Rust, "/// docs\nfn a() {}\n", &[1]),
        vec![1]
    );
}

#[test]
fn rust_allow_attribute_is_a_suppression_but_forbid_is_not() {
    let text =
        "#![forbid(unsafe_code)]\n#[allow(dead_code)]\nfn a() {}\nlet s = \"#[allow(x)]\";\n";
    assert_eq!(
        suppressions_in("src/a.rs", text),
        vec![(2, "allow(dead_code)".to_string())]
    );
}

#[test]
fn script_and_markdown_suppressions_are_found() {
    assert_eq!(
        suppressions_in("npm/a.mjs", "// @ts-ignore\nx();\n").len(),
        1
    );
    assert_eq!(
        suppressions_in("docs/a.md", "<!-- markdownlint-disable MD013 -->\n").len(),
        1
    );
    assert!(suppressions_in("docs/a.md", "Mention `eslint-disable` in prose.\n").is_empty());
}

#[test]
fn reading_an_environment_variable_outside_config_fails() {
    let text = "fn handler() {\n    let token = std::env::var(\"TOKEN\");\n}\n";
    assert_eq!(config_reads_in(text), vec![2]);
}

#[test]
fn receiving_a_typed_value_passes() {
    let text =
        "fn handler(settings: &Settings) -> u32 {\n    settings.limit\n}\nlet s = \"env::var\";\n";
    assert!(config_reads_in(text).is_empty());
}
