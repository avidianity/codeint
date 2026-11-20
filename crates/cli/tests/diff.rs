use std::fs;
use std::path::Path;
use std::process::{Command, Output};

use serde_json::Value;
use tempfile::TempDir;

const FIXTURE_GIT_ENVIRONMENT: [(&str, &str); 6] = [
    ("GIT_CONFIG_GLOBAL", "/dev/null"),
    ("GIT_CONFIG_NOSYSTEM", "1"),
    ("GIT_AUTHOR_NAME", "Fixture"),
    ("GIT_AUTHOR_EMAIL", "fixture@example.invalid"),
    ("GIT_COMMITTER_NAME", "Fixture"),
    ("GIT_COMMITTER_EMAIL", "fixture@example.invalid"),
];

struct Fixture {
    folder: TempDir,
}

impl Fixture {
    fn on_main() -> Fixture {
        let fixture = Fixture {
            folder: tempfile::tempdir().expect("create a temporary folder"),
        };
        fixture.run_git(&["init", "--quiet", "--initial-branch", "main"]);
        fixture
    }

    fn root(&self) -> &Path {
        self.folder.path()
    }

    fn put(&self, relative: &str, text: &str) {
        let full = self.root().join(relative);
        fs::create_dir_all(full.parent().expect("a file has a parent folder"))
            .expect("create the parent folder");
        fs::write(full, text).expect("write the file");
    }

    fn remove(&self, relative: &str) {
        fs::remove_file(self.root().join(relative)).expect("remove the file");
    }

    fn run_git(&self, args: &[&str]) -> String {
        let mut command = Command::new("git");
        command.args(args).current_dir(self.root());
        for (name, value) in FIXTURE_GIT_ENVIRONMENT {
            command.env(name, value);
        }
        let output = command.output().expect("run git");
        assert!(
            output.status.success(),
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).expect("git prints UTF-8")
    }

    fn commit(&self, message: &str) {
        self.run_git(&["add", "--all"]);
        self.run_git(&["commit", "--quiet", "--message", message]);
    }

    fn codeint_diff(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_codeint"))
            .arg("diff")
            .args(args)
            .current_dir(self.root())
            .output()
            .expect("run codeint")
    }

    fn diff_json(&self, base_ref: &str) -> Value {
        let output = self.codeint_diff(&[base_ref, "--format", "json"]);
        assert_eq!(
            output.status.code(),
            Some(0),
            "codeint diff failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        serde_json::from_slice(&output.stdout).expect("the report is JSON")
    }
}

fn eroded_function(name: &str) -> String {
    function_with_if_statements(name, 11)
}

fn function_with_if_statements(name: &str, if_statements: usize) -> String {
    let branches: String = (1..=if_statements)
        .map(|value| format!("  if (value === {value}) return {value};\n"))
        .collect();
    format!("export function {name}(value: number): number {{\n{branches}  return 0;\n}}\n")
}

fn paths_of(findings: &Value) -> Vec<String> {
    findings
        .as_array()
        .expect("a list of Findings")
        .iter()
        .map(|finding| finding["path"].as_str().expect("a path").to_owned())
        .collect()
}

#[test]
fn a_finding_added_in_the_working_tree_is_new_and_one_removed_is_resolved() {
    let fixture = Fixture::on_main();
    fixture.put("src/kept.ts", &eroded_function("kept"));
    fixture.put("src/removed.ts", &eroded_function("removed"));
    fixture.commit("base");
    fixture.remove("src/removed.ts");
    fixture.put("src/added.ts", &eroded_function("added"));

    let report = fixture.diff_json("main");

    let erosion = &report["source_sets"]["production"]["erosion"];
    assert_eq!(paths_of(&erosion["new"]), ["src/added.ts"]);
    assert_eq!(paths_of(&erosion["resolved"]), ["src/removed.ts"]);
    assert_eq!(erosion["unchanged"], 1);
}

#[test]
fn an_uncommitted_change_to_a_committed_file_is_measured() {
    let fixture = Fixture::on_main();
    fixture.put("src/calm.ts", "export const calm = 1;\n");
    fixture.commit("base");
    fixture.put(
        "src/calm.ts",
        &format!("export const calm = 1;\n{}", eroded_function("busy")),
    );

    let report = fixture.diff_json("main");

    let erosion = &report["source_sets"]["production"]["erosion"];
    assert_eq!(paths_of(&erosion["new"]), ["src/calm.ts"]);
    assert_eq!(paths_of(&erosion["resolved"]), Vec::<String>::new());
    assert_eq!(erosion["base_ratio"], 0.0);
    assert_eq!(erosion["head_ratio"], 1.0);
}

#[test]
fn diff_leaves_the_working_tree_the_index_and_head_untouched() {
    let fixture = Fixture::on_main();
    fixture.put("src/kept.ts", &eroded_function("kept"));
    fixture.put("src/staged.ts", "export const staged = 1;\n");
    fixture.commit("base");
    fixture.put("src/staged.ts", &eroded_function("staged"));
    fixture.run_git(&["add", "src/staged.ts"]);
    fixture.put("src/kept.ts", "export const kept = 2;\n");
    fixture.put("src/untracked.ts", &eroded_function("untracked"));
    let state = |fixture: &Fixture| {
        (
            fs::read(fixture.root().join(".git/index")).expect("read the index"),
            fixture.run_git(&[
                "status",
                "--porcelain=v2",
                "--branch",
                "--untracked-files=all",
            ]),
            fixture.run_git(&["rev-parse", "HEAD"]),
            ["src/kept.ts", "src/staged.ts", "src/untracked.ts"]
                .map(|path| fs::read(fixture.root().join(path)).expect("read the file")),
        )
    };
    let before = state(&fixture);

    let report = fixture.diff_json("main");

    assert_eq!(state(&fixture), before);
    let erosion = &report["source_sets"]["production"]["erosion"];
    assert_eq!(
        paths_of(&erosion["new"]),
        ["src/staged.ts", "src/untracked.ts"]
    );
    assert_eq!(paths_of(&erosion["resolved"]), ["src/kept.ts"]);
}

#[test]
fn an_unknown_base_ref_exits_2_and_names_the_ref() {
    let fixture = Fixture::on_main();
    fixture.put("src/a.ts", "export const a = 1;\n");
    fixture.commit("base");

    let output = fixture.codeint_diff(&["no-such-branch"]);

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.starts_with(
            "codeint: the Git ref \"no-such-branch\" does not name a commit in this repository"
        ),
        "unexpected error: {stderr}"
    );
}

#[test]
fn commits_added_to_the_base_branch_after_the_branch_started_are_not_changes() {
    let fixture = Fixture::on_main();
    fixture.put("src/shared.ts", &eroded_function("shared"));
    fixture.commit("start");
    let start = fixture.run_git(&["rev-parse", "HEAD"]);
    fixture.run_git(&["switch", "--quiet", "--create", "feature"]);
    fixture.put("src/feature.ts", &eroded_function("feature"));
    fixture.commit("feature work");
    fixture.run_git(&["switch", "--quiet", "main"]);
    fixture.put("src/later_on_main.ts", &eroded_function("laterOnMain"));
    fixture.remove("src/shared.ts");
    fixture.commit("later work on main");
    fixture.run_git(&["switch", "--quiet", "feature"]);

    let report = fixture.diff_json("main");

    assert_eq!(report["base"]["ref"], "main");
    assert_eq!(report["base"]["merge_base"], start.trim());
    assert_eq!(report["head"], "working tree");
    let erosion = &report["source_sets"]["production"]["erosion"];
    assert_eq!(paths_of(&erosion["new"]), ["src/feature.ts"]);
    assert_eq!(paths_of(&erosion["resolved"]), Vec::<String>::new());
    assert_eq!(erosion["unchanged"], 1);
}

#[test]
fn codeint_json_of_the_working_tree_sorts_the_files_of_both_sides() {
    let fixture = Fixture::on_main();
    fixture.put("src/legacy/old.ts", &eroded_function("old"));
    fixture.commit("base");
    fixture.put(
        "codeint.json",
        r#"{ "source_sets": { "vendored": ["src/legacy/**"] } }"#,
    );

    let report = fixture.diff_json("main");

    let source_sets = report["source_sets"].as_object().expect("Source sets");
    assert_eq!(source_sets.keys().collect::<Vec<_>>(), ["vendored"]);
    assert_eq!(source_sets["vendored"]["erosion"]["unchanged"], 1);
}

#[test]
fn a_file_of_the_merge_base_that_is_not_utf8_is_skipped_like_in_check() {
    let fixture = Fixture::on_main();
    fixture.put("src/a.ts", &eroded_function("a"));
    fs::write(
        fixture.root().join("src/latin1.ts"),
        b"export const caf\xe9 = 1;\n",
    )
    .expect("write the file");
    fixture.commit("base");

    let report = fixture.diff_json("main");

    assert_eq!(
        report["source_sets"]["production"]["erosion"]["unchanged"],
        1
    );
}

#[test]
fn diff_lists_the_20_worst_new_findings_unless_all_is_passed() {
    let fixture = Fixture::on_main();
    fixture.put("src/start.ts", "export const start = 1;\n");
    fixture.commit("base");
    for number in 1..=21 {
        fixture.put(
            &format!("src/f{number:02}.ts"),
            &eroded_function(&format!("f{number}")),
        );
    }

    let limited = fixture.diff_json("main");
    let output = fixture.codeint_diff(&["main", "--format", "json", "--all"]);
    let every: Value = serde_json::from_slice(&output.stdout).expect("the report is JSON");

    let limited = &limited["source_sets"]["production"]["erosion"];
    assert_eq!(paths_of(&limited["new"]).len(), 20);
    assert_eq!(limited["new_left_out"], 1);
    let every = &every["source_sets"]["production"]["erosion"];
    assert_eq!(paths_of(&every["new"]).len(), 21);
    assert_eq!(every.get("new_left_out"), None);
}

#[test]
fn format_markdown_shows_each_metric_on_both_sides_then_the_new_and_resolved_findings() {
    let fixture = Fixture::on_main();
    fixture.put("src/a.ts", &function_with_if_statements("a", 11));
    fixture.commit("base");
    let merge_base = fixture.run_git(&["rev-parse", "HEAD"]);
    fixture.remove("src/a.ts");
    fixture.put("src/b.ts", &function_with_if_statements("b", 12));

    let output = fixture.codeint_diff(&["main", "--format", "markdown"]);

    let eroded_function_table = "| Function | Cyclomatic complexity | Source lines | Function mass |\n\
                                 | -------- | --------------------- | ------------ | ------------- |";
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(
        String::from_utf8(output.stdout)
            .expect("the report is UTF-8")
            .replace(merge_base.trim(), "<merge-base>"),
        format!(
            "# codeint diff\n\
         \n\
         codeint compared the working tree with `<merge-base>`, the merge-base of `main` and HEAD.\n\
         \n\
         | Source set | Metric | Base | Head | New | Resolved | Unchanged |\n\
         | ---------- | ------ | ---- | ---- | --- | -------- | --------- |\n\
         | Production | Erosion | 1 | 1 | 1 | 1 | 0 |\n\
         | Production | Duplication | 0 | 0.8 | 1 | 0 | 0 |\n\
         | Production | Verbosity | 0 | 0.8 | 0 | 0 | 0 |\n\
         | Production | Import cycles | 0 | 0 | 0 | 0 | 0 |\n\
         \n\
         ## Production changes\n\
         \n\
         ### New Erosion Findings\n\
         \n\
         {eroded_function_table}\n\
         | `src/b.ts:1` | 13 | 15 | 50.3488 |\n\
         \n\
         ### Resolved Erosion Findings\n\
         \n\
         {eroded_function_table}\n\
         | `src/a.ts:1` | 12 | 14 | 44.8999 |\n\
         \n\
         ### New Duplication Findings\n\
         \n\
         - 54 tokens: `src/b.ts:2-7`, `src/b.ts:8-13`\n"
        )
    );
}
