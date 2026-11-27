use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use codeint_model::RepoPath;

use crate::{ChurnRange, Repository};

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

struct TempRepo {
    path: PathBuf,
}

impl TempRepo {
    fn new(label: &str) -> TempRepo {
        let id = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "codeint-git-churn-{label}-{}-{id}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("create temp repo dir");
        git(&path, &["init", "-q"]);
        TempRepo { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempRepo {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn path(raw: &str) -> RepoPath {
    RepoPath::new(raw).expect("valid repo path")
}

fn git(dir: &Path, args: &[&str]) {
    let status = Command::new("git")
        .current_dir(dir)
        .args(args)
        .env("GIT_AUTHOR_NAME", "Tester")
        .env("GIT_AUTHOR_EMAIL", "tester@example.com")
        .env("GIT_COMMITTER_NAME", "Tester")
        .env("GIT_COMMITTER_EMAIL", "tester@example.com")
        .env("GIT_AUTHOR_DATE", "1700000000 +0000")
        .env("GIT_COMMITTER_DATE", "1700000000 +0000")
        .status()
        .expect("run git");
    assert!(status.success(), "git {args:?} failed with {status}");
}

fn write_file(dir: &Path, rel: &str, content: &str) {
    let full = dir.join(rel);
    if let Some(parent) = full.parent() {
        fs::create_dir_all(parent).expect("create parent dirs");
    }
    fs::write(full, content).expect("write file");
}

fn commit_all(dir: &Path, message: &str) {
    git(dir, &["add", "-A"]);
    git(dir, &["commit", "-q", "-m", message]);
}

#[test]
fn tree_differ_handles_git_sort_order_for_file_and_directory_prefix_collisions() {
    let temp = TempRepo::new("prefix-collision");
    let root = temp.path();

    write_file(root, "foo", "blob-foo\n");
    write_file(root, "foo-bar.rs", "fn foo_bar() {}\n");
    write_file(root, "foo.rs", "fn foo_rs() {}\n");
    write_file(root, "untouched/keep.rs", "fn keep() {}\n");
    commit_all(root, "initial with file foo");

    fs::remove_file(root.join("foo")).expect("remove foo file");
    write_file(root, "foo/bar.rs", "fn bar() {}\n");
    write_file(root, "foo/nested/baz.rs", "fn baz() {}\n");
    commit_all(root, "replace file foo with directory foo/");

    let repo = Repository::discover(root).expect("discover repo");
    let single = repo
        .file_churn(ChurnRange::Commits(1))
        .expect("churn for file->dir commit");
    assert_eq!(single.commits_analyzed, 1);
    assert_eq!(
        single.touches_by_file,
        BTreeMap::from([
            (path("foo"), 1),
            (path("foo/bar.rs"), 1),
            (path("foo/nested/baz.rs"), 1),
        ])
    );

    fs::remove_dir_all(root.join("foo")).expect("remove foo dir");
    write_file(root, "foo", "restored-blob-foo\n");
    commit_all(root, "replace directory foo/ back with file foo");

    let repo = Repository::discover(root).expect("discover repo");
    let reverse = repo
        .file_churn(ChurnRange::Commits(1))
        .expect("churn for dir->file commit");
    assert_eq!(reverse.commits_analyzed, 1);
    assert_eq!(
        reverse.touches_by_file,
        BTreeMap::from([
            (path("foo"), 1),
            (path("foo/bar.rs"), 1),
            (path("foo/nested/baz.rs"), 1),
        ])
    );

    let all = repo
        .file_churn(ChurnRange::Commits(10))
        .expect("churn across all commits");
    assert_eq!(all.commits_analyzed, 3);
    assert_eq!(
        all.touches_by_file,
        BTreeMap::from([
            (path("foo"), 3),
            (path("foo-bar.rs"), 1),
            (path("foo.rs"), 1),
            (path("foo/bar.rs"), 2),
            (path("foo/nested/baz.rs"), 2),
            (path("untouched/keep.rs"), 1),
        ])
    );
}

#[test]
fn tree_differ_tracks_added_deleted_and_modified_nested_subtrees_and_coupling() {
    let temp = TempRepo::new("nested-subtrees");
    let root = temp.path();

    write_file(root, "a/one.rs", "v1\n");
    write_file(root, "a/two.rs", "v1\n");
    write_file(root, "b/stay.rs", "stay\n");
    write_file(root, "c/drop/x.rs", "x\n");
    commit_all(root, "seed");

    write_file(root, "a/one.rs", "v2\n");
    fs::remove_file(root.join("a/two.rs")).expect("remove a/two.rs");
    write_file(root, "a/three.rs", "v1\n");
    fs::remove_dir_all(root.join("c")).expect("remove c subtree");
    write_file(root, "d/new/y.rs", "y\n");
    commit_all(root, "mixed subtree changes");

    let repo = Repository::discover(root).expect("discover repo");
    let churn = repo
        .file_churn(ChurnRange::Commits(1))
        .expect("churn for mixed commit");
    assert_eq!(
        churn.touches_by_file,
        BTreeMap::from([
            (path("a/one.rs"), 1),
            (path("a/three.rs"), 1),
            (path("a/two.rs"), 1),
            (path("c/drop/x.rs"), 1),
            (path("d/new/y.rs"), 1),
        ])
    );

    let allowed = BTreeSet::from([path("a/one.rs"), path("a/three.rs"), path("b/stay.rs")]);
    let coupling = repo
        .co_change_coupling(ChurnRange::Commits(1), 50, &allowed)
        .expect("co-change coupling");
    assert_eq!(
        coupling.touches_by_file,
        BTreeMap::from([(path("a/one.rs"), 1), (path("a/three.rs"), 1)])
    );
    assert_eq!(
        coupling.co_changes,
        BTreeMap::from([((path("a/one.rs"), path("a/three.rs")), 1)])
    );
}

#[test]
fn tree_differ_detects_mode_only_change_without_content_change() {
    let temp = TempRepo::new("mode-change");
    let root = temp.path();

    write_file(root, "script.sh", "#!/bin/sh\necho hi\n");
    write_file(root, "other.sh", "#!/bin/sh\necho stay\n");
    commit_all(root, "initial scripts");

    git(root, &["update-index", "--chmod=+x", "script.sh"]);
    git(root, &["commit", "-q", "-m", "make script executable"]);

    let repo = Repository::discover(root).expect("discover repo");
    let churn = repo
        .file_churn(ChurnRange::Commits(1))
        .expect("churn for mode change");
    assert_eq!(
        churn.touches_by_file,
        BTreeMap::from([(path("script.sh"), 1)])
    );
}
