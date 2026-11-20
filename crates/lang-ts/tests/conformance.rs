use codeint_lang_ts::TypeScriptAndJavaScript;
use codeint_model::language::{Function, ImportRequest, Resolution};
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
        [function(1, 1, 12), function(5, 2, 7)]
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
    let mut resolutions = plugin().resolve_imports(&paths, vec![request]);
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

    let resolutions = plugin().resolve_imports(&paths, requests);

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
