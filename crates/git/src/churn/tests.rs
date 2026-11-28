use std::collections::{BTreeMap, BTreeSet};
use std::fs;

use codeint_model::RepoPath;

use crate::tests::{TempRepo, git};
use crate::{ChurnRange, Repository};

fn path(raw: &str) -> RepoPath {
    RepoPath::new(raw).expect("valid repo path")
}

#[test]
fn tree_differ_handles_git_sort_order_for_file_and_directory_prefix_collisions() {
    let temp = TempRepo::seed(
        "prefix-collision",
        &[
            ("foo", "blob-foo\n"),
            ("foo-bar.rs", "fn foo_bar() {}\n"),
            ("foo.rs", "fn foo_rs() {}\n"),
            ("untouched/keep.rs", "fn keep() {}\n"),
        ],
        "initial with file foo",
    );
    let root = temp.path();

    fs::remove_file(root.join("foo")).expect("remove foo file");
    temp.write_all(&[
        ("foo/bar.rs", "fn bar() {}\n"),
        ("foo/nested/baz.rs", "fn baz() {}\n"),
    ]);
    temp.commit_all("replace file foo with directory foo/");

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
    temp.write("foo", "restored-blob-foo\n");
    temp.commit_all("replace directory foo/ back with file foo");

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
    let temp = TempRepo::seed(
        "nested-subtrees",
        &[
            ("a/one.rs", "v1\n"),
            ("a/two.rs", "v1\n"),
            ("b/stay.rs", "stay\n"),
            ("c/drop/x.rs", "x\n"),
        ],
        "seed",
    );
    let root = temp.path();

    temp.write("a/one.rs", "v2\n");
    fs::remove_file(root.join("a/two.rs")).expect("remove a/two.rs");
    temp.write("a/three.rs", "v1\n");
    fs::remove_dir_all(root.join("c")).expect("remove c subtree");
    temp.write("d/new/y.rs", "y\n");
    temp.commit_all("mixed subtree changes");

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
    let temp = TempRepo::seed(
        "mode-change",
        &[
            ("script.sh", "#!/bin/sh\necho hi\n"),
            ("other.sh", "#!/bin/sh\necho stay\n"),
        ],
        "initial scripts",
    );
    let root = temp.path();

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
