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
    assert_eq!(stdout(&output), "files: 2\nsource_lines: 5\n");
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
    assert_eq!(stdout(&output), "files: 1\nsource_lines: 5\n");
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
    assert_eq!(stdout(&output), "files: 8\nsource_lines: 8\n");
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
    assert_eq!(stdout(&output), "files: 2\nsource_lines: 3\n");
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
    assert_eq!(stdout(&output), "files: 2\nsource_lines: 3\n");
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
