use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use super::{affected_packages, repeated_flag};

fn packages(root: &Path) -> Vec<(String, PathBuf, BTreeSet<String>)> {
    [
        ("model", "crates/model", vec![]),
        ("metrics", "crates/metrics", vec!["model"]),
        ("cli", "crates/cli", vec!["metrics"]),
        ("xtask", "xtask", vec![]),
    ]
    .into_iter()
    .map(|(name, directory, dependencies)| {
        (
            name.to_string(),
            root.join(directory),
            dependencies.into_iter().map(str::to_string).collect(),
        )
    })
    .collect()
}

fn names(items: &[&str]) -> BTreeSet<String> {
    items.iter().map(|item| (*item).to_string()).collect()
}

#[test]
fn a_change_reaches_its_crate_and_every_dependent() {
    let root = Path::new("/repo");
    let changed = vec!["crates/model/src/lib.rs".to_string()];
    assert_eq!(
        affected_packages(&changed, root, &packages(root)),
        names(&["cli", "metrics", "model"])
    );
}

#[test]
fn a_leaf_change_stays_local() {
    let root = Path::new("/repo");
    let changed = vec!["xtask/src/main.rs".to_string()];
    assert_eq!(
        affected_packages(&changed, root, &packages(root)),
        names(&["xtask"])
    );
}

#[test]
fn documents_affect_nothing_and_the_lockfile_affects_everything() {
    let root = Path::new("/repo");
    assert!(affected_packages(&["README.md".to_string()], root, &packages(root)).is_empty());
    assert_eq!(
        affected_packages(&["Cargo.lock".to_string()], root, &packages(root)),
        names(&["cli", "metrics", "model", "xtask"])
    );
}

#[test]
fn repeated_flag_collects_zero_or_more_values_and_rejects_dangling_flag() {
    let to_args =
        |items: &[&str]| -> Vec<String> { items.iter().map(|s| (*s).to_string()).collect() };
    assert_eq!(
        repeated_flag(&to_args(&["--loop", "ci-gate"]), "--exclude-workflow"),
        Ok(Vec::new())
    );
    assert_eq!(
        repeated_flag(
            &to_args(&[
                "--exclude-workflow",
                "CodeQL",
                "--report",
                "--exclude-workflow",
                "codeint"
            ]),
            "--exclude-workflow"
        ),
        Ok(vec!["CodeQL".to_string(), "codeint".to_string()])
    );
    assert!(repeated_flag(&to_args(&["--exclude-workflow"]), "--exclude-workflow").is_err());
}
