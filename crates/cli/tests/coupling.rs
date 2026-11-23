mod history_support;

use history_support::{
    HistoryFixture, assert_format_contains, assert_no_blended_score_keys,
    assert_shallow_and_non_git,
};

fn commit_const_pair(
    repo: &HistoryFixture,
    left: (&str, &str),
    right: (&str, &str),
    count: usize,
    day_prefix: &str,
) {
    for step in 1..=count {
        repo.save_and_commit(
            &[
                (left.0, format!("export const {} = {step};\n", left.1)),
                (right.0, format!("export const {} = {step};\n", right.1)),
            ],
            &format!("pair {day_prefix} {step}"),
            &format!("2026-05-{day_prefix}{step}T10:00:00Z"),
        );
    }
}

#[test]
fn coupling_detects_co_changed_pairs_flags_direct_imports_and_omits_blended_scores() {
    let repo = HistoryFixture::init();
    for step in 1..=4 {
        let mut files = vec![
            (
                "src/billing/invoice.ts",
                format!("export const invoiceVersion = {step};\n"),
            ),
            (
                "src/shipping/label.ts",
                format!("export const labelVersion = {step};\n"),
            ),
        ];
        if step == 1 {
            files.push((
                "src/unrelated.ts",
                "export const unrelated = 1;\n".to_owned(),
            ));
        }
        repo.save_and_commit(
            &files,
            &format!("hidden pair {step}"),
            &format!("2026-05-0{step}T10:00:00Z"),
        );
    }
    for step in 1..=3 {
        repo.save_and_commit(
            &[
                (
                    "src/orders/model.ts",
                    format!(
                        "export function formatOrder(id: number): number {{ return id + {step}; }}\n"
                    ),
                ),
                (
                    "src/orders/handler.ts",
                    format!(
                        "import {{ formatOrder }} from \"./model\";\nexport const handled = formatOrder({step});\n"
                    ),
                ),
            ],
            &format!("direct import pair {step}"),
            &format!("2026-05-1{step}T10:00:00Z"),
        );
    }
    repo.save_and_commit(
        &[(
            "src/orders/model.ts",
            "export function formatOrder(id: number): number { return id + 99; }\n".to_owned(),
        )],
        "touch model alone",
        "2026-05-15T10:00:00Z",
    );

    let report = repo.run_json("coupling", &[]);
    assert_no_blended_score_keys(&report);
    assert_eq!(report["commits_analyzed"], 8);
    assert_eq!(report["commits_excluded_by_bulk_limit"], 0);
    assert_eq!(report["max_commit_files"], 50);
    assert_eq!(report["min_co_changes"], 3);
    assert_eq!(report["shallow"], false);
    assert!(report.get("warning").is_none());

    let pairs = report["pairs"].as_array().expect("pairs array");
    assert_eq!(pairs.len(), 2);

    assert_eq!(pairs[0]["file_a"], "src/billing/invoice.ts");
    assert_eq!(pairs[0]["file_b"], "src/shipping/label.ts");
    assert_eq!(pairs[0]["co_change_count"], 4);
    assert_eq!(pairs[0]["file_a_commits"], 4);
    assert_eq!(pairs[0]["file_b_commits"], 4);
    assert_eq!(pairs[0]["coupling_ratio"], 1.0);
    assert_eq!(pairs[0]["has_direct_import"], false);

    assert_eq!(pairs[1]["file_a"], "src/orders/handler.ts");
    assert_eq!(pairs[1]["file_b"], "src/orders/model.ts");
    assert_eq!(pairs[1]["co_change_count"], 3);
    assert_eq!(pairs[1]["file_a_commits"], 3);
    assert_eq!(pairs[1]["file_b_commits"], 4);
    assert_eq!(pairs[1]["coupling_ratio"], 0.75);
    assert_eq!(pairs[1]["has_direct_import"], true);

    let strict_ratio = repo.run_json("coupling", &["--min-ratio", "0.9"]);
    let strict_pairs = strict_ratio["pairs"].as_array().expect("strict pairs");
    assert_eq!(strict_pairs.len(), 1);
    assert_eq!(strict_pairs[0]["file_a"], "src/billing/invoice.ts");

    let high_min_count = repo.run_json("coupling", &["--min-co-changes", "4"]);
    let high_pairs = high_min_count["pairs"].as_array().expect("high pairs");
    assert_eq!(high_pairs.len(), 1);
    assert_eq!(high_pairs[0]["file_a"], "src/billing/invoice.ts");
}

#[test]
fn coupling_excludes_bulk_commits_above_max_commit_files_ceiling() {
    let repo = HistoryFixture::init();
    for step in 1..=4 {
        let files: Vec<(&str, String)> = vec![
            ("src/bulk_a.ts", format!("export const a = {step};\n")),
            ("src/bulk_b.ts", format!("export const b = {step};\n")),
            ("src/bulk_c.ts", format!("export const c = {step};\n")),
            ("src/bulk_d.ts", format!("export const d = {step};\n")),
            ("src/bulk_e.ts", format!("export const e = {step};\n")),
            ("src/bulk_f.ts", format!("export const f = {step};\n")),
        ];
        repo.save_and_commit(
            &files,
            &format!("bulk format {step}"),
            &format!("2026-05-0{step}T10:00:00Z"),
        );
    }
    commit_const_pair(
        &repo,
        ("src/real_x.ts", "x"),
        ("src/real_y.ts", "y"),
        3,
        "1",
    );

    let filtered = repo.run_json("coupling", &["--max-commit-files", "4"]);
    assert_eq!(filtered["commits_analyzed"], 7);
    assert_eq!(filtered["commits_excluded_by_bulk_limit"], 4);
    assert_eq!(filtered["max_commit_files"], 4);
    let pairs = filtered["pairs"].as_array().expect("filtered pairs");
    assert_eq!(pairs.len(), 1);
    assert_eq!(pairs[0]["file_a"], "src/real_x.ts");
    assert_eq!(pairs[0]["file_b"], "src/real_y.ts");
    assert_eq!(pairs[0]["co_change_count"], 3);

    let wide = repo.run_json("coupling", &["--max-commit-files", "10"]);
    assert_eq!(wide["commits_excluded_by_bulk_limit"], 0);
    let wide_pairs = wide["pairs"].as_array().expect("wide pairs");
    assert!(
        wide_pairs
            .iter()
            .any(|p| p["file_a"] == "src/bulk_a.ts" && p["file_b"] == "src/bulk_b.ts"),
        "expected bulk pair when ceiling allows it: {wide_pairs:?}"
    );
}

#[test]
fn coupling_focuses_on_production_by_default_and_includes_tests_with_flag() {
    let repo = HistoryFixture::init();
    for step in 1..=4 {
        repo.save_and_commit(
            &[
                ("src/calc.ts", format!("export const calc = {step};\n")),
                (
                    "src/calc.test.ts",
                    format!(
                        "import {{ calc }} from \"./calc\";\nexport const checked = calc + {step};\n"
                    ),
                ),
            ],
            &format!("calc and test {step}"),
            &format!("2026-05-0{step}T10:00:00Z"),
        );
    }
    commit_const_pair(
        &repo,
        ("src/calc.ts", "calc"),
        ("src/audit.ts", "audit"),
        3,
        "1",
    );

    let prod_only = repo.run_json("coupling", &[]);
    assert_eq!(prod_only["source_sets"], serde_json::json!(["production"]));
    let prod_pairs = prod_only["pairs"].as_array().expect("prod pairs");
    assert_eq!(prod_pairs.len(), 1);
    assert_eq!(prod_pairs[0]["file_a"], "src/audit.ts");
    assert_eq!(prod_pairs[0]["file_b"], "src/calc.ts");
    assert_eq!(prod_pairs[0]["has_direct_import"], false);

    let with_tests = repo.run_json("coupling", &["--include-tests"]);
    assert_eq!(
        with_tests["source_sets"],
        serde_json::json!(["production", "test"])
    );
    let test_pairs = with_tests["pairs"].as_array().expect("with_tests pairs");
    assert!(
        test_pairs.iter().any(|p| {
            p["file_a"] == "src/calc.test.ts"
                && p["file_b"] == "src/calc.ts"
                && p["has_direct_import"] == true
        }),
        "expected calc.test.ts + calc.ts pair with --include-tests: {test_pairs:?}"
    );
    assert!(
        test_pairs
            .iter()
            .any(|p| p["file_a"] == "src/audit.ts" && p["file_b"] == "src/calc.ts"),
        "expected production pair to remain with --include-tests: {test_pairs:?}"
    );
}

#[test]
fn coupling_renders_all_formats_and_handles_shallow_non_git_and_invalid_args() {
    let repo = HistoryFixture::init();
    commit_const_pair(
        &repo,
        ("src/alpha.ts", "alpha"),
        ("src/beta.ts", "beta"),
        3,
        "0",
    );

    assert_format_contains(
        &repo,
        "coupling",
        "text",
        &["src/alpha.ts", "src/beta.ts", "hidden"],
    );
    assert_format_contains(
        &repo,
        "coupling",
        "toon",
        &["commits_analyzed: 3", "src/alpha.ts", "src/beta.ts"],
    );
    assert_format_contains(
        &repo,
        "coupling",
        "markdown",
        &["# codeint coupling", "`src/alpha.ts`", "`src/beta.ts`"],
    );

    assert_eq!(
        repo.run("coupling", &["--since", "30d", "--commits", "5"])
            .status
            .code(),
        Some(2)
    );
    assert_eq!(
        repo.run("coupling", &["--min-ratio", "1.5"]).status.code(),
        Some(2)
    );
    assert_eq!(
        repo.run("coupling", &["--max-commit-files", "0"])
            .status
            .code(),
        Some(2)
    );

    assert_shallow_and_non_git(&repo, "coupling", &["--min-co-changes", "2"]);
}
