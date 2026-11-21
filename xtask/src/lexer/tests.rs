use super::{Language, scan};

#[test]
fn rust_line_comment_is_found_and_strings_are_not_comments() {
    let text = "let url = \"https://example.com\";\nlet x = 1; // explains x\n";
    let result = scan(Language::Rust, text);
    assert!(!result.is_comment(1));
    assert!(result.is_comment(2));
    assert_eq!(result.code_lines[1].trim_end(), "let x = 1;");
}

#[test]
fn rust_raw_strings_and_char_literals_hide_slashes() {
    let text = "let a = r#\"// not a comment\"#;\nlet b = '/';\nfn f<'a>(x: &'a str) {}\n";
    let result = scan(Language::Rust, text);
    assert!(result.comment_lines.is_empty());
    assert!(result.code_lines[2].contains("fn f<'a>(x: &'a str)"));
}

#[test]
fn rust_doc_comments_are_marked_as_doc() {
    let text = "/// documents f\nfn f() {}\n//// plain\n/* block\n   spans */\n";
    let result = scan(Language::Rust, text);
    assert!(result.is_doc_comment(1));
    assert!(result.is_comment(3) && !result.is_doc_comment(3));
    assert!(result.is_comment(4) && result.is_comment(5));
}

#[test]
fn rust_nested_block_comments_close_at_the_outer_end() {
    let text = "/* a /* b */ c */ let x = 1;\n";
    let result = scan(Language::Rust, text);
    assert!(result.is_comment(1));
    assert!(result.code_lines[0].contains("let x = 1;"));
}

#[test]
fn wit_triple_slash_is_a_doc_comment() {
    let result = scan(
        Language::Wit,
        "/// The Code model.\nrecord file {}\n// plain\n",
    );
    assert!(result.is_doc_comment(1));
    assert!(result.is_comment(3) && !result.is_doc_comment(3));
}

#[test]
fn script_template_literals_hide_slashes_and_shebang_is_a_comment() {
    let result = scan(
        Language::Script,
        "#!/usr/bin/env node\nconst u = `http://x/${y}`;\n",
    );
    assert!(result.is_comment(1));
    assert!(!result.is_comment(2));
}

#[test]
fn toml_hash_inside_strings_is_not_a_comment() {
    let result = scan(
        Language::Toml,
        "color = \"#ffffff\"\nname = 'a#b' # trailing\n",
    );
    assert!(!result.is_comment(1));
    assert!(result.is_comment(2));
}

#[test]
fn yaml_hash_needs_whitespace_before_it() {
    let result = scan(
        Language::Yaml,
        "url: https://x.dev/#top\nkey: value # note\n# whole line\n",
    );
    assert!(!result.is_comment(1));
    assert!(result.is_comment(2));
    assert!(result.is_comment(3));
}

#[test]
fn shell_hash_in_parameter_expansion_is_not_a_comment() {
    let result = scan(
        Language::Shell,
        "echo \"${#items[@]}\" $#\necho done # note\n",
    );
    assert!(!result.is_comment(1));
    assert!(result.is_comment(2));
}

#[test]
fn jsx_and_tsx_files_are_scripts() {
    assert_eq!(Language::of_path("npm/a.tsx"), Some(Language::Script));
    assert_eq!(Language::of_path("npm/a.jsx"), Some(Language::Script));
    assert_eq!(Language::of_path("install.ps1"), Some(Language::PowerShell));
}

#[test]
fn a_yaml_quote_inside_a_double_quoted_scalar_spans_lines() {
    let text = "key: \"first\n  second # not a comment\"\nnext: 1 # comment\n";
    let result = scan(Language::Yaml, text);
    assert!(!result.is_comment(2));
    assert!(result.is_comment(3));
}

#[test]
fn a_hash_inside_a_yaml_block_scalar_is_text_unless_it_starts_the_line() {
    let text = "steps:\n  - run: |\n      echo \"Pull request #1 moved\"\n      # explains the next line\n      exit 0\n  - name: next # comment\n";
    let result = scan(Language::Yaml, text);
    assert!(!result.is_comment(3));
    assert!(result.is_comment(4));
    assert!(!result.is_comment(5));
    assert!(result.is_comment(6));
}

#[test]
fn script_regex_literals_hide_slashes_and_block_comment_markers() {
    let text = "\
const url = /https?:\\/\\//;
return /a\\/\\*b/;
const cls = /[//]/;
const blockCls = /[/*]/;
const escapedBracket = /[\\]]\\/\\//;
const arrow = (s) => /https?:\\/\\//.test(s);
/https?:\\/\\//.test(url);
const trailing = /a\\/\\/b/g; // real comment
";
    let result = scan(Language::Script, text);
    assert!(!result.is_comment(1));
    assert!(!result.is_comment(2));
    assert!(!result.is_comment(3));
    assert!(!result.is_comment(4));
    assert!(!result.is_comment(5));
    assert!(!result.is_comment(6));
    assert!(!result.is_comment(7));
    assert!(result.is_comment(8));
}

#[test]
fn script_division_and_jsx_closing_tags_still_detect_real_comments() {
    let text = "\
const x = a / b // comment after division
const y = (a + b) / c /* block after division */
const z = i++ / 2 // comment after postfix
const w = obj.return / 2 // comment after property
const el = <div>/text</div> // comment after jsx
const multi = a
  / b // comment after multiline division
";
    let result = scan(Language::Script, text);
    assert!(result.is_comment(1));
    assert!(result.is_comment(2));
    assert!(result.is_comment(3));
    assert!(result.is_comment(4));
    assert!(result.is_comment(5));
    assert!(!result.is_comment(6));
    assert!(result.is_comment(7));
}
