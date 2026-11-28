use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use super::corpus::{CorpusSpec, write_corpus};
use super::{MeasuredRun, PerfVerdict, check_budget, over_budget_message, parse_report_totals};
use crate::config::temp_dir;

static NEXT_DIR_ID: AtomicU64 = AtomicU64::new(1);

struct ScratchDir {
    path: PathBuf,
}

impl ScratchDir {
    fn create(label: &str) -> ScratchDir {
        let id = NEXT_DIR_ID.fetch_add(1, Ordering::Relaxed);
        let path = temp_dir().join(format!(
            "codeint-perf-test-{label}-{}-{id}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&path);
        if let Err(error) = fs::create_dir_all(&path) {
            panic!("cannot create {}: {error}", path.display());
        }
        ScratchDir { path }
    }
}

impl Drop for ScratchDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn collect_tree(root: &Path) -> BTreeMap<String, Vec<u8>> {
    let mut files = BTreeMap::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let entries = match fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(error) => panic!("cannot read {}: {error}", dir.display()),
        };
        for entry in entries {
            record_tree_entry(root, entry, &mut stack, &mut files);
        }
    }
    files
}

fn record_tree_entry(
    root: &Path,
    entry: std::io::Result<fs::DirEntry>,
    stack: &mut Vec<PathBuf>,
    files: &mut BTreeMap<String, Vec<u8>>,
) {
    let entry = match entry {
        Ok(entry) => entry,
        Err(error) => panic!("{error}"),
    };
    let path = entry.path();
    if path.is_dir() {
        stack.push(path);
        return;
    }
    let relative = match path.strip_prefix(root) {
        Ok(relative) => relative.to_string_lossy().replace('\\', "/"),
        Err(error) => panic!("{error}"),
    };
    let bytes = match fs::read(&path) {
        Ok(bytes) => bytes,
        Err(error) => panic!("cannot read {}: {error}", path.display()),
    };
    files.insert(relative, bytes);
}

#[test]
fn corpus_generator_is_deterministic_for_a_given_seed() {
    let first_dir = ScratchDir::create("same-a");
    let second_dir = ScratchDir::create("same-b");
    let other_dir = ScratchDir::create("other");

    let spec = CorpusSpec {
        seed: 7,
        packages: 2,
        files_per_package: 3,
        functions_per_file: 4,
    };
    let first_summary = match write_corpus(&first_dir.path, spec) {
        Ok(summary) => summary,
        Err(error) => panic!("{error}"),
    };
    let second_summary = match write_corpus(&second_dir.path, spec) {
        Ok(summary) => summary,
        Err(error) => panic!("{error}"),
    };
    let other_summary = match write_corpus(&other_dir.path, CorpusSpec { seed: 8, ..spec }) {
        Ok(summary) => summary,
        Err(error) => panic!("{error}"),
    };

    assert_eq!(first_summary, second_summary);
    assert!(first_summary.files >= spec.packages * spec.files_per_package);
    assert!(first_summary.lines > 0);
    assert!(other_summary.lines > 0);

    let first_tree = collect_tree(&first_dir.path);
    let second_tree = collect_tree(&second_dir.path);
    let other_tree = collect_tree(&other_dir.path);
    assert_eq!(first_tree, second_tree);
    assert_ne!(first_tree, other_tree);
    assert!(first_tree.contains_key("package.json"));
}

#[test]
fn budget_passes_when_elapsed_is_within_or_equal_to_budget() {
    assert_eq!(check_budget(9.5, 10.0), PerfVerdict::Within);
    assert_eq!(check_budget(10.0, 10.0), PerfVerdict::Within);
}

#[test]
fn budget_fails_with_elapsed_and_budget_in_message_when_exceeded() {
    assert_eq!(check_budget(12.34, 10.0), PerfVerdict::OverBudget);
    let message = over_budget_message(
        MeasuredRun {
            files: 450,
            source_lines: 150_000,
            elapsed_seconds: 12.34,
        },
        10.0,
    );
    assert!(message.contains("12.34"), "{message}");
    assert!(message.contains("10.00"), "{message}");
    assert!(message.contains("450"), "{message}");
    assert!(message.contains("150000"), "{message}");
}

#[test]
fn parse_report_totals_extracts_files_and_source_lines() {
    let payload = br#"{"files":460,"source_lines":142500,"source_sets":{}}"#;
    let totals = match parse_report_totals(payload) {
        Ok(totals) => totals,
        Err(error) => panic!("{error}"),
    };
    assert_eq!(totals, (460, 142_500));
    assert!(parse_report_totals(br#"{"files":460}"#).is_err());
}
