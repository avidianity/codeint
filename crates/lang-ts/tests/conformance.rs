use codeint_lang_ts::TypeScriptAndJavaScript;
use codeint_model::{FileModel, LanguagePlugin, LowerError, RepoPath};

fn plugin() -> &'static dyn LanguagePlugin {
    &TypeScriptAndJavaScript
}

fn lower(path: &str, text: &str) -> Result<FileModel, LowerError> {
    plugin().lower(&RepoPath::new(path).expect("a valid path"), text)
}

fn source_lines(path: &str, text: &str) -> u32 {
    match lower(path, text) {
        Ok(model) => model.source_lines,
        Err(error) => panic!("{path} did not lower: {error}"),
    }
}

#[test]
fn the_plugin_declares_every_typescript_and_javascript_extension_without_a_leading_dot() {
    let mut extensions = plugin().extensions();
    extensions.sort();

    assert_eq!(
        extensions,
        ["cjs", "cts", "js", "jsx", "mjs", "mts", "ts", "tsx"]
    );
}

#[test]
fn a_file_of_each_declared_extension_lowers_to_its_source_lines() {
    let cases = [
        (
            "src/greet.ts",
            "export function greet(name: string): string {\n  return `Hello, ${name}`;\n}\n",
            3,
        ),
        (
            "src/badge.tsx",
            "export const Badge = (props: { label: string }) => (\n  <span>{props.label}</span>\n);\n",
            3,
        ),
        ("src/module.mts", "export const answer: number = 42;\n", 1),
        (
            "src/legacy.cts",
            "const value: number = 1;\nexport = value;\n",
            2,
        ),
        (
            "src/sum.js",
            "export function sum(left, right) {\n  return left + right;\n}\n",
            3,
        ),
        (
            "src/item.jsx",
            "export const Item = ({ name }) => <li>{name}</li>;\n",
            1,
        ),
        (
            "src/esm.mjs",
            "import { sum } from './sum.js';\nexport const two = sum(1, 1);\n",
            2,
        ),
        (
            "src/common.cjs",
            "const path = require('node:path');\nmodule.exports = { base: path.basename };\n",
            2,
        ),
    ];

    for (path, text, expected) in cases {
        assert_eq!(source_lines(path, text), expected, "{path}");
    }
}

#[test]
fn blank_lines_and_lines_with_only_comments_are_not_source_lines() {
    let text = concat!(
        "// a line comment\n",
        "\n",
        "/**\n",
        " * A block comment\n",
        " * over several lines.\n",
        " */\n",
        "export const total = 1 + 2; // a trailing comment\n",
        "   \t \n",
        "/* a */ /* b */\n",
        "export const quotient = 6 /* inline */ / 3;\n",
    );

    assert_eq!(source_lines("src/commented.ts", text), 2);
}

#[test]
fn comment_markers_inside_strings_and_templates_are_code() {
    let text = concat!(
        "export const url = 'https://example.invalid';\n",
        "export const text = `first\n",
        "/* inside a template */ last`;\n",
    );

    assert_eq!(source_lines("src/strings.ts", text), 3);
}

#[test]
fn crlf_line_endings_count_exactly_like_lf() {
    let lf = "const first = 1;\n\nconst second = 2;\n";
    let crlf = "const first = 1;\r\n\r\nconst second = 2;\r\n";

    assert_eq!(source_lines("src/lf.ts", lf), 2);
    assert_eq!(source_lines("src/crlf.ts", crlf), 2);
}

#[test]
fn a_byte_order_mark_is_not_code() {
    assert_eq!(
        source_lines("src/marked.ts", "\u{feff}\nconst one = 1;\n"),
        1
    );
}

#[test]
fn an_empty_file_has_no_source_lines() {
    assert_eq!(source_lines("src/empty.ts", ""), 0);
}

#[test]
fn a_file_that_does_not_parse_is_unparsable_with_the_parser_message() {
    let result = lower("src/broken.ts", "export const = ;\n");

    assert!(
        matches!(&result, Err(LowerError::Unparsable { message }) if !message.is_empty()),
        "unexpected result: {result:?}"
    );
}

#[test]
fn an_export_in_a_commonjs_script_is_unparsable() {
    let result = lower("src/script.cjs", "export const one = 1;\n");

    assert!(
        matches!(result, Err(LowerError::Unparsable { .. })),
        "unexpected result: {result:?}"
    );
}

#[test]
fn a_file_with_an_undeclared_extension_is_unsupported() {
    let result = lower("scripts/tool.py", "print('codeint')\n");

    assert!(
        matches!(result, Err(LowerError::UnsupportedFile { .. })),
        "unexpected result: {result:?}"
    );
}

fn tokens(path: &str, text: &str) -> Vec<(String, u32)> {
    match lower(path, text) {
        Ok(model) => model
            .tokens
            .into_iter()
            .map(|token| (token.text, token.line))
            .collect(),
        Err(error) => panic!("{path} did not lower: {error}"),
    }
}

fn expected_tokens(tokens: &[(&str, u32)]) -> Vec<(String, u32)> {
    tokens
        .iter()
        .map(|(text, line)| ((*text).to_owned(), *line))
        .collect()
}

#[test]
fn the_token_stream_holds_each_token_with_its_line_and_no_whitespace_or_comments() {
    let text = concat!(
        "#!/usr/bin/env node\n",
        "// a line comment\n",
        "const total = 1 +   2; /* inline */\r\n",
        "\n",
        "/**\n",
        " * A block comment.\n",
        " */\n",
        "export const text = `first\n",
        "${total} last`;\n",
    );

    assert_eq!(
        tokens("src/tokens.ts", text),
        expected_tokens(&[
            ("const", 3),
            ("total", 3),
            ("=", 3),
            ("1", 3),
            ("+", 3),
            ("2", 3),
            (";", 3),
            ("export", 8),
            ("const", 8),
            ("text", 8),
            ("=", 8),
            ("`first\n${", 8),
            ("total", 9),
            ("} last`", 9),
            (";", 9),
        ])
    );
}

#[test]
fn the_token_stream_covers_jsx_and_regular_expressions() {
    let text = "export const Hit = () => <b title=\"x\">{/a+/.test('aa')}</b>;\n";

    assert_eq!(
        tokens("src/hit.tsx", text)
            .into_iter()
            .map(|(text, _)| text)
            .collect::<Vec<_>>(),
        [
            "export", "const", "Hit", "=", "(", ")", "=>", "<", "b", "title", "=", "\"x\"", ">",
            "{", "/a+/", ".", "test", "(", "'aa'", ")", "}", "<", "/", "b", ">", ";",
        ]
    );
}

#[test]
fn an_empty_file_has_no_tokens() {
    assert_eq!(tokens("src/empty.ts", ""), []);
}
