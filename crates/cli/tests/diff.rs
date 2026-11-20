use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;
use std::process::{Command, Output};

use serde_json::{Value, json};
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

    fn revision(&self, args: &[&str]) -> String {
        self.run_git(args).trim().to_owned()
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
        self.diff_json_with(&[base_ref])
    }

    fn diff_json_with(&self, args: &[&str]) -> Value {
        let output = self.codeint_diff(&[args, &["--format", "json"]].concat());
        assert!(
            matches!(output.status.code(), Some(0 | 1)),
            "codeint diff failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        serde_json::from_slice(&output.stdout).expect("the report is JSON")
    }

    fn diff_outcome(&self) -> (Option<i32>, Value, String) {
        let output = self.codeint_diff(&["main", "--format", "json"]);
        let stderr = String::from_utf8(output.stderr).expect("errors are UTF-8");
        let report = serde_json::from_slice(&output.stdout)
            .unwrap_or_else(|error| panic!("the report is JSON ({error}): {stderr}"));
        (output.status.code(), report, stderr)
    }

    fn diff_passing(&self) -> Value {
        let (code, report, stderr) = self.diff_outcome();
        assert_eq!((code, stderr.as_str()), (Some(0), ""));
        assert_eq!(report.get("regressions"), None);
        report
    }

    fn diff_failing_with(&self, expected: &[Value], failure: &str) -> Value {
        let (code, report, stderr) = self.diff_outcome();
        assert_eq!(regressions(&report), expected);
        assert_eq!((code, stderr), (Some(1), format!("codeint: {failure}\n")));
        report
    }

    fn import_cycle_with_modules_cut_from_198_to(kept: usize) -> Fixture {
        let fixture = Fixture::with_committed_import_cycle_and_modules(198);
        fixture.remove_modules(kept + 1..=198);
        fixture
    }

    fn with_committed_function_of_11_conditions() -> Fixture {
        let fixture = Fixture::on_main();
        fixture.put("src/a.ts", &function_with_conditions_on_one_line("a", 11));
        fixture.commit("base");
        fixture
    }

    fn with_import_cycle_replaced_by_another() -> Fixture {
        let fixture = Fixture::with_committed_import_cycle_and_modules(8);
        fixture.remove("src/a.ts");
        fixture.remove("src/b.ts");
        fixture.put_import_cycle("c", "d");
        fixture
    }

    fn with_erosion_finding_moved_to_another_file() -> Fixture {
        let fixture = Fixture::with_committed_function_of_11_conditions();
        fixture.remove("src/a.ts");
        fixture.put("src/b.ts", &function_with_conditions_on_one_line("a", 11));
        fixture
    }

    fn put_regression_limits(&self, regression_limits: &str) {
        self.put(
            "codeint.json",
            &format!(r#"{{ "regression_limits": {regression_limits} }}"#),
        );
    }

    fn with_committed_import_cycle_and_modules(modules: usize) -> Fixture {
        let fixture = Fixture::on_main();
        fixture.put_import_cycle("a", "b");
        fixture.put_modules(1..=modules);
        fixture.commit("base");
        fixture
    }

    fn put_modules(&self, numbers: std::ops::RangeInclusive<usize>) {
        for number in numbers {
            self.put(
                &format!("src/m{number:03}.ts"),
                &format!("export const m{number} = {number};\n"),
            );
        }
    }

    fn remove_modules(&self, numbers: std::ops::RangeInclusive<usize>) {
        for number in numbers {
            self.remove(&format!("src/m{number:03}.ts"));
        }
    }

    fn put_import_cycle(&self, first: &str, second: &str) {
        self.put(
            &format!("src/{first}.ts"),
            &format!("import {{ {second} }} from './{second}';\nexport const {first} = () => {second}();\n"),
        );
        self.put(
            &format!("src/{second}.ts"),
            &format!(
                "import {{ {first} }} from './{first}';\nexport const {second} = () => {first}();\n"
            ),
        );
    }
}

fn function_with_conditions_on_one_line(name: &str, conditions: usize) -> String {
    function_with_joined_conditions(name, conditions, ">", " && ")
}

fn function_with_joined_conditions(
    name: &str,
    conditions: usize,
    comparison: &str,
    join: &str,
) -> String {
    let conditions: Vec<String> = (1..=conditions)
        .map(|value| format!("value {comparison} {value}"))
        .collect();
    format!(
        "export function {name}(value: number): boolean {{\n  return {};\n}}\n",
        conditions.join(join)
    )
}

fn regression(metric: &str, rule: &str, base: f64, head: f64, new_findings: usize) -> Value {
    json!({
        "source_set": "production",
        "metric": metric,
        "rule": rule,
        "base": base,
        "head": head,
        "new_findings": new_findings,
        "ratio_rise_limit": 0.01
    })
}

fn regressions(report: &Value) -> Vec<Value> {
    report
        .get("regressions")
        .map(|regressions| {
            regressions
                .as_array()
                .expect("a list of Regressions")
                .clone()
        })
        .unwrap_or_default()
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

const MEDIAN: &str = concat!(
    "export function median(values: number[]): number {\n",
    "  const sorted = [...values].sort(byValue);\n",
    "  const middle = Math.floor(sorted.length / 2);\n",
    "  if (sorted.length % 2 === 0) {\n",
    "    return (sorted[middle - 1] + sorted[middle]) / 2;\n",
    "  }\n",
    "  return sorted[middle];\n",
    "}\n",
);

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
    fixture.put("src/b.ts", &function_with_if_statements("b", 18));
    fixture.put("src/median.ts", MEDIAN);
    fixture.put("src/middle.ts", MEDIAN);

    let output = fixture.codeint_diff(&["main", "--format", "markdown"]);

    let eroded_function_table = "| Function | Cyclomatic complexity | Source lines | Function mass |\n\
                                 | -------- | --------------------- | ------------ | ------------- |";
    assert_eq!(output.status.code(), Some(1));
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
         | Production | Erosion | 1 | 0.885 | 1 | 1 | 0 |\n\
         | Production | Duplication | 0 | 0.4324 | 1 | 0 | 0 |\n\
         | Production | Verbosity | 0 | 0.4324 | 0 | 0 | 0 |\n\
         | Production | Import cycles | 0 | 0 | 0 | 0 | 0 |\n\
         \n\
         ## Regressions\n\
         \n\
         - The production erosion numerator eroded\\_function\\_mass rose from 44.8999 to 87.0689 with 1 new Finding.\n\
         - The production duplication ratio rose from 0 to 0.4324, more than its Regression limit 0.01.\n\
         - The production duplication numerator clone\\_lines rose from 0 to 16 with 1 new Finding.\n\
         - The production verbosity ratio rose from 0 to 0.4324, more than its Regression limit 0.01.\n\
         \n\
         ## Production changes\n\
         \n\
         ### New Erosion Findings\n\
         \n\
         {eroded_function_table}\n\
         | `src/b.ts:1` | 19 | 21 | 87.0689 |\n\
         \n\
         ### Resolved Erosion Findings\n\
         \n\
         {eroded_function_table}\n\
         | `src/a.ts:1` | 12 | 14 | 44.8999 |\n\
         \n\
         ### New Duplication Findings\n\
         \n\
         - 76 tokens: `src/median.ts:1-8`, `src/middle.ts:1-8`\n"
        )
    );
}

#[test]
fn a_ratio_rise_of_exactly_0_01_is_not_a_regression() {
    let report = Fixture::import_cycle_with_modules_cut_from_198_to(98).diff_passing();

    assert_import_cycles_ratios(&report, 0.01, 0.02);
}

#[test]
fn a_ratio_rise_over_0_01_is_a_regression_named_with_its_base_and_head_ratio() {
    Fixture::import_cycle_with_modules_cut_from_198_to(96).diff_failing_with(
        &[regression("import_cycles", "ratio_rise", 0.01, 0.0204, 0)],
        "the production import_cycles ratio rose from 0.01 to 0.0204, more than its Regression limit 0.01",
    );
}

#[test]
fn a_new_import_cycle_is_a_regression_even_when_neither_the_ratio_nor_the_numerator_rises() {
    Fixture::with_import_cycle_replaced_by_another().diff_failing_with(
        &[regression("import_cycles", "new_findings", 0.2, 0.2, 1)],
        "the production import_cycles Metric has 1 new Finding, and its Regression limit allows none",
    );
}

#[test]
fn an_import_cycle_that_stays_is_not_a_regression() {
    let fixture = Fixture::with_committed_import_cycle_and_modules(8);
    fixture.put_modules(9..=9);

    let report = fixture.diff_passing();

    assert_eq!(
        report["source_sets"]["production"]["import_cycles"]["unchanged"],
        1
    );
}

fn erosion_numerator_rise_with_1_new_finding() -> Fixture {
    let fixture = Fixture::with_committed_function_of_11_conditions();
    fixture.put(
        "src/b.ts",
        &function_with_joined_conditions("b", 11, "<", " || "),
    );
    fixture
}

fn assert_erosion_numerator_rise_fails(fixture: &Fixture) -> Value {
    fixture.diff_failing_with(
        &[regression(
            "erosion",
            "numerator_rise_with_new_findings",
            19.0526,
            38.1051,
            1,
        )],
        "the production erosion numerator eroded_function_mass rose from 19.0526 to 38.1051 with 1 new Finding",
    )
}

#[test]
fn a_numerator_rise_that_adds_new_findings_is_a_regression_named_with_its_base_and_head_numerator()
{
    let fixture = erosion_numerator_rise_with_1_new_finding();
    let report = assert_erosion_numerator_rise_fails(&fixture);

    let erosion = &report["source_sets"]["production"]["erosion"];
    assert_eq!(erosion["base_ratio"], erosion["head_ratio"]);
}

#[test]
fn a_finding_that_moves_to_another_file_without_a_numerator_rise_is_not_a_regression() {
    let report = Fixture::with_erosion_finding_moved_to_another_file().diff_passing();

    let erosion = &report["source_sets"]["production"]["erosion"];
    assert_eq!(
        (paths_of(&erosion["new"]), paths_of(&erosion["resolved"])),
        (vec!["src/b.ts".to_owned()], vec!["src/a.ts".to_owned()])
    );
}

fn import_cycles_ratio_from_0_02_to_0_04_under(regression_limits: &str) -> Fixture {
    let fixture = Fixture::with_committed_import_cycle_and_modules(98);
    fixture.remove_modules(49..=98);
    fixture.put_regression_limits(regression_limits);
    fixture
}

fn assert_import_cycles_ratio_rise_from_0_02_to_0_04_fails(regression_limits: &str, limit: f64) {
    let mut expected = regression("import_cycles", "ratio_rise", 0.02, 0.04, 0);
    expected["ratio_rise_limit"] = json!(limit);

    import_cycles_ratio_from_0_02_to_0_04_under(regression_limits).diff_failing_with(
        &[expected],
        &format!(
            "the production import_cycles ratio rose from 0.02 to 0.04, more than its Regression limit {limit}"
        ),
    );
}

fn assert_import_cycles_ratio_rise_from_0_02_to_0_04_passes(regression_limits: &str) {
    let report = import_cycles_ratio_from_0_02_to_0_04_under(regression_limits).diff_passing();

    assert_import_cycles_ratios(&report, 0.02, 0.04);
}

fn assert_import_cycles_ratio_rise_passes_with_production_and_metric_settings(
    production_setting: &str,
    metric_setting: &str,
) {
    assert_import_cycles_ratio_rise_from_0_02_to_0_04_passes(&format!(
        r#"{{ "production": {{ "import_cycles": {production_setting} }}, "import_cycles": {metric_setting} }}"#
    ));
}

fn assert_import_cycles_ratios(report: &Value, base: f64, head: f64) {
    let import_cycles = &report["source_sets"]["production"]["import_cycles"];
    assert_eq!(
        (&import_cycles["base_ratio"], &import_cycles["head_ratio"]),
        (&json!(base), &json!(head))
    );
}

fn assert_regression_limits_rejected(regression_limits: &str, message: &str, remedy: &str) {
    let fixture = Fixture::with_committed_function_of_11_conditions();
    fixture.put_regression_limits(regression_limits);

    assert_stops_with(
        &fixture.codeint_diff(&["main"]),
        &format!("codeint.json: {message}\n{remedy}\n"),
    );
}

#[test]
fn a_ratio_limit_raised_to_0_02_lets_a_0_02_rise_pass() {
    assert_import_cycles_ratio_rise_from_0_02_to_0_04_passes(
        r#"{ "import_cycles": { "ratio_rise": 0.02 } }"#,
    );
}

#[test]
fn a_0_02_rise_is_a_regression_under_the_default_limit() {
    assert_import_cycles_ratio_rise_from_0_02_to_0_04_fails("{}", 0.01);
}

#[test]
fn a_ratio_regression_names_the_ratio_limit_its_source_set_sets() {
    assert_import_cycles_ratio_rise_from_0_02_to_0_04_fails(
        r#"{ "production": { "import_cycles": { "ratio_rise": 0.015 } } }"#,
        0.015,
    );
}

#[test]
fn a_regression_limit_of_another_source_set_does_not_apply() {
    assert_import_cycles_ratio_rise_from_0_02_to_0_04_fails(
        r#"{ "test": { "import_cycles": { "ratio_rise": 0.02 } } }"#,
        0.01,
    );
}

#[test]
fn a_ratio_limit_of_the_source_set_wins_over_the_ratio_limit_of_the_metric() {
    assert_import_cycles_ratio_rise_passes_with_production_and_metric_settings(
        r#"{ "ratio_rise": 0.02 }"#,
        r#"{ "ratio_rise": 0.005 }"#,
    );
}

#[test]
fn a_source_set_setting_keeps_the_ratio_limit_that_the_metric_setting_raised() {
    assert_import_cycles_ratio_rise_passes_with_production_and_metric_settings(
        r#"{ "new_findings": "any_new_finding" }"#,
        r#"{ "ratio_rise": 0.02 }"#,
    );
}

#[test]
fn new_findings_set_to_any_new_finding_makes_a_moved_erosion_finding_a_regression() {
    let fixture = Fixture::with_erosion_finding_moved_to_another_file();
    fixture.put_regression_limits(r#"{ "erosion": { "new_findings": "any_new_finding" } }"#);

    fixture.diff_failing_with(
        &[regression("erosion", "new_findings", 1.0, 1.0, 1)],
        "the production erosion Metric has 1 new Finding, and its Regression limit allows none",
    );
}

fn assert_replaced_import_cycle_passes_with_new_findings_rule(rule: &str) {
    let fixture = Fixture::with_import_cycle_replaced_by_another();
    fixture.put_regression_limits(&format!(
        r#"{{ "import_cycles": {{ "new_findings": "{rule}" }} }}"#
    ));

    fixture.diff_passing();
}

#[test]
fn new_findings_set_to_numerator_rise_with_new_findings_lets_a_new_import_cycle_without_a_rise_pass()
 {
    assert_replaced_import_cycle_passes_with_new_findings_rule("numerator_rise_with_new_findings");
}

fn erosion_numerator_rise_with_new_findings_never_in(source_set: &str) -> Fixture {
    let fixture = erosion_numerator_rise_with_1_new_finding();
    fixture.put_regression_limits(&format!(
        r#"{{ "{source_set}": {{ "erosion": {{ "new_findings": "never" }} }} }}"#
    ));
    fixture
}

#[test]
fn new_findings_set_to_never_lets_a_numerator_rise_with_new_findings_in_its_source_set_pass() {
    let report = erosion_numerator_rise_with_new_findings_never_in("production").diff_passing();

    assert_eq!(
        paths_of(&report["source_sets"]["production"]["erosion"]["new"]),
        vec!["src/b.ts".to_owned()]
    );
}

#[test]
fn new_findings_set_to_never_in_another_source_set_leaves_the_numerator_rise_a_regression() {
    let fixture = erosion_numerator_rise_with_new_findings_never_in("generated");

    assert_erosion_numerator_rise_fails(&fixture);
}

#[test]
fn new_findings_set_to_never_for_a_metric_lets_a_new_import_cycle_pass() {
    assert_replaced_import_cycle_passes_with_new_findings_rule("never");
}

#[test]
fn a_ratio_limit_over_1_stops_diff_with_exit_code_2_and_names_the_key() {
    assert_regression_limits_rejected(
        r#"{ "production": { "erosion": { "ratio_rise": 1.5 } } }"#,
        "regression_limits.production.erosion.ratio_rise must be between 0 and 1, not 1.5",
        "Set the value to a ratio from 0 to 1, as the report shows ratios.",
    );
}

#[test]
fn an_unknown_new_findings_rule_stops_diff_with_exit_code_2_and_names_the_key_and_the_rules() {
    assert_regression_limits_rejected(
        r#"{ "erosion": { "new_findings": "sometimes" } }"#,
        "regression_limits.erosion.new_findings must be one of any_new_finding, numerator_rise_with_new_findings, never, not \"sometimes\"",
        "Set the value to one of the allowed values; the JSON schema that $schema links to describes each one.",
    );
}

#[test]
fn a_diff_with_no_regression_exits_0_and_prints_no_regressions_key() {
    let fixture = Fixture::on_main();
    fixture.put("src/a.ts", &eroded_function("a"));
    fixture.commit("base");
    fixture.put("src/calm.ts", "export const calm = 1;\n");

    let report = fixture.diff_passing();

    assert_eq!(report.get("over_budget"), None);
}

#[test]
fn diff_applies_budgets_to_the_head_and_exits_1_when_a_metric_is_over_its_budget() {
    let fixture = Fixture::with_committed_function_of_11_conditions();
    fixture.put(
        "codeint.json",
        r#"{ "budgets": { "production": { "erosion": 0.5, "duplication": 0 } } }"#,
    );

    let report =
        fixture.diff_failing_with(&[], "the production erosion ratio 1 is over its Budget 0.5");

    assert_eq!(
        report["over_budget"],
        json!([{
            "source_set": "production",
            "metric": "erosion",
            "ratio": 1.0,
            "budget": 0.5
        }])
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
    fixture.put("src/median.ts", MEDIAN);
    fixture.put("src/middle.ts", MEDIAN);
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

    assert_eq!(output.status.code(), Some(1));
    let markdown = String::from_utf8(output.stdout).expect("the report is UTF-8");
    let expected = format!(
        "codeint compared `feature` at `{feature}` with `{merge_base}`, the merge-base of `main` and `feature`."
    );
    assert_eq!(markdown.lines().nth(2), Some(expected.as_str()));
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

#[test]
fn a_second_diff_with_head_reads_both_commits_from_the_cache() {
    let fixture = Fixture::with_an_eroded_file_removed_since_main();
    fixture.run_git(&["switch", "--quiet", "--create", "feature"]);
    fixture.put("src/added.ts", &eroded_function("added"));
    fixture.commit("feature work");
    let first = fixture.successful_diff(&["main", "--head", "feature"]);
    fixture.forget_committed_file("main:src/removed.ts");
    fixture.forget_committed_file("feature:src/added.ts");

    let second = fixture.successful_diff(&["main", "--head", "feature"]);

    assert_eq!(second, first);
}
