use codeint_lang_ts::TypeScriptAndJavaScript;
use codeint_model::language::{DeclaredDep, DepKind, Function, ImportRequest, Resolution};
use codeint_model::{
    FileModel, LanguagePlugin, LowerError, Manifest, RepoPath, SourceSet, TokenKind,
    token_text_hash,
};

use TokenKind::{Identifier, ImportOrExport, Literal, Other};

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
        [
            "cjs", "cts", "js", "jsx", "mjs", "mts", "svelte", "ts", "tsx", "vue"
        ]
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
fn javascript_files_with_jsx_lower_their_functions_and_tokens() {
    let esm = concat!(
        "export function Badge({ label, active }) {\n",
        "  if (active) {\n",
        "    return <span title=\"on\">{label}</span>;\n",
        "  }\n",
        "  return <span>{label}</span>;\n",
        "}\n",
    );
    let cjs = concat!(
        "function Badge({ label, active }) {\n",
        "  if (active) {\n",
        "    return <span title=\"on\">{label}</span>;\n",
        "  }\n",
        "  return <span>{label}</span>;\n",
        "}\n",
        "module.exports = { Badge };\n",
    );

    let from_jsx = lower("src/badge.jsx", esm).expect("jsx file lowers");
    for path in ["src/badge.js", "src/badge.mjs"] {
        let model =
            lower(path, esm).unwrap_or_else(|error| panic!("{path} did not lower: {error}"));
        assert_eq!(model.source_lines, from_jsx.source_lines, "{path}");
        assert_eq!(
            model.functions,
            [named_function(1, 6, 1, 6, Some("Badge"), 1)],
            "{path}"
        );
        assert_eq!(model.tokens, from_jsx.tokens, "{path}");
    }

    let cjs_model = lower("src/badge.cjs", cjs).expect("cjs file with jsx lowers");
    assert_eq!(cjs_model.source_lines, 7);
    assert_eq!(
        cjs_model.functions,
        [named_function(1, 6, 1, 6, Some("Badge"), 1)]
    );
    assert!(!cjs_model.tokens.is_empty());
}

#[test]
fn a_javascript_file_without_jsx_keeps_its_functions_and_comparison_tokens() {
    let text = concat!(
        "export function inRange(value, low, high) {\n",
        "  return value < high && value > low;\n",
        "}\n",
    );

    assert_eq!(
        functions("src/range.js", text),
        [named_function(1, 3, 1, 3, Some("inRange"), 1)]
    );
    assert_token_kinds(
        "src/range.js",
        text,
        &[
            ("export", Other),
            ("function", Other),
            ("inRange", Identifier),
            ("(", Other),
            ("value", Identifier),
            (",", Other),
            ("low", Identifier),
            (",", Other),
            ("high", Identifier),
            (")", Other),
            ("{", Other),
            ("return", Other),
            ("value", Identifier),
            ("<", Other),
            ("high", Identifier),
            ("&&", Other),
            ("value", Identifier),
            (">", Other),
            ("low", Identifier),
            (";", Other),
            ("}", Other),
        ],
    );
}

#[test]
fn a_flow_annotated_javascript_file_is_unparsable() {
    let text = concat!(
        "// @flow\n",
        "export function greet(name: string): string {\n",
        "  return name;\n",
        "}\n",
    );

    let result = lower("src/flow.js", text);

    assert!(
        matches!(
            &result,
            Err(LowerError::Unparsable { message }) if message.contains("Flow is not supported")
        ),
        "unexpected result: {result:?}"
    );
}

#[test]
fn a_typescript_file_without_tsx_extension_rejects_jsx() {
    let result = lower("src/badge.ts", "export const Badge = () => <span />;\n");

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
fn the_plugin_supplies_the_approved_reference_ranges_for_every_metric() {
    let ranges = plugin().reference_ranges().expect("valid Reference ranges");
    let source = "codeint stratified panel of 100 TypeScript and JavaScript repositories, ADR 0035, codeint 0.8.0";

    for (metric, low, high) in [
        ("erosion", 0.187, 0.671),
        ("cognitive_complexity", 0.121, 0.532),
        ("verbosity", 0.000, 0.002),
        ("duplication", 0.018, 0.252),
        ("import_cycles", 0.000, 0.182),
    ] {
        let range = ranges
            .for_metric(metric)
            .unwrap_or_else(|| panic!("missing Reference range for {metric}"));
        assert_eq!(
            (range.low, range.high, range.source.as_str()),
            (low, high, source),
            "{metric}"
        );
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

fn named_function(
    line: u32,
    end_line: u32,
    decision_points: u32,
    source_lines: u32,
    name: Option<&str>,
    cognitive_complexity: u32,
) -> Function {
    Function {
        line,
        end_line,
        decision_points,
        source_lines,
        name: name.map(str::to_owned),
        cognitive_complexity,
        coverage: None,
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

    assert_eq!(
        functions("src/walk.ts", text),
        [named_function(1, 24, 7, 23, Some("walk"), 8)]
    );
}

#[test]
fn a_function_without_branches_or_loops_has_no_decision_points() {
    let text = "export function one(): number {\n  return 1;\n}\n";

    assert_eq!(
        functions("src/one.ts", text),
        [named_function(1, 3, 0, 3, Some("one"), 0)]
    );
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
            named_function(1, 6, 1, 6, Some("declared"), 1),
            named_function(7, 9, 0, 3, Some("expressed"), 0),
            named_function(10, 14, 1, 5, Some("arrow"), 1),
            named_function(15, 15, 0, 1, Some("concise"), 0),
            named_function(18, 20, 0, 3, Some("constructor"), 0),
            named_function(21, 23, 0, 3, Some("doubled"), 0),
            named_function(24, 28, 1, 5, Some("increment"), 1),
            named_function(31, 33, 0, 3, Some("method"), 0),
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
        [
            named_function(1, 12, 1, 5, Some("outer"), 1),
            named_function(5, 11, 2, 7, None, 3),
        ]
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
            named_function(1, 31, 0, 8, Some("build"), 0),
            named_function(3, 5, 0, 3, Some("method"), 0),
            named_function(9, 11, 0, 3, Some("size"), 0),
            named_function(12, 14, 0, 3, Some("size"), 0),
            named_function(15, 19, 1, 5, Some("grow"), 1),
            named_function(21, 23, 0, 3, Some("named"), 0),
            named_function(24, 24, 0, 1, None, 0),
            named_function(24, 24, 0, 1, None, 0),
            named_function(26, 28, 0, 3, None, 0),
            named_function(28, 30, 0, 3, None, 0),
        ]
    );
}

fn assert_lowered_functions(path: &str, text: &str, expected: &[Function]) {
    assert_eq!(functions(path, text), expected);
}

#[test]
fn a_function_nested_on_every_line_of_its_parent_takes_every_line() {
    assert_lowered_functions(
        "src/handler.ts",
        "export const handler = (rows: number[]) => rows.map((row) => {\n  return row + 1;\n});\n",
        &[
            named_function(1, 3, 0, 0, Some("handler"), 0),
            named_function(1, 3, 0, 3, None, 0),
        ],
    );
}

#[test]
fn blank_and_comment_only_lines_of_a_function_are_not_its_source_lines() {
    let source = [
        "export function documented(flag: boolean) {",
        "  // a comment that explains nothing",
        "",
        "  /* a block",
        "     comment */",
        "  if (flag) {",
        "    return 1;",
        "  }",
        "  return 0;",
        "}",
        "",
    ]
    .join("\n");

    assert_lowered_functions(
        "src/documented.ts",
        &source,
        &[named_function(1, 10, 1, 6, Some("documented"), 1)],
    );
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

    assert_eq!(
        functions("src/overloads.ts", text),
        [named_function(3, 5, 0, 3, Some("pick"), 0)]
    );
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

    assert_eq!(
        functions("src/sign.js", text),
        [named_function(1, 6, 1, 6, Some("sign"), 1)]
    );
}

#[test]
fn a_function_after_a_comment_with_multibyte_characters_starts_on_the_right_line() {
    let text = format!(
        "/* {} */\nconst a = 1;\nfunction later() {{\n  return a;\n}}\n",
        "\u{2713}".repeat(20)
    );

    assert_eq!(
        functions("src/marks.ts", &text),
        [named_function(3, 5, 0, 3, Some("later"), 0)]
    );
}

#[test]
fn a_function_that_starts_after_crlf_lines_starts_on_the_right_line() {
    let text = "const first = 1;\r\n\r\nfunction later() {\r\n  return first;\r\n}\r\n";

    assert_eq!(
        functions("src/crlf.ts", text),
        [named_function(3, 5, 0, 3, Some("later"), 0)]
    );
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
        [
            named_function(1, 4, 0, 3, Some("outer"), 0),
            named_function(2, 2, 1, 1, Some("inner"), 0),
        ]
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
fn every_token_of_an_import_or_export_declaration_is_an_import_or_export_token() {
    let text = concat!(
        "import a, { b as c } from 'd';\n",
        "import type { E } from 'f';\n",
        "import * as g from 'h';\n",
        "import 'i';\n",
        "import j = require('k');\n",
        "export * from 'l';\n",
        "export * as m from 'n';\n",
        "export { o, p as q } from 'r';\n",
        "export type { S } from 't';\n",
        "export { a, g };\n",
    );
    let lowered = tokens("src/barrel.ts", text, &[]);

    assert_eq!(lowered.len(), 75);
    assert!(
        lowered.iter().all(|(kind, _, _)| *kind == ImportOrExport),
        "{lowered:?}"
    );
}

#[test]
fn code_that_declares_and_exports_or_imports_at_runtime_keeps_its_token_kinds() {
    assert_token_kinds(
        "src/exports.ts",
        concat!(
            "export const one = 1;\n",
            "export default function two() {\n",
            "  return import('./three');\n",
            "}\n",
            "export = one;\n",
        ),
        &[
            ("export", Other),
            ("const", Other),
            ("one", Identifier),
            ("=", Other),
            ("1", Literal),
            (";", Other),
            ("export", Other),
            ("default", Other),
            ("function", Other),
            ("two", Identifier),
            ("(", Other),
            (")", Other),
            ("{", Other),
            ("return", Other),
            ("import", Other),
            ("(", Other),
            ("'./three'", Literal),
            (")", Other),
            (";", Other),
            ("}", Other),
            ("export", Other),
            ("=", Other),
            ("one", Identifier),
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
    let mut resolutions = plugin()
        .resolve_imports(&paths, Vec::new(), vec![request], &[])
        .expect("resolve imports");
    assert_eq!(resolutions.len(), 1, "one resolution per request");
    resolutions.remove(0)
}

fn resolved(path: &str) -> Resolution {
    Resolution::Resolved(vec![path.to_owned()])
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
    for (specifier, expected) in [
        ("react", Resolution::External("react".to_owned())),
        (
            "@scope/package/sub",
            Resolution::External("@scope/package".to_owned()),
        ),
        ("node:path", Resolution::Builtin),
        ("fs", Resolution::Builtin),
    ] {
        assert_eq!(
            resolve(&["src/main.ts"], "src/main.ts", specifier),
            expected,
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

    let resolutions = plugin()
        .resolve_imports(&paths, Vec::new(), requests, &[])
        .expect("resolve imports");

    assert_eq!(
        resolutions,
        [
            resolved("b.ts"),
            Resolution::External("react".to_owned()),
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
    let mut resolutions = plugin()
        .resolve_imports(&paths, manifests, vec![request], &[])
        .expect("resolve imports");
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
    for (specifier, expected) in [
        ("react", Resolution::External("react".to_owned())),
        (
            "@scope/package",
            Resolution::External("@scope/package".to_owned()),
        ),
        ("node:fs", Resolution::Builtin),
    ] {
        assert_eq!(
            resolve_in_project(&ALIASED_PROJECT, "src/main.ts", specifier),
            expected,
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
        Resolution::External("@/util".to_owned())
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
        Resolution::External("react".to_owned())
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
fn a_named_package_outside_the_declared_workspaces_is_not_linked() {
    let files = [
        (
            "package.json",
            "{ \"name\": \"shop\", \"workspaces\": [\"packages/*\", \"!packages/retired\"] }",
        ),
        (
            "packages/app/package.json",
            "{ \"name\": \"app\", \"main\": \"src/main.ts\" }",
        ),
        ("packages/app/src/main.ts", ""),
        (
            "packages/retired/package.json",
            "{ \"name\": \"retired\", \"main\": \"index.ts\" }",
        ),
        ("packages/retired/index.ts", ""),
        (
            "tools/scripts/package.json",
            "{ \"name\": \"scripts\", \"main\": \"index.ts\" }",
        ),
        ("tools/scripts/index.ts", ""),
    ];

    assert_eq!(
        resolve_in_project(&files, "tools/scripts/index.ts", "app"),
        resolved("packages/app/src/main.ts")
    );
    for specifier in ["scripts", "retired"] {
        assert_eq!(
            resolve_in_project(&files, "packages/app/src/main.ts", specifier),
            Resolution::External(specifier.to_owned()),
            "{specifier}"
        );
    }
}

#[test]
fn a_named_package_in_a_repository_without_workspaces_is_not_linked() {
    let files = [
        ("package.json", "{ \"name\": \"single\" }"),
        (
            "examples/demo/package.json",
            "{ \"name\": \"demo\", \"main\": \"index.ts\" }",
        ),
        ("examples/demo/index.ts", ""),
        ("src/main.ts", ""),
    ];

    assert_eq!(
        resolve_in_project(&files, "src/main.ts", "demo"),
        Resolution::External("demo".to_owned())
    );
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
fn a_file_under_a_test_or_tests_folder_at_any_depth_is_in_the_test_source_set() {
    for path in [
        "__tests__/greet.ts",
        "src/__tests__/greet.tsx",
        "src/__tests__/helpers/fixtures.js",
        "test/greet.ts",
        "tests/greet.mjs",
        "src/test/greet.ts",
        "src/tests/greet.ts",
        "packages/app/test/fixtures/deep/user.js",
        "packages/app/tests/helpers/setup.tsx",
        "test/test.ts",
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
        "test.ts",
        "tests.ts",
        "src/tests.ts",
        "src/testing/greet.ts",
        "src/my-tests/greet.ts",
        "src/latest/greet.ts",
        "src/Test/greet.ts",
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

fn manifest(path: &str, text: &str) -> Manifest {
    Manifest {
        path: path.to_owned(),
        text: text.to_owned(),
    }
}

fn package_roots(root_manifest: &str, package_folders: &[&str]) -> Vec<String> {
    let mut manifests = vec![manifest("package.json", root_manifest)];
    manifests.extend(
        package_folders
            .iter()
            .map(|folder| manifest(&format!("{folder}/package.json"), "{}")),
    );
    let mut roots: Vec<String> = match plugin().workspace_packages(manifests) {
        Ok(roots) => roots.iter().map(ToString::to_string).collect(),
        Err(error) => panic!("workspace packages not found: {error}"),
    };
    roots.sort();
    roots
}

#[test]
fn the_plugin_reads_package_json_manifests_to_find_workspace_packages() {
    let paths: Vec<RepoPath> = [
        "package.json",
        "packages/app/package.json",
        "packages/app/src/main.ts",
        "packages/group/inner/package.json",
    ]
    .iter()
    .map(|path| RepoPath::new(*path).expect("a valid path"))
    .collect();
    let package_manifests: Vec<String> = plugin()
        .manifest_paths(&paths)
        .into_iter()
        .filter(|path| path.ends_with("package.json"))
        .collect();

    assert_eq!(
        package_manifests,
        [
            "package.json",
            "packages/app/package.json",
            "packages/group/inner/package.json",
        ]
    );
}

#[test]
fn the_plugin_reads_the_pnpm_workspace_yaml_at_the_repository_root_and_no_other() {
    let paths: Vec<RepoPath> = [
        "pnpm-workspace.yaml",
        "packages/app/pnpm-workspace.yaml",
        "packages/app/src/main.ts",
    ]
    .iter()
    .map(|path| RepoPath::new(*path).expect("a valid path"))
    .collect();

    assert_eq!(plugin().manifest_paths(&paths), ["pnpm-workspace.yaml"]);
}

#[test]
fn a_tsconfig_manifest_beside_the_package_manifests_never_makes_a_workspace_package() {
    let roots = plugin()
        .workspace_packages(vec![
            manifest("package.json", r#"{"workspaces": ["packages/*"]}"#),
            manifest("tsconfig.json", "{}"),
            manifest("packages/app/package.json", "{}"),
            manifest("packages/lib/tsconfig.json", "{}"),
        ])
        .expect("the workspace packages");
    let roots: Vec<String> = roots.iter().map(ToString::to_string).collect();

    assert_eq!(roots, ["packages/app"]);
}

#[test]
fn a_negated_workspace_pattern_excludes_the_folders_it_matches() {
    let roots = package_roots(
        r#"{"workspaces": ["packages/*", "!packages/legacy"]}"#,
        &["packages/app", "packages/legacy"],
    );

    assert_eq!(roots, ["packages/app"]);
}

#[test]
fn a_workspace_pattern_with_a_leading_dot_slash_or_a_trailing_slash_matches_like_one_without() {
    let roots = package_roots(
        r#"{"workspaces": ["./apps/*", "tools/", "./libs/core/"]}"#,
        &["apps/web", "tools", "libs/core"],
    );

    assert_eq!(roots, ["apps/web", "libs/core", "tools"]);
}

#[test]
fn a_double_star_workspace_pattern_matches_packages_at_any_depth_below_its_folder() {
    let roots = package_roots(
        r#"{"workspaces": ["packages/**"]}"#,
        &["packages/app", "packages/group/inner", "other/app"],
    );

    assert_eq!(roots, ["packages/app", "packages/group/inner"]);
}

#[test]
fn a_package_inside_node_modules_is_never_a_workspace_package() {
    let roots = package_roots(
        r#"{"workspaces": ["**"]}"#,
        &["app", "app/node_modules/dependency", "node_modules/tool"],
    );

    assert_eq!(roots, ["app"]);
}

#[test]
fn a_root_manifest_without_workspaces_or_no_root_manifest_means_no_workspace_packages() {
    assert!(package_roots(r#"{"name": "single"}"#, &["packages/app"]).is_empty());
    let without_root = plugin()
        .workspace_packages(vec![manifest("packages/app/package.json", "{}")])
        .expect("no root manifest is not an error");
    assert!(without_root.is_empty());
}

#[test]
fn a_manifest_that_is_not_the_root_manifest_is_not_parsed() {
    let roots = package_roots(r#"{"workspaces": ["packages/*"]}"#, &[]);
    assert!(roots.is_empty());
    let broken_member = plugin().workspace_packages(vec![
        manifest("package.json", r#"{"workspaces": ["packages/*"]}"#),
        manifest("packages/app/package.json", "{ not json"),
    ]);
    let roots: Vec<String> = broken_member
        .expect("a broken member manifest is not an error")
        .iter()
        .map(ToString::to_string)
        .collect();
    assert_eq!(roots, ["packages/app"]);
}

fn workspace_error(root_manifest: &str) -> String {
    manifest_workspace_error("package.json", root_manifest)
}

fn manifest_workspace_error(path: &str, text: &str) -> String {
    match plugin().workspace_packages(vec![manifest(path, text)]) {
        Ok(roots) => panic!("expected an error, found {roots:?}"),
        Err(error) => error.to_string(),
    }
}

#[test]
fn a_root_manifest_that_is_not_json_is_an_error_that_names_it() {
    assert_eq!(
        workspace_error("{ \"workspaces\": "),
        "cannot find the workspace packages from package.json: is not valid JSON: EOF while parsing a value at line 1 column 16"
    );
}

#[test]
fn a_workspaces_field_of_the_wrong_shape_is_an_error_that_names_the_expected_shape() {
    let expected = "cannot find the workspace packages from package.json: \
                    the workspaces field must be an array of folder patterns, \
                    or an object whose packages field is one";
    for wrong in [
        r#"{"workspaces": "packages/*"}"#,
        r#"{"workspaces": {"packages": "packages/*"}}"#,
        r#"{"workspaces": ["packages/*", 3]}"#,
    ] {
        assert_eq!(workspace_error(wrong), expected, "{wrong}");
    }
}

#[test]
fn an_invalid_workspace_glob_is_an_error_that_names_the_pattern() {
    let message = workspace_error(r#"{"workspaces": ["packages/[a"]}"#);
    assert!(
        message.starts_with(
            "cannot find the workspace packages from package.json: has the workspace pattern \"packages/[a\", which is not a valid glob: "
        ),
        "{message}"
    );
}

fn pnpm_workspace_error(pnpm_workspace_manifest: &str) -> String {
    manifest_workspace_error("pnpm-workspace.yaml", pnpm_workspace_manifest)
}

#[test]
fn a_pnpm_workspace_yaml_that_is_not_yaml_is_an_error_that_names_it() {
    assert_eq!(
        pnpm_workspace_error("packages: [apps/*\n"),
        "cannot find the workspace packages from pnpm-workspace.yaml: is not valid YAML: unclosed bracket '[' at line 1, column 11"
    );
}

const PNPM_WRONG_SHAPE: &str = "cannot find the workspace packages from pnpm-workspace.yaml: must be a mapping whose packages field is a list of folder patterns";

#[test]
fn a_packages_field_of_the_wrong_shape_is_an_error_that_names_the_expected_shape_and_the_place() {
    for (wrong, place) in [
        (
            "packages: apps/*\n",
            "expected sequence start at line 1, column 11",
        ),
        (
            "packages:\n  - name: app\n",
            "expected string scalar at line 2, column 5",
        ),
        ("- apps/*\n", "expected mapping start at line 1, column 1"),
    ] {
        assert_eq!(
            pnpm_workspace_error(wrong),
            format!("{PNPM_WRONG_SHAPE}: {place}"),
            "{wrong:?}"
        );
    }
}

#[test]
fn a_pnpm_package_pattern_that_is_not_a_glob_is_an_error_that_names_it() {
    assert_eq!(
        pnpm_workspace_error("packages:\n  - packages/[app\n"),
        "cannot find the workspace packages from pnpm-workspace.yaml: has the package pattern \"packages/[app\", which is not a valid glob: error parsing glob '[app': unclosed character class; missing ']'"
    );
}

#[test]
fn deeply_nested_code_is_unparsable_instead_of_overflowing_the_stack() {
    let nested_ifs = format!(
        "export function f(a: boolean) {{\n{}return 1;\n{}}}\n",
        "if (a) {\n".repeat(10_000),
        "}\n".repeat(10_000)
    );
    let nested_objects = format!(
        "export const x = {}1{};\n",
        "{ a: ".repeat(10_000),
        " }".repeat(10_000)
    );
    let nested_arrays = format!(
        "export const x = {}1{};\n",
        "[".repeat(20_000),
        "]".repeat(20_000)
    );
    let nested_parens = format!(
        "export const x = {}1{};\n",
        "(".repeat(20_000),
        ")".repeat(20_000)
    );
    let nested_ternaries = format!(
        "export const x = {}1{};\n",
        "a ? ".repeat(4_000),
        " : 0".repeat(4_000)
    );
    for (name, text) in [
        ("if_blocks", nested_ifs),
        ("objects", nested_objects),
        ("arrays", nested_arrays),
        ("parens", nested_parens),
        ("ternaries", nested_ternaries),
    ] {
        assert!(
            matches!(
                lower("src/deep.ts", &text),
                Err(LowerError::Unparsable { message: _ })
            ),
            "{name} should be unparsable"
        );
    }
}

#[test]
fn brackets_in_strings_and_many_optional_operators_still_lower() {
    let text = format!(
        "export const braces = \"{}\";\nexport type Opt = {{ {} }};\nexport const chain = {};\n",
        "{".repeat(500),
        (0..500)
            .map(|i| format!("f{i}?: number;"))
            .collect::<Vec<_>>()
            .join(" "),
        (0..500).map(|_| "a?.b").collect::<Vec<_>>().join(" ?? ")
    );
    assert!(lower("src/normal.ts", &text).is_ok());
}

fn resolve_for(
    paths: &[&str],
    manifests: &[(&str, &str)],
    requests: &[(&str, &str)],
) -> Vec<Resolution> {
    let repo_paths: Vec<RepoPath> = paths
        .iter()
        .map(|p| RepoPath::new(*p).expect("valid path"))
        .collect();
    let manifest_list: Vec<Manifest> = manifests
        .iter()
        .map(|(path, text)| Manifest {
            path: (*path).to_owned(),
            text: (*text).to_owned(),
        })
        .collect();
    let import_requests: Vec<ImportRequest> = requests
        .iter()
        .map(|(importer, specifier)| ImportRequest {
            importer: (*importer).to_owned(),
            specifier: (*specifier).to_owned(),
        })
        .collect();
    plugin()
        .resolve_imports(&repo_paths, manifest_list, import_requests, &[])
        .expect("resolve imports")
}

#[test]
fn bun_builtins_resolve_as_external_even_with_catch_all_tsconfig_paths() {
    let resolutions = resolve_for(
        &["tsconfig.json", "src/db.ts"],
        &[(
            "tsconfig.json",
            r#"{"compilerOptions": {"baseUrl": ".", "paths": {"*": ["./src/*"]}}}"#,
        )],
        &[
            ("src/db.ts", "bun"),
            ("src/db.ts", "bun:sqlite"),
            ("src/db.ts", "bun:test"),
        ],
    );
    assert_eq!(resolutions, vec![Resolution::Builtin; 3]);
}

#[test]
fn bun_export_condition_resolves_workspace_source_file() {
    let resolutions = resolve_for(
        &[
            "package.json",
            "packages/app/package.json",
            "packages/app/src/index.ts",
            "packages/shared/package.json",
            "packages/shared/src/bun.ts",
        ],
        &[
            ("package.json", r#"{"workspaces": ["packages/*"]}"#),
            ("packages/app/package.json", r#"{"name": "@acme/app"}"#),
            (
                "packages/shared/package.json",
                r#"{"name": "@acme/shared", "exports": {".": {"bun": "./src/bun.ts", "default": "./dist/index.js"}}}"#,
            ),
        ],
        &[("packages/app/src/index.ts", "@acme/shared")],
    );
    assert_eq!(
        resolutions,
        vec![Resolution::Resolved(vec![
            "packages/shared/src/bun.ts".to_owned()
        ])]
    );
}

#[test]
fn workspaces_and_cross_package_imports_work_for_bun_npm_yarn_and_pnpm() {
    for (name, root_path, root_text) in [
        (
            "bun_and_npm_array",
            "package.json",
            r#"{"workspaces": ["packages/*", "!packages/ignored"]}"#,
        ),
        (
            "yarn_object_packages",
            "package.json",
            r#"{"workspaces": {"packages": ["packages/*", "!packages/ignored"]}}"#,
        ),
        (
            "pnpm_yaml",
            "pnpm-workspace.yaml",
            "packages:\n  - 'packages/*'\n  - '!packages/ignored'\n",
        ),
    ] {
        let manifests = [
            (root_path, root_text),
            ("packages/web/package.json", r#"{"name": "@demo/web"}"#),
            (
                "packages/core/package.json",
                r#"{"name": "@demo/core", "exports": {".": "./src/index.ts"}}"#,
            ),
            (
                "packages/ignored/package.json",
                r#"{"name": "@demo/ignored"}"#,
            ),
        ];
        let manifest_vec: Vec<Manifest> = manifests
            .iter()
            .map(|(path, text)| Manifest {
                path: (*path).to_owned(),
                text: (*text).to_owned(),
            })
            .collect();
        let mut packages: Vec<String> = plugin()
            .workspace_packages(manifest_vec)
            .unwrap_or_else(|error| panic!("{name}: {error}"))
            .into_iter()
            .map(|p| p.as_str().to_owned())
            .collect();
        packages.sort();
        assert_eq!(packages, vec!["packages/core", "packages/web"], "{name}");

        let resolutions = resolve_for(
            &[
                root_path,
                "packages/web/package.json",
                "packages/web/src/index.ts",
                "packages/core/package.json",
                "packages/core/src/index.ts",
                "packages/ignored/package.json",
            ],
            &manifests,
            &[("packages/web/src/index.ts", "@demo/core")],
        );
        assert_eq!(
            resolutions,
            vec![Resolution::Resolved(vec![
                "packages/core/src/index.ts".to_owned()
            ])],
            "{name}"
        );
    }
}

#[test]
fn named_functions_methods_and_anonymous_callbacks_report_expected_names() {
    let text = concat!(
        "export function declared() { return 1; }\n",
        "export const arrowNamed = (x: number) => x + 1;\n",
        "export const exprNamed = function () { return 2; };\n",
        "export const exprSelfNamed = function innerName() { return 3; };\n",
        "export default function () { return 4; }\n",
        "export class Widget {\n",
        "  constructor() {}\n",
        "  render() { return 1; }\n",
        "  #secret() { return 2; }\n",
        "  onClick = () => 3;\n",
        "}\n",
        "export const obj = {\n",
        "  shorthand() { return 1; },\n",
        "  propFn: () => 2,\n",
        "};\n",
        "export function withCallbacks(items: number[]) {\n",
        "  return items.map((item) => item * 2).filter(function (v) { return v > 0; });\n",
        "}\n",
    );

    let names: Vec<Option<String>> = functions("src/names.ts", text)
        .into_iter()
        .map(|f| f.name)
        .collect();

    assert_eq!(
        names,
        vec![
            Some("declared".to_owned()),
            Some("arrowNamed".to_owned()),
            Some("exprNamed".to_owned()),
            Some("innerName".to_owned()),
            None,
            Some("constructor".to_owned()),
            Some("render".to_owned()),
            Some("#secret".to_owned()),
            Some("onClick".to_owned()),
            Some("shorthand".to_owned()),
            Some("propFn".to_owned()),
            Some("withCallbacks".to_owned()),
            None,
            None,
        ]
    );
}

#[test]
fn cognitive_complexity_distinguishes_flat_branching_from_deeply_nested_control_flow() {
    let flat_switch = concat!(
        "export function flatSwitch(x: number): number {\n",
        "  switch (x) {\n",
        "    case 1: return 1;\n",
        "    case 2: return 2;\n",
        "    case 3: return 3;\n",
        "    case 4: return 4;\n",
        "    default: return 0;\n",
        "  }\n",
        "}\n",
    );
    let flat_ifs = concat!(
        "export function flatIfs(a: boolean, b: boolean, c: boolean, d: boolean): number {\n",
        "  let n = 0;\n",
        "  if (a) { n += 1; }\n",
        "  if (b) { n += 1; }\n",
        "  if (c) { n += 1; }\n",
        "  if (d) { n += 1; }\n",
        "  return n;\n",
        "}\n",
    );
    let deeply_nested = r#"export function deeplyNested(a: boolean, b: boolean, c: boolean, d: boolean): number {
  if (a) {
    if (b) {
      if (c) {
        if (d) { return 1; }
      }
    }
  }
  return 0;
}
"#;

    let switch_fn = &functions("src/flat_switch.ts", flat_switch)[0];
    let flat_fn = &functions("src/flat_ifs.ts", flat_ifs)[0];
    let nested_fn = &functions("src/deeply_nested.ts", deeply_nested)[0];

    assert_eq!(switch_fn.decision_points, 4);
    assert_eq!(switch_fn.cognitive_complexity, 1);
    assert_eq!(flat_fn.decision_points, 4);
    assert_eq!(flat_fn.cognitive_complexity, 4);
    assert_eq!(nested_fn.decision_points, 4);
    assert_eq!(nested_fn.cognitive_complexity, 10);
}

#[test]
fn cognitive_complexity_handles_else_if_boolean_sequences_labels_recursion_and_nested_isolation() {
    let text = r#"export function complex(a: boolean, b: boolean, c: boolean, d: boolean, items: number[]): number {
  if (a && b && c) {
    outer: for (const item of items) {
      if (item < 0 || item > 10 && d) {
        break outer;
      } else if (item === 0) {
        continue outer;
      } else {
        try {
          items.map((inner) => {
            if (inner > 0) {
              if (inner > 1) { return inner; }
            }
            return 0;
          });
        } catch (err) {
          return complex(false, b, c, d, []);
        }
      }
    }
  }
  return 0;
}
"#;

    let found = functions("src/complex.ts", text);
    assert_eq!(found.len(), 2);
    assert_eq!(found[0].name.as_deref(), Some("complex"));
    assert_eq!(found[0].cognitive_complexity, 18);
    assert_eq!(found[1].name, None);
    assert_eq!(found[1].cognitive_complexity, 3);
}

fn declared_deps(manifests: &[(&str, &str)]) -> Vec<DeclaredDep> {
    plugin()
        .declared_dependencies(
            manifests
                .iter()
                .map(|(path, text)| manifest(path, text))
                .collect(),
        )
        .expect("declared dependencies")
}

fn expected_declared(rows: &[(&str, &str, DepKind)]) -> Vec<DeclaredDep> {
    rows.iter()
        .map(|(root, name, kind)| DeclaredDep {
            package_root: (*root).to_owned(),
            name: (*name).to_owned(),
            kind: *kind,
        })
        .collect()
}

fn resolve_with_tsconfig(tsconfig: &str, specifiers: &[&str]) -> Vec<Resolution> {
    let requests: Vec<(&str, &str)> = specifiers
        .iter()
        .map(|specifier| ("src/app.ts", *specifier))
        .collect();
    resolve_for(
        &["tsconfig.json", "src/app.ts"],
        &[("tsconfig.json", tsconfig)],
        &requests,
    )
}

const CATCH_ALL_TSCONFIG: &str =
    r#"{"compilerOptions": {"baseUrl": ".", "paths": {"*": ["./src/*"]}}}"#;

#[test]
fn node_and_bun_builtins_and_canonical_external_packages_classify_accurately() {
    let catch_all_builtins = resolve_with_tsconfig(
        CATCH_ALL_TSCONFIG,
        &[
            "node:fs",
            "node:test",
            "node:sqlite",
            "node:path/posix",
            "fs",
            "fs/promises",
            "path/posix",
            "stream/promises",
            "bun",
            "bun:sqlite",
        ],
    );
    assert_eq!(catch_all_builtins, vec![Resolution::Builtin; 10]);

    let normal_resolutions = resolve_with_tsconfig(
        r#"{"compilerOptions": {"paths": {"@/*": ["./src/*"]}}}"#,
        &[
            "node:crypto",
            "fs/promises",
            "lodash/fp/map",
            "@scope/pkg/sub/deep",
            "@scope/pkg/sub?raw#hash",
        ],
    );
    assert_eq!(
        normal_resolutions,
        vec![
            Resolution::Builtin,
            Resolution::Builtin,
            Resolution::External("lodash".to_owned()),
            Resolution::External("@scope/pkg".to_owned()),
            Resolution::External("@scope/pkg".to_owned()),
        ]
    );
}

#[test]
fn single_package_json_parses_runtime_dev_optional_and_peer_dependencies() {
    let deps = declared_deps(&[
        (
            "package.json",
            r#"{
              "name": "solo-app",
              "dependencies": {
                "react": "^18.3.0",
                "@scope/ui": "^1.0.0",
                "solo-app": "^1.0.0",
                "   ": "^1.0.0"
              },
              "devDependencies": {
                "vitest": "^2.0.0",
                "@types/node": "^22.0.0"
              },
              "optionalDependencies": {
                "fsevents": "^2.3.0"
              },
              "peerDependencies": {
                "react-dom": "^18.3.0",
                "fsevents": "^2.3.0"
              }
            }"#,
        ),
        ("tsconfig.json", r#"{"compilerOptions": {"strict": true}}"#),
        (
            "node_modules/ignored/package.json",
            r#"{"name": "ignored", "dependencies": {"leak": "1.0.0"}}"#,
        ),
        (
            "examples/demo/package.json",
            r#"{"name": "demo", "dependencies": {"non-workspace": "1.0.0"}}"#,
        ),
    ]);

    assert_eq!(
        deps,
        expected_declared(&[
            ("", "@scope/ui", DepKind::Runtime),
            ("", "react", DepKind::Runtime),
            ("", "vitest", DepKind::Dev),
            ("", "@types/node", DepKind::Optional),
            ("", "fsevents", DepKind::Optional),
            ("", "react-dom", DepKind::Optional),
            ("examples/demo", "non-workspace", DepKind::Runtime),
        ])
    );
}

#[test]
fn workspace_package_jsons_emit_per_package_deps_and_exclude_local_members() {
    let expected = expected_declared(&[
        ("", "vitest", DepKind::Dev),
        ("packages/core", "zod", DepKind::Runtime),
        ("packages/retired", "old-dep", DepKind::Runtime),
        ("packages/web", "react", DepKind::Runtime),
        ("packages/web", "tsx", DepKind::Dev),
        ("packages/web", "react-dom", DepKind::Optional),
    ]);

    for (label, root_path, root_text) in [
        (
            "npm_yarn_bun",
            "package.json",
            r#"{"workspaces": ["packages/*", "!packages/retired"], "devDependencies": {"vitest": "^2.0.0"}}"#,
        ),
        (
            "pnpm",
            "pnpm-workspace.yaml",
            "packages:\n  - 'packages/*'\n  - '!packages/retired'\n",
        ),
    ] {
        let mut manifests = vec![
            (root_path, root_text),
            (
                "packages/core/package.json",
                r#"{"name": "@acme/core", "dependencies": {"zod": "^3.23.0"}}"#,
            ),
            (
                "packages/web/package.json",
                r#"{
                  "name": "@acme/web",
                  "dependencies": {
                    "@acme/core": "^1.0.0",
                    "@acme/other": "workspace:*",
                    "react": "^18.3.0"
                  },
                  "devDependencies": {"tsx": "^4.0.0"},
                  "peerDependencies": {"react-dom": "^18.3.0"}
                }"#,
            ),
            (
                "packages/retired/package.json",
                r#"{"name": "@acme/retired", "dependencies": {"old-dep": "1.0.0"}}"#,
            ),
        ];
        if root_path == "pnpm-workspace.yaml" {
            manifests.push((
                "package.json",
                r#"{"devDependencies": {"vitest": "^2.0.0"}}"#,
            ));
        }
        assert_eq!(declared_deps(&manifests), expected, "{label}");
    }

    let polyglot = declared_deps(&[
        (
            "frontend/package.json",
            r#"{"name": "frontend", "dependencies": {"svelte": "^5.0.0"}, "devDependencies": {"vite": "^6.0.0"}}"#,
        ),
        ("broken/package.json", "{ not valid json"),
        (
            "frontend/node_modules/pkg/package.json",
            r#"{"dependencies": {"leaked": "^1.0.0"}}"#,
        ),
    ]);
    assert_eq!(
        polyglot,
        expected_declared(&[
            ("frontend", "svelte", DepKind::Runtime),
            ("frontend", "vite", DepKind::Dev),
        ])
    );
}

const MONOREPO_WORKSPACE_FILES: &[(&str, &str)] = &[
    ("pnpm-workspace.yaml", "packages:\n  - 'packages/*'\n"),
    ("package.json", r#"{"name": "monorepo", "private": true}"#),
    ("packages/index.ts", ""),
    (
        "packages/zod/package.json",
        r#"{
          "name": "zod",
          "exports": {
            ".": "./dist/index.js",
            "./v4": "./dist/v4/index.js"
          }
        }"#,
    ),
    ("packages/zod/src/index.ts", ""),
    ("packages/zod/src/v4/index.ts", ""),
    (
        "packages/pinia/package.json",
        r#"{"name": "pinia", "main": "dist/pinia.cjs"}"#,
    ),
    ("packages/pinia/src/index.ts", ""),
    ("packages/app/package.json", r#"{"name": "app"}"#),
    ("packages/app/src/main.ts", ""),
];

#[test]
fn root_workspace_package_and_unbuilt_member_exports_do_not_resolve_as_external() {
    let hono_files = [
        (
            "package.json",
            r#"{
              "name": "hono",
              "workspaces": ["adapters/*"],
              "exports": {
                ".": { "types": "./dist/types/index.d.ts", "import": "./dist/index.js" },
                "./adapter": { "types": "./dist/types/adapter/index.d.ts", "import": "./dist/adapter/index.js" },
                "./generated": { "import": "./dist/generated.js" }
              }
            }"#,
        ),
        ("src/index.ts", ""),
        ("src/adapter/index.ts", ""),
        (
            "adapters/aws-lambda/package.json",
            r#"{
              "name": "@hono/aws-lambda",
              "peerDependencies": { "hono": ">=4.13.9" },
              "devDependencies": { "hono": "workspace:*" }
            }"#,
        ),
        ("adapters/aws-lambda/src/index.ts", ""),
    ];
    for (specifier, expected) in [
        ("hono", resolved("src/index.ts")),
        ("hono/adapter", resolved("src/adapter/index.ts")),
        ("hono/generated", Resolution::Builtin),
    ] {
        assert_eq!(
            resolve_in_project(&hono_files, "adapters/aws-lambda/src/index.ts", specifier),
            expected,
            "{specifier}"
        );
    }

    for (specifier, expected) in [
        ("zod", resolved("packages/zod/src/index.ts")),
        ("zod/v4", resolved("packages/zod/src/v4/index.ts")),
        ("pinia", resolved("packages/pinia/src/index.ts")),
    ] {
        assert_eq!(
            resolve_in_project(
                MONOREPO_WORKSPACE_FILES,
                "packages/app/src/main.ts",
                specifier
            ),
            expected,
            "{specifier}"
        );
    }
}

#[test]
fn workspace_fallback_cannot_escape_the_package_directory() {
    for specifier in [
        "zod/../../pinia/src/index",
        "zod/..",
        "pinia/../../zod/src/index",
        "pinia/..",
    ] {
        let resolution = resolve_in_project(
            MONOREPO_WORKSPACE_FILES,
            "packages/app/src/main.ts",
            specifier,
        );
        assert!(
            matches!(resolution, Resolution::Unresolved(_)),
            "{specifier} escaped workspace package: {resolution:?}"
        );
    }
}

#[test]
fn cloudflare_deno_builtins_and_npm_specifiers_classify_accurately() {
    let cases = [
        ("cloudflare:workers", Resolution::Builtin),
        ("deno:kv", Resolution::Builtin),
        (
            "npm:@medley/router",
            Resolution::External("@medley/router".to_owned()),
        ),
        (
            "npm:@medley/router@1.2.3/sub",
            Resolution::External("@medley/router".to_owned()),
        ),
        ("npm:chalk@^5/foo", Resolution::External("chalk".to_owned())),
    ];
    let specifiers: Vec<&str> = cases.iter().map(|(spec, _)| *spec).collect();
    let expected: Vec<Resolution> = cases.into_iter().map(|(_, res)| res).collect();
    assert_eq!(
        resolve_with_tsconfig(CATCH_ALL_TSCONFIG, &specifiers),
        expected
    );
}

#[test]
fn type_test_files_and_folders_are_classified_as_test_source_set() {
    for path in [
        "packages/pinia/test-dts/store.test-d.ts",
        "packages/pinia/test-dts/helper.ts",
        "packages/pinia/type-tests/store.ts",
        "src/store.test-d.ts",
        "src/store.test-d.mts",
        "src/store.test-d.cts",
        "src/store.test-d.tsx",
        "src/store.spec-d.ts",
    ] {
        assert_eq!(source_set(path), SourceSet::Test, "{path}");
    }
    for path in ["src/test-d.ts", "src/.test-d.ts"] {
        assert_eq!(source_set(path), SourceSet::Production, "{path}");
    }
}

#[test]
fn framework_virtual_modules_classify_as_builtins_even_with_catch_all_tsconfig() {
    let resolutions = resolve_with_tsconfig(
        CATCH_ALL_TSCONFIG,
        &[
            "virtual:pwa-register",
            "virtual:uno.css",
            "$app/stores",
            "$env/dynamic/private",
            "$service-worker",
        ],
    );
    assert_eq!(resolutions, vec![Resolution::Builtin; 5]);

    let aliased_files = [
        (
            "tsconfig.json",
            r#"{"compilerOptions":{"paths":{"$app/*":["./src/app/*"]}}}"#,
        ),
        ("src/main.ts", ""),
        ("src/app/foo.ts", ""),
    ];
    assert_eq!(
        resolve_in_project(&aliased_files, "src/main.ts", "$app/foo"),
        resolved("src/app/foo.ts")
    );
    assert_eq!(
        resolve_in_project(&aliased_files, "src/main.ts", "$app/navigation"),
        Resolution::Builtin
    );
}

#[test]
fn pnpm_workspace_root_dev_deps_keep_dev_kind_and_resolve_as_external() {
    let manifests = [
        ("pnpm-workspace.yaml", "packages:\n  - 'packages/*'\n"),
        (
            "package.json",
            r#"{
              "name": "repo-root",
              "private": true,
              "scripts": {
                "lint": "eslint .",
                "typecheck": "tsc --noEmit"
              },
              "devDependencies": {
                "eslint": "^9.0.0",
                "typescript": "^5.7.0",
                "vitest": "^2.1.0",
                "unused-dev": "^1.0.0",
                "@types/node": "^22.0.0"
              }
            }"#,
        ),
        (
            "packages/kit/package.json",
            r#"{
              "name": "@repo/kit",
              "scripts": {
                "test": "vitest run"
              },
              "dependencies": {
                "zod": "^3.23.0"
              }
            }"#,
        ),
    ];

    let expected_root_dev = ["eslint", "typescript", "unused-dev", "vitest"]
        .into_iter()
        .map(|name| ("", name, DepKind::Dev))
        .chain([
            ("", "@types/node", DepKind::Optional),
            ("packages/kit", "zod", DepKind::Runtime),
        ])
        .collect::<Vec<_>>();
    assert_eq!(
        declared_deps(&manifests),
        expected_declared(&expected_root_dev)
    );

    let files = [
        manifests[0],
        manifests[1],
        manifests[2],
        ("packages/kit/src/index.ts", ""),
        ("packages/kit/src/index.test.ts", ""),
        ("scripts/ci.ts", ""),
    ];
    let requests = [
        ("packages/kit/src/index.test.ts", "vitest"),
        ("packages/kit/src/index.ts", "zod"),
        ("packages/kit/src/index.ts", "axios"),
        ("scripts/ci.ts", "vitest"),
    ];
    let resolved: Vec<Resolution> = requests
        .iter()
        .map(|&(importer, specifier)| resolve_in_project(&files, importer, specifier))
        .collect();
    assert_eq!(
        resolved,
        vec![
            Resolution::External("vitest".to_owned()),
            Resolution::External("zod".to_owned()),
            Resolution::External("axios".to_owned()),
            Resolution::External("vitest".to_owned()),
        ]
    );
}
