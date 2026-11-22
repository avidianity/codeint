use std::collections::BTreeSet;
use std::fs;
use std::path::Path;
use std::process::{Command, Output};

use tempfile::TempDir;

struct Repository {
    folder: TempDir,
}

impl Repository {
    fn new() -> Repository {
        let repository = Repository {
            folder: tempfile::tempdir().expect("create a temporary folder"),
        };
        repository.git(&["init", "--quiet", "--initial-branch", "main"]);
        repository
    }

    fn path(&self) -> &Path {
        self.folder.path()
    }

    fn write(&self, relative: &str, text: &str) {
        let full = self.path().join(relative);
        if let Some(parent) = full.parent() {
            fs::create_dir_all(parent).expect("create the parent folder");
        }
        fs::write(full, text).expect("write the file");
    }

    fn write_bytes(&self, relative: &str, bytes: &[u8]) {
        fs::write(self.path().join(relative), bytes).expect("write the file");
    }

    fn git(&self, args: &[&str]) {
        let status = Command::new("git")
            .args(args)
            .current_dir(self.path())
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_AUTHOR_NAME", "Fixture")
            .env("GIT_AUTHOR_EMAIL", "fixture@example.invalid")
            .env("GIT_COMMITTER_NAME", "Fixture")
            .env("GIT_COMMITTER_EMAIL", "fixture@example.invalid")
            .status()
            .expect("run git");
        assert!(status.success(), "git {args:?} exited with {status}");
    }

    fn commit_all(&self) {
        self.git(&["add", "--all"]);
        self.git(&["commit", "--quiet", "--message", "fixture"]);
    }

    fn codeint(&self, args: &[&str]) -> Output {
        run_codeint(self.path(), args)
    }
}

fn run_codeint(directory: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_codeint"))
        .args(args)
        .current_dir(directory)
        .env("RUST_BACKTRACE", "1")
        .output()
        .expect("run codeint")
}

fn stdout(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).expect("standard output is UTF-8")
}

fn summary(output: &Output) -> String {
    stdout(output)
        .lines()
        .take_while(|line| !line.ends_with(':'))
        .map(|line| format!("{line}\n"))
        .collect()
}

fn function_with_if_statements(name: &str, if_statements: usize) -> String {
    let branches: String = (1..=if_statements)
        .map(|value| format!("  if (value === {value}) return {value};\n"))
        .collect();
    format!("export function {name}(value: number): number {{\n{branches}  return 0;\n}}\n")
}

fn assert_success(output: &Output) {
    assert_eq!(
        output.status.code(),
        Some(0),
        "codeint failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

const METRIC_INDENT: usize = 4;

fn metric_section(output: &Output, source_set: &str, metric: &str) -> String {
    let source_set_heading = format!("  {source_set}:");
    let metric_heading = format!("{}{metric}:", " ".repeat(METRIC_INDENT));
    stdout(output)
        .lines()
        .skip_while(|line| *line != source_set_heading)
        .skip_while(|line| *line != metric_heading)
        .enumerate()
        .take_while(|(index, line)| *index == 0 || indent(line) > METRIC_INDENT)
        .map(|(_, line)| {
            format!(
                "{}\n",
                with_finding_id_shown_as_a_placeholder(&line[METRIC_INDENT..])
            )
        })
        .collect()
}

const FINDING_ID_DIGITS: usize = 16;

fn with_finding_id_shown_as_a_placeholder(line: &str) -> String {
    let content = line.trim_start();
    let indent = &line[..line.len() - content.len()];
    let (before_id, rest) = match content.strip_prefix("- id: ") {
        Some(rest) => ("- id: ", rest),
        None => ("", content),
    };
    let (first_digit, id_length) = match rest.as_bytes().get(FINDING_ID_DIGITS + 1) {
        Some(b'"') if rest.starts_with('"') => (1, FINDING_ID_DIGITS + 2),
        _ => (0, FINDING_ID_DIGITS),
    };
    let starts_with_finding_id = rest
        .get(first_digit..first_digit + FINDING_ID_DIGITS)
        .is_some_and(|digits| {
            digits
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        })
        && matches!(rest.as_bytes().get(id_length), None | Some(b','));
    if starts_with_finding_id {
        format!("{indent}{before_id}<id>{}", &rest[id_length..])
    } else {
        line.to_owned()
    }
}

fn production_section(output: &Output, metric: &str) -> String {
    metric_section(output, "production", metric)
}

fn indent(line: &str) -> usize {
    line.len() - line.trim_start().len()
}

fn source_set_sizes(output: &Output) -> Vec<String> {
    let mut sizes = Vec::new();
    for line in stdout(output).lines() {
        match indent(line) {
            2 => sizes.push(line.trim().to_owned()),
            METRIC_INDENT
                if line.trim_start().starts_with("files")
                    || line.trim_start().starts_with("source_lines") =>
            {
                if let Some(last) = sizes.last_mut() {
                    last.push(' ');
                    last.push_str(line.trim());
                }
            }
            _ => {}
        }
    }
    sizes
}

fn report_without_findings(files: usize, source_lines: u64) -> String {
    format!(
        "files: {files}\n\
         source_lines: {source_lines}\n\
         source_sets:\n  \
         production:\n    \
         files: {files}\n    \
         source_lines: {source_lines}\n    \
         erosion:\n      \
         ratio: 0\n      \
         reference_range:\n        \
         low: 0.352\n        \
         high: 0.6\n        \
         source: \"codeint panel of 20 TypeScript and JavaScript repositories, ADR 0008, codeint 0.1.0\"\n      \
         eroded_function_mass: 0\n      \
         total_function_mass: 0\n      \
         findings: []\n    \
         duplication:\n      \
         ratio: 0\n      \
         reference_range:\n        \
         low: 0.017\n        \
         high: 0.346\n        \
         source: \"codeint panel of 20 TypeScript and JavaScript repositories, ADR 0008, codeint 0.1.0\"\n      \
         clone_lines: 0\n      \
         clone_groups: []\n    \
         verbosity:\n      \
         ratio: 0\n      \
         reference_range:\n        \
         low: 0.018\n        \
         high: 0.347\n        \
         source: \"codeint panel of 20 TypeScript and JavaScript repositories, ADR 0008, codeint 0.1.0\"\n      \
         verbose_lines: 0\n      \
         flagged_lines: 0\n      \
         clone_lines: 0\n      \
         findings: []\n    \
         import_cycles:\n      \
         ratio: 0\n      \
         reference_range:\n        \
         low: 0\n        \
         high: 0.308\n        \
         source: \"codeint panel of 20 TypeScript and JavaScript repositories, ADR 0008, codeint 0.1.0\"\n      \
         modules_in_import_cycles: 0\n      \
         modules: {files}\n      \
         findings: []\n      \
         unresolved_imports:\n        \
         count: 0\n        \
         imports: []\n"
    )
}

const SIGN_WITH_AN_ELSE_AFTER_RETURN: &str = "export function sign(value: number): string {
  if (value < 0) {
    return 'negative';
  } else {
    return 'positive';
  }
}
";

#[test]
fn check_reports_verbosity_as_the_share_of_source_lines_that_are_flagged_and_names_each_rule() {
    let repository = Repository::new();
    repository.write("src/sign.ts", SIGN_WITH_AN_ELSE_AFTER_RETURN);
    repository.write(
        "src/size.ts",
        "export function size(count: number): string {
  if (count === 0) {
    return 'none';
  } else {
    if (count === 1) {
      return 'one';
    } else {
      return 'many';
    }
  }
}
",
    );
    repository.write(
        "src/main.ts",
        "import { sign } from './sign';\nconsole.log(sign(-1));\n",
    );
    repository.commit_all();

    let output = repository.codeint(&["check"]);

    assert_success(&output);
    assert_eq!(summary(&output), "files: 3\nsource_lines: 20\n");
    assert_eq!(
        production_section(&output, "verbosity"),
        "verbosity:\n  \
         ratio: 0.15\n  \
         reference_range:\n    \
         low: 0.018\n    \
         high: 0.347\n    \
         source: \"codeint panel of 20 TypeScript and JavaScript repositories, ADR 0008, codeint 0.1.0\"\n  \
         verbose_lines: 3\n  \
         flagged_lines: 3\n  \
         clone_lines: 0\n  \
         findings[3]{id,path,line,rule}:\n    \
         <id>,src/sign.ts,4,else-after-return\n    \
         <id>,src/size.ts,4,else-after-return\n    \
         <id>,src/size.ts,7,else-after-return\n"
    );
}

const PLURAL_LABEL: &str = concat!(
    "export function label(count: number, unit: string, plural: string, prefix: string): string {\n",
    "  if (Number(count) === 0) {\n",
    "    return prefix.trim() + ' no ' + plural + ' left';\n",
    "  } else {\n",
    "    return prefix.trim() + ' ' + String(count) + ' ' + (count === 1 ? unit : plural) + ' left';\n",
    "  }\n",
    "}\n",
);

const SIGN: &str = concat!(
    "export function sign(value: number): string {\n",
    "  if (value < 0) {\n",
    "    return 'negative';\n",
    "  } else {\n",
    "    return 'positive';\n",
    "  }\n",
    "}\n",
);

#[test]
fn verbosity_counts_a_line_that_is_both_flagged_and_a_clone_line_once() {
    let repository = Repository::new();
    repository.write("src/label.ts", PLURAL_LABEL);
    repository.write("src/text.ts", &format!("{PLURAL_LABEL}{SIGN}"));
    repository.commit_all();

    let output = repository.codeint(&["check"]);

    assert_success(&output);
    assert_eq!(summary(&output), "files: 2\nsource_lines: 21\n");
    assert_eq!(
        production_section(&output, "verbosity"),
        concat!(
            "verbosity:\n",
            "  ratio: 0.7143\n",
            "  reference_range:\n",
            "    low: 0.018\n",
            "    high: 0.347\n",
            "    source: \"codeint panel of 20 TypeScript and JavaScript repositories, ADR 0008, codeint 0.1.0\"\n",
            "  verbose_lines: 15\n",
            "  flagged_lines: 3\n",
            "  clone_lines: 14\n",
            "  findings[3]{id,path,line,rule}:\n",
            "    <id>,src/label.ts,4,else-after-return\n",
            "    <id>,src/text.ts,4,else-after-return\n",
            "    <id>,src/text.ts,11,else-after-return\n",
        )
    );
}

#[test]
fn check_reports_the_file_count_and_summed_source_lines_of_two_typescript_files() {
    let repository = Repository::new();
    repository.write(
        "src/greet.ts",
        "export function greet(name: string): string {\n  return `Hello, ${name}`;\n}\n",
    );
    repository.write(
        "src/main.ts",
        "import { greet } from './greet';\nconsole.log(greet('codeint'));\n",
    );
    repository.commit_all();

    let output = repository.codeint(&["check"]);

    assert_success(&output);
    assert_eq!(summary(&output), "files: 2\nsource_lines: 5\n");
}

#[test]
fn blank_lines_and_comment_only_lines_are_not_source_lines() {
    let repository = Repository::new();
    repository.write(
        "src/commented.ts",
        concat!(
            "// a line comment\n",
            "\n",
            "/**\n",
            " * A block comment\n",
            " * that spans lines.\n",
            " */\n",
            "export const url = 'https://example.invalid';\n",
            "   \t \n",
            "export const total = 1 + 2; // a trailing comment\n",
            "/* a */ /* b */\n",
            "export const quotient = 6 /* inline */ / 3;\n",
            "export const text = `first\n",
            "/* inside a template */ last`;\n",
            "\r\n",
        ),
    );
    repository.commit_all();

    let output = repository.codeint(&["check"]);

    assert_success(&output);
    assert_eq!(summary(&output), "files: 1\nsource_lines: 5\n");
}

#[test]
fn every_typescript_and_javascript_extension_is_measured_and_other_files_are_not() {
    let repository = Repository::new();
    for extension in ["ts", "tsx", "mts", "cts", "js", "jsx", "mjs", "cjs"] {
        repository.write(&format!("src/file.{extension}"), "const one = 1;\n");
    }
    repository.write("README.md", "# Fixture\n");
    repository.write("src/data.json", "{\"one\": 1}\n");
    repository.write("scripts/tool.cob", "DISPLAY 'codeint'.\n");
    repository.commit_all();

    let output = repository.codeint(&["check"]);

    assert_success(&output);
    assert_eq!(summary(&output), "files: 8\nsource_lines: 8\n");
}

#[test]
fn files_git_ignores_are_not_measured_and_untracked_files_are() {
    let repository = Repository::new();
    repository.write(".gitignore", "dist/\n*.generated.ts\n");
    repository.write("src/main.ts", "export const main = 1;\n");
    repository.write(
        "dist/main.js",
        "export const main = 1;\nexport const built = 2;\n",
    );
    repository.write("src/schema.generated.ts", "export const schema = 1;\n");
    repository.commit_all();
    repository.write(
        "src/draft/next.ts",
        "export const draft = 1;\nexport const more = 2;\n",
    );

    let output = repository.codeint(&["check"]);

    assert_success(&output);
    assert_eq!(summary(&output), "files: 2\nsource_lines: 3\n");
}

#[test]
fn version_prints_the_crate_version() {
    let folder = tempfile::tempdir().expect("create a temporary folder");

    let output = run_codeint(folder.path(), &["--version"]);

    assert_success(&output);
    assert_eq!(
        stdout(&output),
        format!("codeint {}\n", env!("CARGO_PKG_VERSION"))
    );
}

#[test]
fn check_outside_a_git_repository_exits_with_an_operational_error() {
    let folder = tempfile::tempdir().expect("create a temporary folder");

    let output = run_codeint(folder.path(), &["check"]);

    let canonical = fs::canonicalize(folder.path()).expect("resolve the temporary folder");
    assert_operational_error(&output, &not_in_a_repository_message(&canonical));
}

#[test]
fn a_file_that_does_not_parse_is_skipped_and_listed_with_the_parse_error() {
    let repository = Repository::new();
    repository.write("src/broken.ts", "export const ok = 1;\nexport const = ;\n");
    repository.write(
        "src/main.ts",
        "export const main = 1;\nexport const more = 2;\n",
    );
    repository.commit_all();

    let output = repository.codeint(&["check"]);

    assert_success(&output);
    assert_eq!(
        stdout(&output),
        report_without_findings(1, 2)
            + "skipped[1]{path,reason}:\n  \
             src/broken.ts,\"cannot parse: Unexpected token at line 2, column 14\"\n"
    );
}

#[test]
fn javascript_files_with_jsx_count_toward_erosion_and_duplication_while_flow_is_skipped() {
    let component = concat!(
        "export function StatusCard({ items, status, label, fallback }) {\n",
        "  let total = 0;\n",
        "  for (const item of items) {\n",
        "    if (item.active && item.visible && item.weight > 0) total += item.weight;\n",
        "  }\n",
        "  while (total > 100) total -= 50;\n",
        "  switch (status) {\n",
        "    case 'warn':\n",
        "      return <aside role=\"alert\">{label ?? fallback}</aside>;\n",
        "    case 'error':\n",
        "      return <strong data-total={total || 1}>{label}</strong>;\n",
        "    default:\n",
        "      return <section>{total > 0 ? <span>{label}</span> : <em>{fallback}</em>}</section>;\n",
        "  }\n",
        "}\n",
    );
    let repository = Repository::new();
    repository.write("src/first.js", component);
    repository.write("src/second.js", component);
    repository.write(
        "src/flow.js",
        "// @flow\nexport function greet(name: string): string {\n  return name;\n}\n",
    );
    repository.commit_all();

    let output = repository.codeint(&["check"]);

    assert_success(&output);
    assert_eq!(summary(&output), "files: 2\nsource_lines: 30\n");
    assert_eq!(
        erosion_findings_lines(&output),
        vec![
            erosion_findings_header(2),
            "    <id>,src/first.js,1".to_owned(),
            "    <id>,src/second.js,1".to_owned(),
        ]
    );
    let duplication = production_section(&output, "duplication");
    assert!(
        duplication.contains("clone_lines: 30\n")
            && duplication.contains("src/first.js,1,15\n")
            && duplication.contains("src/second.js,1,15\n"),
        "unexpected duplication section:\n{duplication}"
    );
    assert!(
        stdout(&output).contains(
            "skipped[1]{path,reason}:\n  \
             src/flow.js,\"cannot parse: Flow is not supported at line 1, column 1\"\n"
        ),
        "unexpected output:\n{}",
        stdout(&output)
    );
}

#[test]
fn a_typescript_declaration_file_is_listed_as_declaration_only_and_adds_no_source_lines() {
    let repository = Repository::new();
    repository.write("src/main.ts", "export const main = 1;\n");
    repository.write(
        "types/globals.d.ts",
        "declare const version: string;\ndeclare function greet(name: string): string;\n",
    );
    repository.write(
        "types/module.d.mts",
        "export declare const answer: number;\n",
    );
    repository.write("types/legacy.d.cts", "declare const legacy: number;\n");
    repository.commit_all();

    let output = repository.codeint(&["check"]);

    assert_success(&output);
    assert_eq!(
        stdout(&output),
        report_without_findings(1, 1)
            + "declaration_only[3]: types/globals.d.ts,types/legacy.d.cts,types/module.d.mts\n"
    );
}

#[test]
fn erosion_reports_the_ratio_its_numerator_and_each_eroded_function() {
    let repository = Repository::new();
    repository.write(
        "src/mixed.ts",
        concat!(
            "export function eroded(value: number): number {\n",
            "  const doubled = value * 2;\n",
            "  if (doubled === 2) return 1;\n",
            "  if (doubled === 4) return 2;\n",
            "  if (doubled === 6) return 3;\n",
            "  if (doubled === 8) return 4;\n",
            "  if (doubled === 10) return 5;\n",
            "  for (const step of [1, 2]) value += step;\n",
            "  for (const key in { a: 1 }) value += key.length;\n",
            "  for (let index = 0; index < 2; index++) value += index;\n",
            "  while (value > 100) value -= 100;\n",
            "  do value += 1; while (value < 0);\n",
            "  if (doubled === 12) return 6;\n",
            "  const tripled = value * 3;\n",
            "  return tripled;\n",
            "}\n",
            "\n",
            "export function small(values: number[]): number {\n",
            "  let total = 0;\n",
            "  const count = values.length;\n",
            "  for (const value of values) {\n",
            "    total += value;\n",
            "  }\n",
            "  total += count;\n",
            "  return total;\n",
            "}\n",
        ),
    );
    repository.commit_all();

    let output = repository.codeint(&["check"]);

    assert_success(&output);
    assert_eq!(summary(&output), "files: 1\nsource_lines: 25\n");
    assert_eq!(
        production_section(&output, "erosion"),
        concat!(
            "erosion:\n",
            "  ratio: 0.8889\n",
            "  reference_range:\n",
            "    low: 0.352\n",
            "    high: 0.6\n",
            "    source: \"codeint panel of 20 TypeScript and JavaScript repositories, ADR 0008, codeint 0.1.0\"\n",
            "  eroded_function_mass: 48\n",
            "  total_function_mass: 54\n",
            "  findings[1]{id,path,line,cyclomatic_complexity,source_lines,function_mass}:\n",
            "    <id>,src/mixed.ts,1,12,16,48\n",
        )
    );
}

#[test]
fn a_function_with_10_if_statements_is_eroded_and_one_with_9_is_not() {
    let repository = Repository::new();
    repository.write("src/nine.ts", &function_with_if_statements("nine", 9));
    repository.write("src/ten.ts", &function_with_if_statements("ten", 10));
    repository.write("src/eleven.ts", &function_with_if_statements("eleven", 11));
    repository.commit_all();

    let output = repository.codeint(&["check"]);

    assert_success(&output);
    assert_eq!(summary(&output), "files: 3\nsource_lines: 39\n");
    assert_eq!(
        production_section(&output, "erosion"),
        concat!(
            "erosion:\n",
            "  ratio: 0.7094\n",
            "  reference_range:\n",
            "    low: 0.352\n",
            "    high: 0.6\n",
            "    source: \"codeint panel of 20 TypeScript and JavaScript repositories, ADR 0008, codeint 0.1.0\"\n",
            "  eroded_function_mass: 84.561\n",
            "  total_function_mass: 119.202\n",
            "  findings[2]{id,path,line,cyclomatic_complexity,source_lines,function_mass}:\n",
            "    <id>,src/eleven.ts,1,12,14,44.8999\n",
            "    <id>,src/ten.ts,1,11,13,39.6611\n",
        )
    );
}

#[test]
fn a_repository_without_functions_reports_erosion_0() {
    let repository = Repository::new();
    repository.write("src/constants.ts", "export const one = 1;\n");
    repository.commit_all();

    let output = repository.codeint(&["check"]);

    assert_success(&output);
    assert_eq!(summary(&output), "files: 1\nsource_lines: 1\n");
    assert_eq!(
        production_section(&output, "erosion"),
        concat!(
            "erosion:\n",
            "  ratio: 0\n",
            "  reference_range:\n",
            "    low: 0.352\n",
            "    high: 0.6\n",
            "    source: \"codeint panel of 20 TypeScript and JavaScript repositories, ADR 0008, codeint 0.1.0\"\n",
            "  eroded_function_mass: 0\n",
            "  total_function_mass: 0\n",
            "  findings: []\n",
        )
    );
}

#[test]
fn the_typescript_and_javascript_report_shows_the_approved_reference_ranges_with_their_source_label()
 {
    let repository = Repository::new();
    repository.write("src/eleven.ts", &function_with_if_statements("eleven", 11));
    repository.write("src/sum.js", "export const sum = (a, b) => a + b;\n");
    repository.write("src/sum.test.js", "export const one = 1;\n");
    repository.commit_all();

    let output = repository.codeint(&["check"]);

    assert_success(&output);
    let source =
        "codeint panel of 20 TypeScript and JavaScript repositories, ADR 0008, codeint 0.1.0";
    for (metric, low, high) in [
        ("erosion", "0.352", "0.6"),
        ("duplication", "0.017", "0.346"),
        ("verbosity", "0.018", "0.347"),
        ("import_cycles", "0", "0.308"),
    ] {
        let expected = format!(
            "  reference_range:\n    low: {low}\n    high: {high}\n    source: \"{source}\"\n"
        );
        let section = production_section(&output, metric);
        assert!(
            section.contains(&expected),
            "missing Reference range for {metric} in:\n{section}"
        );
    }
}

#[test]
fn a_callback_with_11_if_statements_is_the_finding_and_not_its_parent() {
    let repository = Repository::new();
    let branches: String = (1..=11)
        .map(|value| format!("    if (row === {value}) return;\n"))
        .collect();
    repository.write(
        "src/outer.ts",
        &format!(
            "export function outer(rows: number[]): void {{\n  rows.forEach((row) => {{\n{branches}  }});\n}}\n"
        ),
    );
    repository.commit_all();

    let output = repository.codeint(&["check"]);

    assert_success(&output);
    assert_eq!(summary(&output), "files: 1\nsource_lines: 15\n");
    assert_eq!(
        production_section(&output, "erosion"),
        concat!(
            "erosion:\n",
            "  ratio: 0.9683\n",
            "  reference_range:\n",
            "    low: 0.352\n",
            "    high: 0.6\n",
            "    source: \"codeint panel of 20 TypeScript and JavaScript repositories, ADR 0008, codeint 0.1.0\"\n",
            "  eroded_function_mass: 43.2666\n",
            "  total_function_mass: 44.6808\n",
            "  findings[1]{id,path,line,cyclomatic_complexity,source_lines,function_mass}:\n",
            "    <id>,src/outer.ts,2,12,13,43.2666\n",
        )
    );
}

fn write_eroded_functions_with_more_if_statements_in_later_files(
    repository: &Repository,
    count: usize,
) {
    for index in 1..=count {
        repository.write(
            &format!("src/f{index:02}.ts"),
            &function_with_if_statements(&format!("f{index:02}"), 9 + index),
        );
    }
}

fn erosion_findings_header(count: usize) -> String {
    format!("  findings[{count}]{{id,path,line,cyclomatic_complexity,source_lines,function_mass}}:")
}

fn erosion_findings_lines(output: &Output) -> Vec<String> {
    production_section(output, "erosion")
        .lines()
        .skip_while(|line| !line.starts_with("  findings"))
        .map(|line| match line.strip_prefix("    ") {
            Some(row) => format!(
                "    {}",
                row.splitn(4, ',').take(3).collect::<Vec<_>>().join(",")
            ),
            None => line.to_string(),
        })
        .collect()
}

#[test]
fn the_report_lists_the_20_eroded_functions_of_most_function_mass_and_says_how_many_it_left_out() {
    let repository = Repository::new();
    write_eroded_functions_with_more_if_statements_in_later_files(&repository, 25);
    repository.commit_all();

    let output = repository.codeint(&["check"]);

    assert_success(&output);
    let mut expected = vec![erosion_findings_header(20)];
    expected.extend(
        (6..=25)
            .rev()
            .map(|index| format!("    <id>,src/f{index:02}.ts,1")),
    );
    expected.push("  findings_left_out: 5".to_string());
    assert_eq!(erosion_findings_lines(&output), expected);
}

#[test]
fn all_lists_every_eroded_function_and_leaves_none_out() {
    let repository = Repository::new();
    write_eroded_functions_with_more_if_statements_in_later_files(&repository, 25);
    repository.commit_all();

    let output = repository.codeint(&["check", "--all"]);

    assert_success(&output);
    let mut expected = vec![erosion_findings_header(25)];
    expected.extend(
        (1..=25)
            .rev()
            .map(|index| format!("    <id>,src/f{index:02}.ts,1")),
    );
    assert_eq!(erosion_findings_lines(&output), expected);
}

#[test]
fn eroded_functions_of_equal_function_mass_are_listed_by_path_then_line_and_the_last_are_left_out()
{
    let repository = Repository::new();
    let eroded = function_with_if_statements("eroded", 10);
    for index in (1..=20).rev() {
        repository.write(&format!("src/t{index:02}.ts"), &eroded);
    }
    repository.write(
        "src/t00.ts",
        &format!("{eroded}{}", function_with_if_statements("twin", 10)),
    );
    repository.commit_all();

    let output = repository.codeint(&["check"]);

    assert_success(&output);
    let mut expected = vec![
        erosion_findings_header(20),
        "    <id>,src/t00.ts,1".to_string(),
        "    <id>,src/t00.ts,14".to_string(),
    ];
    expected.extend((1..=18).map(|index| format!("    <id>,src/t{index:02}.ts,1")));
    expected.push("  findings_left_out: 2".to_string());
    assert_eq!(erosion_findings_lines(&output), expected);
}

#[test]
fn every_kind_of_decision_point_counts_toward_erosion() {
    let repository = Repository::new();
    repository.write(
        "src/route.ts",
        concat!(
            "export function route(input?: { kind?: string; size?: number }, fallback = 'none'): string {\n",
            "  const kind = input?.kind ?? fallback;\n",
            "  const size = input?.size || 0;\n",
            "  try {\n",
            "    switch (kind) {\n",
            "      case 'small':\n",
            "        return size > 0 && size < 10 ? 'tiny' : 'small';\n",
            "      case 'large':\n",
            "        return 'large';\n",
            "      default:\n",
            "        return fallback;\n",
            "    }\n",
            "  } catch {\n",
            "    return 'error';\n",
            "  } finally {\n",
            "    console.log(kind);\n",
            "  }\n",
            "}\n",
        ),
    );
    repository.commit_all();

    let output = repository.codeint(&["check"]);

    assert_success(&output);
    assert_eq!(summary(&output), "files: 1\nsource_lines: 18\n");
    assert_eq!(
        production_section(&output, "erosion"),
        concat!(
            "erosion:\n",
            "  ratio: 1\n",
            "  reference_range:\n",
            "    low: 0.352\n",
            "    high: 0.6\n",
            "    source: \"codeint panel of 20 TypeScript and JavaScript repositories, ADR 0008, codeint 0.1.0\"\n",
            "  eroded_function_mass: 46.669\n",
            "  total_function_mass: 46.669\n",
            "  findings[1]{id,path,line,cyclomatic_complexity,source_lines,function_mass}:\n",
            "    <id>,src/route.ts,1,11,18,46.669\n",
        )
    );
}

#[test]
fn functions_of_test_files_count_toward_the_test_source_set_only() {
    let repository = Repository::new();
    repository.write("src/nine.ts", &function_with_if_statements("nine", 9));
    repository.write("src/ten.ts", &function_with_if_statements("ten", 10));
    repository.write(
        "src/eleven.test.ts",
        &function_with_if_statements("eleven", 11),
    );
    repository.write("src/__tests__/setup.ts", "export const ready = true;\n");
    repository.commit_all();

    let output = repository.codeint(&["check"]);

    assert_success(&output);
    assert_eq!(summary(&output), "files: 4\nsource_lines: 40\n");
    assert_eq!(
        source_set_sizes(&output),
        [
            "production: files: 2 source_lines: 25",
            "test: files: 2 source_lines: 15",
        ]
    );
    assert_eq!(
        metric_section(&output, "production", "erosion"),
        concat!(
            "erosion:\n",
            "  ratio: 0.5338\n",
            "  reference_range:\n",
            "    low: 0.352\n",
            "    high: 0.6\n",
            "    source: \"codeint panel of 20 TypeScript and JavaScript repositories, ADR 0008, codeint 0.1.0\"\n",
            "  eroded_function_mass: 39.6611\n",
            "  total_function_mass: 74.3021\n",
            "  findings[1]{id,path,line,cyclomatic_complexity,source_lines,function_mass}:\n",
            "    <id>,src/ten.ts,1,11,13,39.6611\n",
        )
    );
    assert_eq!(
        metric_section(&output, "test", "erosion"),
        concat!(
            "erosion:\n",
            "  ratio: 1\n",
            "  reference_range:\n",
            "    low: 0.352\n",
            "    high: 0.6\n",
            "    source: \"codeint panel of 20 TypeScript and JavaScript repositories, ADR 0008, codeint 0.1.0\"\n",
            "  eroded_function_mass: 44.8999\n",
            "  total_function_mass: 44.8999\n",
            "  findings[1]{id,path,line,cyclomatic_complexity,source_lines,function_mass}:\n",
            "    <id>,src/eleven.test.ts,1,12,14,44.8999\n",
        )
    );
}

#[test]
fn a_file_under_a_vendor_or_third_party_folder_at_any_depth_is_vendored() {
    let repository = Repository::new();
    for path in [
        "vendor/left-pad.js",
        "vendor/lodash/lib/chunk.ts",
        "packages/app/vendor/deep/a/b/c/d.ts",
        "third_party/parser.ts",
        "packages/app/third_party/zlib/inflate.mjs",
        "vendor/lodash/chunk.test.ts",
        "vendor/__tests__/setup.ts",
    ] {
        repository.write(path, "export const vendored = 1;\n");
    }
    for path in [
        "src/vendor.ts",
        "src/third_party.ts",
        "vendors/lib.ts",
        "src/my_vendor/lib.ts",
        "src/third-party/lib.ts",
        "src/Vendor/lib.ts",
    ] {
        repository.write(path, "export const own = 1;\nexport const code = 2;\n");
    }
    repository.commit_all();

    let output = repository.codeint(&["check"]);

    assert_success(&output);
    assert_eq!(
        source_set_sizes(&output),
        [
            "production: files: 6 source_lines: 12",
            "vendored: files: 7 source_lines: 7",
        ]
    );
}

fn source_set_report(output: &Output, source_set: &str) -> String {
    let heading = format!("  {source_set}:");
    stdout(output)
        .lines()
        .skip_while(|line| *line != heading)
        .enumerate()
        .take_while(|(index, line)| *index == 0 || indent(line) > 2)
        .map(|(_, line)| format!("{line}\n"))
        .collect()
}

#[test]
fn vendored_files_do_not_change_production_metrics() {
    let sign = "export function sign(value: number): string {
  if (value < 0) {
    return 'negative';
  } else {
    return 'positive';
  }
}
";
    let repository = Repository::new();
    repository.write("src/nine.ts", &function_with_if_statements("nine", 9));
    repository.write("src/ten.ts", &function_with_if_statements("ten", 10));
    repository.write("src/sign.ts", sign);
    repository.commit_all();
    let without_vendored = repository.codeint(&["check"]);
    assert_success(&without_vendored);

    repository.write("vendor/ten.ts", &function_with_if_statements("ten", 10));
    repository.write(
        "third_party/eleven.ts",
        &function_with_if_statements("eleven", 11),
    );
    repository.write("vendor/lib/sign.ts", sign);
    repository.commit_all();
    let with_vendored = repository.codeint(&["check"]);

    assert_success(&with_vendored);
    let production = source_set_report(&with_vendored, "production");
    assert!(
        production.contains("src/ten.ts"),
        "unexpected report: {}",
        stdout(&with_vendored)
    );
    assert_eq!(
        production,
        source_set_report(&without_vendored, "production")
    );
    assert_eq!(
        source_set_sizes(&with_vendored),
        [
            "production: files: 3 source_lines: 32",
            "vendored: files: 3 source_lines: 34",
        ]
    );
}

#[test]
fn a_source_set_without_files_is_left_out_of_the_report() {
    let repository = Repository::new();
    repository.write(
        "src/greet.spec.ts",
        "export function greet(): string {\n  return 'Hello';\n}\n",
    );
    repository.write("README.md", "# Fixture\n");
    repository.commit_all();

    let output = repository.codeint(&["check"]);

    assert_success(&output);
    assert_eq!(
        source_set_sizes(&output),
        ["test: files: 1 source_lines: 3"]
    );
    assert_eq!(
        metric_section(&output, "test", "erosion"),
        concat!(
            "erosion:\n",
            "  ratio: 0\n",
            "  reference_range:\n",
            "    low: 0.352\n",
            "    high: 0.6\n",
            "    source: \"codeint panel of 20 TypeScript and JavaScript repositories, ADR 0008, codeint 0.1.0\"\n",
            "  eroded_function_mass: 0\n",
            "  total_function_mass: 1.7321\n",
            "  findings: []\n",
        )
    );
}

#[test]
fn a_repository_without_measured_files_reports_no_source_set() {
    let repository = Repository::new();
    repository.write("README.md", "# Fixture\n");
    repository.commit_all();

    let output = repository.codeint(&["check"]);

    assert_success(&output);
    assert_eq!(stdout(&output), "files: 0\nsource_lines: 0\nsource_sets:\n");
}

fn not_in_a_repository_message(folder: &Path) -> String {
    format!(
        "codeint: {} is not inside a Git repository\n\
         Run codeint inside a Git repository, or pass a folder inside one: codeint check <path>\n",
        folder.display()
    )
}

fn assert_operational_error(output: &Output, expected_stderr: &str) {
    assert_eq!(output.status.code(), Some(2));
    assert!(
        stdout(output).is_empty(),
        "unexpected output: {}",
        stdout(output)
    );
    assert_eq!(String::from_utf8_lossy(&output.stderr), expected_stderr);
}

#[test]
fn check_of_a_path_that_does_not_exist_names_the_path_and_exits_2() {
    let repository = Repository::new();
    repository.write("src/main.ts", "export const main = 1;\n");
    repository.commit_all();

    let output = repository.codeint(&["check", "src/missing"]);

    assert_operational_error(
        &output,
        "codeint: src/missing does not exist\n\
         Pass a folder that exists, or leave the path out to measure the current folder.\n",
    );
}

#[test]
fn check_of_a_file_path_says_to_pass_a_folder_and_exits_2() {
    let repository = Repository::new();
    repository.write("src/main.ts", "export const main = 1;\n");
    repository.commit_all();

    let output = repository.codeint(&["check", "src/main.ts"]);

    assert_operational_error(
        &output,
        "codeint: src/main.ts is not a folder\n\
         Pass a folder, not a file; codeint measures the whole working tree that contains it.\n",
    );
}

#[test]
fn check_of_a_subfolder_measures_the_whole_working_tree() {
    let repository = Repository::new();
    repository.write("src/main.ts", "export const main = 1;\n");
    repository.write(
        "lib/util.ts",
        "export const util = 1;\nexport const more = 2;\n",
    );
    repository.commit_all();

    let output = repository.codeint(&["check", "src"]);

    assert_success(&output);
    assert_eq!(summary(&output), "files: 2\nsource_lines: 3\n");
}

#[test]
fn check_of_a_folder_outside_a_git_repository_names_the_folder_and_exits_2() {
    let repository = Repository::new();
    let outside = tempfile::tempdir().expect("create a temporary folder");
    let canonical = fs::canonicalize(outside.path()).expect("resolve the temporary folder");

    let output = repository.codeint(&["check", &canonical.display().to_string()]);

    assert_operational_error(&output, &not_in_a_repository_message(&canonical));
}

#[test]
fn an_unknown_option_prints_the_usage_error_and_exits_2() {
    let repository = Repository::new();

    let output = repository.codeint(&["check", "--unknown"]);

    assert_operational_error(
        &output,
        "error: unexpected argument '--unknown' found\n\
         \n\
         \x20 tip: to pass '--unknown' as a value, use '-- --unknown'\n\
         \n\
         Usage: codeint check [OPTIONS] [PATH]\n\
         \n\
         For more information, try '--help'.\n",
    );
}

#[test]
fn an_unknown_format_names_the_formats_codeint_prints_and_exits_2() {
    let repository = Repository::new();

    let output = repository.codeint(&["check", "--format", "yaml"]);

    assert_operational_error(
        &output,
        "error: invalid value 'yaml' for '--format <FORMAT>'\n\
         \x20 [possible values: toon, json, sarif, markdown]\n\
         \n\
         For more information, try '--help'.\n",
    );
}

#[test]
fn an_unknown_command_prints_the_usage_error_and_exits_2() {
    let repository = Repository::new();

    let output = repository.codeint(&["measure"]);

    assert_operational_error(
        &output,
        "error: unrecognized subcommand 'measure'\n\
         \n\
         Usage: codeint <COMMAND>\n\
         \n\
         For more information, try '--help'.\n",
    );
}

#[cfg(unix)]
fn run_codeint_with_closed_stdout(directory: &Path, args: &[&str]) -> Output {
    let script = concat!(
        "import os, subprocess, sys\n",
        "reader, writer = os.pipe()\n",
        "os.close(reader)\n",
        "try:\n",
        "    completed = subprocess.run(sys.argv[1:], stdout=writer, stderr=subprocess.PIPE)\n",
        "finally:\n",
        "    os.close(writer)\n",
        "sys.stderr.buffer.write(completed.stderr)\n",
        "sys.exit(completed.returncode)\n",
    );
    Command::new("python3")
        .args(["-c", script])
        .arg(env!("CARGO_BIN_EXE_codeint"))
        .args(args)
        .current_dir(directory)
        .env("RUST_BACKTRACE", "1")
        .output()
        .expect("run codeint with closed standard output")
}

#[cfg(not(unix))]
fn run_codeint_with_closed_stdout(directory: &Path, args: &[&str]) -> Output {
    let (reader, writer) = std::io::pipe().expect("create a pipe");
    drop(reader);
    Command::new(env!("CARGO_BIN_EXE_codeint"))
        .args(args)
        .current_dir(directory)
        .env("RUST_BACKTRACE", "1")
        .stdout(writer)
        .output()
        .expect("run codeint with closed standard output")
}

#[test]
fn a_closed_standard_output_is_an_operational_error_and_not_a_panic() {
    let repository = Repository::new();
    repository.write("src/main.ts", "export const main = 1;\n");
    repository.commit_all();

    let output = run_codeint_with_closed_stdout(repository.path(), &["check"]);

    assert_eq!(output.status.code(), Some(2));
    let message = String::from_utf8_lossy(&output.stderr);
    assert!(
        message.starts_with("codeint: cannot write the report to standard output: ")
            && message.ends_with("\nKeep standard output open until codeint exits.\n"),
        "unexpected message: {message}"
    );
}

#[test]
fn output_writes_exactly_the_standard_output_report_to_the_file_and_prints_nothing() {
    let repository = Repository::new();
    repository.write(
        "src/main.ts",
        "export const main = 1;\nexport const more = 2;\n",
    );
    repository.commit_all();
    let printed = repository.codeint(&["check"]);
    assert_success(&printed);

    let output = repository.codeint(&["check", "--output", "report.toon"]);

    assert_success(&output);
    assert!(
        stdout(&output).is_empty(),
        "unexpected output: {}",
        stdout(&output)
    );
    assert!(output.stderr.is_empty());
    let saved = fs::read(repository.path().join("report.toon")).expect("read the report file");
    assert_eq!(saved, printed.stdout);
    assert_eq!(saved, report_without_findings(1, 2).as_bytes());
}

#[test]
fn output_replaces_an_existing_file() {
    let repository = Repository::new();
    repository.write("src/main.ts", "export const main = 1;\n");
    repository.commit_all();
    fs::write(
        repository.path().join("report.toon"),
        "an older and longer report\nthat spans two lines\n",
    )
    .expect("write the older report");

    let output = repository.codeint(&["check", "--output", "report.toon"]);

    assert_success(&output);
    assert_eq!(
        fs::read_to_string(repository.path().join("report.toon")).expect("read the report file"),
        report_without_findings(1, 1)
    );
}

#[test]
fn output_in_a_folder_that_does_not_exist_names_the_path_and_exits_2_before_measuring() {
    let repository = Repository::new();
    repository.write("src/broken.ts", "export const = ;\n");
    repository.commit_all();

    let output = repository.codeint(&["check", "--output", "reports/today/report.toon"]);

    assert_operational_error(
        &output,
        "codeint: cannot write the report to reports/today/report.toon: reports/today is not a folder that exists\n\
         Create the folder first, or pass --output a file path in a folder that exists.\n",
    );
    assert!(!repository.path().join("reports").exists());
}

#[test]
fn output_to_a_folder_names_the_path_and_exits_2() {
    let repository = Repository::new();
    repository.write("src/main.ts", "export const main = 1;\n");
    repository.commit_all();

    let output = repository.codeint(&["check", "--output", "src"]);

    assert_unwritable_output(&output, "src");
}

#[cfg(unix)]
#[test]
fn output_to_a_read_only_file_names_the_path_exits_2_and_keeps_the_file() {
    use std::os::unix::fs::PermissionsExt;

    let repository = Repository::new();
    repository.write("src/main.ts", "export const main = 1;\n");
    repository.commit_all();
    let report = repository.path().join("report.toon");
    fs::write(&report, "kept\n").expect("write the read-only report");
    fs::set_permissions(&report, fs::Permissions::from_mode(0o444)).expect("make it read-only");

    let output = repository.codeint(&["check", "--output", "report.toon"]);

    assert_unwritable_output(&output, "report.toon");
    assert_eq!(
        fs::read_to_string(&report).expect("read the report"),
        "kept\n"
    );
}

fn assert_unwritable_output(output: &Output, path: &str) {
    assert_eq!(output.status.code(), Some(2));
    assert!(stdout(output).is_empty());
    let message = String::from_utf8_lossy(&output.stderr);
    assert!(
        message.starts_with(&format!("codeint: cannot write the report to {path}: "))
            && message.ends_with(
                "\nPass --output a file path that you have permission to write, not a folder.\n"
            )
            && message.lines().count() == 2,
        "unexpected message: {message}"
    );
}

const NO_DUPLICATION: &str = "duplication:\n  ratio: 0\n  reference_range:\n    low: 0.017\n    high: 0.346\n    source: \"codeint panel of 20 TypeScript and JavaScript repositories, ADR 0008, codeint 0.1.0\"\n  clone_lines: 0\n  clone_groups: []\n";

const CART_TOTAL: &str = concat!(
    "export function total(items: { price: number; discount: number; quantity: number }[]): number {\n",
    "  let sum = 0;\n",
    "  for (const item of items.filter(Boolean)) {\n",
    "    if (item.quantity > 0) {\n",
    "      sum += (item.price - item.discount) * item.quantity;\n",
    "    }\n",
    "  }\n",
    "  return Math.round(sum * 100) / 100;\n",
    "}\n",
);

#[test]
fn two_copies_of_one_function_across_files_form_one_clone_group_with_both_locations() {
    let repository = Repository::new();
    repository.write("src/cart.ts", CART_TOTAL);
    repository.write(
        "src/invoice.ts",
        &format!("const currency = 'EUR';\n{CART_TOTAL}export const label = currency;\n"),
    );
    repository.commit_all();

    let output = repository.codeint(&["check"]);

    assert_success(&output);
    assert_eq!(summary(&output), "files: 2\nsource_lines: 20\n");
    assert_eq!(
        production_section(&output, "duplication"),
        concat!(
            "duplication:\n",
            "  ratio: 0.9\n",
            "  reference_range:\n",
            "    low: 0.017\n",
            "    high: 0.346\n",
            "    source: \"codeint panel of 20 TypeScript and JavaScript repositories, ADR 0008, codeint 0.1.0\"\n",
            "  clone_lines: 18\n",
            "  clone_groups[1]:\n",
            "    - id: <id>\n",
            "      tokens: 83\n",
            "      locations[2]{path,start_line,end_line}:\n",
            "        src/cart.ts,1,9\n",
            "        src/invoice.ts,2,10\n",
        )
    );
}

const ORDER_TOTAL_RENAMED: &str = concat!(
    "export function amount(lines: { cost: number; rebate: number; count: number }[]): number {\n",
    "  let result = 100;\n",
    "  for (const line of lines.filter(Present)) {\n",
    "    if (line.count > 5) {\n",
    "      result += (line.cost - line.rebate) * line.count;\n",
    "    }\n",
    "  }\n",
    "  return Tally.floor(result * 10) / 1000;\n",
    "}\n",
);

const CART_TOTAL_RESTRUCTURED: &str = concat!(
    "export function total(items: { price: number; discount: number; quantity: number }[]): number {\n",
    "  let sum = 0;\n",
    "  for (const item of items.filter(Boolean)) {\n",
    "    while (item.quantity > 0) {\n",
    "      sum += (item.price - item.discount) * item.quantity;\n",
    "    }\n",
    "  }\n",
    "  return Math.round(sum * 100) / 100 + 1;\n",
    "}\n",
);

#[test]
fn copies_that_differ_only_in_names_and_literal_values_form_one_clone_group() {
    let repository = Repository::new();
    repository.write("src/cart.ts", CART_TOTAL);
    repository.write("src/order.ts", ORDER_TOTAL_RENAMED);
    repository.commit_all();

    let output = repository.codeint(&["check"]);

    assert_success(&output);
    assert_eq!(
        production_section(&output, "duplication"),
        concat!(
            "duplication:\n",
            "  ratio: 1\n",
            "  reference_range:\n",
            "    low: 0.017\n",
            "    high: 0.346\n",
            "    source: \"codeint panel of 20 TypeScript and JavaScript repositories, ADR 0008, codeint 0.1.0\"\n",
            "  clone_lines: 18\n",
            "  clone_groups[1]:\n",
            "    - id: <id>\n",
            "      tokens: 83\n",
            "      locations[2]{path,start_line,end_line}:\n",
            "        src/cart.ts,1,9\n",
            "        src/order.ts,1,9\n",
        )
    );
}

#[test]
fn copies_with_a_different_structure_do_not_form_a_clone_group() {
    let repository = Repository::new();
    repository.write("src/cart.ts", CART_TOTAL);
    repository.write("src/restructured.ts", CART_TOTAL_RESTRUCTURED);
    repository.commit_all();

    let output = repository.codeint(&["check"]);

    assert_success(&output);
    assert_eq!(production_section(&output, "duplication"), NO_DUPLICATION);
}

#[test]
fn copies_of_a_fragment_below_the_minimum_clone_size_are_ignored() {
    let small = "export function double(value: number): number {\n  return value * 2;\n}\n";
    let repository = Repository::new();
    repository.write("src/first.ts", small);
    repository.write("src/second.ts", small);
    repository.commit_all();

    let output = repository.codeint(&["check"]);

    assert_success(&output);
    assert_eq!(summary(&output), "files: 2\nsource_lines: 6\n");
    assert_eq!(production_section(&output, "duplication"), NO_DUPLICATION);
}

const AREA_OF_75_TOKENS: &str = concat!(
    "export default function area(width: number, height: number, depth: number) {\n",
    "  const base = width * height;\n",
    "  const side = height * depth;\n",
    "  const volume = base * depth / 2;\n",
    "  const half = volume - side % 3;\n",
    "  if (base > side) {\n",
    "    return base * depth + half;\n",
    "  }\n",
    "  return side * width - base + 1;\n",
    "}\n",
);

#[test]
fn a_copy_of_74_tokens_is_not_a_clone_group_and_a_copy_of_75_tokens_is() {
    let without_default =
        AREA_OF_75_TOKENS.replacen("export default function", "export function", 1);
    let below = Repository::new();
    below.write("src/first.ts", &without_default);
    below.write("src/second.ts", &without_default);
    below.commit_all();
    let at = Repository::new();
    at.write("src/first.ts", AREA_OF_75_TOKENS);
    at.write("src/second.ts", AREA_OF_75_TOKENS);
    at.commit_all();

    let below_output = below.codeint(&["check"]);
    let at_output = at.codeint(&["check"]);

    assert_success(&below_output);
    assert_success(&at_output);
    assert_eq!(
        production_section(&at_output, "duplication"),
        concat!(
            "duplication:\n",
            "  ratio: 1\n",
            "  reference_range:\n",
            "    low: 0.017\n",
            "    high: 0.346\n",
            "    source: \"codeint panel of 20 TypeScript and JavaScript repositories, ADR 0008, codeint 0.1.0\"\n",
            "  clone_lines: 20\n",
            "  clone_groups[1]:\n",
            "    - id: <id>\n",
            "      tokens: 75\n",
            "      locations[2]{path,start_line,end_line}:\n",
            "        src/first.ts,1,10\n",
            "        src/second.ts,1,10\n",
        )
    );
    assert_eq!(
        production_section(&below_output, "duplication"),
        NO_DUPLICATION
    );
}

fn write_21_clone_groups_of_equal_size(repository: &Repository) {
    let one_token_swaps = [
        ("let sum", "var sum"),
        ("const item", "let item"),
        (" > ", " < "),
        (" += ", " -= "),
        (" * ", " / "),
    ];
    for index in 1..=21 {
        let total = one_token_swaps.iter().enumerate().fold(
            CART_TOTAL.to_owned(),
            |total, (bit, (from, to))| {
                if index & (1 << bit) == 0 {
                    total
                } else {
                    total.replace(from, to)
                }
            },
        );
        repository.write(&format!("src/a{index:02}.ts"), &total);
        repository.write(&format!("src/b{index:02}.ts"), &total);
    }
}

fn clone_groups_by_first_location(output: &Output) -> Vec<String> {
    production_section(output, "duplication")
        .lines()
        .map(str::trim_start)
        .filter(|line| {
            line.starts_with("clone_groups")
                || line.starts_with("findings_left_out")
                || line.starts_with("src/a")
        })
        .map(str::to_string)
        .collect()
}

#[test]
fn the_report_lists_20_clone_groups_and_says_how_many_it_left_out() {
    let repository = Repository::new();
    write_21_clone_groups_of_equal_size(&repository);
    repository.commit_all();

    let output = repository.codeint(&["check"]);

    assert_success(&output);
    let mut expected = vec!["clone_groups[20]:".to_string()];
    expected.extend((1..=20).map(|index| format!("src/a{index:02}.ts,1,9")));
    expected.push("findings_left_out: 1".to_string());
    assert_eq!(clone_groups_by_first_location(&output), expected);
}

#[test]
fn all_lists_every_clone_group() {
    let repository = Repository::new();
    write_21_clone_groups_of_equal_size(&repository);
    repository.commit_all();

    let output = repository.codeint(&["check", "--all"]);

    assert_success(&output);
    let mut expected = vec!["clone_groups[21]:".to_string()];
    expected.extend((1..=21).map(|index| format!("src/a{index:02}.ts,1,9")));
    assert_eq!(clone_groups_by_first_location(&output), expected);
}

fn import_and_export_declarations(prefix: &str) -> String {
    format!(
        concat!(
            "import {p}Default, {{ {p}One, {p}Two as {p}Three }} from '{p}-alpha';\n",
            "import * as {p}All from '{p}-beta';\n",
            "import type {{ {p}Shape, {p}Size }} from '{p}-gamma';\n",
            "import '{p}-side-effect';\n",
            "import {p}Legacy = require('{p}-legacy');\n",
            "export * from '{p}-delta';\n",
            "export * as {p}Bundle from '{p}-epsilon';\n",
            "export {{ {p}Four, {p}Five as {p}Six }} from '{p}-zeta';\n",
            "export type {{ {p}Seven }} from '{p}-eta';\n",
            "export {{ {p}Default, {p}One as {p}Eight, {p}All, {p}Legacy }};\n",
        ),
        p = prefix
    )
}

#[test]
fn files_whose_only_shared_code_is_a_long_import_list_form_no_clone_group() {
    let repository = Repository::new();
    repository.write(
        "src/first.ts",
        &format!(
            "{}export const first = 1;\n",
            import_and_export_declarations("first")
        ),
    );
    repository.write(
        "src/second.ts",
        &format!(
            "{}export function second(): string {{\n  return 'two';\n}}\n",
            import_and_export_declarations("second")
        ),
    );
    repository.commit_all();

    let output = repository.codeint(&["check"]);

    assert_success(&output);
    assert_eq!(production_section(&output, "duplication"), NO_DUPLICATION);
}

#[test]
fn a_copied_function_after_a_shared_import_list_forms_a_clone_group_of_the_function_alone() {
    let repository = Repository::new();
    repository.write(
        "src/first.ts",
        &format!("{}{CART_TOTAL}", import_and_export_declarations("first")),
    );
    repository.write(
        "src/second.ts",
        &format!(
            "{}{ORDER_TOTAL_RENAMED}",
            import_and_export_declarations("second")
        ),
    );
    repository.commit_all();

    let output = repository.codeint(&["check"]);

    assert_success(&output);
    assert_eq!(
        production_section(&output, "duplication"),
        concat!(
            "duplication:\n",
            "  ratio: 0.4737\n",
            "  reference_range:\n",
            "    low: 0.017\n",
            "    high: 0.346\n",
            "    source: \"codeint panel of 20 TypeScript and JavaScript repositories, ADR 0008, codeint 0.1.0\"\n",
            "  clone_lines: 18\n",
            "  clone_groups[1]:\n",
            "    - id: <id>\n",
            "      tokens: 83\n",
            "      locations[2]{path,start_line,end_line}:\n",
            "        src/first.ts,11,19\n",
            "        src/second.ts,11,19\n",
        )
    );
}

fn data_table(name: &str, rows: usize) -> String {
    let rows: String = (1..=rows)
        .map(|row| format!("  {name}{row}: '{name}-value-{row}',\n"))
        .collect();
    format!("export const {name} = {{\n{rows}}};\n")
}

#[test]
fn a_long_table_of_same_shaped_rows_forms_no_clone_group() {
    let repository = Repository::new();
    repository.write("src/colors.ts", &data_table("color", 40));
    repository.write("src/labels.ts", &data_table("label", 40));
    repository.commit_all();

    let output = repository.codeint(&["check"]);

    assert_success(&output);
    assert_eq!(summary(&output), "files: 2\nsource_lines: 84\n");
    assert_eq!(production_section(&output, "duplication"), NO_DUPLICATION);
}

fn no_import_cycles(modules: usize) -> String {
    format!(
        "import_cycles:\n\
         \x20 ratio: 0\n\
         \x20 reference_range:\n\
         \x20   low: 0\n\
         \x20   high: 0.308\n\
         \x20   source: \"codeint panel of 20 TypeScript and JavaScript repositories, ADR 0008, codeint 0.1.0\"\n\
         \x20 modules_in_import_cycles: 0\n\
         \x20 modules: {modules}\n\
         \x20 findings: []\n\
         \x20 unresolved_imports:\n\
         \x20   count: 0\n\
         \x20   imports: []\n"
    )
}

#[test]
fn three_files_that_import_each_other_in_a_ring_form_one_import_cycle() {
    let repository = Repository::new();
    repository.write(
        "src/a.ts",
        "import { b } from './b';\nexport const a = () => b();\n",
    );
    repository.write(
        "src/b.ts",
        "import { c } from './c.ts';\nexport const b = () => c();\n",
    );
    repository.write(
        "src/c.ts",
        "import { a } from './a.js';\nexport const c = () => a();\n",
    );
    repository.write("src/main.ts", "import { a } from './a';\na();\n");
    repository.commit_all();

    let output = repository.codeint(&["check"]);

    assert_success(&output);
    assert_eq!(
        production_section(&output, "import_cycles"),
        concat!(
            "import_cycles:\n",
            "  ratio: 0.75\n",
            "  reference_range:\n",
            "    low: 0\n",
            "    high: 0.308\n",
            "    source: \"codeint panel of 20 TypeScript and JavaScript repositories, ADR 0008, codeint 0.1.0\"\n",
            "  modules_in_import_cycles: 3\n",
            "  modules: 4\n",
            "  findings[1]:\n",
            "    - id: <id>\n",
            "      modules[3]: src/a.ts,src/b.ts,src/c.ts\n",
            "  unresolved_imports:\n",
            "    count: 0\n",
            "    imports: []\n",
        )
    );
}

#[test]
fn a_cycle_through_a_tsconfig_path_alias_is_an_import_cycle() {
    let repository = Repository::new();
    repository.write(
        "tsconfig.base.json",
        "{\n  \"compilerOptions\": {\n    \"baseUrl\": \".\",\n    \"paths\": { \"@app/*\": [\"src/app/*\"] }\n  }\n}\n",
    );
    repository.write(
        "tsconfig.json",
        "{ \"extends\": \"./tsconfig.base.json\", \"include\": [\"src\"] }\n",
    );
    repository.write(
        "src/app/orders.ts",
        "import { users } from '@app/users';\nexport const orders = () => users();\n",
    );
    repository.write(
        "src/app/users.ts",
        "import { orders } from 'src/app/orders';\nexport const users = () => orders();\n",
    );
    repository.write(
        "src/main.ts",
        "import { orders } from '@app/orders';\norders();\n",
    );
    repository.commit_all();

    let output = repository.codeint(&["check"]);

    assert_success(&output);
    assert_eq!(
        production_section(&output, "import_cycles"),
        concat!(
            "import_cycles:\n",
            "  ratio: 0.6667\n",
            "  reference_range:\n",
            "    low: 0\n",
            "    high: 0.308\n",
            "    source: \"codeint panel of 20 TypeScript and JavaScript repositories, ADR 0008, codeint 0.1.0\"\n",
            "  modules_in_import_cycles: 2\n",
            "  modules: 3\n",
            "  findings[1]:\n",
            "    - id: <id>\n",
            "      modules[2]: src/app/orders.ts,src/app/users.ts\n",
            "  unresolved_imports:\n",
            "    count: 0\n",
            "    imports: []\n",
        )
    );
}

#[test]
fn a_cycle_through_the_exports_of_a_workspace_package_is_an_import_cycle() {
    let repository = Repository::new();
    repository.write(
        "package.json",
        "{ \"name\": \"shop\", \"private\": true, \"workspaces\": [\"packages/*\"] }\n",
    );
    repository.write(
        "packages/billing/package.json",
        concat!(
            "{\n",
            "  \"name\": \"@shop/billing\",\n",
            "  \"exports\": {\n",
            "    \".\": { \"types\": \"./src/index.ts\", \"import\": \"./dist/index.js\" },\n",
            "    \"./invoices\": \"./src/invoices/public.ts\"\n",
            "  }\n",
            "}\n",
        ),
    );
    repository.write(
        "packages/billing/src/index.ts",
        "import { customer } from '@shop/accounts';\nexport const bill = () => customer();\n",
    );
    repository.write(
        "packages/billing/src/invoices/public.ts",
        "export const invoice = () => 1;\n",
    );
    repository.write(
        "packages/accounts/package.json",
        "{ \"name\": \"@shop/accounts\", \"exports\": \"./lib/accounts.ts\" }\n",
    );
    repository.write(
        "packages/accounts/lib/accounts.ts",
        concat!(
            "import { bill } from '@shop/billing';\n",
            "import { invoice } from '@shop/billing/invoices';\n",
            "export const customer = () => bill() + invoice();\n",
        ),
    );
    repository.commit_all();

    let output = repository.codeint(&["check"]);

    assert_success(&output);
    assert_eq!(
        production_section(&output, "import_cycles"),
        concat!(
            "import_cycles:\n",
            "  ratio: 0.6667\n",
            "  reference_range:\n",
            "    low: 0\n",
            "    high: 0.308\n",
            "    source: \"codeint panel of 20 TypeScript and JavaScript repositories, ADR 0008, codeint 0.1.0\"\n",
            "  modules_in_import_cycles: 2\n",
            "  modules: 3\n",
            "  findings[1]:\n",
            "    - id: <id>\n",
            "      modules[2]: packages/accounts/lib/accounts.ts,packages/billing/src/index.ts\n",
            "  unresolved_imports:\n",
            "    count: 0\n",
            "    imports: []\n",
        )
    );
}

#[test]
fn a_diamond_of_imports_without_a_cycle_reports_no_import_cycle() {
    let repository = Repository::new();
    repository.write(
        "src/main.ts",
        "import { left } from './left';\nimport { right } from './right';\nleft(right());\n",
    );
    repository.write(
        "src/left.ts",
        "import { shared } from './shared';\nexport const left = (value: number) => shared(value);\n",
    );
    repository.write(
        "src/right.ts",
        "import { shared } from './shared';\nexport const right = () => shared(1);\n",
    );
    repository.write(
        "src/shared.ts",
        "export const shared = (value: number) => value;\n",
    );
    repository.commit_all();

    let output = repository.codeint(&["check"]);

    assert_success(&output);
    assert_eq!(
        production_section(&output, "import_cycles"),
        no_import_cycles(4)
    );
}

#[test]
fn dynamic_import_and_require_are_runtime_imports_that_close_a_cycle() {
    let repository = Repository::new();
    repository.write(
        "lib/loader.js",
        "export const load = () => import(`./plugin.js`);\n",
    );
    repository.write(
        "lib/plugin.js",
        "const helpers = require('./helpers');\nexport default helpers;\n",
    );
    repository.write(
        "lib/helpers.js",
        "const { load } = require('./loader');\nmodule.exports = { load };\n",
    );
    repository.write(
        "lib/standalone.js",
        "const path = require('node:path');\nexport const join = path.join;\n",
    );
    repository.commit_all();

    let output = repository.codeint(&["check"]);

    assert_success(&output);
    assert_eq!(
        production_section(&output, "import_cycles"),
        concat!(
            "import_cycles:\n",
            "  ratio: 0.75\n",
            "  reference_range:\n",
            "    low: 0\n",
            "    high: 0.308\n",
            "    source: \"codeint panel of 20 TypeScript and JavaScript repositories, ADR 0008, codeint 0.1.0\"\n",
            "  modules_in_import_cycles: 3\n",
            "  modules: 4\n",
            "  findings[1]:\n",
            "    - id: <id>\n",
            "      modules[3]: lib/helpers.js,lib/loader.js,lib/plugin.js\n",
            "  unresolved_imports:\n",
            "    count: 0\n",
            "    imports: []\n",
        )
    );
}

#[test]
fn type_only_imports_and_exports_do_not_form_an_import_cycle() {
    let repository = Repository::new();
    repository.write(
        "src/user.ts",
        "import type { Order } from './order';\nexport interface User { orders: Order[] }\n",
    );
    repository.write(
        "src/order.ts",
        "import { type User } from './user';\nexport interface Order { owner: User }\n",
    );
    repository.write(
        "src/index.ts",
        "export type { User } from './user';\nexport type * from './order';\n",
    );
    repository.write(
        "src/barrel.ts",
        "import { type Order } from './order';\nexport * from './index';\nexport const none: Order[] = [];\n",
    );
    repository.commit_all();

    let output = repository.codeint(&["check"]);

    assert_success(&output);
    assert_eq!(
        production_section(&output, "import_cycles"),
        no_import_cycles(4)
    );
}

#[test]
fn a_mixed_import_or_reexport_with_one_value_specifier_closes_an_import_cycle() {
    let repository = Repository::new();
    repository.write(
        "src/shape.ts",
        "import { type Unit, scale } from './unit';\nexport interface Shape { unit: Unit }\nconst area = (side: number) => scale(side * side);\nexport default area;\n",
    );
    repository.write(
        "src/unit.ts",
        "export { type Ratio, ratio } from './ratio';\nexport type Unit = 'cm';\nexport const scale = (value: number) => value;\n",
    );
    repository.write(
        "src/ratio.ts",
        "import area, { type Shape } from './shape';\nexport type Ratio = number;\nexport const ratio = (shape: Shape) => area(1) / shape.unit.length;\n",
    );
    repository.commit_all();

    let output = repository.codeint(&["check"]);

    assert_success(&output);
    assert_eq!(
        production_section(&output, "import_cycles"),
        concat!(
            "import_cycles:\n",
            "  ratio: 1\n",
            "  reference_range:\n",
            "    low: 0\n",
            "    high: 0.308\n",
            "    source: \"codeint panel of 20 TypeScript and JavaScript repositories, ADR 0008, codeint 0.1.0\"\n",
            "  modules_in_import_cycles: 3\n",
            "  modules: 3\n",
            "  findings[1]:\n",
            "    - id: <id>\n",
            "      modules[3]: src/ratio.ts,src/shape.ts,src/unit.ts\n",
            "  unresolved_imports:\n",
            "    count: 0\n",
            "    imports: []\n",
        )
    );
}

#[test]
fn each_import_cycle_is_a_finding_and_the_ratio_counts_every_module_in_one() {
    let repository = Repository::new();
    repository.write("src/a.ts", "export * from './b';\n");
    repository.write(
        "src/b.ts",
        "export { a } from './a';\nexport const b = 1;\n",
    );
    repository.write("src/x/index.ts", "import '../y';\n");
    repository.write("src/y.ts", "import './x';\nimport './z';\n");
    repository.write("src/z.ts", "import 'lodash';\nimport './missing';\n");
    repository.write("src/self.ts", "import './self';\n");
    repository.write("src/data.json", "{}\n");
    repository.write(
        "src/reader.ts",
        "import data from './data.json';\nexport { data };\n",
    );
    repository.commit_all();

    let output = repository.codeint(&["check"]);

    assert_success(&output);
    assert_eq!(summary(&output), "files: 7\nsource_lines: 11\n");
    assert_eq!(
        production_section(&output, "import_cycles"),
        concat!(
            "import_cycles:\n",
            "  ratio: 0.7143\n",
            "  reference_range:\n",
            "    low: 0\n",
            "    high: 0.308\n",
            "    source: \"codeint panel of 20 TypeScript and JavaScript repositories, ADR 0008, codeint 0.1.0\"\n",
            "  modules_in_import_cycles: 5\n",
            "  modules: 7\n",
            "  findings[3]:\n",
            "    - id: <id>\n",
            "      modules[2]: src/a.ts,src/b.ts\n",
            "    - id: <id>\n",
            "      modules[2]: src/x/index.ts,src/y.ts\n",
            "    - id: <id>\n",
            "      modules[1]: src/self.ts\n",
            "  unresolved_imports:\n",
            "    count: 1\n",
            "    imports[1]{importer,specifier,reason}:\n",
            "      src/z.ts,./missing,\"no file of the repository matches \\\"./missing\\\"\"\n",
        )
    );
}

fn write_21_self_importing_modules(repository: &Repository) {
    for index in 1..=21 {
        repository.write(
            &format!("src/self{index:02}.ts"),
            &format!("import './self{index:02}';\n"),
        );
    }
}

fn import_cycle_findings(output: &Output) -> Vec<String> {
    production_section(output, "import_cycles")
        .lines()
        .map(str::trim_start)
        .skip_while(|line| !line.starts_with("findings"))
        .take_while(|line| *line != "unresolved_imports:")
        .map(str::to_string)
        .collect()
}

#[test]
fn the_report_lists_20_import_cycles_and_says_how_many_it_left_out() {
    let repository = Repository::new();
    write_21_self_importing_modules(&repository);
    repository.commit_all();

    let output = repository.codeint(&["check"]);

    assert_success(&output);
    let mut expected = vec!["findings[20]:".to_string()];
    expected.extend((1..=20).flat_map(|index| {
        [
            "- id: <id>".to_string(),
            format!("modules[1]: src/self{index:02}.ts"),
        ]
    }));
    expected.push("findings_left_out: 1".to_string());
    assert_eq!(import_cycle_findings(&output), expected);
}

#[test]
fn all_lists_every_import_cycle() {
    let repository = Repository::new();
    write_21_self_importing_modules(&repository);
    repository.commit_all();

    let output = repository.codeint(&["check", "--all"]);

    assert_success(&output);
    let mut expected = vec!["findings[21]:".to_string()];
    expected.extend((1..=21).flat_map(|index| {
        [
            "- id: <id>".to_string(),
            format!("modules[1]: src/self{index:02}.ts"),
        ]
    }));
    assert_eq!(import_cycle_findings(&output), expected);
}

#[test]
fn each_source_set_finds_import_cycles_only_among_its_own_modules() {
    let repository = Repository::new();
    repository.write(
        "src/greet.ts",
        "import { check } from './greet.test';\nexport const greet = () => check;\n",
    );
    repository.write(
        "src/greet.test.ts",
        "import { greet } from './greet';\nimport { helper } from './helper.test';\nexport const check = greet;\nexport const used = helper;\n",
    );
    repository.write(
        "src/helper.test.ts",
        "import { used } from './greet.test';\nexport const helper = () => used;\n",
    );
    repository.commit_all();

    let output = repository.codeint(&["check"]);

    assert_success(&output);
    assert_eq!(
        metric_section(&output, "production", "import_cycles"),
        no_import_cycles(1)
    );
    assert_eq!(
        metric_section(&output, "test", "import_cycles"),
        concat!(
            "import_cycles:\n",
            "  ratio: 1\n",
            "  reference_range:\n",
            "    low: 0\n",
            "    high: 0.308\n",
            "    source: \"codeint panel of 20 TypeScript and JavaScript repositories, ADR 0008, codeint 0.1.0\"\n",
            "  modules_in_import_cycles: 2\n",
            "  modules: 2\n",
            "  findings[1]:\n",
            "    - id: <id>\n",
            "      modules[2]: src/greet.test.ts,src/helper.test.ts\n",
            "  unresolved_imports:\n",
            "    count: 0\n",
            "    imports: []\n",
        )
    );
}

#[test]
fn unresolved_imports_are_counted_and_listed_and_the_import_cycle_is_still_found() {
    let repository = Repository::new();
    repository.write(
        "src/a.ts",
        "import { b } from './b';\nimport { gone } from './gone';\nexport const a = () => b(gone);\n",
    );
    repository.write(
        "src/b.ts",
        "import { a } from './a';\nexport const b = (value: unknown) => a() ?? value;\n",
    );
    repository.write(
        "src/c.ts",
        "const outside = require('../../outside');\nexport const c = outside;\n",
    );
    repository.commit_all();

    let output = repository.codeint(&["check"]);

    assert_success(&output);
    assert_eq!(
        production_section(&output, "import_cycles"),
        concat!(
            "import_cycles:\n",
            "  ratio: 0.6667\n",
            "  reference_range:\n",
            "    low: 0\n",
            "    high: 0.308\n",
            "    source: \"codeint panel of 20 TypeScript and JavaScript repositories, ADR 0008, codeint 0.1.0\"\n",
            "  modules_in_import_cycles: 2\n",
            "  modules: 3\n",
            "  findings[1]:\n",
            "    - id: <id>\n",
            "      modules[2]: src/a.ts,src/b.ts\n",
            "  unresolved_imports:\n",
            "    count: 2\n",
            "    imports[2]{importer,specifier,reason}:\n",
            "      src/a.ts,./gone,\"no file of the repository matches \\\"./gone\\\"\"\n",
            "      src/c.ts,../../outside,\"\\\"../../outside\\\" leads outside the repository\"\n",
        )
    );
}

#[test]
fn bare_imports_of_external_packages_and_built_in_modules_are_not_unresolved() {
    let repository = Repository::new();
    repository.write(
        "src/main.ts",
        "import 'lodash';\nimport { render } from 'react-dom/client';\nimport { z } from '@scope/schema/v4';\nimport { readFile } from 'node:fs/promises';\nconst path = require('path');\nexport const load = () => import('chalk');\nexport const all = [render, z, readFile, path];\n",
    );
    repository.commit_all();

    let output = repository.codeint(&["check"]);

    assert_success(&output);
    assert_eq!(
        production_section(&output, "import_cycles"),
        no_import_cycles(1)
    );
}

fn write_21_unresolved_imports_in_two_files(repository: &Repository) {
    let imports = |names: std::ops::RangeInclusive<usize>| -> String {
        names
            .rev()
            .map(|index| format!("import './missing{index:02}';\n"))
            .collect()
    };
    repository.write("src/later.ts", &imports(12..=21));
    repository.write("src/earlier.ts", &imports(1..=11));
}

fn unresolved_import_rows(output: &Output) -> Vec<String> {
    production_section(output, "import_cycles")
        .lines()
        .map(str::trim_start)
        .skip_while(|line| *line != "unresolved_imports:")
        .skip(1)
        .map(|line| line.split(",\"").next().unwrap_or(line).to_string())
        .collect()
}

fn unresolved_import_row(importer: &str, index: usize) -> String {
    format!("{importer},./missing{index:02}")
}

fn unresolved_import_rows_of_both_files(later_from: usize) -> Vec<String> {
    (1..=11)
        .rev()
        .map(|index| unresolved_import_row("src/earlier.ts", index))
        .chain(
            (later_from..=21)
                .rev()
                .map(|index| unresolved_import_row("src/later.ts", index)),
        )
        .collect()
}

#[test]
fn the_report_lists_the_first_20_unresolved_imports_by_importer_and_says_how_many_it_left_out() {
    let repository = Repository::new();
    write_21_unresolved_imports_in_two_files(&repository);
    repository.commit_all();

    let output = repository.codeint(&["check"]);

    assert_success(&output);
    let mut expected = vec![
        "count: 21".to_string(),
        "imports[20]{importer,specifier,reason}:".to_string(),
    ];
    expected.extend(unresolved_import_rows_of_both_files(13));
    expected.push("imports_left_out: 1".to_string());
    assert_eq!(unresolved_import_rows(&output), expected);
}

#[test]
fn all_lists_every_unresolved_import() {
    let repository = Repository::new();
    write_21_unresolved_imports_in_two_files(&repository);
    repository.commit_all();

    let output = repository.codeint(&["check", "--all"]);

    assert_success(&output);
    let mut expected = vec![
        "count: 21".to_string(),
        "imports[21]{importer,specifier,reason}:".to_string(),
    ];
    expected.extend(unresolved_import_rows_of_both_files(12));
    assert_eq!(unresolved_import_rows(&output), expected);
}

#[test]
fn an_unresolved_import_of_a_test_file_is_reported_in_the_test_source_set_only() {
    let repository = Repository::new();
    repository.write("src/greet.ts", "export const greet = 'hello';\n");
    repository.write(
        "src/greet.test.ts",
        "import { greet } from './greet';\nimport { fixture } from './fixtures/greeting';\nexport const check = [greet, fixture];\n",
    );
    repository.commit_all();

    let output = repository.codeint(&["check"]);

    assert_success(&output);
    assert_eq!(
        metric_section(&output, "production", "import_cycles"),
        no_import_cycles(1)
    );
    assert_eq!(
        metric_section(&output, "test", "import_cycles"),
        concat!(
            "import_cycles:\n",
            "  ratio: 0\n",
            "  reference_range:\n",
            "    low: 0\n",
            "    high: 0.308\n",
            "    source: \"codeint panel of 20 TypeScript and JavaScript repositories, ADR 0008, codeint 0.1.0\"\n",
            "  modules_in_import_cycles: 0\n",
            "  modules: 1\n",
            "  findings: []\n",
            "  unresolved_imports:\n",
            "    count: 1\n",
            "    imports[1]{importer,specifier,reason}:\n",
            "      src/greet.test.ts,./fixtures/greeting,\"no file of the repository matches \\\"./fixtures/greeting\\\"\"\n",
        )
    );
}

#[test]
fn files_under_a_build_output_folder_at_any_depth_are_in_the_generated_source_set() {
    let repository = Repository::new();
    repository.write("dist/index.js", "export const a = 1;\n");
    repository.write("packages/web/build/main.js", "export const b = 2;\n");
    repository.write("out/server.mjs", "export const c = 3;\n");
    repository.write("coverage/lcov-report/sorter.js", "export const d = 4;\n");
    repository.write("dist/greet.test.ts", "export const e = 5;\n");
    repository.commit_all();

    let output = repository.codeint(&["check"]);

    assert_success(&output);
    assert_eq!(
        source_set_sizes(&output),
        ["generated: files: 5 source_lines: 5"]
    );
}

#[test]
fn a_file_or_folder_that_only_looks_like_a_build_output_folder_stays_in_production() {
    let repository = Repository::new();
    repository.write("src/dist.ts", "export const a = 1;\n");
    repository.write("src/distance/metres.ts", "export const b = 2;\n");
    repository.write("src/outbox/send.ts", "export const c = 3;\n");
    repository.write("src/builder.ts", "export const d = 4;\n");
    repository.commit_all();

    let output = repository.codeint(&["check"]);

    assert_success(&output);
    assert_eq!(
        source_set_sizes(&output),
        ["production: files: 4 source_lines: 4"]
    );
}

fn line_of_length(name: &str, length: usize) -> String {
    let prefix = format!("export const {name} = \"");
    let padding = length - prefix.len() - "\";".len();
    format!("{prefix}{}\";", "x".repeat(padding))
}

#[test]
fn a_file_whose_lines_average_over_250_characters_is_generated_and_one_of_250_is_not() {
    let repository = Repository::new();
    repository.write(
        "src/long.js",
        &format!(
            "{}\n{}\n",
            line_of_length("a", 250),
            line_of_length("b", 252)
        ),
    );
    repository.write(
        "src/typical.js",
        &format!(
            "{}\n{}\n",
            line_of_length("c", 249),
            line_of_length("d", 251)
        ),
    );
    repository.commit_all();

    let output = repository.codeint(&["check"]);

    assert_success(&output);
    assert_eq!(
        source_set_sizes(&output),
        [
            "production: files: 1 source_lines: 2",
            "generated: files: 1 source_lines: 2",
        ]
    );
}

#[test]
fn blank_lines_do_not_lower_the_average_line_length() {
    let repository = Repository::new();
    repository.write(
        "src/spaced.js",
        &format!("{}\n\n\n  \n\r\n", line_of_length("a", 300)),
    );
    repository.commit_all();

    let output = repository.codeint(&["check"]);

    assert_success(&output);
    assert_eq!(
        source_set_sizes(&output),
        ["generated: files: 1 source_lines: 1"]
    );
}

#[test]
fn line_length_counts_characters_and_not_bytes() {
    let repository = Repository::new();
    let wide_line = format!("export const a = \"{}\";", "é".repeat(230));
    repository.write("src/accents.js", &format!("{wide_line}\n"));
    repository.commit_all();

    let output = repository.codeint(&["check"]);

    assert_success(&output);
    assert_eq!(
        source_set_sizes(&output),
        ["production: files: 1 source_lines: 1"]
    );
}

fn minified_bundle_with_an_eroded_function() -> String {
    let branches: String = (1..=12)
        .map(|value| format!("if(v==={value})return {value};"))
        .collect();
    format!(
        "export function render(v){{{branches}return 0}}export function size(v){{return v.length}}\n"
    )
}

#[test]
fn a_minified_bundle_is_generated() {
    let repository = Repository::new();
    repository.write(
        "public/app.min.js",
        &minified_bundle_with_an_eroded_function(),
    );
    repository.commit_all();

    let output = repository.codeint(&["check"]);

    assert_success(&output);
    assert_eq!(
        source_set_sizes(&output),
        ["generated: files: 1 source_lines: 1"]
    );
}

#[test]
fn a_file_with_a_generated_marker_comment_is_generated_and_one_without_stays_in_production() {
    let repository = Repository::new();
    repository.write(
        "src/api/client.ts",
        "// Code generated by openapi-tool. DO NOT EDIT.\nexport const client = 1;\n",
    );
    repository.write(
        "src/api/handwritten.ts",
        "// Wraps the client.\nexport const wrapper = 2;\n",
    );
    repository.commit_all();

    let output = repository.codeint(&["check"]);

    assert_success(&output);
    assert_eq!(
        source_set_sizes(&output),
        [
            "production: files: 1 source_lines: 1",
            "generated: files: 1 source_lines: 1",
        ]
    );
}

#[test]
fn a_vendored_file_stays_vendored_even_when_a_generated_rule_matches_it() {
    let repository = Repository::new();
    repository.write("vendor/chart/dist/chart.js", "export const a = 1;\n");
    repository.write("build/third_party/zlib.js", "export const b = 2;\n");
    repository.write(
        "vendor/app.min.js",
        &minified_bundle_with_an_eroded_function(),
    );
    repository.write(
        "third_party/schema.ts",
        "// @generated by schema-tool\nexport const c = 3;\n",
    );
    repository.commit_all();

    let output = repository.codeint(&["check"]);

    assert_success(&output);
    assert_eq!(
        source_set_sizes(&output),
        ["vendored: files: 4 source_lines: 4"]
    );
}

#[test]
fn generated_files_do_not_change_production_metrics() {
    let production_only = Repository::new();
    production_only.write("src/nine.ts", &function_with_if_statements("nine", 9));
    production_only.write("src/eleven.ts", &function_with_if_statements("eleven", 11));
    production_only.commit_all();
    let with_generated = Repository::new();
    with_generated.write("src/nine.ts", &function_with_if_statements("nine", 9));
    with_generated.write("src/eleven.ts", &function_with_if_statements("eleven", 11));
    with_generated.write(
        "public/app.min.js",
        &minified_bundle_with_an_eroded_function(),
    );
    with_generated.write("dist/eleven.ts", &function_with_if_statements("eleven", 11));
    with_generated.write("dist/nine.ts", &function_with_if_statements("nine", 9));
    with_generated.commit_all();

    let expected = production_only.codeint(&["check"]);
    let output = with_generated.codeint(&["check"]);

    assert_success(&expected);
    assert_success(&output);
    for metric in ["erosion", "verbosity", "duplication"] {
        let production = production_section(&output, metric);
        assert!(!production.is_empty(), "the report shows {metric}");
        assert_eq!(
            production,
            production_section(&expected, metric),
            "{metric}"
        );
    }
    assert_eq!(
        source_set_sizes(&output),
        [
            "production: files: 2 source_lines: 26",
            "generated: files: 3 source_lines: 27",
        ]
    );
    assert!(
        metric_section(&output, "generated", "erosion").contains("public/app.min.js,1,13,1,13"),
        "the eroded function of the bundle counts toward the Generated Source set"
    );
}

#[test]
fn a_production_override_in_codeint_json_wins_over_the_test_folder_rule() {
    let repository = Repository::new();
    repository.write(
        "codeint.json",
        r#"{
  "source_sets": {
    "production": ["test/support/**"]
  }
}
"#,
    );
    repository.write("src/main.ts", "export const main = 1;\n");
    repository.write(
        "test/main.ts",
        "export const one = 1;\nexport const two = 2;\n",
    );
    repository.write(
        "packages/app/tests/deep/user.ts",
        "export const user = 1;\nexport const name = 2;\nexport const age = 3;\n",
    );
    repository.write(
        "test/support/server.ts",
        "export const port = 1;\nexport const host = 2;\nexport const path = 3;\nexport const ready = 4;\n",
    );
    repository.commit_all();

    let output = repository.codeint(&["check"]);

    assert_success(&output);
    assert_eq!(
        source_set_sizes(&output),
        [
            "production: files: 2 source_lines: 5",
            "test: files: 2 source_lines: 5",
        ]
    );
}

#[test]
fn a_source_set_override_in_codeint_json_moves_each_matching_file_into_the_named_source_set() {
    let repository = Repository::new();
    repository.write(
        "codeint.json",
        r#"{
  "$schema": "https://raw.githubusercontent.com/avidianity/codeint/v0.1.0/schema/codeint.schema.json",
  "source_sets": {
    "production": ["src/fixtures.test.ts", "dist/handwritten.ts"],
    "test": ["e2e/**"],
    "generated": ["src/proto/*.ts"],
    "vendored": ["lib/legacy/**/*.js"]
  }
}
"#,
    );
    repository.write("src/main.ts", "export const main = 1;\n");
    repository.write(
        "src/fixtures.test.ts",
        "export const fixture = 1;\nexport const other = 2;\n",
    );
    repository.write(
        "e2e/login.ts",
        "export const login = 1;\nexport const user = 2;\nexport const page = 3;\n",
    );
    repository.write("src/proto/user.ts", "export const user = 1;\n");
    repository.write("src/proto/nested/kept.ts", "export const kept = 1;\n");
    repository.write("dist/handwritten.ts", "export const handwritten = 1;\n");
    repository.write("lib/legacy/a/b.js", "export const legacy = 1;\n");
    repository.commit_all();

    let output = repository.codeint(&["check", "src"]);

    assert_success(&output);
    assert_eq!(
        source_set_sizes(&output),
        [
            "production: files: 4 source_lines: 5",
            "test: files: 1 source_lines: 3",
            "generated: files: 1 source_lines: 1",
            "vendored: files: 1 source_lines: 1",
        ]
    );
}

#[test]
fn a_file_that_matches_overrides_of_several_source_sets_goes_to_the_first_of_vendored_generated_test_production()
 {
    let repository = Repository::new();
    repository.write(
        "codeint.json",
        r#"{
  "source_sets": {
    "production": ["src/**"],
    "test": ["src/**"],
    "generated": ["src/api/**"],
    "vendored": ["src/api/external/**"]
  }
}
"#,
    );
    repository.write("src/main.ts", "export const main = 1;\n");
    repository.write("src/api/client.ts", "export const client = 1;\n");
    repository.write("src/api/external/sdk.ts", "export const sdk = 1;\n");
    repository.write("lib/vendor/kept.ts", "export const kept = 1;\n");
    repository.commit_all();

    let output = repository.codeint(&["check"]);

    assert_success(&output);
    assert_eq!(
        source_set_sizes(&output),
        [
            "test: files: 1 source_lines: 1",
            "generated: files: 1 source_lines: 1",
            "vendored: files: 2 source_lines: 2",
        ]
    );
}

const UNKNOWN_KEY_REMEDY: &str =
    "Remove the key, or correct its spelling to one of the allowed keys.";

const WRONG_TYPE_REMEDY: &str = "Change the value to the expected type; the JSON schema that $schema links to describes every key.";

fn assert_codeint_json_is_rejected(codeint_json: &str, message: &str, remedy: &str) {
    let repository = Repository::new();
    repository.write("codeint.json", codeint_json);
    repository.write("src/main.ts", "export const main = 1;\n");
    repository.commit_all();

    let output = repository.codeint(&["check", "src"]);

    assert_operational_error(&output, &format!("codeint: {message}\n{remedy}\n"));
}

#[test]
fn an_unknown_key_in_codeint_json_is_named_with_the_allowed_keys_and_exits_2() {
    assert_codeint_json_is_rejected(
        r#"{ "source_sets": { "generatd": ["dist/**"] } }"#,
        "codeint.json has an unknown key source_sets.generatd; \
         the keys allowed there are production, test, generated, vendored",
        UNKNOWN_KEY_REMEDY,
    );
}

#[test]
fn an_unknown_top_level_key_in_codeint_json_is_named_and_exits_2() {
    assert_codeint_json_is_rejected(
        r#"{ "budget": {} }"#,
        "codeint.json has an unknown key budget; the keys allowed there are $schema, source_sets, budgets, regression_limits, languages, plugins, plugin_limits",
        UNKNOWN_KEY_REMEDY,
    );
}

#[test]
fn a_value_of_the_wrong_type_in_codeint_json_names_the_key_and_the_expected_type_and_exits_2() {
    assert_codeint_json_is_rejected(
        r#"{ "source_sets": { "generated": "dist/**" } }"#,
        "codeint.json: source_sets.generated must be an array, not a string",
        WRONG_TYPE_REMEDY,
    );
}

#[test]
fn an_item_of_the_wrong_type_in_codeint_json_names_its_index_and_exits_2() {
    assert_codeint_json_is_rejected(
        r#"{ "source_sets": { "test": ["e2e/**", 42] } }"#,
        "codeint.json: source_sets.test[1] must be a string, not an integer",
        WRONG_TYPE_REMEDY,
    );
}

#[test]
fn an_invalid_glob_in_codeint_json_names_the_key_and_exits_2() {
    assert_codeint_json_is_rejected(
        r#"{ "source_sets": { "vendored": ["lib/[abc"] } }"#,
        "codeint.json: source_sets.vendored[0] is not a valid glob: \
         error parsing glob 'lib/[abc': unclosed character class; missing ']'",
        "Fix the glob: * matches within one folder, ** matches across folders, and {a,b} matches either a or b.",
    );
}

#[test]
fn codeint_json_that_is_not_json_names_the_line_and_column_and_exits_2() {
    assert_codeint_json_is_rejected(
        "{\n  \"source_sets\": {},\n}\n",
        "codeint.json is not valid JSON: trailing comma at line 3 column 1",
        "Fix the JSON syntax of codeint.json at that line and column.",
    );
}

fn write_files_of_distinct_line_counts(repository: &Repository, paths: &[&str]) {
    for (power, path) in paths.iter().enumerate() {
        let lines: String = (0..1usize << power)
            .map(|line| format!("export const value{line} = {line};\n"))
            .collect();
        repository.write(path, &lines);
    }
}

fn check_with_languages(languages: &str, paths: &[&str]) -> Output {
    let repository = Repository::new();
    repository.write(
        "codeint.json",
        &format!(r#"{{ "languages": {languages} }}"#),
    );
    write_files_of_distinct_line_counts(&repository, paths);
    repository.commit_all();
    repository.codeint(&["check"])
}

#[test]
fn a_languages_entry_measures_only_the_folders_it_includes() {
    let output = check_with_languages(
        r#"[{ "plugin": "typescript-javascript", "include": ["packages/web", "scripts/*.ts"] }]"#,
        &[
            "packages/web/src/main.ts",
            "packages/web/index.js",
            "packages/api/main.ts",
            "scripts/build.ts",
            "scripts/nested/deploy.ts",
            "main.ts",
        ],
    );

    assert_success(&output);
    assert_eq!(
        source_set_sizes(&output),
        ["production: files: 3 source_lines: 11"]
    );
}

#[test]
fn an_exclude_glob_leaves_out_a_folder_inside_an_included_one() {
    let output = check_with_languages(
        r#"[{
            "plugin": "typescript-javascript",
            "include": ["packages/**"],
            "exclude": ["packages/web/legacy", "**/*.js"]
        }]"#,
        &[
            "packages/web/main.ts",
            "packages/web/legacy/old.ts",
            "packages/web/legacy/deep/older.ts",
            "packages/api/main.ts",
            "packages/api/bundle.js",
        ],
    );

    assert_success(&output);
    assert_eq!(
        source_set_sizes(&output),
        ["production: files: 2 source_lines: 9"]
    );
}

#[test]
fn two_entries_for_one_language_plugin_both_apply() {
    let output = check_with_languages(
        r#"[
            { "plugin": "typescript-javascript", "include": ["web"] },
            { "plugin": "typescript-javascript", "include": ["api"], "exclude": ["api/generated"] }
        ]"#,
        &[
            "web/main.ts",
            "api/main.ts",
            "api/generated/client.ts",
            "tools/lint.ts",
        ],
    );

    assert_success(&output);
    assert_eq!(
        source_set_sizes(&output),
        ["production: files: 2 source_lines: 3"]
    );
}

#[test]
fn without_the_languages_setting_every_language_plugin_measures_the_whole_repository() {
    let output = check_with_languages("null", &["web/main.ts", "api/main.ts", "tools/lint.js"]);

    assert_success(&output);
    assert_eq!(
        source_set_sizes(&output),
        ["production: files: 3 source_lines: 7"]
    );
}

#[test]
fn an_empty_languages_setting_measures_nothing() {
    let output = check_with_languages("[]", &["web/main.ts"]);

    assert_success(&output);
    assert_eq!(source_set_sizes(&output), Vec::<String>::new());
}

#[test]
fn a_languages_entry_that_names_an_unknown_language_plugin_names_it_and_exits_2() {
    assert_codeint_json_is_rejected(
        r#"{ "languages": [
            { "plugin": "typescript-javascript", "include": ["web"] },
            { "plugin": "cobol", "include": ["scripts"] }
        ] }"#,
        "codeint.json names an unknown Language plugin cobol at languages[1].plugin; \
         the Language plugins are typescript-javascript, rust, php, dart, python",
        "Name one of the Language plugins listed, or remove the entry from languages.",
    );
}

#[test]
fn a_languages_entry_without_include_names_the_missing_key_and_exits_2() {
    assert_codeint_json_is_rejected(
        r#"{ "languages": [{ "plugin": "typescript-javascript" }] }"#,
        "codeint.json: languages[0] needs the key include",
        "Add the key; the JSON schema that $schema links to describes every key.",
    );
}

#[test]
fn an_invalid_exclude_glob_names_its_key_and_exits_2() {
    assert_codeint_json_is_rejected(
        r#"{ "languages": [{ "plugin": "typescript-javascript", "include": ["**"], "exclude": ["web/[abc"] }] }"#,
        "codeint.json: languages[0].exclude[0] is not a valid glob: \
         error parsing glob 'web/[abc': unclosed character class; missing ']'",
        "Fix the glob: * matches within one folder, ** matches across folders, and {a,b} matches either a or b.",
    );
}

#[test]
fn a_latin_1_file_is_skipped_and_listed_with_the_reason() {
    let repository = Repository::new();
    repository.write("src/main.ts", "export const main = 1;\n");
    repository.write_bytes("src/latin1.ts", b"export const name = 'caf\xe9';\n");
    repository.commit_all();

    let output = repository.codeint(&["check"]);

    assert_success(&output);
    assert_eq!(
        stdout(&output),
        report_without_findings(1, 1)
            + "skipped[1]{path,reason}:\n  \
             src/latin1.ts,\"not valid UTF-8: invalid utf-8 sequence of 1 bytes from index 24\"\n"
    );
}

const PRICE_LABEL: &str = concat!(
    "/**\n",
    " * Formats a price for the receipt.\n",
    " */\n",
    "export function priceLabel(items: { price: number; discount: number; quantity: number }[], currency: string): string {\n",
    "  let sum = 0;\n",
    "  for (const item of items.filter(Boolean)) {\n",
    "    sum += (item.price - item.discount) * item.quantity;\n",
    "  }\n",
    "  return `total:\n",
    "${Math.round(sum * 100) / 100} ${currency}\n",
    "end`;\n",
    "}\n",
);

const SIGN_WITH_ELSE_AFTER_RETURN: &str = concat!(
    "import { label } from './label';\n",
    "export function sign(value: number): string {\n",
    "  if (value < 0) {\n",
    "    return label('negative \\\n",
    "minus');\n",
    "  } else {\n",
    "    return label('positive');\n",
    "  }\n",
    "}\n",
);

const LABEL_IN_A_CYCLE: &str = concat!(
    "import { sign } from './sign';\n",
    "export const label = (text: string) => text + sign.name;\n",
);

fn with_crlf(text: &str) -> String {
    text.replace('\n', "\r\n")
}

fn write_every_metric_fixture(repository: &Repository, line_ending: fn(&str) -> String) {
    repository.write("src/cart.ts", &line_ending(PRICE_LABEL));
    repository.write(
        "src/invoice.ts",
        &line_ending(&format!("const currency = 'EUR';\n{PRICE_LABEL}")),
    );
    repository.write(
        "src/eleven.ts",
        &line_ending(&function_with_if_statements("eleven", 11)),
    );
    repository.write("src/sign.ts", &line_ending(SIGN_WITH_ELSE_AFTER_RETURN));
    repository.write("src/label.ts", &line_ending(LABEL_IN_A_CYCLE));
    repository.write(
        "src/broken.ts",
        &line_ending("export const ok = 1;\nexport const = ;\n"),
    );
    repository.commit_all();
}

#[test]
fn the_same_files_with_crlf_and_with_lf_line_endings_give_the_same_report() {
    let lf = Repository::new();
    write_every_metric_fixture(&lf, str::to_owned);
    let crlf = Repository::new();
    write_every_metric_fixture(&crlf, with_crlf);

    let expected = lf.codeint(&["check"]);
    let output = crlf.codeint(&["check"]);

    assert_success(&expected);
    assert_success(&output);
    for (metric, numerator) in [
        ("erosion", "eroded_function_mass: 0\n"),
        ("duplication", "clone_lines: 0\n"),
        ("verbosity", "verbose_lines: 0\n"),
        ("import_cycles", "modules_in_import_cycles: 0\n"),
    ] {
        assert!(
            !production_section(&expected, metric).contains(numerator),
            "the fixture has a {metric} Finding"
        );
    }
    assert!(
        stdout(&expected)
            .contains("src/broken.ts,\"cannot parse: Unexpected token at line 2, column 14\""),
        "the fixture has a parse failure"
    );
    assert_eq!(stdout(&output), stdout(&expected));
}

#[test]
fn a_copy_with_crlf_line_endings_and_a_copy_with_lf_line_endings_form_one_clone_group() {
    let lf = Repository::new();
    lf.write("src/cart.ts", PRICE_LABEL);
    lf.write("src/invoice.ts", PRICE_LABEL);
    lf.commit_all();
    let mixed = Repository::new();
    mixed.write("src/cart.ts", PRICE_LABEL);
    mixed.write("src/invoice.ts", &with_crlf(PRICE_LABEL));
    mixed.commit_all();

    let expected = lf.codeint(&["check"]);
    let output = mixed.codeint(&["check"]);

    assert_success(&expected);
    assert_success(&output);
    assert_ne!(production_section(&expected, "duplication"), NO_DUPLICATION);
    assert_eq!(stdout(&output), stdout(&expected));
}

#[cfg(unix)]
#[test]
fn symbolic_links_to_a_measured_file_to_a_folder_and_outside_the_repository_add_nothing() {
    let outside = tempfile::tempdir().expect("create a temporary folder");
    fs::write(outside.path().join("outside.ts"), CART_TOTAL).expect("write the file");
    let plain = Repository::new();
    plain.write("src/cart.ts", CART_TOTAL);
    plain.commit_all();
    let linked = Repository::new();
    linked.write("src/cart.ts", CART_TOTAL);
    let link = |target: &Path, relative: &str| {
        std::os::unix::fs::symlink(target, linked.path().join(relative))
            .expect("create the symbolic link");
    };
    link(Path::new("cart.ts"), "src/alias.ts");
    link(Path::new("src"), "mirror");
    link(&outside.path().join("outside.ts"), "src/outside.ts");
    linked.commit_all();

    let expected = plain.codeint(&["check"]);
    let output = linked.codeint(&["check"]);

    assert_success(&expected);
    assert_success(&output);
    assert_eq!(stdout(&output), stdout(&expected));
}

fn repository_with_a_submodule() -> (Repository, Repository) {
    let library = Repository::new();
    library.write("src/library.ts", CART_TOTAL);
    library.commit_all();
    let repository = Repository::new();
    repository.write("src/main.ts", "export const main = 1;\n");
    repository.git(&[
        "-c",
        "protocol.file.allow=always",
        "submodule",
        "add",
        "--quiet",
        library
            .path()
            .to_str()
            .expect("the temporary path is UTF-8"),
        "modules/library",
    ]);
    repository.commit_all();
    (repository, library)
}

fn report_with_one_submodule() -> String {
    report_without_findings(1, 1) + "submodules[1]: modules/library\n"
}

#[test]
fn a_git_submodule_is_listed_and_adds_nothing() {
    let (repository, _library) = repository_with_a_submodule();

    let output = repository.codeint(&["check"]);

    assert_success(&output);
    assert_eq!(stdout(&output), report_with_one_submodule());
}

#[test]
fn a_git_submodule_that_is_not_checked_out_is_listed_and_adds_nothing() {
    let (repository, _library) = repository_with_a_submodule();
    repository.git(&[
        "submodule",
        "deinit",
        "--quiet",
        "--force",
        "modules/library",
    ]);

    let output = repository.codeint(&["check"]);

    assert_success(&output);
    assert_eq!(stdout(&output), report_with_one_submodule());
}

fn repository_with_a_finding_a_skipped_file_and_a_declaration_file() -> Repository {
    let repository = Repository::new();
    repository.write("src/sign.ts", SIGN_WITH_AN_ELSE_AFTER_RETURN);
    repository.write("src/broken.ts", "export const = ;\n");
    repository.write("src/types.d.ts", "export type Sign = string;\n");
    repository.commit_all();
    repository
}

#[test]
fn format_json_prints_the_report_as_json_with_the_documented_shape() {
    let repository = repository_with_a_finding_a_skipped_file_and_a_declaration_file();

    let output = repository.codeint(&["check", "--format", "json"]);

    assert_success(&output);
    assert_eq!(
        stdout(&output),
        r#"{
  "files": 1,
  "source_lines": 7,
  "source_sets": {
    "production": {
      "files": 1,
      "source_lines": 7,
      "erosion": {
        "ratio": 0.0,
        "reference_range": {
          "low": 0.352,
          "high": 0.6,
          "source": "codeint panel of 20 TypeScript and JavaScript repositories, ADR 0008, codeint 0.1.0"
        },
        "eroded_function_mass": 0.0,
        "total_function_mass": 5.2915,
        "findings": []
      },
      "duplication": {
        "ratio": 0.0,
        "reference_range": {
          "low": 0.017,
          "high": 0.346,
          "source": "codeint panel of 20 TypeScript and JavaScript repositories, ADR 0008, codeint 0.1.0"
        },
        "clone_lines": 0,
        "clone_groups": []
      },
      "verbosity": {
        "ratio": 0.1429,
        "reference_range": {
          "low": 0.018,
          "high": 0.347,
          "source": "codeint panel of 20 TypeScript and JavaScript repositories, ADR 0008, codeint 0.1.0"
        },
        "verbose_lines": 1,
        "flagged_lines": 1,
        "clone_lines": 0,
        "findings": [
          {
            "id": "2a57800b29941394",
            "path": "src/sign.ts",
            "line": 4,
            "rule": "else-after-return"
          }
        ]
      },
      "import_cycles": {
        "ratio": 0.0,
        "reference_range": {
          "low": 0.0,
          "high": 0.308,
          "source": "codeint panel of 20 TypeScript and JavaScript repositories, ADR 0008, codeint 0.1.0"
        },
        "modules_in_import_cycles": 0,
        "modules": 1,
        "findings": [],
        "unresolved_imports": {
          "count": 0,
          "imports": []
        }
      }
    }
  },
  "skipped": [
    {
      "path": "src/broken.ts",
      "reason": "cannot parse: Unexpected token at line 1, column 14"
    }
  ],
  "declaration_only": [
    "src/types.d.ts"
  ]
}
"#
    );
}

fn with_every_number_as_f64(value: serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Number(number) => {
            serde_json::Value::from(number.as_f64().expect("every report number fits in an f64"))
        }
        serde_json::Value::Array(items) => {
            items.into_iter().map(with_every_number_as_f64).collect()
        }
        serde_json::Value::Object(fields) => fields
            .into_iter()
            .map(|(key, field)| (key, with_every_number_as_f64(field)))
            .collect(),
        other => other,
    }
}

#[test]
fn format_json_carries_every_field_and_value_of_the_toon_report() {
    let repository = repository_with_every_optional_part_of_the_report();

    let toon = repository.codeint(&["check"]);
    let json = repository.codeint(&["check", "--format", "json"]);

    assert_success(&toon);
    assert_success(&json);
    let from_toon: serde_json::Value =
        toon_format::decode_default(&stdout(&toon)).expect("the TOON report decodes");
    let from_json: serde_json::Value =
        serde_json::from_str(&stdout(&json)).expect("the JSON report parses");
    let production = &from_json["source_sets"]["production"];
    assert!(
        production["erosion"]["findings_left_out"].is_number()
            && production["duplication"]["clone_groups"][0].is_object()
            && production["import_cycles"]["findings"][0].is_object()
            && from_json["source_sets"]["test"].is_object()
            && from_json["skipped"][0].is_object()
            && from_json["declaration_only"][0].is_string(),
        "the fixture covers every optional part of the report: {from_json}"
    );
    assert_eq!(
        with_every_number_as_f64(from_json),
        with_every_number_as_f64(from_toon)
    );
}

fn repository_with_every_optional_part_of_the_report() -> Repository {
    let repository = repository_with_a_finding_a_skipped_file_and_a_declaration_file();
    write_eroded_functions_with_more_if_statements_in_later_files(&repository, 21);
    repository.write(
        "src/a.ts",
        "import { b } from './b';\nexport const a = b;\n",
    );
    repository.write(
        "src/b.ts",
        "import { a } from './a';\nexport const b = a;\n",
    );
    repository.write("src/cart.ts", CART_TOTAL);
    repository.write("src/order.ts", ORDER_TOTAL_RENAMED);
    repository.write(
        "src/sign.test.ts",
        "import { sign } from './sign';\nsign(1);\n",
    );
    repository.commit_all();
    repository
}

#[test]
fn format_markdown_prints_each_metric_then_the_findings_then_the_files_not_measured() {
    let repository = repository_with_a_finding_a_skipped_file_and_a_declaration_file();

    let output = repository.codeint(&["check", "--format", "markdown"]);

    assert_success(&output);
    assert_eq!(
        stdout(&output),
        include_str!("snapshots/markdown-report.md")
    );
}

#[test]
fn format_markdown_lists_every_optional_part_of_the_report_and_escapes_markdown_in_paths_and_text()
{
    let repository = repository_with_every_optional_part_of_the_report();
    repository.write(
        "src/pipe.ts",
        "import { x } from './a|b`c';\nexport const y = x;\n",
    );
    repository.commit_all();

    let output = repository.codeint(&["check", "--format", "markdown"]);

    assert_success(&output);
    assert_eq!(
        stdout(&output),
        include_str!("snapshots/markdown-report-every-part.md")
    );
}

#[test]
fn format_markdown_lists_a_git_submodule_under_files_not_measured() {
    let (repository, _library) = repository_with_a_submodule();

    let output = repository.codeint(&["check", "--format", "markdown"]);

    assert_success(&output);
    assert!(
        stdout(&output)
            .ends_with("\n\n## Files not measured\n\n- `modules/library`: Git submodule\n"),
        "unexpected report: {}",
        stdout(&output)
    );
}

fn repository_with_a_nested_path_in_every_report_section() -> Repository {
    let repository = Repository::new();
    repository.write(
        "codeint.json",
        r#"{ "source_sets": { "test": ["packages/app/checks/**"] } }"#,
    );
    repository.write(
        "packages/app/src/deep/eroded.ts",
        &function_with_if_statements("route", 11),
    );
    repository.write("packages/app/src/cart.ts", CART_TOTAL);
    repository.write("packages/lib/src/invoice.ts", CART_TOTAL);
    repository.write(
        "packages/app/src/ring/a.ts",
        "import { b } from '../../../lib/src/ring/b';\nexport const a = () => b();\n",
    );
    repository.write(
        "packages/lib/src/ring/b.ts",
        "import { a } from '../../../app/src/ring/a';\nexport const b = () => a();\n",
    );
    repository.write("packages/app/checks/smoke.ts", "export const smoke = 1;\n");
    repository.write(
        "packages/lib/src/broken.ts",
        "export const ok = 1;\nexport const = ;\n",
    );
    repository.write(
        "packages/lib/types/index.d.ts",
        "declare const version: string;\n",
    );
    repository.commit_all();
    repository
}

#[test]
fn repository_root_check_from_a_nested_subfolder_prints_the_same_report_as_from_the_root() {
    let repository = repository_with_a_nested_path_in_every_report_section();

    for format in ["toon", "json"] {
        let arguments = ["check", "--all", "--format", format];
        let from_root = repository.codeint(&arguments);
        let from_subfolder =
            run_codeint(&repository.path().join("packages/lib/src/ring"), &arguments);

        assert_success(&from_root);
        assert_success(&from_subfolder);
        assert_eq!(stdout(&from_subfolder), stdout(&from_root), "{format}");
    }
}

fn json_strings_naming_a_file(value: &serde_json::Value, strings: &mut Vec<String>) {
    match value {
        serde_json::Value::String(text) if text.ends_with(".ts") => strings.push(text.clone()),
        serde_json::Value::Array(items) => items
            .iter()
            .for_each(|item| json_strings_naming_a_file(item, strings)),
        serde_json::Value::Object(fields) => fields
            .values()
            .for_each(|field| json_strings_naming_a_file(field, strings)),
        _ => {}
    }
}

#[test]
fn repository_root_relative_paths_with_forward_slashes_appear_in_the_toon_and_json_reports() {
    let repository = repository_with_a_nested_path_in_every_report_section();
    let subfolder = repository.path().join("packages/app");

    let output = run_codeint(&subfolder, &["check"]);
    let json = run_codeint(&subfolder, &["check", "--format", "json"]);

    assert_success(&output);
    assert_success(&json);
    let report = stdout(&output);
    assert!(!report.contains('\\'), "unexpected backslash: {report}");
    let path_lines: Vec<String> = report
        .lines()
        .filter(|line| line.contains(".ts"))
        .map(|line| with_finding_id_shown_as_a_placeholder(line.trim()))
        .collect();
    assert_eq!(
        path_lines,
        [
            "<id>,packages/app/src/deep/eroded.ts,1,12,14,44.8999",
            "packages/app/src/cart.ts,1,9",
            "packages/lib/src/invoice.ts,1,9",
            "modules[2]: packages/app/src/ring/a.ts,packages/lib/src/ring/b.ts",
            "packages/lib/src/broken.ts,\"cannot parse: Unexpected token at line 2, column 14\"",
            "declaration_only[1]: packages/lib/types/index.d.ts",
        ]
    );
    let json_report: serde_json::Value =
        serde_json::from_str(&stdout(&json)).expect("the JSON report parses");
    let mut json_paths = Vec::new();
    json_strings_naming_a_file(&json_report, &mut json_paths);
    json_paths.sort();
    assert_eq!(
        json_paths,
        [
            "packages/app/src/cart.ts",
            "packages/app/src/deep/eroded.ts",
            "packages/app/src/ring/a.ts",
            "packages/lib/src/broken.ts",
            "packages/lib/src/invoice.ts",
            "packages/lib/src/ring/b.ts",
            "packages/lib/types/index.d.ts",
        ]
    );
}

fn write_two_package_workspace(repository: &Repository, root_manifest: &str) {
    repository.write("package.json", root_manifest);
    repository.write("packages/eroded/package.json", "{\"name\": \"eroded\"}\n");
    repository.write(
        "packages/eroded/src/branches.ts",
        &function_with_if_statements("branches", 11),
    );
    repository.write("packages/small/package.json", "{\"name\": \"small\"}\n");
    repository.write(
        "packages/small/src/sum.ts",
        &function_with_if_statements("sum", 3),
    );
    repository.write("scripts/build.ts", "export const build = 1;\n");
    repository.write(
        "docs/package.json",
        "{\"name\": \"not-a-workspace-package\"}\n",
    );
}

const TWO_PACKAGE_EROSION: &str = concat!(
    "erosion:\n",
    "  ratio: 0.8209\n",
    "  reference_range:\n",
    "    low: 0.352\n",
    "    high: 0.6\n",
    "    source: \"codeint panel of 20 TypeScript and JavaScript repositories, ADR 0008, codeint 0.1.0\"\n",
    "  eroded_function_mass: 44.8999\n",
    "  total_function_mass: 54.6978\n",
    "  findings[1]{id,path,line,cyclomatic_complexity,source_lines,function_mass}:\n",
    "    <id>,packages/eroded/src/branches.ts,1,12,14,44.8999\n",
);

const TWO_PACKAGE_ROWS: &str = concat!(
    "  .,1,1,0,0,0,0,0,0,0,0,0\n",
    "  packages/eroded,1,14,1,44.8999,44.8999,0,0,0,0,0,0\n",
    "  packages/small,1,6,0,0,9.798,0,0,0,0,0,0\n",
);

fn assert_two_package_report(output: &Output) {
    assert_success(output);
    assert_eq!(summary(output), "files: 3\nsource_lines: 21\n");
    assert_eq!(
        source_set_sizes(output),
        ["production: files: 3 source_lines: 21"]
    );
    assert_eq!(production_section(output, "erosion"), TWO_PACKAGE_EROSION);
    assert_eq!(
        packages_table(output, "production"),
        PACKAGES_HEADER.to_owned() + TWO_PACKAGE_ROWS
    );
}

#[test]
fn workspace_packages_of_the_array_form_each_report_erosion_and_the_total_sums_their_numerators() {
    let repository = Repository::new();
    write_two_package_workspace(&repository, "{\"workspaces\": [\"packages/*\"]}\n");
    repository.commit_all();

    let output = repository.codeint(&["check"]);

    assert_two_package_report(&output);
}

#[test]
fn workspace_packages_of_the_object_form_with_packages_are_reported_like_the_array_form() {
    let repository = Repository::new();
    write_two_package_workspace(
        &repository,
        "{\"workspaces\": {\"packages\": [\"packages/*\"], \"nohoist\": [\"**/react\"]}}\n",
    );
    repository.commit_all();

    let output = repository.codeint(&["check"]);

    assert_two_package_report(&output);
}

#[test]
fn a_workspaces_field_of_the_wrong_shape_names_the_manifest_and_exits_2() {
    let repository = Repository::new();
    repository.write("package.json", "{\"workspaces\": \"packages/*\"}\n");
    repository.write("packages/app/package.json", "{}\n");
    repository.write("packages/app/index.ts", "export const app = 1;\n");
    repository.commit_all();

    let output = repository.codeint(&["check"]);

    assert_operational_error(
        &output,
        concat!(
            "codeint: cannot find the workspace packages from package.json: ",
            "the workspaces field must be an array of folder patterns, ",
            "or an object whose packages field is one\n",
            "Fix the manifest that the message names so that your package manager accepts it, ",
            "then run codeint again.\n",
        ),
    );
}

#[test]
fn pnpm_workspace_packages_each_report_erosion_and_a_negated_pattern_excludes_a_package() {
    let repository = Repository::new();
    write_two_package_workspace(&repository, "{\"name\": \"shop\"}\n");
    repository.write(
        "pnpm-workspace.yaml",
        concat!(
            "# The packages of the shop.\n",
            "packages:\n",
            "  - packages/*\n",
            "  - \"!packages/legacy\"\n",
            "catalog:\n",
            "  react: ^19.0.0\n",
        ),
    );
    repository.write("packages/legacy/package.json", "{\"name\": \"legacy\"}\n");
    repository.commit_all();

    let output = repository.codeint(&["check"]);

    assert_two_package_report(&output);
}

#[test]
fn a_pnpm_workspace_yaml_that_is_not_yaml_names_the_manifest_and_exits_2() {
    let repository = Repository::new();
    repository.write("package.json", "{\"name\": \"shop\"}\n");
    repository.write("pnpm-workspace.yaml", "packages: [packages/*\n");
    repository.write("packages/app/package.json", "{}\n");
    repository.write("packages/app/index.ts", "export const app = 1;\n");
    repository.commit_all();

    let output = repository.codeint(&["check"]);

    assert_operational_error(
        &output,
        concat!(
            "codeint: cannot find the workspace packages from pnpm-workspace.yaml: ",
            "is not valid YAML: unclosed bracket '[' at line 1, column 11\n",
            "Fix the manifest that the message names so that your package manager accepts it, ",
            "then run codeint again.\n",
        ),
    );
}

const PACKAGES_HEADER: &str = concat!(
    "packages[3]{path,files,source_lines,",
    "erosion{ratio,eroded_function_mass,total_function_mass},",
    "duplication{ratio,clone_lines},verbosity{ratio,verbose_lines},",
    "import_cycles{ratio,modules_in_import_cycles}}:\n",
);

fn packages_table(output: &Output, source_set: &str) -> String {
    let source_set_heading = format!("  {source_set}:");
    stdout(output)
        .lines()
        .skip_while(|line| *line != source_set_heading)
        .skip_while(|line| !line.starts_with("    packages["))
        .enumerate()
        .take_while(|(index, line)| *index == 0 || indent(line) > METRIC_INDENT)
        .map(|(_, line)| format!("{}\n", &line[METRIC_INDENT..]))
        .collect()
}

#[test]
fn each_workspace_package_reports_its_own_clone_lines_and_verbose_lines() {
    let repository = Repository::new();
    repository.write("package.json", "{\"workspaces\": [\"packages/*\"]}\n");
    repository.write("packages/cart/package.json", "{}\n");
    repository.write("packages/cart/src/total.ts", CART_TOTAL);
    repository.write("packages/invoice/package.json", "{}\n");
    repository.write(
        "packages/invoice/src/total.ts",
        &format!("const currency = 'EUR';\n{CART_TOTAL}export const label = currency;\n"),
    );
    repository.write(
        "packages/invoice/src/sign.ts",
        concat!(
            "export function sign(value: number): string {\n",
            "  if (value < 0) {\n",
            "    return 'negative';\n",
            "  } else {\n",
            "    return 'positive';\n",
            "  }\n",
            "}\n",
        ),
    );
    repository.commit_all();

    let output = repository.codeint(&["check"]);

    assert_success(&output);
    assert_eq!(summary(&output), "files: 3\nsource_lines: 27\n");
    assert_eq!(
        packages_table(&output, "production"),
        PACKAGES_HEADER.to_owned()
            + concat!(
                "  .,0,0,0,0,0,0,0,0,0,0,0\n",
                "  packages/cart,1,9,0,0,9,1,9,1,9,0,0\n",
                "  packages/invoice,2,18,0,0,14.2915,0.5,9,0.5556,10,0,0\n",
            )
    );
    assert!(production_section(&output, "duplication").contains("  clone_lines: 18\n"));
    assert!(production_section(&output, "verbosity").contains("  verbose_lines: 19\n"));
}

#[test]
fn each_source_set_splits_into_every_workspace_package() {
    let repository = Repository::new();
    repository.write("package.json", "{\"workspaces\": [\"packages/*\"]}\n");
    repository.write("packages/app/package.json", "{}\n");
    repository.write("packages/app/src/main.ts", "export const main = 1;\n");
    repository.write(
        "packages/app/src/main.test.ts",
        "export const checked = 1;\nexport const twice = 2;\n",
    );
    repository.write("packages/lib/package.json", "{}\n");
    repository.write("packages/lib/src/lib.ts", "export const lib = 1;\n");
    repository.commit_all();

    let output = repository.codeint(&["check"]);

    assert_success(&output);
    assert_eq!(
        packages_table(&output, "production"),
        PACKAGES_HEADER.to_owned()
            + concat!(
                "  .,0,0,0,0,0,0,0,0,0,0,0\n",
                "  packages/app,1,1,0,0,0,0,0,0,0,0,0\n",
                "  packages/lib,1,1,0,0,0,0,0,0,0,0,0\n",
            )
    );
    assert_eq!(
        packages_table(&output, "test"),
        PACKAGES_HEADER.to_owned()
            + concat!(
                "  .,0,0,0,0,0,0,0,0,0,0,0\n",
                "  packages/app,1,2,0,0,0,0,0,0,0,0,0\n",
                "  packages/lib,0,0,0,0,0,0,0,0,0,0,0\n",
            )
    );
}

#[test]
fn an_import_cycle_across_two_workspace_packages_counts_each_module_in_its_own_package() {
    let repository = Repository::new();
    repository.write("package.json", "{\"workspaces\": [\"packages/*\"]}\n");
    repository.write("packages/orders/package.json", "{}\n");
    repository.write(
        "packages/orders/src/order.ts",
        "import { bill } from '../../billing/src/bill';\nexport const order = () => bill();\n",
    );
    repository.write("packages/billing/package.json", "{}\n");
    repository.write(
        "packages/billing/src/bill.ts",
        "import { order } from '../../orders/src/order';\nexport const bill = () => order();\n",
    );
    repository.write("packages/billing/src/rate.ts", "export const rate = 2;\n");
    repository.commit_all();

    let output = repository.codeint(&["check"]);

    assert_success(&output);
    assert!(
        production_section(&output, "import_cycles").contains("  modules_in_import_cycles: 2\n")
    );
    assert_eq!(
        packages_table(&output, "production"),
        PACKAGES_HEADER.to_owned()
            + concat!(
                "  .,0,0,0,0,0,0,0,0,0,0,0\n",
                "  packages/billing,2,3,0,0,1,0,0,0,0,0.5,1\n",
                "  packages/orders,1,2,0,0,1,0,0,0,0,1,1\n",
            )
    );
}

#[test]
fn a_member_manifest_that_is_not_utf_8_still_makes_its_folder_a_workspace_package() {
    let repository = Repository::new();
    repository.write("package.json", "{\"workspaces\": [\"packages/*\"]}\n");
    repository.write("packages/cafe/index.ts", "export const cafe = 1;\n");
    repository.write_bytes("packages/cafe/package.json", b"{\"name\": \"caf\xe9\"}\n");
    repository.write("packages/shop/package.json", "{\"name\": \"shop\"}\n");
    repository.write("packages/shop/index.ts", "export const shop = 1;\n");
    repository.commit_all();

    let output = repository.codeint(&["check"]);

    assert_success(&output);
    assert_eq!(
        packages_table(&output, "production"),
        PACKAGES_HEADER.to_owned()
            + concat!(
                "  .,0,0,0,0,0,0,0,0,0,0,0\n",
                "  packages/cafe,1,1,0,0,0,0,0,0,0,0,0\n",
                "  packages/shop,1,1,0,0,0,0,0,0,0,0,0\n",
            )
    );
}

#[test]
fn the_json_report_lists_each_workspace_package_with_its_numerators() {
    let repository = Repository::new();
    write_two_package_workspace(&repository, "{\"workspaces\": [\"packages/*\"]}\n");
    repository.commit_all();

    let output = repository.codeint(&["check", "--format", "json"]);

    assert_success(&output);
    let report: serde_json::Value =
        serde_json::from_str(&stdout(&output)).expect("the JSON report parses");
    let packages = &report["source_sets"]["production"]["packages"];
    assert_eq!(
        packages[1],
        serde_json::json!({
            "path": "packages/eroded",
            "files": 1,
            "source_lines": 14,
            "erosion": {
                "ratio": 1.0,
                "eroded_function_mass": 44.8999,
                "total_function_mass": 44.8999
            },
            "duplication": { "ratio": 0.0, "clone_lines": 0 },
            "verbosity": { "ratio": 0.0, "verbose_lines": 0 },
            "import_cycles": { "ratio": 0.0, "modules_in_import_cycles": 0 }
        })
    );
    assert_eq!(packages.as_array().map(Vec::len), Some(3));
}

fn repository_with_budgets(budgets: &str) -> Repository {
    let repository = Repository::new();
    repository.write("codeint.json", &format!("{{ \"budgets\": {budgets} }}\n"));
    repository.write("src/eleven.ts", &function_with_if_statements("eleven", 11));
    repository.commit_all();
    repository
}

fn stderr(output: &Output) -> String {
    String::from_utf8(output.stderr.clone()).expect("standard error is UTF-8")
}

fn assert_over_budget(output: &Output, report: &str, failures: &[&str]) {
    assert_eq!(output.status.code(), Some(1));
    let named: String = failures
        .iter()
        .map(|failure| format!("codeint: {failure}\n"))
        .collect();
    assert_eq!(stderr(output), named);
    assert!(
        report.ends_with(&over_budget_rows(failures.len())),
        "unexpected report:\n{report}"
    );
}

fn over_budget_rows(count: usize) -> String {
    format!("over_budget[{count}]{{source_set,metric,ratio,budget}}:\n")
}

fn check_with_a_production_erosion_budget_of_half(args: &[&str]) -> (Repository, Output) {
    let repository = repository_with_budgets(r#"{ "production": { "erosion": 0.5 } }"#);
    let output = repository.codeint(args);
    (repository, output)
}

const PRODUCTION_EROSION_OVER_HALF: &str = "  production,erosion,1,0.5\n";

fn assert_production_erosion_over_its_budget_of_half(output: &Output, report: &str) {
    assert!(
        report.ends_with(PRODUCTION_EROSION_OVER_HALF),
        "unexpected report:\n{report}"
    );
    assert_over_budget(
        output,
        report.trim_end_matches(PRODUCTION_EROSION_OVER_HALF),
        &["the production erosion ratio 1 is over its Budget 0.5"],
    );
}

fn assert_within_budget(output: &Output) {
    assert_success(output);
    assert_eq!(stderr(output), "");
    assert!(!stdout(output).contains("over_budget"));
}

#[test]
fn a_production_erosion_over_its_budget_is_listed_named_on_standard_error_and_exits_1() {
    let (_repository, output) = check_with_a_production_erosion_budget_of_half(&["check"]);

    let report = stdout(&output);
    assert_production_erosion_over_its_budget_of_half(&output, &report);
}

#[test]
fn a_ratio_equal_to_its_budget_as_the_report_shows_it_passes_though_its_unrounded_value_is_higher()
{
    let repository = Repository::new();
    repository.write(
        "codeint.json",
        r#"{ "budgets": { "production": { "erosion": 0.9667 } } }"#,
    );
    repository.write("src/twelve.ts", &function_with_if_statements("twelve", 12));
    repository.write(
        "src/small.ts",
        "export function small(): number {\n  return 1;\n}\n",
    );
    repository.commit_all();

    let output = repository.codeint(&["check"]);

    assert_within_budget(&output);
    assert!(
        production_section(&output, "erosion").contains("  ratio: 0.9667\n"),
        "the report shows the ratio rounded down to the Budget"
    );
}

#[test]
fn a_test_budget_does_not_apply_to_the_production_source_set() {
    let repository = repository_with_budgets(r#"{ "test": { "erosion": 0 } }"#);

    let output = repository.codeint(&["check"]);

    assert_within_budget(&output);
}

#[test]
fn every_metric_over_its_budget_is_named_in_source_set_and_metric_order() {
    let repository = repository_with_budgets(
        r#"{
  "test": { "erosion": 0.5 },
  "production": { "import_cycles": 0, "erosion": 0.25, "duplication": 1 }
}"#,
    );
    repository.write(
        "src/eleven.test.ts",
        &function_with_if_statements("eleven", 11),
    );
    repository.write("src/ring.ts", "import './ring';\nexport const ring = 1;\n");
    repository.commit_all();

    let output = repository.codeint(&["check"]);

    let report = stdout(&output);
    let rows = concat!(
        "  production,erosion,1,0.25\n",
        "  production,import_cycles,0.5,0\n",
        "  test,erosion,1,0.5\n",
    );
    assert!(report.ends_with(rows), "unexpected report:\n{report}");
    assert_over_budget(
        &output,
        report.trim_end_matches(rows),
        &[
            "the production erosion ratio 1 is over its Budget 0.25",
            "the production import_cycles ratio 0.5 is over its Budget 0",
            "the test erosion ratio 1 is over its Budget 0.5",
        ],
    );
}

#[test]
fn a_metric_over_its_budget_exits_1_after_it_writes_the_report_to_the_output_file() {
    let (repository, output) =
        check_with_a_production_erosion_budget_of_half(&["check", "--output", "report.toon"]);

    assert_eq!(stdout(&output), "");
    let report =
        fs::read_to_string(repository.path().join("report.toon")).expect("read the report");
    assert_production_erosion_over_its_budget_of_half(&output, &report);
}

fn assert_budget_out_of_range(budgets: &str, key: &str, found: &str) {
    assert_codeint_json_is_rejected(
        &format!(r#"{{ "budgets": {budgets} }}"#),
        &format!("codeint.json: {key} must be between 0 and 1, not {found}"),
        "Set the value to a ratio from 0 to 1, as the report shows ratios.",
    );
}

#[test]
fn a_budget_above_1_names_the_key_and_the_allowed_range_and_exits_2() {
    assert_budget_out_of_range(
        r#"{ "production": { "erosion": 1.5 } }"#,
        "budgets.production.erosion",
        "1.5",
    );
}

#[test]
fn a_negative_budget_names_the_key_and_the_allowed_range_and_exits_2() {
    assert_budget_out_of_range(
        r#"{ "test": { "duplication": -1 } }"#,
        "budgets.test.duplication",
        "-1",
    );
}

#[test]
fn a_budget_for_an_unknown_metric_names_the_metrics_and_exits_2() {
    assert_codeint_json_is_rejected(
        r#"{ "budgets": { "production": { "complexity": 0.1 } } }"#,
        "codeint.json has an unknown key budgets.production.complexity; \
         the keys allowed there are erosion, duplication, verbosity, import_cycles",
        UNKNOWN_KEY_REMEDY,
    );
}

#[test]
fn a_budget_that_is_not_a_number_names_the_key_and_exits_2() {
    assert_codeint_json_is_rejected(
        r#"{ "budgets": { "production": { "erosion": "10%" } } }"#,
        "codeint.json: budgets.production.erosion must be a number, not a string",
        WRONG_TYPE_REMEDY,
    );
}

#[test]
fn format_markdown_lists_each_metric_over_its_budget_after_the_metric_table_and_exits_1() {
    let repository = repository_with_budgets(r#"{ "production": { "erosion": 0.5 } }"#);

    let output = repository.codeint(&["check", "--format", "markdown"]);

    assert_eq!(output.status.code(), Some(1));
    assert!(
        stdout(&output).contains(
            "| 0 to 0.308, source: codeint panel of 20 TypeScript and JavaScript repositories, ADR 0008, codeint 0.1.0 |\n\
             \n\
             ## Over Budget\n\
             \n\
             - The production erosion ratio 1 is over its Budget 0.5.\n\
             \n\
             ## Production Findings\n"
        ),
        "unexpected report: {}",
        stdout(&output)
    );
}

#[test]
fn format_markdown_lists_the_ratios_of_each_workspace_package_after_the_metric_table() {
    let repository = Repository::new();
    write_two_package_workspace(&repository, "{\"workspaces\": [\"packages/*\"]}\n");
    repository.commit_all();

    let output = repository.codeint(&["check", "--format", "markdown"]);

    assert_success(&output);
    assert!(
        stdout(&output).contains(
            "| 0 to 0.308, source: codeint panel of 20 TypeScript and JavaScript repositories, ADR 0008, codeint 0.1.0 |\n\
             \n\
             ## Workspace packages\n\
             \n\
             | Source set | Package | Files | Source lines | Erosion | Duplication | Verbosity | Import cycles |\n\
             | ---------- | ------- | ----- | ------------ | ------- | ----------- | --------- | ------------- |\n\
             | Production | `.` | 1 | 1 | 0 | 0 | 0 | 0 |\n\
             | Production | `packages/eroded` | 1 | 14 | 1 | 0 | 0 | 0 |\n\
             | Production | `packages/small` | 1 | 6 | 0 | 0 | 0 | 0 |\n\
             \n\
             ## Production Findings\n"
        ),
        "unexpected report: {}",
        stdout(&output)
    );
}

fn json_report(repository: &Repository) -> serde_json::Value {
    let output = repository.codeint(&["check", "--format", "json"]);
    assert_success(&output);
    serde_json::from_str(&stdout(&output)).expect("the JSON report parses")
}

fn finding_ids(report: &serde_json::Value, metric: &str, list: &str) -> Vec<String> {
    report["source_sets"]["production"][metric][list]
        .as_array()
        .expect("a list of Findings")
        .iter()
        .map(|finding| {
            finding["id"]
                .as_str()
                .expect("every Finding has an id")
                .to_owned()
        })
        .collect()
}

fn eroded_function_ids(repository: &Repository) -> Vec<String> {
    finding_ids(&json_report(repository), "erosion", "findings")
}

#[test]
fn moving_an_eroded_function_down_its_file_keeps_its_finding_id() {
    let repository = Repository::new();
    let eroded = function_with_if_statements("eroded", 11);
    repository.write("src/eroded.ts", &eroded);
    repository.commit_all();
    let before = eroded_function_ids(&repository);

    repository.write(
        "src/eroded.ts",
        &format!("export const first = 1;\n\n\nexport const second = 2;\n{eroded}"),
    );
    let after = eroded_function_ids(&repository);

    assert_eq!(before.len(), 1);
    assert_eq!(after, before);
}

#[test]
fn changing_the_body_of_an_eroded_function_changes_its_finding_id() {
    let repository = Repository::new();
    let eroded = function_with_if_statements("eroded", 11);
    repository.write("src/eroded.ts", &eroded);
    repository.commit_all();
    let before = eroded_function_ids(&repository);

    repository.write("src/eroded.ts", &eroded.replace("return 0;", "return -1;"));
    let after = eroded_function_ids(&repository);

    assert_eq!(after.len(), 1);
    assert_ne!(after, before);
}

#[test]
fn a_finding_id_is_the_same_in_every_run_on_every_platform_and_with_crlf_line_endings() {
    let eroded = function_with_if_statements("eroded", 11);
    let expected = vec!["aab96e99646c4ad1".to_owned()];
    for text in [eroded.clone(), eroded.replace('\n', "\r\n")] {
        let repository = Repository::new();
        repository.write("src/eroded.ts", &text);
        repository.commit_all();

        assert_eq!(eroded_function_ids(&repository), expected);
        assert_eq!(eroded_function_ids(&repository), expected);
    }
}

#[test]
fn two_identical_eroded_functions_in_one_file_have_distinct_finding_ids() {
    let repository = Repository::new();
    let handler = function_with_if_statements("eroded", 11)
        .replace("export function eroded", "  function")
        .replace("\n}\n", "\n},\n");
    repository.write(
        "src/handlers.ts",
        &format!("export const handlers = [\n{handler}{handler}];\n"),
    );
    repository.write(
        "src/other.ts",
        &format!("export const handlers = [\n{handler}];\n"),
    );
    repository.commit_all();

    let mut ids = eroded_function_ids(&repository);
    ids.sort();
    ids.dedup();

    assert_eq!(ids.len(), 3);
}

#[test]
fn moving_a_flagged_line_down_its_file_keeps_its_finding_id() {
    let sign = "export function sign(value: number): number {\n  if (value < 0) {\n    return -1;\n  } else {\n    return 1;\n  }\n}\n";
    let repository = Repository::new();
    repository.write("src/sign.ts", sign);
    repository.commit_all();
    let before = finding_ids(&json_report(&repository), "verbosity", "findings");

    repository.write("src/sign.ts", &format!("export const one = 1;\n\n{sign}"));
    let after = finding_ids(&json_report(&repository), "verbosity", "findings");

    assert_eq!(before.len(), 1);
    assert_eq!(after, before);
}

#[test]
fn moving_a_copy_of_a_clone_group_down_its_file_keeps_its_finding_id() {
    let repository = Repository::new();
    repository.write("src/cart.ts", CART_TOTAL);
    repository.write("src/invoice.ts", CART_TOTAL);
    repository.commit_all();
    let before = finding_ids(&json_report(&repository), "duplication", "clone_groups");

    repository.write(
        "src/invoice.ts",
        &format!("export const currency = 'EUR';\n\n{CART_TOTAL}"),
    );
    let after = finding_ids(&json_report(&repository), "duplication", "clone_groups");

    assert_eq!(before.len(), 1);
    assert_eq!(after, before);
}

#[test]
fn an_import_cycle_keeps_its_finding_id_when_its_modules_change_and_loses_it_when_a_module_leaves()
{
    let repository = Repository::new();
    repository.write(
        "src/a.ts",
        "import { b } from './b';\nexport const a = () => b();\n",
    );
    repository.write(
        "src/b.ts",
        "import { a } from './a';\nexport const b = () => a();\n",
    );
    repository.write("src/c.ts", "export const c = () => 3;\n");
    repository.commit_all();
    let before = finding_ids(&json_report(&repository), "import_cycles", "findings");

    repository.write(
        "src/b.ts",
        "import { a } from './a';\n\nexport const b = () => a() + 1;\n",
    );
    let edited = finding_ids(&json_report(&repository), "import_cycles", "findings");
    repository.write(
        "src/b.ts",
        "import { c } from './c';\nexport const b = () => c();\n",
    );
    repository.write(
        "src/c.ts",
        "import { a } from './a';\nexport const c = () => a();\n",
    );
    let longer = finding_ids(&json_report(&repository), "import_cycles", "findings");

    assert_eq!(before.len(), 1);
    assert_eq!(edited, before);
    assert_eq!(longer.len(), 1);
    assert_ne!(longer, before);
}

const SARIF_RULES: &str = r#"          "rules": [
            {
              "id": "erosion",
              "name": "Erosion",
              "shortDescription": {
                "text": "A function with cyclomatic complexity over 10"
              },
              "fullDescription": {
                "text": "Erosion is the share of total Function mass held by functions whose cyclomatic complexity is greater than 10. Function mass is cyclomatic complexity times the square root of source lines."
              },
              "help": {
                "text": "Split the function into smaller functions, each with a cyclomatic complexity of 10 or less."
              }
            },
            {
              "id": "duplication",
              "name": "Duplication",
              "shortDescription": {
                "text": "A Clone group: code fragments with the same token sequence"
              },
              "fullDescription": {
                "text": "Duplication is the share of source lines that belong to at least one Clone group. A Clone group is two or more code fragments with the same token sequence once identifiers and literals are normalized."
              },
              "help": {
                "text": "Extract the fragments of the Clone group into one shared function or module."
              }
            },
            {
              "id": "verbosity",
              "name": "Verbosity",
              "shortDescription": {
                "text": "A Flagged line: code that can be deleted or collapsed"
              },
              "fullDescription": {
                "text": "Verbosity is the share of source lines that are Flagged lines, clone lines, or both. A Flagged line matches a verbose-code pattern of the source language, and the result names the rule it matches."
              },
              "help": {
                "text": "Delete or collapse the line as its rule describes; the behavior stays the same."
              }
            },
            {
              "id": "import_cycles",
              "name": "Import cycles",
              "shortDescription": {
                "text": "An Import cycle among runtime imports"
              },
              "fullDescription": {
                "text": "Import cycles is the share of modules that belong to at least one Import cycle. An Import cycle is a group of modules in which every module can reach every other module through runtime imports."
              },
              "help": {
                "text": "Break the Import cycle, for example by moving the code that the modules share into a module that imports none of them."
              }
            }
          ]"#;

#[test]
fn format_sarif_prints_a_sarif_2_1_0_log_with_one_result_per_finding_and_a_rule_per_metric() {
    let repository = repository_with_a_finding_a_skipped_file_and_a_declaration_file();

    let output = repository.codeint(&["check", "--format", "sarif"]);

    assert_success(&output);
    assert_eq!(
        stdout(&output),
        format!(
            r#"{{
  "$schema": "https://docs.oasis-open.org/sarif/sarif/v2.1.0/errata01/os/schemas/sarif-schema-2.1.0.json",
  "version": "2.1.0",
  "runs": [
    {{
      "tool": {{
        "driver": {{
          "name": "codeint",
          "version": "{version}",
          "informationUri": "https://github.com/avidianity/codeint",
{SARIF_RULES}
        }}
      }},
      "originalUriBaseIds": {{
        "SRCROOT": {{
          "description": {{
            "text": "The root of the Git repository"
          }}
        }}
      }},
      "results": [
        {{
          "ruleId": "verbosity",
          "ruleIndex": 2,
          "level": "note",
          "message": {{
            "text": "Flagged line: rule else-after-return."
          }},
          "locations": [
            {{
              "physicalLocation": {{
                "artifactLocation": {{
                  "uri": "src/sign.ts",
                  "uriBaseId": "SRCROOT"
                }},
                "region": {{
                  "startLine": 4
                }}
              }}
            }}
          ],
          "partialFingerprints": {{
            "codeintFindingId": "2a57800b29941394"
          }},
          "properties": {{
            "sourceSet": "production"
          }}
        }}
      ]
    }}
  ]
}}
"#,
            version = env!("CARGO_PKG_VERSION"),
        )
    );
}

fn repository_with_a_finding_of_every_metric_in_nested_folders() -> Repository {
    let repository = Repository::new();
    repository.write("packages/web/src/sign.ts", SIGN_WITH_AN_ELSE_AFTER_RETURN);
    repository.write(
        "packages/web/src/eroded.ts",
        &function_with_if_statements("eroded", 12),
    );
    repository.write("packages/web/src/cart.ts", CART_TOTAL);
    repository.write("packages/api/lib/order.ts", CART_TOTAL);
    repository.write(
        "packages/web/src/a.ts",
        "import { b } from '../../api/lib/b';\nexport const a = b;\n",
    );
    repository.write(
        "packages/api/lib/b.ts",
        "import { a } from '../../web/src/a';\nexport const b = a;\n",
    );
    repository.write(
        "packages/web/test/sign.test.ts",
        &function_with_if_statements("tested", 11),
    );
    repository.commit_all();
    repository
}

type FindingLocation = (String, String, String, u64);

fn finding_locations_of_the_json_report(report: &serde_json::Value) -> Vec<FindingLocation> {
    let mut locations = Vec::new();
    for (source_set, section) in report["source_sets"].as_object().expect("source sets") {
        let rows = |metric: &str, list: &str| {
            section[metric][list]
                .as_array()
                .expect("a Finding list")
                .clone()
        };
        let mut add = |metric: &str, path: &serde_json::Value, line: u64| {
            locations.push((
                source_set.clone(),
                metric.to_owned(),
                path.as_str().expect("a path").to_owned(),
                line,
            ));
        };
        for finding in rows("erosion", "findings") {
            add(
                "erosion",
                &finding["path"],
                finding["line"].as_u64().expect("a line"),
            );
        }
        for group in rows("duplication", "clone_groups") {
            let first = &group["locations"][0];
            add(
                "duplication",
                &first["path"],
                first["start_line"].as_u64().expect("a line"),
            );
        }
        for finding in rows("verbosity", "findings") {
            add(
                "verbosity",
                &finding["path"],
                finding["line"].as_u64().expect("a line"),
            );
        }
        for cycle in rows("import_cycles", "findings") {
            add("import_cycles", &cycle["modules"][0], 1);
        }
    }
    locations.sort();
    locations
}

fn primary_locations_of_the_sarif_log(log: &serde_json::Value) -> Vec<FindingLocation> {
    let mut locations: Vec<FindingLocation> = log["runs"][0]["results"]
        .as_array()
        .expect("results")
        .iter()
        .map(|result| {
            let location = &result["locations"][0]["physicalLocation"];
            (
                result["properties"]["sourceSet"]
                    .as_str()
                    .expect("a Source set")
                    .to_owned(),
                result["ruleId"].as_str().expect("a rule id").to_owned(),
                location["artifactLocation"]["uri"]
                    .as_str()
                    .expect("a uri")
                    .to_owned(),
                location["region"]["startLine"]
                    .as_u64()
                    .expect("a start line"),
            )
        })
        .collect();
    locations.sort();
    locations
}

#[test]
fn every_sarif_result_has_the_metric_rule_and_the_repository_relative_path_and_line_of_its_finding()
{
    let repository = repository_with_a_finding_of_every_metric_in_nested_folders();
    let subfolder = repository.path().join("packages/web");

    let json = run_codeint(&subfolder, &["check", "--format", "json"]);
    let sarif = run_codeint(&subfolder, &["check", "--format", "sarif"]);

    assert_success(&json);
    assert_success(&sarif);
    let report: serde_json::Value =
        serde_json::from_str(&stdout(&json)).expect("the JSON report parses");
    let log: serde_json::Value =
        serde_json::from_str(&stdout(&sarif)).expect("the SARIF log parses");
    let from_json = finding_locations_of_the_json_report(&report);
    let metrics: BTreeSet<&str> = from_json
        .iter()
        .map(|(_, metric, _, _)| metric.as_str())
        .collect();
    assert_eq!(
        metrics,
        BTreeSet::from(["duplication", "erosion", "import_cycles", "verbosity"]),
        "the fixture has a Finding of every Metric: {report}"
    );
    assert_eq!(primary_locations_of_the_sarif_log(&log), from_json);
}

#[test]
fn a_clone_group_and_an_import_cycle_list_their_other_locations_as_related_locations() {
    let repository = repository_with_a_finding_of_every_metric_in_nested_folders();

    let output = repository.codeint(&["check", "--format", "sarif"]);

    assert_success(&output);
    let log: serde_json::Value =
        serde_json::from_str(&stdout(&output)).expect("the SARIF log parses");
    let results = log["runs"][0]["results"].as_array().expect("results");
    let result_of = |rule: &str| {
        results
            .iter()
            .find(|result| result["ruleId"] == rule)
            .unwrap_or_else(|| panic!("a {rule} result in {log}"))
    };
    let related = |rule: &str| -> Vec<(u64, String, u64, Option<u64>)> {
        result_of(rule)["relatedLocations"]
            .as_array()
            .expect("related locations")
            .iter()
            .map(|location| {
                let physical = &location["physicalLocation"];
                (
                    location["id"].as_u64().expect("an id"),
                    physical["artifactLocation"]["uri"]
                        .as_str()
                        .expect("a uri")
                        .to_owned(),
                    physical["region"]["startLine"].as_u64().expect("a line"),
                    physical["region"]["endLine"].as_u64(),
                )
            })
            .collect()
    };
    assert_eq!(
        result_of("duplication")["message"]["text"],
        "Clone group of 83 tokens in 2 fragments; the other fragments: packages/web/src/cart.ts lines 1-9."
    );
    assert_eq!(
        related("duplication"),
        vec![(1, "packages/web/src/cart.ts".to_owned(), 1, Some(9))]
    );
    assert_eq!(
        result_of("import_cycles")["message"]["text"],
        "Import cycle of 2 modules: packages/api/lib/b.ts, packages/web/src/a.ts."
    );
    assert_eq!(
        related("import_cycles"),
        vec![(1, "packages/web/src/a.ts".to_owned(), 1, None)]
    );
}

#[test]
fn an_eroded_function_result_names_its_cyclomatic_complexity_source_lines_and_function_mass() {
    let repository = Repository::new();
    repository.write("src/eroded.ts", &function_with_if_statements("eroded", 12));
    repository.commit_all();

    let output = repository.codeint(&["check", "--format", "sarif"]);

    assert_success(&output);
    let log: serde_json::Value =
        serde_json::from_str(&stdout(&output)).expect("the SARIF log parses");
    assert_eq!(
        log["runs"][0]["results"][0]["message"]["text"],
        "Eroded function: cyclomatic complexity 13, over 10; 15 source lines; Function mass 50.3488."
    );
}

#[test]
fn a_sarif_location_percent_encodes_a_space_in_the_path() {
    let repository = Repository::new();
    repository.write("src/my sign.ts", SIGN_WITH_AN_ELSE_AFTER_RETURN);
    repository.commit_all();

    let output = repository.codeint(&["check", "--format", "sarif"]);

    assert_success(&output);
    let log: serde_json::Value =
        serde_json::from_str(&stdout(&output)).expect("the SARIF log parses");
    assert_eq!(
        log["runs"][0]["results"][0]["locations"][0]["physicalLocation"]["artifactLocation"]["uri"],
        "src/my%20sign.ts"
    );
}

#[test]
fn a_repository_without_findings_prints_a_sarif_log_with_no_results() {
    let repository = Repository::new();
    repository.write("src/one.ts", "export const one = 1;\n");
    repository.commit_all();

    let output = repository.codeint(&["check", "--format", "sarif"]);

    assert_success(&output);
    let log: serde_json::Value =
        serde_json::from_str(&stdout(&output)).expect("the SARIF log parses");
    assert_eq!(log["runs"][0]["results"], serde_json::json!([]));
    assert_eq!(
        log["runs"][0]["tool"]["driver"]["rules"]
            .as_array()
            .map(Vec::len),
        Some(4)
    );
}

#[test]
fn format_sarif_follows_all_and_output_like_every_other_format() {
    let repository = Repository::new();
    write_eroded_functions_with_more_if_statements_in_later_files(&repository, 21);
    repository.commit_all();

    let top = repository.codeint(&["check", "--format", "sarif"]);
    let all = repository.codeint(&["check", "--format", "sarif", "--all"]);
    let to_file = repository.codeint(&[
        "check",
        "--format",
        "sarif",
        "--all",
        "--output",
        "codeint.sarif",
    ]);

    assert_success(&top);
    assert_success(&all);
    assert_success(&to_file);
    let eroded_functions = |output: &Output| {
        serde_json::from_str::<serde_json::Value>(&stdout(output)).expect("the SARIF log parses")
            ["runs"][0]["results"]
            .as_array()
            .expect("results")
            .iter()
            .filter(|result| result["ruleId"] == "erosion")
            .count()
    };
    assert_eq!((eroded_functions(&top), eroded_functions(&all)), (20, 21));
    assert_eq!(stdout(&to_file), "");
    assert_eq!(
        fs::read_to_string(repository.path().join("codeint.sarif")).expect("read the report"),
        stdout(&all)
    );
}

fn assert_valid_sarif_location(location: &serde_json::Value, base_ids: &serde_json::Value) {
    let artifact = &location["physicalLocation"]["artifactLocation"];
    let uri = artifact["uri"].as_str().expect("a uri");
    assert!(
        !uri.is_empty()
            && !uri.starts_with('/')
            && !uri.contains(['\\', ' ', ':'])
            && base_ids
                .get(artifact["uriBaseId"].as_str().expect("a uriBaseId"))
                .is_some(),
        "a relative uri reference with a declared uriBaseId: {location}"
    );
    let region = &location["physicalLocation"]["region"];
    let start_line = region["startLine"].as_u64().expect("a start line");
    assert!(start_line >= 1, "a positive start line: {location}");
    if let Some(end_line) = region["endLine"].as_u64() {
        assert!(
            end_line >= start_line,
            "an end line after the start line: {location}"
        );
    }
}

#[test]
fn the_sarif_log_holds_every_property_and_value_that_sarif_2_1_0_requires() {
    let repository = repository_with_a_finding_of_every_metric_in_nested_folders();

    let output = repository.codeint(&["check", "--format", "sarif"]);

    assert_success(&output);
    let log: serde_json::Value =
        serde_json::from_str(&stdout(&output)).expect("the SARIF log parses");
    let top_level: BTreeSet<&str> = log
        .as_object()
        .expect("an object")
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(top_level, BTreeSet::from(["$schema", "runs", "version"]));
    assert_eq!(log["version"], "2.1.0");
    let run = &log["runs"][0];
    assert_eq!(log["runs"].as_array().map(Vec::len), Some(1));
    assert_eq!(run["tool"]["driver"]["name"], "codeint");
    let rules = run["tool"]["driver"]["rules"].as_array().expect("rules");
    let rule_ids: BTreeSet<&str> = rules
        .iter()
        .map(|rule| rule["id"].as_str().expect("a rule id"))
        .collect();
    assert_eq!(rule_ids.len(), rules.len(), "unique rule ids");
    let base_ids = &run["originalUriBaseIds"];
    let results = run["results"].as_array().expect("results");
    assert!(!results.is_empty(), "the fixture has results");
    for result in results {
        let rule_index = usize::try_from(result["ruleIndex"].as_u64().expect("a rule index"))
            .expect("a small rule index");
        assert_eq!(result["ruleId"], rules[rule_index]["id"], "{result}");
        assert!(
            ["none", "note", "warning", "error"]
                .contains(&result["level"].as_str().expect("a level")),
            "{result}"
        );
        assert!(
            !result["message"]["text"]
                .as_str()
                .expect("a message")
                .is_empty(),
            "{result}"
        );
        let locations = result["locations"].as_array().expect("locations");
        assert_eq!(locations.len(), 1, "{result}");
        let related = result["relatedLocations"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        for location in locations.iter().chain(&related) {
            assert_valid_sarif_location(location, base_ids);
        }
    }
}

fn assert_built_in_workspace_cycle_report(
    repository: &Repository,
    expected_files: u64,
    expected_package: &str,
) {
    let output = repository.codeint(&["check", "--format", "json"]);
    assert_success(&output);
    let report: serde_json::Value = serde_json::from_str(&stdout(&output)).expect("valid json");
    let prod = &report["source_sets"]["production"];
    assert_eq!(prod["files"], expected_files);
    assert!(prod["verbosity"]["flagged_lines"].as_u64().unwrap_or(0) >= 1);
    assert_eq!(prod["import_cycles"]["modules_in_import_cycles"], 2);
    let packages = prod["packages"].as_array().expect("packages");
    assert!(packages.iter().any(|p| p["path"] == expected_package));
}

#[test]
fn built_in_rust_plugin_measures_cargo_workspace_metrics_and_cycles() {
    let repository = Repository::new();
    repository.write("Cargo.toml", "[workspace]\nmembers = [\"crates/*\"]\n");
    repository.write("crates/app/Cargo.toml", "[package]\nname = \"app\"\n");
    repository.write(
        "crates/app/src/lib.rs",
        concat!(
            "mod a;\n",
            "mod b;\n",
            "pub fn verbose(x: bool) -> bool {\n",
            "    if x { true } else { false }\n",
            "}\n",
            "#[cfg(test)]\n",
            "mod tests {\n",
            "    fn inline_test() {\n",
            "        if true { return; } else { return; }\n",
            "    }\n",
            "}\n",
        ),
    );
    repository.write(
        "crates/app/src/a.rs",
        "use crate::b::second;\npub fn first() -> i32 { second() }\n",
    );
    repository.write(
        "crates/app/src/b.rs",
        "use crate::a::first;\npub fn second() -> i32 { first() }\n",
    );
    repository.commit_all();
    assert_built_in_workspace_cycle_report(&repository, 3, "crates/app");
}

#[test]
fn built_in_php_plugin_measures_composer_workspace_metrics_and_psr4_cycles() {
    let repository = Repository::new();
    repository.write(
        "composer.json",
        r#"{"repositories":[{"type":"path","url":"modules/*"}]}"#,
    );
    repository.write(
        "modules/core/composer.json",
        r#"{"name":"demo/core","autoload":{"psr-4":{"Demo\\Core\\":"src/"}}}"#,
    );
    repository.write(
        "modules/core/src/Alpha.php",
        "<?php\nnamespace Demo\\Core;\nuse Demo\\Core\\Beta;\nclass Alpha { public function check(bool $x): bool { if ($x) { return true; } else { return false; } } }\n",
    );
    repository.write(
        "modules/core/src/Beta.php",
        "<?php\nnamespace Demo\\Core;\nuse Demo\\Core\\Alpha;\nclass Beta { public function run(): int { return 1; } }\n",
    );
    repository.commit_all();
    assert_built_in_workspace_cycle_report(&repository, 2, "modules/core");
}

#[test]
fn built_in_dart_plugin_measures_pub_workspace_metrics_and_cycles() {
    let repository = Repository::new();
    for (path, body) in [
        (
            "pubspec.yaml",
            "name: ws\nworkspace:\n  - packages/mobile\n",
        ),
        ("packages/mobile/pubspec.yaml", "name: mobile\n"),
        (
            "packages/mobile/lib/alpha.dart",
            "import 'package:mobile/beta.dart';\nbool check(bool x) { if (x) { return true; } else { return false; } }\n",
        ),
        (
            "packages/mobile/lib/beta.dart",
            "import 'package:mobile/alpha.dart';\nint run() => 1;\n",
        ),
        (
            "packages/mobile/lib/alpha.g.dart",
            "int generatedHelper() => 42;\n",
        ),
    ] {
        repository.write(path, body);
    }
    repository.commit_all();
    assert_built_in_workspace_cycle_report(&repository, 2, "packages/mobile");
}
