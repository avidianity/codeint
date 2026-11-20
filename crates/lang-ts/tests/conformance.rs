use codeint_lang_ts::TypeScriptAndJavaScript;
use codeint_model::language::{Function, ImportRequest, Manifest, Resolution};
use codeint_model::{
    FileModel, LanguagePlugin, LowerError, RepoPath, SourceSet, TokenKind, token_text_hash,
};

use TokenKind::{Identifier, Literal, Other};

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
fn a_typescript_declaration_file_is_declaration_only() {
    for path in [
        "types/globals.d.ts",
        "types/module.d.mts",
        "types/legacy.d.cts",
        "src/styles.d.css.ts",
    ] {
        let result = lower(path, "declare const version: string;\n");

        assert!(
            matches!(result, Err(LowerError::DeclarationOnly)),
            "{path}: unexpected result: {result:?}"
        );
    }
}

#[test]
fn a_file_named_like_a_declaration_without_the_d_suffix_is_code() {
    assert_eq!(source_lines("src/d.ts", "const one = 1;\n"), 1);
    assert_eq!(source_lines("src/build.ts", "const one = 1;\n"), 1);
}

#[test]
fn the_plugin_supplies_no_reference_range_because_none_is_published_for_typescript_or_javascript() {
    let ranges = plugin().reference_ranges().expect("valid Reference ranges");

    for metric in ["erosion", "verbosity", "duplication", "import_cycles"] {
        assert!(ranges.for_metric(metric).is_none(), "{metric}");
    }
}

#[test]
fn a_file_with_an_undeclared_extension_is_unsupported() {
    let result = lower("scripts/tool.py", "print('codeint')\n");

    assert!(
        matches!(result, Err(LowerError::UnsupportedFile { .. })),
        "unexpected result: {result:?}"
    );
}

fn functions(path: &str, text: &str) -> Vec<Function> {
    match lower(path, text) {
        Ok(model) => model.functions,
        Err(error) => panic!("{path} did not lower: {error}"),
    }
}

fn function(line: u32, decision_points: u32, source_lines: u32) -> Function {
    Function {
        line,
        decision_points,
        source_lines,
    }
}

#[test]
fn each_if_statement_and_loop_is_a_decision_point_of_its_function() {
    let text = concat!(
        "export function walk(items: number[], limit: number): number {\n",
        "  let total = 0;\n",
        "\n",
        "  for (let index = 0; index < limit; index++) {\n",
        "    if (items[index] > 0) {\n",
        "      total += items[index];\n",
        "    } else if (items[index] < -10) {\n",
        "      total -= 1;\n",
        "    }\n",
        "  }\n",
        "  for (const key in items) {\n",
        "    total += key.length;\n",
        "  }\n",
        "  for (const item of items) {\n",
        "    total += item;\n",
        "  }\n",
        "  while (total > 100) {\n",
        "    total -= 100;\n",
        "  }\n",
        "  do {\n",
        "    total += 1;\n",
        "  } while (total < 0);\n",
        "  return total;\n",
        "}\n",
    );

    assert_eq!(functions("src/walk.ts", text), [function(1, 7, 23)]);
}

#[test]
fn a_function_without_branches_or_loops_has_no_decision_points() {
    let text = "export function one(): number {\n  return 1;\n}\n";

    assert_eq!(functions("src/one.ts", text), [function(1, 0, 3)]);
}

#[test]
fn a_file_without_functions_has_none() {
    assert_eq!(functions("src/constants.ts", "export const one = 1;\n"), []);
}

#[test]
fn every_kind_of_function_is_lowered_in_the_order_of_its_first_line() {
    let text = concat!(
        "export function declared(flag: boolean) {\n",
        "  if (flag) {\n",
        "    return 1;\n",
        "  }\n",
        "  return 0;\n",
        "}\n",
        "export const expressed = function (flag: boolean) {\n",
        "  return flag;\n",
        "};\n",
        "export const arrow = (flag: boolean) => {\n",
        "  while (flag) {\n",
        "    flag = false;\n",
        "  }\n",
        "};\n",
        "export const concise = (value: number) => value + 1;\n",
        "export class Counter {\n",
        "  count = 0;\n",
        "  constructor(start: number) {\n",
        "    this.count = start;\n",
        "  }\n",
        "  get doubled(): number {\n",
        "    return this.count * 2;\n",
        "  }\n",
        "  increment(): void {\n",
        "    if (this.count < 10) {\n",
        "      this.count++;\n",
        "    }\n",
        "  }\n",
        "}\n",
        "export const literal = {\n",
        "  method() {\n",
        "    return 1;\n",
        "  },\n",
        "};\n",
    );

    assert_eq!(
        functions("src/kinds.ts", text),
        [
            function(1, 1, 6),
            function(7, 0, 3),
            function(10, 1, 5),
            function(15, 0, 1),
            function(18, 0, 3),
            function(21, 0, 3),
            function(24, 1, 5),
            function(31, 0, 3),
        ]
    );
}

#[test]
fn a_nested_function_adds_no_decision_points_to_the_function_around_it() {
    let text = concat!(
        "export function outer(rows: number[][]) {\n",
        "  if (rows.length === 0) {\n",
        "    return;\n",
        "  }\n",
        "  rows.forEach((row) => {\n",
        "    for (const cell of row) {\n",
        "      if (cell > 0) {\n",
        "        console.log(cell);\n",
        "      }\n",
        "    }\n",
        "  });\n",
        "}\n",
    );

    assert_eq!(
        functions("src/nested.ts", text),
        [function(1, 1, 5), function(5, 2, 7)]
    );
}

#[test]
fn each_nested_function_takes_its_lines_and_decision_points_away_from_its_parent() {
    let text = concat!(
        "export function build(flag: boolean) {\n",
        "  const literal = {\n",
        "    method() {\n",
        "      return 1;\n",
        "    },\n",
        "  };\n",
        "  class Box {\n",
        "    value = 0;\n",
        "    get size(): number {\n",
        "      return this.value;\n",
        "    }\n",
        "    set size(next: number) {\n",
        "      this.value = next;\n",
        "    }\n",
        "    grow(): void {\n",
        "      if (flag) {\n",
        "        this.value++;\n",
        "      }\n",
        "    }\n",
        "  }\n",
        "  const named = function () {\n",
        "    return 2;\n",
        "  };\n",
        "  const pair = [() => 3, () => 4];\n",
        "  return Promise.resolve(literal)\n",
        "    .then(() => {\n",
        "      return new Box();\n",
        "    }).then(() => {\n",
        "      return named() + pair.length;\n",
        "    });\n",
        "}\n",
    );

    assert_eq!(
        functions("src/build.ts", text),
        [
            function(1, 0, 8),
            function(3, 0, 3),
            function(9, 0, 3),
            function(12, 0, 3),
            function(15, 1, 5),
            function(21, 0, 3),
            function(24, 0, 1),
            function(24, 0, 1),
            function(26, 0, 3),
            function(28, 0, 3),
        ]
    );
}

#[test]
fn a_function_nested_on_every_line_of_its_parent_takes_every_line() {
    let text = concat!(
        "export const handler = (rows: number[]) => rows.map((row) => {\n",
        "  return row + 1;\n",
        "});\n",
    );

    assert_eq!(
        functions("src/handler.ts", text),
        [function(1, 0, 0), function(1, 0, 3)]
    );
}

#[test]
fn blank_and_comment_only_lines_of_a_function_are_not_its_source_lines() {
    let text = concat!(
        "export function documented(flag: boolean) {\n",
        "  // a comment that explains nothing\n",
        "\n",
        "  /* a block\n",
        "     comment */\n",
        "  if (flag) {\n",
        "    return 1;\n",
        "  }\n",
        "  return 0;\n",
        "}\n",
    );

    assert_eq!(functions("src/documented.ts", text), [function(1, 1, 6)]);
}

#[test]
fn a_function_signature_without_a_body_is_not_a_function() {
    let text = concat!(
        "export function pick(value: string): string;\n",
        "export function pick(value: number): number;\n",
        "export function pick(value: string | number) {\n",
        "  return value;\n",
        "}\n",
        "declare function external(): void;\n",
        "export abstract class Shape {\n",
        "  abstract area(): number;\n",
        "}\n",
    );

    assert_eq!(functions("src/overloads.ts", text), [function(3, 0, 3)]);
}

#[test]
fn a_function_in_a_javascript_file_is_lowered_too() {
    let text = concat!(
        "export function sign(value) {\n",
        "  if (value < 0) {\n",
        "    return -1;\n",
        "  }\n",
        "  return 1;\n",
        "}\n",
    );

    assert_eq!(functions("src/sign.js", text), [function(1, 1, 6)]);
}

#[test]
fn a_function_after_a_comment_with_multibyte_characters_starts_on_the_right_line() {
    let text = format!(
        "/* {} */\nconst a = 1;\nfunction later() {{\n  return a;\n}}\n",
        "\u{2713}".repeat(20)
    );

    assert_eq!(functions("src/marks.ts", &text), [function(3, 0, 3)]);
}

#[test]
fn a_function_that_starts_after_crlf_lines_starts_on_the_right_line() {
    let text = "const first = 1;\r\n\r\nfunction later() {\r\n  return first;\r\n}\r\n";

    assert_eq!(functions("src/crlf.ts", text), [function(3, 0, 3)]);
}

fn cyclomatic_complexity(path: &str, text: &str) -> u32 {
    match functions(path, text).as_slice() {
        [only] => 1 + only.decision_points,
        other => panic!("{path} lowered to {} functions, not 1", other.len()),
    }
}

fn assert_cyclomatic_complexities(cases: &[(&str, &str, u32)]) {
    for (construct, body, expected) in cases {
        let text = format!(
            "export function measured(a: any, b: any, c: any): any {{\n  {body}\n  return a;\n}}\n"
        );
        assert_eq!(
            cyclomatic_complexity("src/measured.ts", &text),
            *expected,
            "{construct}: {body}"
        );
    }
}

#[test]
fn each_branch_and_loop_statement_adds_one_to_cyclomatic_complexity() {
    assert_cyclomatic_complexities(&[
        ("if", "if (a) { b(); }", 2),
        ("if with else", "if (a) { b(); } else { c(); }", 2),
        ("else if", "if (a) { b(); } else if (c) { c(); }", 3),
        ("for", "for (let i = 0; i < a; i++) { b(); }", 2),
        ("for in", "for (const key in a) { b(key); }", 2),
        ("for of", "for (const item of a) { b(item); }", 2),
        ("while", "while (a) { a = b(); }", 2),
        ("do while", "do { a = b(); } while (a);", 2),
    ]);
}

#[test]
fn each_non_default_case_adds_one_and_a_default_case_adds_nothing() {
    assert_cyclomatic_complexities(&[
        ("one case", "switch (a) { case 1: b(); }", 2),
        (
            "two cases",
            "switch (a) { case 1: b(); break; case 2: c(); }",
            3,
        ),
        (
            "fallthrough cases",
            "switch (a) { case 1: case 2: b(); }",
            3,
        ),
        ("only a default case", "switch (a) { default: b(); }", 1),
        (
            "a case and a default case",
            "switch (a) { case 1: b(); break; default: c(); }",
            2,
        ),
    ]);
}

#[test]
fn a_catch_clause_adds_one_and_a_finally_block_adds_nothing() {
    assert_cyclomatic_complexities(&[
        ("catch", "try { b(); } catch (error) { c(error); }", 2),
        ("catch without a binding", "try { b(); } catch { c(); }", 2),
        ("finally", "try { b(); } finally { c(); }", 1),
        (
            "catch and finally",
            "try { b(); } catch { c(); } finally { c(); }",
            2,
        ),
    ]);
}

#[test]
fn each_conditional_operator_adds_one() {
    assert_cyclomatic_complexities(&[
        ("conditional", "a = b ? 1 : 2;", 2),
        ("nested conditional", "a = b ? 1 : c ? 2 : 3;", 3),
    ]);
}

#[test]
fn each_logical_operator_and_its_assignment_form_adds_one() {
    assert_cyclomatic_complexities(&[
        ("and", "a = b && c;", 2),
        ("or", "a = b || c;", 2),
        ("nullish coalescing", "a = b ?? c;", 2),
        ("a chain of three operators", "a = (a && b || c) ?? 1;", 4),
        ("and assignment", "a &&= b;", 2),
        ("or assignment", "a ||= b;", 2),
        ("nullish assignment", "a ??= b;", 2),
        ("plain assignment", "a = b;", 1),
        ("compound assignment", "a += b;", 1),
        ("bitwise operators", "a = b & c | 1;", 1),
    ]);
}

#[test]
fn each_optional_chain_link_adds_one() {
    assert_cyclomatic_complexities(&[
        ("optional property", "b(a?.name);", 2),
        ("optional element", "b(a?.[0]);", 2),
        ("optional call", "a?.();", 2),
        ("optional method call", "a.run?.();", 2),
        (
            "one optional link before plain links",
            "b(a?.first.second[0]());",
            2,
        ),
        ("two optional links", "b(a?.first?.second);", 3),
        ("three optional links", "a?.first?.[0]?.();", 4),
        (
            "plain member access and calls",
            "b(a.first[0].second());",
            1,
        ),
    ]);
}

#[test]
fn an_optional_private_field_link_adds_one() {
    let text = concat!(
        "export class Vault {\n",
        "  #secret = 1;\n",
        "  read(other?: Vault) {\n",
        "    return other?.#secret;\n",
        "  }\n",
        "}\n",
    );

    assert_eq!(cyclomatic_complexity("src/vault.ts", text), 2);
}

#[test]
fn each_default_parameter_adds_one_to_its_own_function() {
    let cases = [
        (
            "one default",
            "export function f(a = 1) {\n  return a;\n}\n",
            2,
        ),
        (
            "two defaults and a plain parameter",
            "export function f(a: number, b = 1, c: string = 'c') {\n  return a + b + c;\n}\n",
            3,
        ),
        ("arrow function", "export const f = (a = 1) => a;\n", 2),
        (
            "method",
            "export class Greeter {\n  greet(name = 'you') {\n    return name;\n  }\n}\n",
            2,
        ),
        (
            "optional parameter without a default",
            "export function f(a?: number) {\n  return a;\n}\n",
            1,
        ),
        (
            "rest parameter",
            "export function f(...rest: number[]) {\n  return rest;\n}\n",
            1,
        ),
    ];

    for (construct, text, expected) in cases {
        assert_eq!(
            cyclomatic_complexity("src/defaults.ts", text),
            expected,
            "{construct}"
        );
    }
}

#[test]
fn a_default_parameter_of_a_nested_function_counts_only_toward_the_nested_function() {
    let text = concat!(
        "export function outer() {\n",
        "  const inner = (limit = 10) => limit;\n",
        "  return inner();\n",
        "}\n",
    );

    assert_eq!(
        functions("src/outer.ts", text),
        [function(1, 0, 3), function(2, 1, 1)]
    );
}

fn tokens(path: &str, text: &str, texts: &[&str]) -> Vec<(TokenKind, String, u32)> {
    let text_of = |hash: u64| {
        texts
            .iter()
            .find(|text| token_text_hash(text) == hash)
            .map_or_else(
                || format!("unknown text {hash:#x}"),
                |text| (*text).to_owned(),
            )
    };
    match lower(path, text) {
        Ok(model) => model
            .tokens
            .into_iter()
            .map(|token| (token.kind, text_of(token.text_hash), token.line))
            .collect(),
        Err(error) => panic!("{path} did not lower: {error}"),
    }
}

fn assert_tokens(path: &str, text: &str, expected: &[(TokenKind, &str, u32)]) {
    let texts: Vec<&str> = expected.iter().map(|(_, text, _)| *text).collect();
    let expected: Vec<(TokenKind, String, u32)> = expected
        .iter()
        .map(|(kind, text, line)| (*kind, (*text).to_owned(), *line))
        .collect();
    assert_eq!(tokens(path, text, &texts), expected);
}

fn assert_token_kinds(path: &str, text: &str, expected: &[(&str, TokenKind)]) {
    let texts: Vec<&str> = expected.iter().map(|(text, _)| *text).collect();
    let actual: Vec<(String, TokenKind)> = tokens(path, text, &texts)
        .into_iter()
        .map(|(kind, text, _)| (text, kind))
        .collect();
    let expected: Vec<(String, TokenKind)> = expected
        .iter()
        .map(|(text, kind)| ((*text).to_owned(), *kind))
        .collect();
    assert_eq!(actual, expected);
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

    assert_tokens(
        "src/tokens.ts",
        text,
        &[
            (Other, "const", 3),
            (Identifier, "total", 3),
            (Other, "=", 3),
            (Literal, "1", 3),
            (Other, "+", 3),
            (Literal, "2", 3),
            (Other, ";", 3),
            (Other, "export", 8),
            (Other, "const", 8),
            (Identifier, "text", 8),
            (Other, "=", 8),
            (Literal, "`first\n${", 8),
            (Identifier, "total", 9),
            (Literal, "} last`", 9),
            (Other, ";", 9),
        ],
    );
}

#[test]
fn decision_points_in_a_javascript_file_count_the_same_way() {
    let text = concat!(
        "export function pick(options = {}) {\n",
        "  const name = options?.name ?? 'none';\n",
        "  switch (name) {\n",
        "    case 'a':\n",
        "      return options.a || 1;\n",
        "    default:\n",
        "      return name ? 2 : 3;\n",
        "  }\n",
        "}\n",
    );

    assert_eq!(cyclomatic_complexity("src/pick.js", text), 7);
}

#[test]
fn the_token_stream_covers_jsx_and_regular_expressions() {
    assert_token_kinds(
        "src/hit.tsx",
        "export const Hit = () => <b title=\"x\">{/a+/.test('aa')}</b>;\n",
        &[
            ("export", Other),
            ("const", Other),
            ("Hit", Identifier),
            ("=", Other),
            ("(", Other),
            (")", Other),
            ("=>", Other),
            ("<", Other),
            ("b", Identifier),
            ("title", Identifier),
            ("=", Other),
            ("\"x\"", Literal),
            (">", Other),
            ("{", Other),
            ("/a+/", Literal),
            (".", Other),
            ("test", Identifier),
            ("(", Other),
            ("'aa'", Literal),
            (")", Other),
            ("}", Other),
            ("<", Other),
            ("/", Other),
            ("b", Identifier),
            (">", Other),
            (";", Other),
        ],
    );
}

#[test]
fn a_keyword_used_as_a_name_is_an_identifier_and_a_keyword_used_as_a_keyword_is_not() {
    assert_token_kinds(
        "src/names.ts",
        "type of = number;\nconst type = item.true ?? #seen in this;\n",
        &[
            ("type", Other),
            ("of", Identifier),
            ("=", Other),
            ("number", Other),
            (";", Other),
            ("const", Other),
            ("type", Identifier),
            ("=", Other),
            ("item", Identifier),
            (".", Other),
            ("true", Identifier),
            ("??", Other),
            ("#seen", Identifier),
            ("in", Other),
            ("this", Other),
            (";", Other),
        ],
    );
}

#[test]
fn every_kind_of_literal_value_is_a_literal() {
    assert_token_kinds(
        "src/values.tsx",
        "f(true, false, null, 0x1f, 10n, 1e3, 'a', `b`, /c/g);\n<p>text</p>;\n",
        &[
            ("f", Identifier),
            ("(", Other),
            ("true", Literal),
            (",", Other),
            ("false", Literal),
            (",", Other),
            ("null", Literal),
            (",", Other),
            ("0x1f", Literal),
            (",", Other),
            ("10n", Literal),
            (",", Other),
            ("1e3", Literal),
            (",", Other),
            ("'a'", Literal),
            (",", Other),
            ("`b`", Literal),
            (",", Other),
            ("/c/g", Literal),
            (")", Other),
            (";", Other),
            ("<", Other),
            ("p", Identifier),
            (">", Other),
            ("text", Literal),
            ("<", Other),
            ("/", Other),
            ("p", Identifier),
            (">", Other),
            (";", Other),
        ],
    );
}

#[test]
fn an_empty_file_has_no_tokens() {
    assert_eq!(tokens("src/empty.ts", "", &[]), []);
}

fn runtime_imports(path: &str, text: &str) -> Vec<String> {
    match lower(path, text) {
        Ok(model) => model.runtime_imports,
        Err(error) => panic!("{path} did not lower: {error}"),
    }
}

#[test]
fn every_runtime_import_form_is_reported_in_source_order() {
    let text = concat!(
        "import './side-effect';\n",
        "import main from './default';\n",
        "import * as all from './namespace';\n",
        "import { named, type Shape } from './mixed';\n",
        "export * from './everything';\n",
        "export { renamed as alias } from './reexport';\n",
        "import legacy = require('./legacy');\n",
        "export async function load() {\n",
        "  const lazy = await import('./lazy');\n",
        "  const templated = await import(`./templated`);\n",
        "  return [main, all, named, legacy, lazy, templated, require('./required')];\n",
        "}\n",
    );

    assert_eq!(
        runtime_imports("src/forms.ts", text),
        [
            "./side-effect",
            "./default",
            "./namespace",
            "./mixed",
            "./everything",
            "./reexport",
            "./legacy",
            "./lazy",
            "./templated",
            "./required",
        ]
    );
}

#[test]
fn type_only_imports_and_exports_are_not_runtime_imports() {
    let text = concat!(
        "import type { First } from './first';\n",
        "import { type Second, type Third } from './second';\n",
        "export type { Fourth } from './fourth';\n",
        "export { type Fifth } from './fifth';\n",
        "export type * from './sixth';\n",
        "import type Seventh = require('./seventh');\n",
        "export const shapes: [First, Second, Third, Seventh] | null = null;\n",
    );

    assert_eq!(runtime_imports("src/types.ts", text), Vec::<String>::new());
}

#[test]
fn an_import_with_a_computed_specifier_is_not_reported() {
    let text = concat!(
        "const name = './computed';\n",
        "export const load = () => import(name);\n",
        "export const read = () => require(`./${name}`);\n",
    );

    assert_eq!(
        runtime_imports("src/computed.js", text),
        Vec::<String>::new()
    );
}

fn resolve(repository_paths: &[&str], importer: &str, specifier: &str) -> Resolution {
    let paths: Vec<RepoPath> = repository_paths
        .iter()
        .map(|path| RepoPath::new(*path).expect("a valid path"))
        .collect();
    let request = ImportRequest {
        importer: importer.to_owned(),
        specifier: specifier.to_owned(),
    };
    let mut resolutions = plugin().resolve_imports(&paths, Vec::new(), vec![request]);
    assert_eq!(resolutions.len(), 1, "one resolution per request");
    resolutions.remove(0)
}

fn resolved(path: &str) -> Resolution {
    Resolution::Resolved(path.to_owned())
}

#[test]
fn a_relative_specifier_resolves_to_the_first_matching_file_in_typescript_order() {
    let files = [
        "src/main.ts",
        "src/util.ts",
        "src/util.js",
        "src/view.tsx",
        "src/legacy.js",
        "src/esm.mts",
        "src/common.cts",
        "src/data.json",
        "src/widgets/index.tsx",
        "lib/shared.ts",
        "index.js",
    ];
    let cases = [
        ("./util", resolved("src/util.ts")),
        ("./util.js", resolved("src/util.js")),
        ("./view.js", resolved("src/view.tsx")),
        ("./view.jsx", resolved("src/view.tsx")),
        ("./legacy", resolved("src/legacy.js")),
        ("./esm.mjs", resolved("src/esm.mts")),
        ("./common.cjs", resolved("src/common.cts")),
        ("./data.json", resolved("src/data.json")),
        ("./widgets", resolved("src/widgets/index.tsx")),
        ("./widgets/", resolved("src/widgets/index.tsx")),
        ("../lib/shared", resolved("lib/shared.ts")),
        ("./../src/./util", resolved("src/util.ts")),
        ("..", resolved("index.js")),
    ];

    for (specifier, expected) in cases {
        assert_eq!(
            resolve(&files, "src/main.ts", specifier),
            expected,
            "{specifier}"
        );
    }
}

#[test]
fn a_bare_specifier_names_a_module_outside_the_repository() {
    for specifier in ["react", "@scope/package/sub", "node:path", "fs"] {
        assert_eq!(
            resolve(&["src/main.ts"], "src/main.ts", specifier),
            Resolution::External,
            "{specifier}"
        );
    }
}

#[test]
fn a_relative_specifier_without_a_matching_file_is_unresolved() {
    for specifier in [
        "./missing",
        "./main.ts/extra",
        "../../outside",
        "/absolute/path",
    ] {
        let resolution = resolve(&["src/main.ts"], "src/main.ts", specifier);
        assert!(
            matches!(&resolution, Resolution::Unresolved(reason) if reason.contains(specifier)),
            "{specifier}: {resolution:?}"
        );
    }
}

#[test]
fn resolutions_come_back_one_per_request_in_request_order() {
    let paths: Vec<RepoPath> = ["a.ts", "b.ts"]
        .iter()
        .map(|path| RepoPath::new(*path).expect("a valid path"))
        .collect();
    let requests = [
        ("a.ts", "./b"),
        ("b.ts", "react"),
        ("b.ts", "./a"),
        ("a.ts", "./c"),
    ]
    .iter()
    .map(|(importer, specifier)| ImportRequest {
        importer: (*importer).to_owned(),
        specifier: (*specifier).to_owned(),
    })
    .collect();

    let resolutions = plugin().resolve_imports(&paths, Vec::new(), requests);

    assert_eq!(
        resolutions,
        [
            resolved("b.ts"),
            Resolution::External,
            resolved("a.ts"),
            Resolution::Unresolved("no file of the repository matches \"./c\"".to_owned()),
        ]
    );
}

fn resolve_in_project(files: &[(&str, &str)], importer: &str, specifier: &str) -> Resolution {
    let paths: Vec<RepoPath> = files
        .iter()
        .map(|(path, _)| RepoPath::new(*path).expect("a valid path"))
        .collect();
    let manifests = plugin()
        .manifest_paths(&paths)
        .into_iter()
        .map(|path| {
            let text = files
                .iter()
                .find(|(file, _)| *file == path)
                .map(|(_, text)| (*text).to_owned())
                .expect("the plugin chooses a file of the repository");
            Manifest { path, text }
        })
        .collect();
    let request = ImportRequest {
        importer: importer.to_owned(),
        specifier: specifier.to_owned(),
    };
    let mut resolutions = plugin().resolve_imports(&paths, manifests, vec![request]);
    assert_eq!(resolutions.len(), 1, "one resolution per request");
    resolutions.remove(0)
}

#[test]
fn the_plugin_asks_the_host_to_read_every_package_manifest_and_tsconfig_file() {
    let paths: Vec<RepoPath> = [
        "package.json",
        "tsconfig.json",
        "tsconfig.base.json",
        "packages/app/package.json",
        "packages/app/tsconfig.build.json",
        "packages/app/src/main.ts",
        "packages/app/src/package.json.ts",
        "data/settings.json",
        "jsconfig.json",
    ]
    .iter()
    .map(|path| RepoPath::new(*path).expect("a valid path"))
    .collect();

    assert_eq!(
        plugin().manifest_paths(&paths),
        [
            "package.json",
            "tsconfig.json",
            "tsconfig.base.json",
            "packages/app/package.json",
            "packages/app/tsconfig.build.json",
        ]
    );
}

const ALIASED_PROJECT: [(&str, &str); 5] = [
    (
        "tsconfig.json",
        "{ \"compilerOptions\": { \"baseUrl\": \".\", \"paths\": { \"@/*\": [\"src/*\"], \"config\": [\"src/settings/index.ts\"] } } }",
    ),
    ("src/main.ts", ""),
    ("src/views/list.tsx", ""),
    ("src/settings/index.ts", ""),
    ("lib/format.ts", ""),
];

#[test]
fn a_tsconfig_path_alias_or_base_url_resolves_to_its_file() {
    let cases = [
        ("@/views/list", resolved("src/views/list.tsx")),
        ("config", resolved("src/settings/index.ts")),
        ("lib/format", resolved("lib/format.ts")),
        ("./views/list.js", resolved("src/views/list.tsx")),
    ];

    for (specifier, expected) in cases {
        assert_eq!(
            resolve_in_project(&ALIASED_PROJECT, "src/main.ts", specifier),
            expected,
            "{specifier}"
        );
    }
}

#[test]
fn a_path_alias_without_a_matching_file_is_unresolved_and_other_bare_specifiers_are_external() {
    let missing = resolve_in_project(&ALIASED_PROJECT, "src/main.ts", "@/views/missing");
    assert!(
        matches!(&missing, Resolution::Unresolved(reason) if reason.contains("@/views/missing")),
        "{missing:?}"
    );
    for specifier in ["react", "@scope/package", "node:fs"] {
        assert_eq!(
            resolve_in_project(&ALIASED_PROJECT, "src/main.ts", specifier),
            Resolution::External,
            "{specifier}"
        );
    }
}

#[test]
fn a_tsconfig_that_does_not_include_the_importer_does_not_alias_its_imports() {
    let files = [
        (
            "tsconfig.json",
            "{ \"include\": [\"src\"], \"compilerOptions\": { \"paths\": { \"@/*\": [\"./src/*\"] } } }",
        ),
        ("src/util.ts", ""),
        ("scripts/build.ts", ""),
    ];

    assert_eq!(
        resolve_in_project(&files, "scripts/build.ts", "@/util"),
        Resolution::External
    );
}

#[test]
fn imports_still_resolve_when_a_tsconfig_extends_a_config_outside_the_repository() {
    let files = [
        (
            "tsconfig.json",
            "{ \"extends\": \"@tsconfig/node22/tsconfig.json\" }",
        ),
        ("src/main.ts", ""),
        ("src/util.ts", ""),
    ];

    assert_eq!(
        resolve_in_project(&files, "src/main.ts", "./util"),
        resolved("src/util.ts")
    );
    assert_eq!(
        resolve_in_project(&files, "src/main.ts", "react"),
        Resolution::External
    );
}

const WORKSPACE: [(&str, &str); 7] = [
    (
        "package.json",
        "{ \"name\": \"shop\", \"workspaces\": [\"packages/*\"] }",
    ),
    (
        "packages/ui/package.json",
        "{ \"name\": \"@shop/ui\", \"exports\": { \".\": \"./src/index.ts\", \"./button\": { \"import\": \"./src/button.tsx\" }, \"./internal/*\": null } }",
    ),
    ("packages/ui/src/index.ts", ""),
    ("packages/ui/src/button.tsx", ""),
    ("packages/ui/src/internal/theme.ts", ""),
    (
        "packages/legacy/package.json",
        "{ \"name\": \"legacy\", \"main\": \"lib/main.js\" }",
    ),
    ("packages/legacy/lib/main.js", ""),
];

#[test]
fn a_workspace_package_resolves_through_its_exports_or_main_field() {
    let cases = [
        ("@shop/ui", resolved("packages/ui/src/index.ts")),
        ("@shop/ui/button", resolved("packages/ui/src/button.tsx")),
        ("legacy", resolved("packages/legacy/lib/main.js")),
    ];

    for (specifier, expected) in cases {
        assert_eq!(
            resolve_in_project(&WORKSPACE, "packages/app/src/main.ts", specifier),
            expected,
            "{specifier}"
        );
    }
}

#[test]
fn a_subpath_that_a_workspace_package_does_not_export_is_unresolved() {
    for specifier in ["@shop/ui/internal/theme", "@shop/ui/missing"] {
        let resolution = resolve_in_project(&WORKSPACE, "packages/legacy/lib/main.js", specifier);
        assert!(
            matches!(&resolution, Resolution::Unresolved(reason) if reason.contains(specifier)),
            "{specifier}: {resolution:?}"
        );
    }
}

fn source_set(path: &str) -> SourceSet {
    match lower(path, "const one = 1;\n") {
        Ok(model) => model.source_set,
        Err(error) => panic!("{path} did not lower: {error}"),
    }
}

#[test]
fn a_test_or_spec_file_of_every_extension_is_in_the_test_source_set() {
    for extension in ["ts", "tsx", "mts", "cts", "js", "jsx", "mjs", "cjs"] {
        for path in [
            format!("src/greet.test.{extension}"),
            format!("src/greet.spec.{extension}"),
            format!("src/app.module.spec.{extension}"),
        ] {
            assert_eq!(source_set(&path), SourceSet::Test, "{path}");
        }
    }
}

#[test]
fn a_file_under_a_tests_folder_at_any_depth_is_in_the_test_source_set() {
    for path in [
        "__tests__/greet.ts",
        "src/__tests__/greet.tsx",
        "src/__tests__/helpers/fixtures.js",
    ] {
        assert_eq!(source_set(path), SourceSet::Test, "{path}");
    }
}

#[test]
fn a_file_that_no_test_rule_matches_is_in_the_production_source_set() {
    for path in [
        "src/greet.ts",
        "src/test.ts",
        "src/spec.ts",
        "src/.test.ts",
        "src/greet.testing.ts",
        "src/latest.ts",
        "src/test/greet.ts",
        "src/tests/greet.ts",
        "src/__tests__.ts",
        "src/my__tests__/greet.ts",
        "src/greet.test.helpers.ts",
    ] {
        assert_eq!(source_set(path), SourceSet::Production, "{path}");
    }
}

fn source_set_of_text(path: &str, text: &str) -> SourceSet {
    match lower(path, text) {
        Ok(model) => model.source_set,
        Err(error) => panic!("{path} did not lower: {error}"),
    }
}

#[test]
fn a_file_with_a_generated_marker_in_a_comment_before_its_code_is_in_the_generated_source_set() {
    for text in [
        "// @generated\nexport const one = 1;\n",
        "/* @generated SignedSource<<0123abcd>> */\nexport const one = 1;\n",
        "/**\n * This file was automatically generated by json-schema-to-typescript.\n */\nexport type One = 1;\n",
        "// Code generated by protoc-gen-ts. DO NOT EDIT.\nexport const one = 1;\n",
        "/* eslint-disable */\n// This file is auto-generated.\nexport const one = 1;\n",
        "/* !!! This is code generated by Prisma. Do not edit directly. !!! */\nexport const one = 1;\n",
        "/* NOTE: This class is auto generated by OpenAPI Generator. */\nexport class One {}\n",
        "// AUTOGENERATED FILE\nexport const one = 1;\n",
        "// Generated by CoffeeScript 1.12.7\nexport const one = 1;\n",
        "#!/usr/bin/env node\n// @generated\nexport const one = 1;\n",
        "// @generated\n",
    ] {
        assert_eq!(
            source_set_of_text("src/one.ts", text),
            SourceSet::Generated,
            "{text}"
        );
    }
}

#[test]
fn a_generated_marker_wins_over_a_test_rule() {
    assert_eq!(
        source_set_of_text(
            "src/__tests__/fixtures.ts",
            "// @generated\nexport const one = 1;\n"
        ),
        SourceSet::Generated
    );
}

#[test]
fn a_generated_marker_after_the_first_code_or_outside_a_comment_does_not_make_a_file_generated() {
    for text in [
        "export const one = 1;\n// @generated\n",
        "import { two } from './two';\n/* DO NOT EDIT below this line */\nexport const one = two;\n",
        "const banner = '// @generated';\nexport { banner };\n",
        "// The generator of this report reads the config.\nexport const one = 1;\n",
        "// Generates the next id, so edit with care.\nexport const one = 1;\n",
    ] {
        assert_eq!(
            source_set_of_text("src/one.ts", text),
            SourceSet::Production,
            "{text}"
        );
    }
}
