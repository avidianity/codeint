use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use tempfile::TempDir;

struct Repository {
    folder: TempDir,
}

impl Repository {
    fn new() -> Repository {
        let folder = tempfile::tempdir().expect("create a temporary folder");
        let status = Command::new("git")
            .args(["init", "--quiet"])
            .current_dir(folder.path())
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .status()
            .expect("run git init");
        assert!(status.success(), "git init exited with {status}");
        Repository { folder }
    }

    fn root(&self) -> PathBuf {
        fs::canonicalize(self.folder.path()).expect("resolve the temporary folder")
    }

    fn add(&self, relative: &str, text: &str) {
        let full = self.folder.path().join(relative);
        fs::create_dir_all(full.parent().expect("a file has a parent folder"))
            .expect("create the parent folder");
        fs::write(full, text).expect("write the file");
    }

    fn configuration(&self) -> String {
        fs::read_to_string(self.folder.path().join("codeint.jsonc")).expect("read codeint.jsonc")
    }
}

fn codeint(folder: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_codeint"))
        .args(args)
        .current_dir(folder)
        .output()
        .expect("run codeint")
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

fn assert_exit_code(output: &Output, code: i32) {
    assert_eq!(
        output.status.code(),
        Some(code),
        "standard output: {}\nstandard error: {}",
        text(&output.stdout),
        text(&output.stderr)
    );
}

fn schema_line() -> String {
    format!(
        "  \"$schema\": \"https://raw.githubusercontent.com/avidianity/codeint/v{}/schema/codeint.schema.json\",\n",
        env!("CARGO_PKG_VERSION")
    )
}

const DEFAULT_SOURCE_SETS_BUDGETS_AND_REGRESSION_LIMITS: &str = r#"  "source_sets": {
    "production": [],
    "test": [],
    "generated": [],
    "vendored": []
  },
  "budgets": {
    "production": {
      // "test_gap": 0.2
    },
    "test": {},
    "generated": {},
    "vendored": {}
  },
  "regression_limits": {}"#;

#[test]
fn init_from_a_subfolder_writes_codeint_jsonc_at_the_root_with_the_schema_link_defaults_and_each_folder_a_language_plugin_claims()
 {
    let repository = Repository::new();
    repository.add("index.js", "export const index = 1;\n");
    repository.add("src/main.ts", "export const main = 1;\n");
    repository.add("packages/web/src/app.tsx", "export const app = 1;\n");
    repository.add("docs/guide.md", "# Guide\n");
    repository.add("README.md", "# Readme\n");

    let output = codeint(&repository.root().join("packages/web"), &["init"]);

    assert_exit_code(&output, 0);
    assert_eq!(
        text(&output.stdout),
        format!(
            "Wrote {}\n",
            repository.root().join("codeint.jsonc").display()
        )
    );
    assert!(!repository.root().join("codeint.json").exists());
    assert_eq!(
        repository.configuration(),
        format!(
            "{{\n{}{DEFAULT_SOURCE_SETS_BUDGETS_AND_REGRESSION_LIMITS},\n{}",
            schema_line(),
            r#"  "languages": [
    {
      "plugin": "typescript-javascript",
      "include": [
        "index.js",
        "packages",
        "src"
      ],
      "exclude": []
    }
  ],
  "coverage_reports": []
}
"#
        )
    );
}

#[test]
fn check_accepts_the_codeint_jsonc_that_init_writes_and_measures_every_detected_file() {
    let repository = Repository::new();
    repository.add("index.js", "export const index = 1;\n");
    repository.add("src/main.ts", "export const main = 1;\n");
    repository.add("packages/web/src/app.tsx", "export const app = 1;\n");
    assert_exit_code(&codeint(&repository.root(), &["init"]), 0);

    let output = codeint(&repository.root(), &["check"]);

    assert_exit_code(&output, 0);
    assert!(
        text(&output.stdout).starts_with("files: 3\nsource_lines: 3\n"),
        "{}",
        text(&output.stdout)
    );
}

#[test]
fn init_escapes_glob_characters_in_a_folder_name_so_that_check_measures_that_folder() {
    let repository = Repository::new();
    repository.add("[draft]/notes.ts", "export const notes = 1;\n");
    repository.add("{a,b}/c.ts", "export const c = 1;\n");
    repository.add("a/hidden.ts", "export const hidden = 1;\n");
    assert_exit_code(&codeint(&repository.root(), &["init"]), 0);

    let configuration = repository.configuration();
    let output = codeint(&repository.root(), &["check"]);

    assert!(
        configuration.contains(
            "      \"include\": [\n        \"[[]draft[]]\",\n        \"a\",\n        \"[{]a,b[}]\"\n      ],\n"
        ),
        "{configuration}"
    );
    assert_exit_code(&output, 0);
    assert!(
        text(&output.stdout).starts_with("files: 3\nsource_lines: 3\n"),
        "{}",
        text(&output.stdout)
    );
}

#[test]
fn init_in_a_repository_without_files_a_language_plugin_claims_leaves_languages_out_so_that_every_plugin_measures_everything()
 {
    let repository = Repository::new();
    repository.add("README.md", "# Readme\n");

    let output = codeint(&repository.root(), &["init"]);

    assert_exit_code(&output, 0);
    assert_eq!(
        repository.configuration(),
        format!(
            "{{\n{}{DEFAULT_SOURCE_SETS_BUDGETS_AND_REGRESSION_LIMITS},\n  \"coverage_reports\": []\n}}\n",
            schema_line()
        )
    );
    assert_exit_code(&codeint(&repository.root(), &["check"]), 0);
}

fn assert_init_refuses_existing_config(file_name: &str, body: &str) {
    let repository = Repository::new();
    repository.add("src/main.ts", "export const main = 1;\n");
    repository.add(file_name, body);

    let output = codeint(&repository.root().join("src"), &["init"]);

    assert_exit_code(&output, 2);
    assert_eq!(text(&output.stdout), "");
    assert_eq!(
        text(&output.stderr),
        format!(
            "codeint: {} already exists, and codeint init never replaces it\n\
             Edit the existing {file_name}, or delete it and run codeint init again.\n",
            repository.root().join(file_name).display()
        )
    );
    assert_eq!(
        fs::read_to_string(repository.root().join(file_name)).expect("read existing config"),
        body
    );
    let other_file = if file_name == "codeint.json" {
        "codeint.jsonc"
    } else {
        "codeint.json"
    };
    assert!(!repository.root().join(other_file).exists());
}

#[test]
fn init_refuses_to_replace_an_existing_codeint_json_exits_2_and_keeps_the_file() {
    assert_init_refuses_existing_config("codeint.json", "{ \"budgets\": {} }\n");
}

#[test]
fn init_refuses_when_codeint_jsonc_already_exists_exits_2_and_keeps_the_file() {
    assert_init_refuses_existing_config("codeint.jsonc", "// keep me\n{ \"budgets\": {}, }\n");
}

#[test]
fn init_outside_a_git_repository_names_the_folder_and_exits_2_without_writing() {
    let folder = tempfile::tempdir().expect("create a temporary folder");
    let canonical = fs::canonicalize(folder.path()).expect("resolve the temporary folder");

    let output = codeint(&canonical, &["init"]);

    assert_exit_code(&output, 2);
    assert_eq!(
        text(&output.stderr),
        format!(
            "codeint: {} is not inside a Git repository\n\
             Run codeint init inside a Git repository; it writes codeint.jsonc at the root of the repository.\n",
            canonical.display()
        )
    );
    assert!(!canonical.join("codeint.jsonc").exists());
    assert!(!canonical.join("codeint.json").exists());
}
