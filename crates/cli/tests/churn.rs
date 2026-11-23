use std::fs;
use std::path::Path;
use std::process::{Command, Output};

use serde_json::Value;
use tempfile::TempDir;

struct ChurnFixture(TempDir);

impl ChurnFixture {
    fn init() -> ChurnFixture {
        let dir = tempfile::tempdir().expect("tempdir");
        run_git(
            dir.path(),
            &["init", "-q", "-b", "main"],
            "2026-05-01T10:00:00Z",
        );
        ChurnFixture(dir)
    }

    fn root(&self) -> &Path {
        self.0.path()
    }

    fn save_and_commit(&self, files: &[(&str, String)], msg: &str, when: &str) {
        for (rel, content) in files {
            let dest = self.root().join(rel);
            fs::create_dir_all(dest.parent().expect("parent")).expect("mkdir");
            fs::write(dest, content).expect("write");
        }
        run_git(self.root(), &["add", "-A"], when);
        run_git(self.root(), &["commit", "-q", "-m", msg], when);
    }

    fn churn(&self, extra: &[&str]) -> Output {
        exec_churn(self.root(), extra)
    }

    fn churn_json(&self, extra: &[&str]) -> Value {
        let args: Vec<&str> = extra.iter().copied().chain(["--format", "json"]).collect();
        let out = self.churn(&args);
        assert_eq!(
            out.status.code(),
            Some(0),
            "churn stderr: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        serde_json::from_slice(&out.stdout).expect("valid churn JSON")
    }
}

fn run_git(cwd: &Path, args: &[&str], when: &str) {
    let vars = [
        ("GIT_CONFIG_GLOBAL", "/dev/null"),
        ("GIT_CONFIG_NOSYSTEM", "1"),
        ("GIT_AUTHOR_NAME", "Churn"),
        ("GIT_AUTHOR_EMAIL", "churn@example.invalid"),
        ("GIT_COMMITTER_NAME", "Churn"),
        ("GIT_COMMITTER_EMAIL", "churn@example.invalid"),
        ("GIT_AUTHOR_DATE", when),
        ("GIT_COMMITTER_DATE", when),
    ];
    let out = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .envs(vars)
        .output()
        .expect("git");
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

fn exec_churn(cwd: &Path, extra: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_codeint"))
        .arg("churn")
        .args(extra)
        .current_dir(cwd)
        .output()
        .expect("run codeint churn")
}

fn eroded_ts(name: &str, salt: usize) -> String {
    let checks: String = (1..=12)
        .map(|i| format!("  if (v === {i}) return {};\n", i + salt))
        .collect();
    format!("export function {name}(v: number): number {{\n{checks}  return {salt};\n}}\n")
}

fn duplicated_ts(prefix: &str, extra: &str) -> String {
    format!(
        "export function {prefix}Calc(items: number[], fee: number): number {{\n\
         \x20 let acc = 0;\n\
         \x20 for (const item of items) {{\n\
         \x20   if (item > 0) {{\n\
         \x20     acc += item * 2;\n\
         \x20   }} else {{\n\
         \x20     acc -= item;\n\
         \x20   }}\n\
         \x20 }}\n\
         \x20 const net = acc - fee;\n\
         \x20 const rounded = Math.round(net * 100) / 100;\n\
         \x20 const floored = Math.max(rounded, 0);\n\
         \x20 const finalValue = floored + 1;\n\
         \x20 return finalValue;\n\
         }}\n\
         {extra}\n"
    )
}

fn assert_no_blended_score_keys(value: &Value) {
    let forbidden = ["score", "index", "grade", "rating", "hotspot_score"];
    match value {
        Value::Object(map) => {
            for (k, v) in map {
                let lower = k.to_ascii_lowercase();
                for word in forbidden {
                    assert!(
                        !lower.contains(word),
                        "ADR 0003 violation: unexpected blended key {k:?}"
                    );
                }
                assert_no_blended_score_keys(v);
            }
        }
        Value::Array(items) => items.iter().for_each(assert_no_blended_score_keys),
        _ => {}
    }
}

fn assert_format_contains(repo: &ChurnFixture, format: &str, needles: &[&str]) {
    let out = repo.churn(&["--format", format]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "{format} stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    for needle in needles {
        assert!(
            stdout.contains(needle),
            "{format} missing {needle:?}: {stdout}"
        );
    }
}

#[test]
fn churn_ranks_eroded_and_duplicated_files_independently_by_commit_count_with_no_score() {
    let repo = ChurnFixture::init();
    repo.save_and_commit(
        &[
            ("src/cold_eroded.ts", eroded_ts("coldEroded", 0)),
            ("src/hot_eroded.ts", eroded_ts("hotEroded", 1)),
            ("src/cold_dup.ts", duplicated_ts("cold", "")),
            (
                "src/hot_dup.ts",
                duplicated_ts("hot", "export const v1 = 1;"),
            ),
            ("src/clean.ts", "export const clean = 1;\n".to_owned()),
        ],
        "initial",
        "2026-05-01T10:00:00Z",
    );
    repo.save_and_commit(
        &[
            ("src/hot_eroded.ts", eroded_ts("hotEroded", 2)),
            (
                "src/hot_dup.ts",
                duplicated_ts("hot", "export const v1 = 2;"),
            ),
            ("src/clean.ts", "export const clean = 2;\n".to_owned()),
        ],
        "touch hot files 2",
        "2026-05-02T10:00:00Z",
    );
    repo.save_and_commit(
        &[
            ("src/hot_eroded.ts", eroded_ts("hotEroded", 3)),
            (
                "src/hot_dup.ts",
                duplicated_ts("hot", "export const v1 = 3;"),
            ),
            ("src/clean.ts", "export const clean = 3;\n".to_owned()),
        ],
        "touch hot files 3",
        "2026-05-03T10:00:00Z",
    );
    repo.save_and_commit(
        &[
            ("src/hot_eroded.ts", eroded_ts("hotEroded", 4)),
            ("src/clean.ts", "export const clean = 4;\n".to_owned()),
        ],
        "touch hot eroded 4",
        "2026-05-04T10:00:00Z",
    );

    let report = repo.churn_json(&[]);
    assert_no_blended_score_keys(&report);
    assert_eq!(report["commits_analyzed"], 4);
    assert_eq!(report["shallow"], false);
    assert!(report.get("warning").is_none());

    let eroded = report["source_sets"]["production"]["erosion"]["findings"]
        .as_array()
        .expect("eroded findings");
    assert_eq!(eroded.len(), 2);
    assert_eq!(eroded[0]["path"], "src/hot_eroded.ts");
    assert_eq!(eroded[0]["commits"], 4);
    assert_eq!(eroded[0]["eroded_functions"], 1);
    assert!(eroded[0]["eroded_function_mass"].as_f64().expect("mass") > 0.0);
    assert_eq!(eroded[1]["path"], "src/cold_eroded.ts");
    assert_eq!(eroded[1]["commits"], 1);

    let duplicated = report["source_sets"]["production"]["duplication"]["findings"]
        .as_array()
        .expect("duplication findings");
    assert_eq!(duplicated.len(), 2);
    assert_eq!(duplicated[0]["path"], "src/hot_dup.ts");
    assert_eq!(duplicated[0]["commits"], 3);
    assert!(duplicated[0]["clone_lines"].as_u64().expect("clone_lines") > 0);
    assert_eq!(duplicated[0]["clone_groups"], 1);
    assert_eq!(duplicated[1]["path"], "src/cold_dup.ts");
    assert_eq!(duplicated[1]["commits"], 1);
}

#[test]
fn churn_respects_since_days_since_date_and_commits_window() {
    let repo = ChurnFixture::init();
    for (label, salt, when) in [
        ("old", 1, "2026-01-01T10:00:00Z"),
        ("mid", 2, "2026-05-01T10:00:00Z"),
        ("recent", 3, "2026-05-10T10:00:00Z"),
        ("head", 4, "2026-05-15T10:00:00Z"),
    ] {
        repo.save_and_commit(&[("src/eroded.ts", eroded_ts(label, salt))], label, when);
    }

    let by_days = repo.churn_json(&["--since", "30d"]);
    assert_eq!(by_days["commits_analyzed"], 3);
    assert_eq!(
        by_days["source_sets"]["production"]["erosion"]["findings"][0]["commits"],
        3
    );

    let by_date = repo.churn_json(&["--since", "2026-05-05"]);
    assert_eq!(by_date["commits_analyzed"], 2);
    assert_eq!(
        by_date["source_sets"]["production"]["erosion"]["findings"][0]["commits"],
        2
    );

    let by_commits = repo.churn_json(&["--commits", "2"]);
    assert_eq!(by_commits["commits_analyzed"], 2);
    assert_eq!(
        by_commits["source_sets"]["production"]["erosion"]["findings"][0]["commits"],
        2
    );

    assert_eq!(
        repo.churn(&["--since", "30d", "--commits", "5"])
            .status
            .code(),
        Some(2)
    );
    assert_eq!(repo.churn(&["--since", "0d"]).status.code(), Some(2));
}

#[test]
fn churn_renders_text_toon_json_and_markdown_formats() {
    let repo = ChurnFixture::init();
    repo.save_and_commit(
        &[("src/eroded.ts", eroded_ts("hot", 1))],
        "c1",
        "2026-05-01T10:00:00Z",
    );
    repo.save_and_commit(
        &[("src/eroded.ts", eroded_ts("hot", 2))],
        "c2",
        "2026-05-02T10:00:00Z",
    );

    assert_format_contains(&repo, "text", &["src/eroded.ts", "Erosion"]);
    assert_format_contains(&repo, "toon", &["commits_analyzed: 2", "src/eroded.ts"]);
    assert_format_contains(&repo, "markdown", &["# codeint churn", "`src/eroded.ts`"]);
}

#[test]
fn churn_handles_shallow_clones_and_non_git_directories_gracefully() {
    let origin = ChurnFixture::init();
    for i in 1..=4 {
        origin.save_and_commit(
            &[("src/eroded.ts", eroded_ts("hot", i))],
            &format!("c{i}"),
            &format!("2026-05-0{i}T10:00:00Z"),
        );
    }

    let shallow = ChurnFixture::init();
    let url = format!("file://{}", origin.root().display());
    run_git(
        shallow.root(),
        &["fetch", "-q", "--depth", "2", &url, "main"],
        "2026-05-05T10:00:00Z",
    );
    run_git(
        shallow.root(),
        &["reset", "-q", "--hard", "FETCH_HEAD"],
        "2026-05-05T10:00:00Z",
    );

    let report = shallow.churn_json(&[]);
    assert_eq!(report["shallow"], true);
    assert_eq!(report["commits_analyzed"], 2);
    let warning = report["warning"].as_str().expect("shallow clone warning");
    assert!(warning.contains("shallow"), "warning: {warning}");
    assert!(warning.contains("--unshallow"), "warning: {warning}");

    let non_git = tempfile::tempdir().expect("non-git tempdir");
    let out = exec_churn(non_git.path(), &[]);
    assert_eq!(out.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("not inside a Git repository"),
        "stderr: {stderr}"
    );
}
