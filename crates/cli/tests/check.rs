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

fn assert_success(output: &Output) {
    assert_eq!(
        output.status.code(),
        Some(0),
        "codeint failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn acyclic_report(files: usize, source_lines: usize) -> String {
    format!(
        "files: {files}\n\
         source_lines: {source_lines}\n\
         import_cycles:\n\
         \x20 ratio: 0\n\
         \x20 modules_in_import_cycles: 0\n\
         \x20 modules: {files}\n\
         \x20 findings: []\n"
    )
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
    assert_eq!(stdout(&output), acyclic_report(2, 5));
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
    assert_eq!(stdout(&output), acyclic_report(1, 5));
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
    assert_eq!(stdout(&output), acyclic_report(8, 8));
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
    assert_eq!(stdout(&output), acyclic_report(2, 3));
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
fn check_stops_with_an_operational_error_on_a_file_that_does_not_parse() {
    let repository = Repository::new();
    repository.write("src/broken.ts", "export const = ;\n");
    repository.commit_all();

    let output = repository.codeint(&["check"]);

    assert_eq!(output.status.code(), Some(2));
    assert!(stdout(&output).is_empty());
    let message = String::from_utf8_lossy(&output.stderr);
    assert!(
        message.starts_with("codeint: src/broken.ts: cannot parse")
            && message.ends_with(
                "\nFix the file named above so that it parses, then run codeint again.\n"
            )
            && message.lines().count() == 2,
        "unexpected message: {message}"
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
    assert_eq!(stdout(&output), acyclic_report(2, 3));
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
         Usage: codeint check [PATH]\n\
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
        stdout(&output),
        concat!(
            "files: 4\n",
            "source_lines: 8\n",
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
    assert_eq!(stdout(&output), acyclic_report(4, 8));
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
        stdout(&output),
        concat!(
            "files: 4\n",
            "source_lines: 7\n",
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
    assert_eq!(stdout(&output), acyclic_report(4, 9));
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
    assert_eq!(
        stdout(&output),
        concat!(
            "files: 7\n",
            "source_lines: 11\n",
            "import_cycles:\n",
            "  ratio: 0.7142857142857143\n",
            "  modules_in_import_cycles: 5\n",
            "  modules: 7\n",
            "  findings[3]:\n",
            "    - modules[2]: src/a.ts,src/b.ts\n",
            "    - modules[1]: src/self.ts\n",
            "    - modules[2]: src/x/index.ts,src/y.ts\n",
        )
    );
}
