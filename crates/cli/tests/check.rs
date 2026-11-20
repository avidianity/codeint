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
        "files: 1\n\
         source_lines: 2\n\
         erosion:\n  \
         ratio: 0\n  \
         eroded_function_mass: 0\n  \
         total_function_mass: 0\n  \
         findings: []\n\
         import_cycles:\n  \
         ratio: 0\n  \
         modules_in_import_cycles: 0\n  \
         modules: 1\n  \
         findings: []\n\
         skipped[1]{path,reason}:\n  \
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
        "files: 1\n\
         source_lines: 1\n\
         erosion:\n  \
         ratio: 0\n  \
         eroded_function_mass: 0\n  \
         total_function_mass: 0\n  \
         findings: []\n\
         import_cycles:\n  \
         ratio: 0\n  \
         modules_in_import_cycles: 0\n  \
         modules: 1\n  \
         findings: []\n\
         declaration_only[3]: types/globals.d.ts,types/legacy.d.cts,types/module.d.mts\n"
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
    assert_eq!(
        stdout(&output),
        format!(
            "{}{}",
            concat!(
                "files: 1\n",
                "source_lines: 25\n",
                "erosion:\n",
                "  ratio: 0.8889\n",
                "  eroded_function_mass: 48\n",
                "  total_function_mass: 54\n",
                "  findings[1]{path,line,cyclomatic_complexity,source_lines,function_mass}:\n",
                "    src/mixed.ts,1,12,16,48\n",
            ),
            no_import_cycles(1)
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
    assert_eq!(
        stdout(&output),
        format!(
            "{}{}",
            concat!(
                "files: 3\n",
                "source_lines: 39\n",
                "erosion:\n",
                "  ratio: 0.7094\n",
                "  eroded_function_mass: 84.561\n",
                "  total_function_mass: 119.202\n",
                "  findings[2]{path,line,cyclomatic_complexity,source_lines,function_mass}:\n",
                "    src/eleven.ts,1,12,14,44.8999\n",
                "    src/ten.ts,1,11,13,39.6611\n",
            ),
            no_import_cycles(3)
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
    assert_eq!(
        stdout(&output),
        format!(
            "{}{}",
            concat!(
                "files: 1\n",
                "source_lines: 1\n",
                "erosion:\n",
                "  ratio: 0\n",
                "  eroded_function_mass: 0\n",
                "  total_function_mass: 0\n",
                "  findings: []\n",
            ),
            no_import_cycles(1)
        )
    );
}

#[test]
fn decision_points_of_a_callback_count_only_toward_the_callback() {
    let repository = Repository::new();
    let branches: String = (1..=10)
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
    assert_eq!(
        stdout(&output),
        format!(
            "{}{}",
            concat!(
                "files: 1\n",
                "source_lines: 14\n",
                "erosion:\n",
                "  ratio: 0.9106\n",
                "  eroded_function_mass: 38.1051\n",
                "  total_function_mass: 41.8468\n",
                "  findings[1]{path,line,cyclomatic_complexity,source_lines,function_mass}:\n",
                "    src/outer.ts,2,11,12,38.1051\n",
            ),
            no_import_cycles(1)
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
    assert_eq!(
        saved,
        b"files: 1\nsource_lines: 2\nerosion:\n  ratio: 0\n  eroded_function_mass: 0\n  total_function_mass: 0\n  findings: []\nimport_cycles:\n  ratio: 0\n  modules_in_import_cycles: 0\n  modules: 1\n  findings: []\n"
    );
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
        "files: 1\nsource_lines: 1\nerosion:\n  ratio: 0\n  eroded_function_mass: 0\n  total_function_mass: 0\n  findings: []\nimport_cycles:\n  ratio: 0\n  modules_in_import_cycles: 0\n  modules: 1\n  findings: []\n"
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

fn import_cycles_section(output: &Output) -> String {
    stdout(output)
        .lines()
        .skip_while(|line| *line != "import_cycles:")
        .take_while(|line| *line == "import_cycles:" || line.starts_with(' '))
        .map(|line| format!("{line}\n"))
        .collect()
}

fn no_import_cycles(modules: usize) -> String {
    format!(
        "import_cycles:\n\
         \x20 ratio: 0\n\
         \x20 modules_in_import_cycles: 0\n\
         \x20 modules: {modules}\n\
         \x20 findings: []\n"
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
        import_cycles_section(&output),
        concat!(
            "import_cycles:\n",
            "  ratio: 0.75\n",
            "  modules_in_import_cycles: 3\n",
            "  modules: 4\n",
            "  findings[1]:\n",
            "    - modules[3]: src/a.ts,src/b.ts,src/c.ts\n",
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
    assert_eq!(import_cycles_section(&output), no_import_cycles(4));
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
        import_cycles_section(&output),
        concat!(
            "import_cycles:\n",
            "  ratio: 0.75\n",
            "  modules_in_import_cycles: 3\n",
            "  modules: 4\n",
            "  findings[1]:\n",
            "    - modules[3]: lib/helpers.js,lib/loader.js,lib/plugin.js\n",
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
    assert_eq!(import_cycles_section(&output), no_import_cycles(4));
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
        import_cycles_section(&output),
        concat!(
            "import_cycles:\n",
            "  ratio: 0.7143\n",
            "  modules_in_import_cycles: 5\n",
            "  modules: 7\n",
            "  findings[3]:\n",
            "    - modules[2]: src/a.ts,src/b.ts\n",
            "    - modules[1]: src/self.ts\n",
            "    - modules[2]: src/x/index.ts,src/y.ts\n",
        )
    );
}
