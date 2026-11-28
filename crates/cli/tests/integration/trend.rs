use std::fs;
use std::path::Path;
use std::process::{Command, Output};

use serde_json::Value;
use tempfile::TempDir;

const ISOLATED_GIT: [(&str, &str); 4] = [
    ("GIT_CONFIG_GLOBAL", "/dev/null"),
    ("GIT_CONFIG_NOSYSTEM", "1"),
    ("GIT_AUTHOR_NAME", "Trend fixture"),
    ("GIT_AUTHOR_EMAIL", "trend@example.invalid"),
];

struct History {
    repository: TempDir,
    cache: TempDir,
}

impl History {
    fn empty() -> History {
        let history = History {
            repository: tempfile::tempdir().expect("create the repository folder"),
            cache: tempfile::tempdir().expect("create the result cache folder"),
        };
        history.git(&["init", "--quiet", "--initial-branch", "main"]);
        history
    }

    fn with_commits(count: usize) -> History {
        let history = History::empty();
        for number in 1..=count {
            history.write(&format!("src/step{number}.ts"), &branching_function(number));
            history.commit_on(&format!("step {number}"), "2026-03-01T12:00:00Z");
        }
        history
    }

    fn folder(&self) -> &Path {
        self.repository.path()
    }

    fn write(&self, relative: &str, text: &str) {
        let path = self.folder().join(relative);
        fs::create_dir_all(path.parent().expect("a file sits in a folder"))
            .expect("create the folder of the file");
        fs::write(path, text).expect("write the file");
    }

    fn git(&self, args: &[&str]) -> String {
        self.git_at(args, "2026-03-01T12:00:00Z")
    }

    fn git_at(&self, args: &[&str], committer_date: &str) -> String {
        let output = Command::new("git")
            .args(args)
            .current_dir(self.folder())
            .envs(ISOLATED_GIT)
            .env("GIT_COMMITTER_NAME", "Trend fixture")
            .env("GIT_COMMITTER_EMAIL", "trend@example.invalid")
            .env("GIT_COMMITTER_DATE", committer_date)
            .env("GIT_AUTHOR_DATE", committer_date)
            .output()
            .expect("run git");
        assert!(
            output.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).expect("git prints UTF-8")
    }

    fn commit_on(&self, message: &str, committer_date: &str) {
        self.git(&["add", "--all"]);
        self.git_at(&["commit", "--quiet", "--message", message], committer_date);
    }

    fn main_line_oldest_first(&self) -> Vec<String> {
        self.git(&["rev-list", "--first-parent", "--reverse", "HEAD"])
            .lines()
            .map(str::to_owned)
            .collect()
    }

    fn trend(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_codeint"))
            .arg("trend")
            .args(args)
            .current_dir(self.folder())
            .env("CODEINT_CACHE_DIR", self.cache.path())
            .output()
            .expect("run codeint trend")
    }

    fn trend_json(&self, args: &[&str]) -> Value {
        let output = self.trend(&[args, &["--format", "json"]].concat());
        assert_eq!(
            output.status.code(),
            Some(0),
            "codeint trend failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        serde_json::from_slice(&output.stdout).expect("the trend is JSON")
    }
}

fn branching_function(branches: usize) -> String {
    let body: String = (1..=branches)
        .map(|value| format!("  if (input === {value}) return {value};\n"))
        .collect();
    format!("export function step{branches}(input: number): number {{\n{body}  return 0;\n}}\n")
}

fn production_commits(trend: &Value) -> Vec<String> {
    trend["source_sets"]["production"]
        .as_array()
        .expect("one row per commit")
        .iter()
        .map(|row| row["commit"].as_str().expect("a commit id").to_owned())
        .collect()
}

#[test]
fn three_commits_print_three_rows_oldest_first() {
    let history = History::with_commits(3);

    let trend = history.trend_json(&[]);

    assert_eq!(production_commits(&trend), history.main_line_oldest_first());
    assert_eq!(trend["commits"], 3);
}

fn eroded_function() -> String {
    branching_function(12)
}

fn forget_blob(history: &History, spec: &str) {
    let object = history.git(&["rev-parse", spec]);
    let (folder, file) = object.trim().split_at(2);
    fs::remove_file(
        history
            .folder()
            .join(".git/objects")
            .join(folder)
            .join(file),
    )
    .expect("remove the loose blob object");
}

fn stderr_of(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

#[test]
fn commits_of_a_merged_side_branch_are_not_rows_but_the_merge_commit_is() {
    let history = History::with_commits(1);
    history.git(&["switch", "--quiet", "--create", "side"]);
    history.write("src/side.ts", &eroded_function());
    history.commit_on("side work", "2026-03-02T12:00:00Z");
    history.write("src/side_more.ts", &branching_function(2));
    history.commit_on("more side work", "2026-03-03T12:00:00Z");
    let side_commits: Vec<String> = history
        .git(&["rev-list", "main..side"])
        .lines()
        .map(str::to_owned)
        .collect();
    history.git(&["switch", "--quiet", "main"]);
    history.write("src/main_work.ts", &branching_function(3));
    history.commit_on("main work", "2026-03-04T12:00:00Z");
    history.git_at(
        &[
            "merge",
            "--quiet",
            "--no-ff",
            "--message",
            "merge side",
            "side",
        ],
        "2026-03-05T12:00:00Z",
    );

    let rows = production_commits(&history.trend_json(&[]));

    assert_eq!(rows.len(), 3);
    assert_eq!(rows, history.main_line_oldest_first());
    assert!(side_commits.iter().all(|commit| !rows.contains(commit)));
}

#[test]
fn without_since_the_latest_20_main_line_commits_are_rows_and_since_lifts_that_limit() {
    let history = History::with_commits(23);
    let main_line = history.main_line_oldest_first();

    let latest = production_commits(&history.trend_json(&[]));
    let since = production_commits(&history.trend_json(&["--since", "2026-01-01"]));

    assert_eq!(latest, main_line[3..]);
    assert_eq!(since, main_line);
}

#[test]
fn since_keeps_the_commits_committed_on_or_after_the_start_of_that_day_in_utc() {
    let history = History::empty();
    for (number, committed_at) in [
        (1, "2026-01-10T12:00:00Z"),
        (2, "2026-02-10T00:00:00Z"),
        (3, "2026-03-10T23:59:59Z"),
    ] {
        history.write(&format!("src/step{number}.ts"), &branching_function(number));
        history.commit_on(&format!("step {number}"), committed_at);
    }
    let main_line = history.main_line_oldest_first();

    let trend = history.trend_json(&["--since", "2026-02-10"]);

    assert_eq!(production_commits(&trend), main_line[1..]);
    let days: Vec<&str> = trend["source_sets"]["production"]
        .as_array()
        .expect("rows")
        .iter()
        .map(|row| row["committed_on"].as_str().expect("a day"))
        .collect();
    assert_eq!(days, ["2026-02-10", "2026-03-10"]);
}

#[test]
fn since_after_the_latest_commit_prints_no_rows() {
    let history = History::with_commits(2);

    let trend = history.trend_json(&["--since", "2026-03-02"]);

    assert_eq!(trend["commits"], 0);
    assert_eq!(trend["source_sets"], serde_json::json!({}));
}

#[test]
fn each_row_shows_the_ratio_of_each_metric_at_that_commit() {
    let history = History::with_commits(1);
    history.write("src/eroded.ts", &eroded_function());
    history.commit_on("add an eroded function", "2026-03-02T12:00:00Z");

    let trend = history.trend_json(&[]);

    let rows = trend["source_sets"]["production"].as_array().expect("rows");
    assert_eq!(rows[0]["erosion"], 0.0);
    assert!(rows[1]["erosion"].as_f64().expect("a ratio") > 0.0);
    assert_eq!(rows[1]["files"], 2);
    for metric in ["duplication", "verbosity", "import_cycles"] {
        assert!(rows[1][metric].is_number(), "{metric} is missing");
    }
}

#[test]
fn a_repeated_trend_reads_each_commit_from_the_cache() {
    let history = History::with_commits(3);
    let first = history.trend(&[]);
    assert_eq!(first.status.code(), Some(0), "{}", stderr_of(&first));
    forget_blob(&history, "HEAD:src/step1.ts");

    let second = history.trend(&[]);

    assert_eq!(second.status.code(), Some(0), "{}", stderr_of(&second));
    assert_eq!(second.stdout, first.stdout);
}

#[test]
fn without_a_cached_result_trend_reads_every_commit_from_git_objects() {
    let history = History::with_commits(2);
    forget_blob(&history, "HEAD:src/step1.ts");

    let output = history.trend(&[]);

    assert_eq!(output.status.code(), Some(2));
    assert!(
        stderr_of(&output).starts_with("codeint: cannot read src/step1.ts from commit"),
        "unexpected error: {}",
        stderr_of(&output)
    );
}

#[test]
fn trend_prints_toon_by_default_with_one_table_per_source_set() {
    let history = History::with_commits(2);
    history.write("src/step3.test.ts", &branching_function(3));
    history.commit_on("add a test", "2026-03-02T12:00:00Z");
    let [first, second, third] =
        <[String; 3]>::try_from(history.main_line_oldest_first()).expect("three commits");

    let output = history.trend(&[]);

    assert_eq!(output.status.code(), Some(0), "{}", stderr_of(&output));
    let columns = "{commit,committed_on,files,erosion,cognitive_complexity,duplication,verbosity,import_cycles}";
    let expected = format!(
        "commits: 3\nmetric_versions:\n  cognitive_complexity: 1\n  duplication: 1\n  erosion: 1\n  import_cycles: 1\n  verbosity: 2\nsource_sets:\n  production[3]{columns}:\n    {first},2026-03-01,1,0,0,0,0,0\n    {second},2026-03-01,2,0,0,0,0,0\n    {third},2026-03-02,2,0,0,0,0,0\n  test[3]{columns}:\n    {first},2026-03-01,0,0,0,0,0,0\n    {second},2026-03-01,0,0,0,0,0,0\n    {third},2026-03-02,1,0,0,0,0,0\n"
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout), expected);
}

#[test]
fn format_markdown_shows_one_table_per_source_set_oldest_commit_first() {
    let history = History::with_commits(2);
    let [first, second] =
        <[String; 2]>::try_from(history.main_line_oldest_first()).expect("two commits");

    let output = history.trend(&["--format", "markdown"]);

    assert_eq!(output.status.code(), Some(0), "{}", stderr_of(&output));
    let expected = format!(
        "# codeint trend\n\ncodeint measured 2 commits of the main line of HEAD, oldest first.\n\n## Production\n\n| Commit | Committed on | Files | Erosion | Cognitive complexity | Duplication | Verbosity | Import cycles |\n| ------ | ------------ | ----- | ------- | -------------------- | ----------- | --------- | ------------- |\n| `{first}` | 2026-03-01 | 1 | 0 | 0 | 0 | 0 | 0 |\n| `{second}` | 2026-03-01 | 2 | 0 | 0 | 0 | 0 | 0 |\n"
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout), expected);
}

#[test]
fn trend_on_a_branch_without_commits_exits_2_and_says_so() {
    let history = History::empty();

    let output = history.trend(&[]);

    assert_eq!(output.status.code(), Some(2));
    assert!(
        stderr_of(&output).starts_with(
            "codeint: HEAD does not name a commit yet, so the current branch has no history to measure"
        ),
        "unexpected error: {}",
        stderr_of(&output)
    );
}

#[test]
fn since_that_is_not_a_yyyy_mm_dd_day_exits_2_and_names_the_form() {
    let history = History::with_commits(1);

    for text in ["2026-02-30", "2026-2-01", "yesterday", "2026-02-01T00:00"] {
        let output = history.trend(&["--since", text]);

        assert_eq!(output.status.code(), Some(2), "{text}");
        assert!(
            stderr_of(&output).contains(&format!(
                "{text:?} is not a real day written as YYYY-MM-DD, such as 2026-01-31"
            )),
            "unexpected error for {text}: {}",
            stderr_of(&output)
        );
    }
}

#[test]
fn format_sarif_is_not_offered_because_a_trend_lists_no_findings() {
    let history = History::with_commits(1);

    let output = history.trend(&["--format", "sarif"]);

    assert_eq!(output.status.code(), Some(2));
    assert!(
        stderr_of(&output).contains("[possible values: toon, text, json, markdown]"),
        "unexpected error: {}",
        stderr_of(&output)
    );
}

#[test]
fn trend_in_a_shallow_clone_stops_at_the_shallow_boundary() {
    let origin = History::with_commits(4);
    let clone = History::empty();
    let source = format!("file://{}", origin.folder().display());
    clone.git(&["fetch", "--quiet", "--depth", "2", &source, "main"]);
    clone.git(&["reset", "--quiet", "--hard", "FETCH_HEAD"]);

    let rows = production_commits(&clone.trend_json(&[]));

    assert_eq!(rows, origin.main_line_oldest_first()[2..]);
}

#[test]
fn trend_leaves_the_working_tree_the_index_and_head_untouched() {
    let history = History::with_commits(2);
    history.write("src/uncommitted.ts", &eroded_function());
    let status = || {
        history.git(&[
            "status",
            "--porcelain=v2",
            "--branch",
            "--untracked-files=all",
        ])
    };
    let index = || fs::read(history.folder().join(".git/index")).expect("read the index");
    let before = (status(), index());

    let trend = history.trend_json(&[]);

    assert_eq!(trend["commits"], 2);
    assert_eq!((status(), index()), before);
}

#[test]
fn a_metric_version_change_invalidates_the_trend_commit_cache_and_recomputes() {
    let history = History::with_commits(1);
    let initial = history.trend_json(&[]);
    assert_eq!(initial["metric_versions"]["erosion"], 1);
    assert_eq!(initial["metric_versions"]["verbosity"], 2);

    let cache_file = fs::read_dir(history.cache.path())
        .expect("list cache")
        .next()
        .expect("one entry")
        .expect("entry")
        .path();
    let stored: Value = serde_json::from_slice(&fs::read(cache_file).expect("read")).expect("json");
    let mut settings: Value =
        serde_json::from_str(stored["settings"].as_str().expect("settings")).expect("settings");
    assert_eq!(settings["metric_versions"]["verbosity"], 2);
    settings["metric_versions"]["verbosity"] = serde_json::json!(3);
    let mut stale = stored["result"].clone();
    stale["production"]["verbosity"]["ratio"] = serde_json::json!(0.9999);

    let head = codeint_git::Repository::discover(history.folder())
        .expect("repo")
        .resolve("HEAD")
        .expect("HEAD");
    let fresh_dir = tempfile::tempdir().expect("fresh cache");
    codeint_git::ResultCache::in_folder(fresh_dir.path().to_path_buf())
        .write(head, &settings, &stale)
        .expect("seed trend cache");

    let run = Command::new(env!("CARGO_BIN_EXE_codeint"))
        .args(["trend", "--format", "json"])
        .current_dir(history.folder())
        .env("CODEINT_CACHE_DIR", fresh_dir.path())
        .output()
        .expect("run trend");
    assert!(run.status.success(), "{}", stderr_of(&run));
    let recomputed: Value = serde_json::from_slice(&run.stdout).expect("trend json");
    assert_eq!(recomputed["source_sets"]["production"][0]["verbosity"], 0.0);
    assert_eq!(fs::read_dir(fresh_dir.path()).expect("dir").count(), 2);
}

#[test]
fn trend_since_tolerates_out_of_order_skew_within_14_days_and_stops_beyond() {
    let history = History::empty();
    history.write("src/step1.ts", &branching_function(1));
    history.commit_on("unreachable newer", "2026-05-12T12:00:00Z");
    history.write("src/step2.ts", &branching_function(2));
    history.commit_on("older than 14d", "2026-04-25T23:59:59Z");
    history.write("src/step3.ts", &branching_function(3));
    history.commit_on("newer behind skew", "2026-05-11T12:00:00Z");
    history.write("src/step4.ts", &branching_function(4));
    history.commit_on("1-day older skew", "2026-05-09T12:00:00Z");
    history.write("src/step5.ts", &branching_function(5));
    history.commit_on("head", "2026-05-15T12:00:00Z");

    let commits = history.main_line_oldest_first();
    let report = history.trend_json(&["--since", "2026-05-10"]);
    assert_eq!(
        production_commits(&report),
        vec![commits[2].clone(), commits[4].clone()]
    );
}
