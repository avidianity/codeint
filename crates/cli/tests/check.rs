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
        .map(|(_, line)| format!("{}\n", &line[METRIC_INDENT..]))
        .collect()
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
         reference_range: no published Reference range for this language\n      \
         eroded_function_mass: 0\n      \
         total_function_mass: 0\n      \
         findings: []\n    \
         duplication:\n      \
         ratio: 0\n      \
         reference_range: no published Reference range for this language\n      \
         clone_lines: 0\n      \
         clone_groups: []\n    \
         verbosity:\n      \
         ratio: 0\n      \
         reference_range: no published Reference range for this language\n      \
         verbose_lines: 0\n      \
         findings: []\n    \
         import_cycles:\n      \
         ratio: 0\n      \
         reference_range: no published Reference range for this language\n      \
         modules_in_import_cycles: 0\n      \
         modules: {files}\n      \
         findings: []\n      \
         unresolved_imports:\n        \
         count: 0\n        \
         imports: []\n"
    )
}

#[test]
fn check_reports_verbosity_as_the_share_of_source_lines_that_are_flagged_and_names_each_rule() {
    let repository = Repository::new();
    repository.write(
        "src/sign.ts",
        "export function sign(value: number): string {
  if (value < 0) {
    return 'negative';
  } else {
    return 'positive';
  }
}
",
    );
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
         reference_range: no published Reference range for this language\n  \
         verbose_lines: 3\n  \
         findings[3]{path,line,rule}:\n    \
         src/sign.ts,4,else-after-return\n    \
         src/size.ts,4,else-after-return\n    \
         src/size.ts,7,else-after-return\n"
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
    repository.write("scripts/tool.py", "print('codeint')\n");
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
            "  reference_range: no published Reference range for this language\n",
            "  eroded_function_mass: 48\n",
            "  total_function_mass: 54\n",
            "  findings[1]{path,line,cyclomatic_complexity,source_lines,function_mass}:\n",
            "    src/mixed.ts,1,12,16,48\n",
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
            "  reference_range: no published Reference range for this language\n",
            "  eroded_function_mass: 84.561\n",
            "  total_function_mass: 119.202\n",
            "  findings[2]{path,line,cyclomatic_complexity,source_lines,function_mass}:\n",
            "    src/eleven.ts,1,12,14,44.8999\n",
            "    src/ten.ts,1,11,13,39.6611\n",
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
            "  reference_range: no published Reference range for this language\n",
            "  eroded_function_mass: 0\n",
            "  total_function_mass: 0\n",
            "  findings: []\n",
        )
    );
}

#[test]
fn the_typescript_and_javascript_report_says_that_no_published_reference_range_exists_for_each_metric()
 {
    let repository = Repository::new();
    repository.write("src/eleven.ts", &function_with_if_statements("eleven", 11));
    repository.write("src/sum.js", "export const sum = (a, b) => a + b;\n");
    repository.write("src/sum.test.js", "export const one = 1;\n");
    repository.commit_all();

    let output = repository.codeint(&["check"]);

    assert_success(&output);
    let report = stdout(&output);
    let metric_sections = report
        .lines()
        .filter(|line| indent(line) == METRIC_INDENT && line.ends_with(':'))
        .count();
    let unpublished = report
        .lines()
        .filter(|line| {
            *line == "      reference_range: no published Reference range for this language"
        })
        .count();
    assert!(metric_sections >= 6, "unexpected report: {report}");
    assert_eq!(unpublished, metric_sections, "unexpected report: {report}");
    assert!(!report.contains("low:"), "unexpected report: {report}");
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
            "  reference_range: no published Reference range for this language\n",
            "  eroded_function_mass: 43.2666\n",
            "  total_function_mass: 44.6808\n",
            "  findings[1]{path,line,cyclomatic_complexity,source_lines,function_mass}:\n",
            "    src/outer.ts,2,12,13,43.2666\n",
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
    format!("  findings[{count}]{{path,line,cyclomatic_complexity,source_lines,function_mass}}:")
}

fn erosion_findings_lines(output: &Output) -> Vec<String> {
    production_section(output, "erosion")
        .lines()
        .skip_while(|line| !line.starts_with("  findings"))
        .map(|line| match line.strip_prefix("    ") {
            Some(row) => format!(
                "    {}",
                row.splitn(3, ',').take(2).collect::<Vec<_>>().join(",")
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
            .map(|index| format!("    src/f{index:02}.ts,1")),
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
            .map(|index| format!("    src/f{index:02}.ts,1")),
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
        "    src/t00.ts,1".to_string(),
        "    src/t00.ts,14".to_string(),
    ];
    expected.extend((1..=18).map(|index| format!("    src/t{index:02}.ts,1")));
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
            "  reference_range: no published Reference range for this language\n",
            "  eroded_function_mass: 46.669\n",
            "  total_function_mass: 46.669\n",
            "  findings[1]{path,line,cyclomatic_complexity,source_lines,function_mass}:\n",
            "    src/route.ts,1,11,18,46.669\n",
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
            "  reference_range: no published Reference range for this language\n",
            "  eroded_function_mass: 39.6611\n",
            "  total_function_mass: 74.3021\n",
            "  findings[1]{path,line,cyclomatic_complexity,source_lines,function_mass}:\n",
            "    src/ten.ts,1,11,13,39.6611\n",
        )
    );
    assert_eq!(
        metric_section(&output, "test", "erosion"),
        concat!(
            "erosion:\n",
            "  ratio: 1\n",
            "  reference_range: no published Reference range for this language\n",
            "  eroded_function_mass: 44.8999\n",
            "  total_function_mass: 44.8999\n",
            "  findings[1]{path,line,cyclomatic_complexity,source_lines,function_mass}:\n",
            "    src/eleven.test.ts,1,12,14,44.8999\n",
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
            "  reference_range: no published Reference range for this language\n",
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

#[test]
fn a_closed_standard_output_is_an_operational_error_and_not_a_panic() {
    let repository = Repository::new();
    repository.write("src/main.ts", "export const main = 1;\n");
    repository.commit_all();
    let (reader, writer) = std::io::pipe().expect("create a pipe");
    drop(reader);

    let output = Command::new(env!("CARGO_BIN_EXE_codeint"))
        .arg("check")
        .current_dir(repository.path())
        .env("RUST_BACKTRACE", "1")
        .stdout(writer)
        .output()
        .expect("run codeint");

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

const NO_DUPLICATION: &str = "duplication:\n  ratio: 0\n  reference_range: no published Reference range for this language\n  clone_lines: 0\n  clone_groups: []\n";

const CART_TOTAL: &str = concat!(
    "export function total(items: { price: number; quantity: number }[]): number {\n",
    "  let sum = 0;\n",
    "  for (const item of items) {\n",
    "    if (item.quantity > 0) {\n",
    "      sum += item.price * item.quantity;\n",
    "    }\n",
    "  }\n",
    "  return sum;\n",
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
            "  reference_range: no published Reference range for this language\n",
            "  clone_lines: 18\n",
            "  clone_groups[1]:\n",
            "    - tokens: 59\n",
            "      locations[2]{path,start_line,end_line}:\n",
            "        src/cart.ts,1,9\n",
            "        src/invoice.ts,2,10\n",
        )
    );
}

const ORDER_TOTAL_RENAMED: &str = concat!(
    "export function amount(lines: { cost: number; count: number }[]): number {\n",
    "  let result = 100;\n",
    "  for (const line of lines) {\n",
    "    if (line.count > 5) {\n",
    "      result += line.cost * line.count;\n",
    "    }\n",
    "  }\n",
    "  return result;\n",
    "}\n",
);

const CART_TOTAL_RESTRUCTURED: &str = concat!(
    "export function total(items: { price: number; quantity: number }[]): number {\n",
    "  let sum = 0;\n",
    "  for (const item of items) {\n",
    "    while (item.quantity > 0) {\n",
    "      sum += item.price * item.quantity;\n",
    "    }\n",
    "  }\n",
    "  return sum + 1;\n",
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
            "  reference_range: no published Reference range for this language\n",
            "  clone_lines: 18\n",
            "  clone_groups[1]:\n",
            "    - tokens: 59\n",
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

fn no_import_cycles(modules: usize) -> String {
    format!(
        "import_cycles:\n\
         \x20 ratio: 0\n\
         \x20 reference_range: no published Reference range for this language\n\
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
            "  reference_range: no published Reference range for this language\n",
            "  modules_in_import_cycles: 3\n",
            "  modules: 4\n",
            "  findings[1]:\n",
            "    - modules[3]: src/a.ts,src/b.ts,src/c.ts\n",
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
            "  reference_range: no published Reference range for this language\n",
            "  modules_in_import_cycles: 3\n",
            "  modules: 4\n",
            "  findings[1]:\n",
            "    - modules[3]: lib/helpers.js,lib/loader.js,lib/plugin.js\n",
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
            "  reference_range: no published Reference range for this language\n",
            "  modules_in_import_cycles: 3\n",
            "  modules: 3\n",
            "  findings[1]:\n",
            "    - modules[3]: src/ratio.ts,src/shape.ts,src/unit.ts\n",
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
            "  reference_range: no published Reference range for this language\n",
            "  modules_in_import_cycles: 5\n",
            "  modules: 7\n",
            "  findings[3]:\n",
            "    - modules[2]: src/a.ts,src/b.ts\n",
            "    - modules[1]: src/self.ts\n",
            "    - modules[2]: src/x/index.ts,src/y.ts\n",
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
    expected.extend((1..=20).map(|index| format!("- modules[1]: src/self{index:02}.ts")));
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
    expected.extend((1..=21).map(|index| format!("- modules[1]: src/self{index:02}.ts")));
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
            "  reference_range: no published Reference range for this language\n",
            "  modules_in_import_cycles: 2\n",
            "  modules: 2\n",
            "  findings[1]:\n",
            "    - modules[2]: src/greet.test.ts,src/helper.test.ts\n",
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
            "  reference_range: no published Reference range for this language\n",
            "  modules_in_import_cycles: 2\n",
            "  modules: 3\n",
            "  findings[1]:\n",
            "    - modules[2]: src/a.ts,src/b.ts\n",
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
            "  reference_range: no published Reference range for this language\n",
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
        "codeint.json has an unknown key budget; the keys allowed there are $schema, source_sets",
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
