use super::{comment_lines_added, config_reads_in, suppressions_in};
use crate::lexer::Language;
use std::path::Path;

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

#[test]
fn a_shebang_is_allowed_only_in_script_languages() {
    let rust = "#![forbid(unsafe_code)] // added comment\nfn a() {}\n";
    assert_eq!(comment_lines_added(Language::Rust, rust, &[1]), vec![1]);
    let yaml = "#! added comment in a YAML file\nname: ci\n";
    assert_eq!(comment_lines_added(Language::Yaml, yaml, &[1]), vec![1]);
    let script = "#!/usr/bin/env node\nconsole.log(1);\n";
    assert!(comment_lines_added(Language::Script, script, &[1]).is_empty());
}

#[test]
fn quotes_the_language_does_not_open_do_not_hide_a_comment() {
    let regex = "const q = /\"/;\n// added comment\n";
    assert_eq!(comment_lines_added(Language::Script, regex, &[2]), vec![2]);
    let yaml = "size: 12\" # added comment\n";
    assert_eq!(comment_lines_added(Language::Yaml, yaml, &[1]), vec![1]);
    let shell = "echo 5\\\"\n# added comment\n";
    assert_eq!(comment_lines_added(Language::Shell, shell, &[2]), vec![2]);
    let toml = "b = \"\"\"x\"\"\"\"\nc = 2 # added comment\n";
    assert_eq!(comment_lines_added(Language::Toml, toml, &[2]), vec![2]);
}

#[test]
fn powershell_block_comments_and_wit_mixed_comments_are_found() {
    let powershell = "Write-Host hi\n<#added comment#>\n";
    assert_eq!(
        comment_lines_added(Language::PowerShell, powershell, &[2]),
        vec![2]
    );
    let wit = "/** doc */ // added plain comment\ninterface a {}\n";
    assert_eq!(comment_lines_added(Language::Wit, wit, &[1]), vec![1]);
}

#[test]
fn every_markdownlint_suppression_form_is_found() {
    for line in [
        "<!--markdownlint-disable-->",
        "<!-- MARKDOWNLINT-DISABLE -->",
        "<!-- markdownlint-disable-next-line MD013 -->",
        "<!-- markdownlint-configure-file { \"default\": false } -->",
    ] {
        assert_eq!(suppressions_in("docs/a.md", line).len(), 1, "{line}");
    }
    assert!(suppressions_in("docs/a.md", "<!-- markdownlint is the linter -->").is_empty());
}

#[test]
fn a_file_that_is_not_utf8_is_still_scanned() {
    let folder = std::env::temp_dir().join(format!("xtask-latin1-{}", std::process::id()));
    std::fs::create_dir_all(&folder).expect("create the folder");
    let file = folder.join("install.sh");
    std::fs::write(
        &file,
        b"#!/bin/sh\n# shellcheck disable=SC2086\necho caf\xe9\n",
    )
    .expect("write");
    let text = super::read_text(Path::new(&file))
        .expect("read")
        .expect("text");
    assert_eq!(suppressions_in("install.sh", &text).len(), 1);
    std::fs::remove_dir_all(&folder).expect("clean up");
}

#[test]
fn an_aliased_environment_module_is_a_configuration_read() {
    let text = "use std::env as process_environment;\nfn a() -> bool {\n    process_environment::var(\"HOME\").is_ok()\n}\n";
    assert_eq!(config_reads_in(text), vec![1]);
    assert!(config_reads_in("fn a(my_env: u8) -> u8 {\n    my_env as u8\n}\n").is_empty());
}
