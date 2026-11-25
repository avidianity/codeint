use std::fs;
use std::path::Path;

use serde_json::Value;

use crate::common::{eroded_ts, init_git_repo, run_codeint_text, run_git, write_repo_files};

const RATCHET_FILE: &str = ".codeint/ratchet.json";
const BASELINE_FILE: &str = ".codeint/baseline.json";

fn tidy_ts(name: &str) -> String {
    format!("export function {name}(input: number): number {{\n  return input * 2;\n}}\n")
}

fn legacy_repository() -> tempfile::TempDir {
    init_git_repo(&[
        ("src/legacy.ts", &eroded_ts("legacy", 12)),
        ("src/older.ts", &eroded_ts("older", 11)),
        ("src/clean.ts", &tidy_ts("clean")),
    ])
}

fn stored_json(root: &Path, relative: &str) -> Value {
    let text = fs::read_to_string(root.join(relative)).expect("read the stored file");
    serde_json::from_str(&text).expect("the stored file is JSON")
}

#[test]
fn adopt_writes_the_config_ratchet_and_baseline_then_check_passes_and_a_new_finding_fails() {
    let repo = legacy_repository();

    let (stdout, _) = run_codeint_text(repo.path(), &["init", "--adopt"], 0);

    let config = fs::read_to_string(repo.path().join("codeint.jsonc")).expect("read config");
    assert!(
        config.contains("\"regression_limits\": {\n    \"per_file\": true\n  }"),
        "expected per_file enabled, got:\n{config}"
    );
    assert!(
        config.contains("// \"new_file_budgets\": {")
            && config.contains("//     \"erosion\": 0")
            && config.contains("//     \"hazards\": 0"),
        "expected commented new_file_budgets defaults, got:\n{config}"
    );
    let ratchet = stored_json(repo.path(), RATCHET_FILE);
    assert!(
        ratchet["source_sets"]["production"]["erosion"]["ceiling"]
            .as_f64()
            .is_some_and(|ceiling| ceiling > 0.0),
        "expected a positive erosion ceiling, got: {ratchet}"
    );
    let baseline = stored_json(repo.path(), BASELINE_FILE);
    let baselined = baseline["source_sets"]["production"]["erosion"]["findings"]
        .as_array()
        .expect("baselined erosion findings");
    assert_eq!(baselined.len(), 2);
    for expected in [
        "Wrote ",
        "codeint.jsonc",
        RATCHET_FILE,
        BASELINE_FILE,
        "pull_request",
        "avidianity/codeint@main",
        "fetch-depth: 0",
        "codeint ratchet update",
        "codeint baseline update",
    ] {
        assert!(
            stdout.contains(expected),
            "expected {expected:?} in:\n{stdout}"
        );
    }

    run_codeint_text(repo.path(), &["config", "--resolved"], 0);
    run_codeint_text(repo.path(), &["check"], 0);

    write_repo_files(repo.path(), &[("src/fresh.ts", &eroded_ts("fresh", 11))]);
    run_codeint_text(repo.path(), &["check"], 1);
}

#[test]
fn adopt_enables_per_file_so_diff_fails_a_worsened_legacy_file_even_when_another_file_shrinks() {
    let repo = legacy_repository();
    run_codeint_text(repo.path(), &["init", "--adopt"], 0);
    run_git(repo.path(), &["add", "."]);
    run_git(repo.path(), &["commit", "-q", "-m", "adopt codeint"]);
    run_git(repo.path(), &["checkout", "-q", "-b", "feature"]);
    write_repo_files(
        repo.path(),
        &[
            ("src/legacy.ts", &eroded_ts("legacy", 13)),
            ("src/older.ts", &tidy_ts("older")),
        ],
    );
    run_git(repo.path(), &["add", "."]);
    run_git(repo.path(), &["commit", "-q", "-m", "shift legacy debt"]);

    let (_, stderr) = run_codeint_text(repo.path(), &["diff", "main"], 1);

    assert!(stderr.contains("src/legacy.ts"), "got: {stderr}");
}

#[test]
fn adopt_never_replaces_an_existing_config_and_writes_nothing_else() {
    let repo = legacy_repository();
    write_repo_files(repo.path(), &[("codeint.jsonc", "// keep me\n{}\n")]);

    let (_, stderr) = run_codeint_text(repo.path(), &["init", "--adopt"], 2);

    assert!(
        stderr.contains("already exists, and codeint init never replaces it"),
        "got: {stderr}"
    );
    assert_eq!(
        fs::read_to_string(repo.path().join("codeint.jsonc")).expect("read config"),
        "// keep me\n{}\n"
    );
    assert!(!repo.path().join(".codeint").exists());
}

#[test]
fn adopt_never_replaces_an_existing_ratchet_or_baseline_and_writes_nothing() {
    for (existing, update_command) in [
        (RATCHET_FILE, "codeint ratchet update"),
        (BASELINE_FILE, "codeint baseline update"),
    ] {
        let repo = legacy_repository();
        write_repo_files(repo.path(), &[(existing, "{ \"keep\": \"me\" }\n")]);

        let (_, stderr) = run_codeint_text(repo.path(), &["init", "--adopt"], 2);

        assert!(
            stderr.contains(existing)
                && stderr.contains("already exists, and codeint init --adopt never replaces it")
                && stderr.contains(update_command),
            "got: {stderr}"
        );
        assert_eq!(
            fs::read_to_string(repo.path().join(existing)).expect("read the existing file"),
            "{ \"keep\": \"me\" }\n"
        );
        assert!(!repo.path().join("codeint.jsonc").exists());
        let other = if existing == RATCHET_FILE {
            BASELINE_FILE
        } else {
            RATCHET_FILE
        };
        assert!(!repo.path().join(other).exists());
    }
}

#[test]
fn init_without_adopt_writes_no_state_files_and_keeps_regression_limits_empty() {
    let repo = legacy_repository();

    run_codeint_text(repo.path(), &["init"], 0);

    assert!(!repo.path().join(".codeint").exists());
    let config = fs::read_to_string(repo.path().join("codeint.jsonc")).expect("read config");
    assert!(config.contains("\"regression_limits\": {}"));
    assert!(!config.contains("new_file_budgets"));
}
