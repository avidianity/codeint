mod history_support;

use history_support::{
    HistoryFixture, assert_format_contains, assert_no_blended_score_keys,
    assert_shallow_and_non_git, assert_unborn_repo_remedy,
};

fn eroded_ts(name: &str, variant: usize) -> String {
    let checks: String = (1..=12)
        .map(|i| format!("  if (v === {i}) return {};\n", i + variant))
        .collect();
    format!("export function {name}(v: number): number {{\n{checks}  return {variant};\n}}\n")
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

#[test]
fn churn_ranks_eroded_and_duplicated_files_independently_by_commit_count_with_no_score() {
    let repo = HistoryFixture::init();
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

    let report = repo.run_json("churn", &[]);
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
    let repo = HistoryFixture::init();
    for (label, variant, when) in [
        ("old", 1, "2026-01-01T10:00:00Z"),
        ("mid", 2, "2026-05-01T10:00:00Z"),
        ("recent", 3, "2026-05-10T10:00:00Z"),
        ("head", 4, "2026-05-15T10:00:00Z"),
    ] {
        repo.save_and_commit(&[("src/eroded.ts", eroded_ts(label, variant))], label, when);
    }

    let by_days = repo.run_json("churn", &["--since", "30d"]);
    assert_eq!(by_days["commits_analyzed"], 3);
    assert_eq!(
        by_days["source_sets"]["production"]["erosion"]["findings"][0]["commits"],
        3
    );

    let by_date = repo.run_json("churn", &["--since", "2026-05-05"]);
    assert_eq!(by_date["commits_analyzed"], 2);
    assert_eq!(
        by_date["source_sets"]["production"]["erosion"]["findings"][0]["commits"],
        2
    );

    let by_commits = repo.run_json("churn", &["--commits", "2"]);
    assert_eq!(by_commits["commits_analyzed"], 2);
    assert_eq!(
        by_commits["source_sets"]["production"]["erosion"]["findings"][0]["commits"],
        2
    );

    assert_eq!(
        repo.run("churn", &["--since", "30d", "--commits", "5"])
            .status
            .code(),
        Some(2)
    );
    assert_eq!(repo.run("churn", &["--since", "0d"]).status.code(), Some(2));
}

#[test]
fn churn_renders_text_toon_json_and_markdown_formats() {
    let repo = HistoryFixture::init();
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

    assert_format_contains(&repo, "churn", "text", &["src/eroded.ts", "Erosion"]);
    assert_format_contains(
        &repo,
        "churn",
        "toon",
        &["commits_analyzed: 2", "src/eroded.ts"],
    );
    assert_format_contains(
        &repo,
        "churn",
        "markdown",
        &["# codeint churn", "`src/eroded.ts`"],
    );
}

#[test]
fn churn_handles_shallow_clones_and_non_git_directories_gracefully() {
    let origin = HistoryFixture::init();
    for i in 1..=4 {
        origin.save_and_commit(
            &[("src/eroded.ts", eroded_ts("hot", i))],
            &format!("c{i}"),
            &format!("2026-05-0{i}T10:00:00Z"),
        );
    }
    assert_shallow_and_non_git(&origin, "churn", &[]);
}

#[test]
fn churn_shallow_boundary_does_not_fabricate_touches_for_untouched_files() {
    let origin = HistoryFixture::init();
    origin.save_and_commit(
        &[
            ("src/cold_eroded.ts", eroded_ts("coldEroded", 1)),
            ("src/hot_eroded.ts", eroded_ts("hotEroded", 1)),
        ],
        "c1",
        "2026-05-01T10:00:00Z",
    );
    for step in 2..=4 {
        origin.save_and_commit(
            &[("src/hot_eroded.ts", eroded_ts("hotEroded", step))],
            &format!("c{step}"),
            &format!("2026-05-0{step}T10:00:00Z"),
        );
    }

    let depth_one = origin.shallow_clone_of("1").run_json("churn", &[]);
    assert_eq!(depth_one["shallow"], true);
    assert_eq!(depth_one["commits_analyzed"], 0);
    let depth_one_findings = depth_one["source_sets"]["production"]["erosion"]["findings"]
        .as_array()
        .expect("depth 1 findings");
    for row in depth_one_findings {
        assert_eq!(row["commits"], 0, "unexpected fabricated touch: {row}");
    }

    let depth_two = origin.shallow_clone_of("2").run_json("churn", &[]);
    assert_eq!(depth_two["shallow"], true);
    assert_eq!(depth_two["commits_analyzed"], 1);
    let depth_two_findings = depth_two["source_sets"]["production"]["erosion"]["findings"]
        .as_array()
        .expect("depth 2 findings");
    let commits_of = |path: &str| {
        depth_two_findings
            .iter()
            .find(|row| row["path"] == path)
            .expect("path present")["commits"]
            .as_u64()
            .expect("commits count")
    };
    assert_eq!(commits_of("src/hot_eroded.ts"), 1);
    assert_eq!(commits_of("src/cold_eroded.ts"), 0);
}

#[test]
fn churn_on_unborn_repository_gives_command_agnostic_remedy() {
    assert_unborn_repo_remedy("churn");
}
