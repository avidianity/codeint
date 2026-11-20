use std::collections::{BTreeMap, BTreeSet};
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

    fn revision(&self, args: &[&str]) -> String {
        self.run_git(args).trim().to_owned()
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
        self.diff_json_with(&[base_ref])
    }

    fn diff_json_with(&self, args: &[&str]) -> Value {
        let output = self.codeint_diff(&[args, &["--format", "json"]].concat());
        assert_eq!(
            output.status.code(),
            Some(0),
            "codeint diff failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        serde_json::from_slice(&output.stdout).expect("the report is JSON")
    }
}

fn repository_state(fixture: &Fixture) -> (Vec<u8>, String, String) {
    (
        fs::read(fixture.root().join(".git/index")).expect("read the index"),
        fixture.run_git(&[
            "status",
            "--porcelain=v2",
            "--branch",
            "--untracked-files=all",
        ]),
        fixture.run_git(&["rev-parse", "HEAD"]),
    )
}

fn assert_stops_with(output: &Output, message: &str) {
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.starts_with(&format!("codeint: {message}")),
        "unexpected error: {stderr}"
    );
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
            repository_state(fixture),
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

    assert_stops_with(
        &output,
        "the Git ref \"no-such-branch\" does not name a commit in this repository",
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

const METRIC_FINDINGS: [(&str, &str); 4] = [
    ("erosion", "findings"),
    ("duplication", "clone_groups"),
    ("verbosity", "findings"),
    ("import_cycles", "findings"),
];

#[derive(Debug, PartialEq, Eq)]
struct Classification {
    new: BTreeSet<String>,
    resolved: BTreeSet<String>,
    unchanged: usize,
}

struct FeatureRevisions {
    merge_base: String,
    feature: String,
}

fn ids_of(findings: &Value) -> BTreeSet<String> {
    findings
        .as_array()
        .map(|listed| {
            listed
                .iter()
                .map(|finding| finding["id"].as_str().expect("an id").to_owned())
                .collect()
        })
        .unwrap_or_default()
}

fn source_set_names(report: &Value) -> BTreeSet<String> {
    report["source_sets"]
        .as_object()
        .map(|sets| sets.keys().cloned().collect())
        .unwrap_or_default()
}

fn check_json_of_checkout(fixture: &Fixture, revision: &str, checkouts: &Path) -> Value {
    let checkout = checkouts.join(revision.replace('/', "-"));
    fixture.run_git(&[
        "worktree",
        "add",
        "--quiet",
        "--detach",
        checkout.to_str().expect("a UTF-8 path"),
        revision,
    ]);
    let output = Command::new(env!("CARGO_BIN_EXE_codeint"))
        .args(["check", "--all", "--format", "json"])
        .current_dir(&checkout)
        .output()
        .expect("run codeint check");
    assert!(
        matches!(output.status.code(), Some(0 | 1)),
        "codeint check failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("the report is JSON")
}

fn per_metric(
    source_sets: BTreeSet<String>,
    classify: impl Fn(&str, &str, &str) -> Classification,
) -> BTreeMap<String, Classification> {
    source_sets
        .iter()
        .flat_map(|source_set| {
            METRIC_FINDINGS.map(|(metric, findings)| {
                (
                    format!("{source_set}.{metric}"),
                    classify(source_set, metric, findings),
                )
            })
        })
        .collect()
}

fn classification_by_hand(base: &Value, head: &Value) -> BTreeMap<String, Classification> {
    let source_sets = &source_set_names(base) | &source_set_names(head);
    per_metric(source_sets, |source_set, metric, findings| {
        let base_ids = ids_of(&base["source_sets"][source_set][metric][findings]);
        let head_ids = ids_of(&head["source_sets"][source_set][metric][findings]);
        Classification {
            new: &head_ids - &base_ids,
            resolved: &base_ids - &head_ids,
            unchanged: head_ids.intersection(&base_ids).count(),
        }
    })
}

fn classification_of_diff(report: &Value) -> BTreeMap<String, Classification> {
    per_metric(source_set_names(report), |source_set, metric, _| {
        let changes = &report["source_sets"][source_set][metric];
        let unchanged = changes["unchanged"].as_u64().expect("a count");
        Classification {
            new: ids_of(&changes["new"]),
            resolved: ids_of(&changes["resolved"]),
            unchanged: usize::try_from(unchanged).expect("a small count"),
        }
    })
}

fn two_branches_and_a_dirty_working_tree() -> Fixture {
    let fixture = Fixture::on_main();
    fixture.put(
        "src/a.ts",
        "import { b } from \"./b\";\nexport const a = b;\n",
    );
    fixture.put("src/kept.ts", &eroded_function("kept"));
    fixture.put("src/b.ts", "export const b = 2;\n");
    fixture.put("src/removed.ts", &eroded_function("removed"));
    fixture.put("tests/kept.test.ts", &eroded_function("keptTest"));
    fixture.commit("start");
    fixture.run_git(&["switch", "--quiet", "--create", "feature"]);
    fixture.remove("src/removed.ts");
    fixture.put("src/added.ts", &function_with_if_statements("added", 12));
    fixture.put(
        "src/b.ts",
        "import { a } from \"./a\";\nexport const b = 2;\nexport const c = a;\n",
    );
    fixture.put("tests/added.test.ts", &eroded_function("addedTest"));
    fixture.commit("feature work");
    fixture.run_git(&["switch", "--quiet", "main"]);
    fixture.put("src/later_on_main.ts", &eroded_function("laterOnMain"));
    fixture.commit("later work on main");
    fixture.put("src/uncommitted.ts", &eroded_function("uncommitted"));
    fixture.remove("src/kept.ts");
    fixture
}

fn feature_revisions(fixture: &Fixture) -> FeatureRevisions {
    FeatureRevisions {
        merge_base: fixture.revision(&["merge-base", "main", "feature"]),
        feature: fixture.revision(&["rev-parse", "feature"]),
    }
}

#[test]
fn diff_with_head_classifies_two_commits_like_checking_each_out_by_hand() {
    let checkouts = tempfile::tempdir().expect("create a temporary folder");
    let fixture = two_branches_and_a_dirty_working_tree();

    let report = fixture.diff_json_with(&["main", "--all", "--head", "feature"]);

    let merge_base = fixture.revision(&["merge-base", "main", "feature"]);
    let base_by_hand = check_json_of_checkout(&fixture, &merge_base, checkouts.path());
    let head_by_hand = check_json_of_checkout(&fixture, "feature", checkouts.path());
    let by_hand = classification_by_hand(&base_by_hand, &head_by_hand);
    assert_eq!(classification_of_diff(&report), by_hand);
    assert_eq!(by_hand["production.erosion"].new.len(), 1);
    assert_eq!(by_hand["production.erosion"].resolved.len(), 1);
    assert_eq!(by_hand["production.erosion"].unchanged, 1);
    assert!(!by_hand["production.duplication"].new.is_empty());
    assert_eq!(by_hand["production.import_cycles"].new.len(), 1);
    assert_eq!(by_hand["test.erosion"].new.len(), 1);
}

#[test]
fn diff_with_head_names_the_head_ref_and_takes_its_merge_base_with_the_base_ref() {
    let fixture = two_branches_and_a_dirty_working_tree();
    let revisions = feature_revisions(&fixture);

    let report = fixture.diff_json_with(&["main", "--head", "feature"]);

    assert_eq!(report["base"]["ref"], "main");
    assert_eq!(report["base"]["merge_base"], revisions.merge_base.as_str());
    assert_eq!(report["head"]["ref"], "feature");
    assert_eq!(report["head"]["commit"], revisions.feature.as_str());
}

#[test]
fn diff_with_head_leaves_the_working_tree_the_index_and_head_untouched() {
    let fixture = two_branches_and_a_dirty_working_tree();
    let before = repository_state(&fixture);

    fixture.diff_json_with(&["main", "--head", "feature"]);

    assert_eq!(repository_state(&fixture), before);
}

#[test]
fn an_unknown_head_ref_exits_2_and_names_the_ref() {
    let fixture = Fixture::on_main();
    fixture.put("src/a.ts", "export const a = 1;\n");
    fixture.commit("base");

    let output = fixture.codeint_diff(&["main", "--head", "no-such-branch"]);

    assert_stops_with(
        &output,
        "the Git ref \"no-such-branch\" does not name a commit in this repository",
    );
}

#[test]
fn a_head_ref_without_history_in_common_with_the_base_ref_exits_2_and_names_both_refs() {
    let fixture = Fixture::on_main();
    fixture.put("src/on_main.ts", "export const onMain = 1;\n");
    fixture.commit("work on main");
    fixture.run_git(&["switch", "--quiet", "--orphan", "unrelated"]);
    fixture.put("src/b.ts", "export const b = 1;\n");
    fixture.commit("unrelated");

    let output = fixture.codeint_diff(&["main", "--head", "unrelated"]);

    assert_stops_with(
        &output,
        "\"main\" and \"unrelated\" have no commit in common, so they have no merge-base",
    );
}

#[test]
fn format_markdown_with_head_names_the_head_ref_and_its_commit() {
    let fixture = two_branches_and_a_dirty_working_tree();
    let FeatureRevisions {
        merge_base,
        feature,
    } = feature_revisions(&fixture);

    let output = fixture.codeint_diff(&["main", "--head", "feature", "--format", "markdown"]);

    assert_eq!(output.status.code(), Some(0));
    let markdown = String::from_utf8(output.stdout).expect("the report is UTF-8");
    let expected = format!(
        "codeint compared `feature` at `{feature}` with `{merge_base}`, the merge-base of `main` and `feature`."
    );
    assert_eq!(markdown.lines().nth(2), Some(expected.as_str()));
}
