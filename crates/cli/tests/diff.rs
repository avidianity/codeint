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
    result_cache: TempDir,
}

impl Fixture {
    fn on_main() -> Fixture {
        let fixture = Fixture {
            folder: tempfile::tempdir().expect("create a temporary folder"),
            result_cache: tempfile::tempdir().expect("create a temporary result cache folder"),
        };
        fixture.run_git(&["init", "--quiet", "--initial-branch", "main"]);
        fixture
    }

    fn with_an_eroded_file_removed_since_main() -> Fixture {
        let fixture = Fixture::on_main();
        fixture.put("src/removed.ts", &eroded_function("removed"));
        fixture.commit("base");
        fixture.remove("src/removed.ts");
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
        self.codeint_diff_with_result_cache(self.result_cache.path(), args)
    }

    fn codeint_diff_with_result_cache(&self, result_cache: &Path, args: &[&str]) -> Output {
        let mut command = self.codeint_diff_command(args);
        command.env("CODEINT_CACHE_DIR", result_cache);
        command.output().expect("run codeint")
    }

    fn codeint_diff_command(&self, args: &[&str]) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_codeint"));
        command.arg("diff").args(args).current_dir(self.root());
        command
    }

    fn corrupt_every_result_cache_entry(&self, bytes: &[u8]) -> usize {
        let entries: Vec<_> = fs::read_dir(self.result_cache.path())
            .expect("list the result cache folder")
            .map(|entry| entry.expect("read a result cache entry").path())
            .collect();
        for entry in &entries {
            fs::write(entry, bytes).expect("overwrite the result cache entry");
        }
        entries.len()
    }

    fn successful_diff(&self, args: &[&str]) -> String {
        let output = self.codeint_diff(args);
        assert_eq!(
            output.status.code(),
            Some(0),
            "codeint diff failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).expect("the report is UTF-8")
    }

    fn forget_committed_file(&self, spec: &str) {
        let object = self.run_git(&["rev-parse", spec]);
        let (folder, file) = object.trim().split_at(2);
        fs::remove_file(self.root().join(".git/objects").join(folder).join(file))
            .expect("remove the loose object of the committed file");
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

fn assert_only_vendored_with_one_unchanged_eroded_function(report: &Value) {
    let source_sets = report["source_sets"].as_object().expect("Source sets");
    assert_eq!(source_sets.keys().collect::<Vec<_>>(), ["vendored"]);
    assert_eq!(source_sets["vendored"]["erosion"]["unchanged"], 1);
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

    assert_only_vendored_with_one_unchanged_eroded_function(&report);
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
fn format_sarif_lists_only_the_new_findings_so_a_pull_request_annotates_what_it_added() {
    let fixture = Fixture::on_main();
    fixture.put("src/kept.ts", &eroded_function("kept"));
    fixture.put("src/removed.ts", &eroded_function("removed"));
    fixture.commit("base");
    fixture.remove("src/removed.ts");
    let eleven_conditions: Vec<String> = (1..=11).map(|value| format!("value > {value}")).collect();
    fixture.put(
        "src/added.ts",
        &format!(
            "export function added(value: number): boolean {{\n  return {};\n}}\n",
            eleven_conditions.join(" && ")
        ),
    );

    let output = fixture.codeint_diff(&["main", "--format", "sarif"]);

    assert_eq!(output.status.code(), Some(0));
    let log: Value = serde_json::from_slice(&output.stdout).expect("the log is JSON");
    assert_eq!(log["version"], "2.1.0");
    let results: Vec<(String, String)> = log["runs"][0]["results"]
        .as_array()
        .expect("a list of results")
        .iter()
        .map(|result| {
            (
                result["ruleId"].as_str().expect("a rule id").to_owned(),
                result["locations"][0]["physicalLocation"]["artifactLocation"]["uri"]
                    .as_str()
                    .expect("a uri")
                    .to_owned(),
            )
        })
        .collect();
    assert_eq!(results, [("erosion".to_owned(), "src/added.ts".to_owned())]);
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

#[test]
fn a_second_diff_against_the_same_base_reads_the_merge_base_from_the_cache_and_gives_the_same_report()
 {
    let fixture = Fixture::with_an_eroded_file_removed_since_main();
    fixture.put("src/added.ts", &eroded_function("added"));
    let first = fixture.successful_diff(&["main", "--all"]);
    fixture.forget_committed_file("main:src/removed.ts");

    let second = fixture.successful_diff(&["main", "--all"]);

    assert_eq!(second, first);
    assert!(
        first.contains("src/removed.ts"),
        "unexpected report: {first}"
    );
}

#[test]
fn without_a_cached_result_diff_reads_the_files_of_the_merge_base() {
    let fixture = Fixture::with_an_eroded_file_removed_since_main();
    fixture.forget_committed_file("main:src/removed.ts");

    let output = fixture.codeint_diff(&["main"]);

    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.starts_with("codeint: cannot read src/removed.ts from commit"),
        "unexpected error: {stderr}"
    );
}

#[test]
fn a_configuration_change_measures_the_merge_base_again_instead_of_reading_the_cache() {
    let fixture = Fixture::on_main();
    fixture.put("src/legacy/old.ts", &eroded_function("old"));
    fixture.commit("base");
    let before = fixture.diff_json("main");
    fixture.put(
        "codeint.json",
        r#"{ "source_sets": { "vendored": ["src/legacy/**"] } }"#,
    );

    let after = fixture.diff_json("main");

    assert_eq!(
        before["source_sets"]["production"]["erosion"]["unchanged"],
        1
    );
    assert_only_vendored_with_one_unchanged_eroded_function(&after);
}

#[test]
fn the_schema_address_of_codeint_json_is_not_part_of_the_configuration_that_keys_the_cache() {
    let fixture = Fixture::with_an_eroded_file_removed_since_main();
    fixture.put(
        "codeint.json",
        r#"{ "$schema": "https://example.invalid/old.json" }"#,
    );
    let first = fixture.successful_diff(&["main"]);
    fixture.forget_committed_file("main:src/removed.ts");
    fixture.put(
        "codeint.json",
        r#"{ "$schema": "https://example.invalid/new.json", "source_sets": {} }"#,
    );

    let second = fixture.successful_diff(&["main"]);

    assert_eq!(second, first);
}

#[test]
fn a_corrupt_cache_entry_is_ignored_and_rebuilt() {
    for corruption in [&b"{\"commit\": "[..], b"[]", b"\xff\xfe not JSON"] {
        let fixture = Fixture::with_an_eroded_file_removed_since_main();
        let first = fixture.successful_diff(&["main"]);
        assert!(fixture.corrupt_every_result_cache_entry(corruption) > 0);

        let rebuilt = fixture.successful_diff(&["main"]);
        fixture.forget_committed_file("main:src/removed.ts");
        let read_back = fixture.successful_diff(&["main"]);

        assert_eq!(rebuilt, first, "{corruption:?}");
        assert_eq!(read_back, first, "{corruption:?}");
    }
}

#[test]
fn a_cache_folder_that_cannot_be_written_warns_and_the_diff_still_succeeds() {
    let fixture = Fixture::with_an_eroded_file_removed_since_main();
    let not_a_folder = fixture.result_cache.path().join("a-file");
    fs::write(&not_a_folder, "").expect("write a file where the cache folder should be");

    let output =
        fixture.codeint_diff_with_result_cache(&not_a_folder, &["main", "--format", "json"]);

    assert_eq!(output.status.code(), Some(0));
    let report: Value = serde_json::from_slice(&output.stdout).expect("the report is JSON");
    assert_eq!(
        paths_of(&report["source_sets"]["production"]["erosion"]["resolved"]),
        ["src/removed.ts"]
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.starts_with(&format!(
            "codeint: cannot write the cache entry {}",
            not_a_folder.display()
        )),
        "unexpected warning: {stderr}"
    );
    assert!(
        stderr.contains("; the next run measures commit "),
        "unexpected warning: {stderr}"
    );
}

#[test]
fn without_codeint_cache_dir_the_cache_is_the_codeint_folder_of_the_user_cache_folder() {
    let fixture = Fixture::with_an_eroded_file_removed_since_main();
    let user_cache = fixture.result_cache.path();

    let output = fixture
        .codeint_diff_command(&["main"])
        .env_remove("CODEINT_CACHE_DIR")
        .env("XDG_CACHE_HOME", user_cache)
        .env("LOCALAPPDATA", user_cache)
        .env("HOME", user_cache)
        .output()
        .expect("run codeint");

    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let entries = fs::read_dir(user_cache.join("codeint"))
        .expect("the codeint folder of the user cache folder exists")
        .count();
    assert_eq!(entries, 1);
}

#[test]
fn diff_runs_that_race_on_one_cache_entry_all_succeed_and_leave_a_readable_entry() {
    let fixture = Fixture::with_an_eroded_file_removed_since_main();
    let runs: Vec<_> = (0..8)
        .map(|_| {
            fixture
                .codeint_diff_command(&["main"])
                .env("CODEINT_CACHE_DIR", fixture.result_cache.path())
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .spawn()
                .expect("start codeint")
        })
        .collect();
    let reports: Vec<Output> = runs
        .into_iter()
        .map(|run| run.wait_with_output().expect("wait for codeint"))
        .collect();
    fixture.forget_committed_file("main:src/removed.ts");

    let read_back = fixture.successful_diff(&["main"]);

    for report in &reports {
        assert_eq!(report.status.code(), Some(0));
        assert!(
            report.stderr.is_empty(),
            "{}",
            String::from_utf8_lossy(&report.stderr)
        );
        assert_eq!(String::from_utf8_lossy(&report.stdout), read_back);
    }
    let entries = fs::read_dir(fixture.result_cache.path())
        .expect("list the result cache folder")
        .count();
    assert_eq!(entries, 1);
}
