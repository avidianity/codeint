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

fn section(output: &Output, name: &str) -> String {
    let heading = format!("{name}:");
    stdout(output)
        .lines()
        .skip_while(|line| *line != heading)
        .enumerate()
        .take_while(|(index, line)| *index == 0 || line.starts_with(' '))
        .map(|(_, line)| format!("{line}\n"))
        .collect()
}

fn report_without_findings(files: usize, source_lines: u64) -> String {
    format!(
        "files: {files}\n\
         source_lines: {source_lines}\n\
         erosion:\n  \
         ratio: 0\n  \
         eroded_function_mass: 0\n  \
         total_function_mass: 0\n  \
         findings: []\n\
         duplication:\n  \
         ratio: 0\n  \
         clone_lines: 0\n  \
         clone_groups: []\n\
         verbosity:\n  \
         ratio: 0\n  \
         verbose_lines: 0\n  \
         findings: []\n"
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
        section(&output, "verbosity"),
        "verbosity:\n  \
         ratio: 0.15\n  \
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
        section(&output, "erosion"),
        concat!(
            "erosion:\n",
            "  ratio: 0.8889\n",
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
        section(&output, "erosion"),
        concat!(
            "erosion:\n",
            "  ratio: 0.7094\n",
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
        section(&output, "erosion"),
        concat!(
            "erosion:\n",
            "  ratio: 0\n",
            "  eroded_function_mass: 0\n",
            "  total_function_mass: 0\n",
            "  findings: []\n",
        )
    );
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
        section(&output, "erosion"),
        concat!(
            "erosion:\n",
            "  ratio: 0.9683\n",
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
    section(output, "erosion")
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
        section(&output, "erosion"),
        concat!(
            "erosion:\n",
            "  ratio: 1\n",
            "  eroded_function_mass: 46.669\n",
            "  total_function_mass: 46.669\n",
            "  findings[1]{path,line,cyclomatic_complexity,source_lines,function_mass}:\n",
            "    src/route.ts,1,11,18,46.669\n",
        )
    );
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

const NO_DUPLICATION: &str = "duplication:\n  ratio: 0\n  clone_lines: 0\n  clone_groups: []\n";

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
        section(&output, "duplication"),
        concat!(
            "duplication:\n",
            "  ratio: 0.9\n",
            "  clone_lines: 18\n",
            "  clone_groups[1]:\n",
            "    - tokens: 59\n",
            "      locations[2]{path,start_line,end_line}:\n",
            "        src/cart.ts,1,9\n",
            "        src/invoice.ts,2,10\n",
        )
    );
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
    assert_eq!(section(&output, "duplication"), NO_DUPLICATION);
}

fn write_21_clone_groups_of_equal_size(repository: &Repository) {
    for index in 1..=21 {
        let total = CART_TOTAL
            .replace("total", &format!("total{index:02}"))
            .replace("sum", &format!("sum{index:02}"))
            .replace("item", &format!("item{index:02}"));
        repository.write(&format!("src/a{index:02}.ts"), &total);
        repository.write(&format!("src/b{index:02}.ts"), &total);
    }
}

fn clone_groups_by_first_location(output: &Output) -> Vec<String> {
    section(output, "duplication")
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
