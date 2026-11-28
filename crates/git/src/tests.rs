use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime};

use codeint_model::RepoPath;

use crate::history::SECONDS_PER_DAY;
use crate::{ChurnRange, CommitId, CommitRange, Repository};

static NEXT_REPO_ID: AtomicU64 = AtomicU64::new(0);

const MAY_10_UTC: i64 = 1_778_371_200;

pub(crate) struct TempRepo {
    path: PathBuf,
}

impl TempRepo {
    fn init() -> TempRepo {
        let unique = NEXT_REPO_ID.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "codeint-git-skew-test-{}-{unique}",
            std::process::id()
        ));
        if path.exists() {
            fs::remove_dir_all(&path).expect("clean old test dir");
        }
        fs::create_dir_all(&path).expect("create test dir");
        let repo = TempRepo { path };
        repo.git(&["init", "-q", "-b", "main"], "2026-05-01T00:00:00Z");
        repo
    }

    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    fn commit(&self, files: &[(&str, &str)], message: &str, committer_date: &str) -> CommitId {
        for (relative, content) in files {
            let dest = self.path.join(relative);
            fs::create_dir_all(dest.parent().expect("file parent")).expect("mkdir");
            fs::write(dest, content).expect("write file");
        }
        self.git(&["add", "-A"], committer_date);
        self.git(&["commit", "-q", "-m", message], committer_date);
        Repository::discover(self.path())
            .expect("open repo")
            .resolve("HEAD")
            .expect("resolve HEAD")
    }

    fn git(&self, args: &[&str], committer_date: &str) {
        let output = Command::new("git")
            .args(args)
            .current_dir(&self.path)
            .envs([
                ("GIT_CONFIG_GLOBAL", "/dev/null"),
                ("GIT_CONFIG_NOSYSTEM", "1"),
                ("GIT_AUTHOR_NAME", "Test"),
                ("GIT_AUTHOR_EMAIL", "test@example.invalid"),
                ("GIT_COMMITTER_NAME", "Test"),
                ("GIT_COMMITTER_EMAIL", "test@example.invalid"),
                ("GIT_AUTHOR_DATE", committer_date),
                ("GIT_COMMITTER_DATE", committer_date),
            ])
            .output()
            .expect("run git");
        assert!(
            output.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

impl Drop for TempRepo {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn repo_path(path: &str) -> RepoPath {
    RepoPath::new(path).expect("valid repo path")
}

fn first_parent_ids_since(temp: &TempRepo, unix_seconds: i64) -> Vec<CommitId> {
    let repo = Repository::discover(temp.path()).expect("discover repo");
    repo.first_parent_history(CommitRange::CommittedSince { unix_seconds })
        .expect("walk history")
        .into_iter()
        .map(|entry| entry.commit)
        .collect()
}

#[test]
fn first_parent_history_tolerates_14_day_skew_and_stops_beyond_14_days() {
    let temp = TempRepo::init();
    let _beyond_skew_newer = temp.commit(
        &[("src/beyond.ts", "export const beyond = 1;\n")],
        "newer commit hidden behind >14d commit",
        "2026-05-12T12:00:00Z",
    );
    let _beyond_skew_older = temp.commit(
        &[("src/old_stop.ts", "export const oldStop = 1;\n")],
        "commit older than cutoff by >14 days",
        "2026-04-25T23:59:59Z",
    );
    let behind_skew = temp.commit(
        &[("src/behind_skew.ts", "export const behind = 1;\n")],
        "newer commit behind 1-day skew",
        "2026-05-11T12:00:00Z",
    );
    let _skewed_older = temp.commit(
        &[("src/skewed.ts", "export const skewed = 1;\n")],
        "1-day older skewed commit",
        "2026-05-09T12:00:00Z",
    );
    let head = temp.commit(
        &[("src/head.ts", "export const head = 1;\n")],
        "head commit",
        "2026-05-15T12:00:00Z",
    );

    assert_eq!(
        first_parent_ids_since(&temp, MAY_10_UTC),
        vec![behind_skew, head]
    );
}

#[test]
fn first_parent_history_exact_14_day_boundary_scans_at_14_days_and_stops_past_it() {
    let temp = TempRepo::init();
    let _unreachable = temp.commit(
        &[("src/unreachable.ts", "export const u = 1;\n")],
        "newer commit behind 14d+1s commit",
        &format!("@{} +0000", MAY_10_UTC + SECONDS_PER_DAY),
    );
    let _just_past_window = temp.commit(
        &[("src/stop.ts", "export const s = 1;\n")],
        "commit at cutoff - 14d - 1s",
        &format!("@{} +0000", MAY_10_UTC - 14 * SECONDS_PER_DAY - 1),
    );
    let reachable = temp.commit(
        &[("src/reachable.ts", "export const r = 1;\n")],
        "newer commit behind exact 14d commit",
        &format!("@{} +0000", MAY_10_UTC + 2 * SECONDS_PER_DAY),
    );
    let _exact_boundary = temp.commit(
        &[("src/boundary.ts", "export const b = 1;\n")],
        "commit at exact cutoff - 14d",
        &format!("@{} +0000", MAY_10_UTC - 14 * SECONDS_PER_DAY),
    );
    let head = temp.commit(
        &[("src/head.ts", "export const h = 1;\n")],
        "head commit",
        &format!("@{} +0000", MAY_10_UTC + 3 * SECONDS_PER_DAY),
    );

    assert_eq!(
        first_parent_ids_since(&temp, MAY_10_UTC),
        vec![reachable, head]
    );
}

#[test]
fn churn_and_coupling_tolerate_14_day_skew_in_since_and_days_walks() {
    let temp = TempRepo::init();
    temp.commit(
        &[
            ("src/beyond.ts", "export const beyond = 1;\n"),
            ("src/shared.ts", "export const shared = 1;\n"),
        ],
        "newer commit behind >14d commit",
        "2026-05-12T12:00:00Z",
    );
    temp.commit(
        &[("src/old_stop.ts", "export const oldStop = 1;\n")],
        "commit older than cutoff by >14 days",
        "2026-04-25T23:59:59Z",
    );
    temp.commit(
        &[
            ("src/behind_skew.ts", "export const behind = 1;\n"),
            ("src/shared.ts", "export const shared = 2;\n"),
        ],
        "newer commit behind 1-day skew",
        "2026-05-11T12:00:00Z",
    );
    temp.commit(
        &[
            ("src/excluded_skew.ts", "export const excluded = 1;\n"),
            ("src/shared.ts", "export const shared = 3;\n"),
        ],
        "1-day older skewed commit",
        "2026-05-09T12:00:00Z",
    );
    temp.commit(
        &[
            ("src/head.ts", "export const head = 1;\n"),
            ("src/shared.ts", "export const shared = 4;\n"),
        ],
        "head commit",
        "2026-05-15T12:00:00Z",
    );

    let repo = Repository::discover(temp.path()).expect("discover repo");
    let allowed: BTreeSet<RepoPath> = [
        "src/beyond.ts",
        "src/old_stop.ts",
        "src/behind_skew.ts",
        "src/excluded_skew.ts",
        "src/head.ts",
        "src/shared.ts",
    ]
    .into_iter()
    .map(repo_path)
    .collect();

    for range in [
        ChurnRange::Since {
            unix_seconds: MAY_10_UTC,
        },
        ChurnRange::Days(5),
    ] {
        let churn = repo.file_churn(range).expect("file churn");
        assert_eq!(churn.commits_analyzed, 2, "range={range:?}");
        assert_eq!(
            churn.touches_by_file.get(&repo_path("src/behind_skew.ts")),
            Some(&1)
        );
        assert_eq!(
            churn.touches_by_file.get(&repo_path("src/head.ts")),
            Some(&1)
        );
        assert_eq!(
            churn.touches_by_file.get(&repo_path("src/shared.ts")),
            Some(&2)
        );
        assert!(
            !churn
                .touches_by_file
                .contains_key(&repo_path("src/excluded_skew.ts"))
        );
        assert!(
            !churn
                .touches_by_file
                .contains_key(&repo_path("src/old_stop.ts"))
        );
        assert!(
            !churn
                .touches_by_file
                .contains_key(&repo_path("src/beyond.ts"))
        );

        let coupling = repo
            .co_change_coupling(range, 50, &allowed)
            .expect("co-change coupling");
        assert_eq!(coupling.commits_analyzed, 2, "range={range:?}");
        assert_eq!(
            coupling
                .co_changes
                .get(&(repo_path("src/behind_skew.ts"), repo_path("src/shared.ts"))),
            Some(&1)
        );
        assert_eq!(
            coupling
                .co_changes
                .get(&(repo_path("src/head.ts"), repo_path("src/shared.ts"))),
            Some(&1)
        );
        assert!(
            !coupling
                .touches_by_file
                .contains_key(&repo_path("src/excluded_skew.ts"))
        );
        assert!(
            !coupling
                .touches_by_file
                .contains_key(&repo_path("src/beyond.ts"))
        );
    }
}

const TEST_GIT_ENV: [(&str, &str); 6] = [
    ("GIT_CONFIG_GLOBAL", "/dev/null"),
    ("GIT_CONFIG_NOSYSTEM", "1"),
    ("GIT_AUTHOR_NAME", "Tester"),
    ("GIT_AUTHOR_EMAIL", "tester@example.invalid"),
    ("GIT_COMMITTER_NAME", "Tester"),
    ("GIT_COMMITTER_EMAIL", "tester@example.invalid"),
];

impl TempRepo {
    pub(crate) fn new(label: &str) -> TempRepo {
        let path = std::env::temp_dir().join(format!(
            "codeint-git-{label}-{}-{}",
            std::process::id(),
            NEXT_REPO_ID.fetch_add(1, Ordering::Relaxed)
        ));
        fs::remove_dir_all(&path).ok();
        fs::create_dir_all(&path).expect("create temp repo");
        git(&path, &["init", "-q", "-b", "main"]);
        TempRepo { path }
    }

    pub(crate) fn seed(label: &str, files: &[(&str, &str)], message: &str) -> TempRepo {
        let repo = TempRepo::new(label);
        repo.write_all(files);
        repo.commit_all(message);
        repo
    }

    pub(crate) fn write(&self, relative: &str, body: &str) {
        let target = self.path.join(relative);
        fs::create_dir_all(target.parent().expect("relative path has parent")).expect("mkdir");
        fs::write(&target, body).expect("write");
    }

    pub(crate) fn write_all(&self, files: &[(&str, &str)]) {
        for &(relative, body) in files {
            self.write(relative, body);
        }
    }

    fn backdate(&self, relative: &str, seconds_ago: u64) {
        let target = self.path.join(relative);
        let file = fs::OpenOptions::new()
            .write(true)
            .open(target)
            .expect("open for backdate");
        let past = SystemTime::now() - Duration::from_secs(seconds_ago);
        file.set_modified(past).expect("set_modified");
    }

    pub(crate) fn commit_all(&self, message: &str) {
        git(&self.path, &["add", "--all"]);
        git(&self.path, &["commit", "-q", "-m", message]);
    }
}

pub(crate) fn git(cwd: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .envs(TEST_GIT_ENV)
        .output()
        .expect("spawn git");
    assert!(out.status.success(), "git {args:?} failed");
    String::from_utf8(out.stdout).expect("utf8 stdout")
}

fn repo_paths(raw: &[&str]) -> Vec<RepoPath> {
    raw.iter().copied().map(repo_path).collect()
}

#[test]
fn clean_tracked_file_skips_blob_and_disk_read_via_index_stat_cache() {
    let temp = TempRepo::new("clean-stat-fast-path");
    temp.write("clean.txt", "unchanged content\n");
    temp.write("dirty.txt", "before\n");
    temp.backdate("clean.txt", 10);
    temp.backdate("dirty.txt", 10);
    temp.commit_all("base");

    let clean_oid = git(temp.path(), &["rev-parse", "HEAD:clean.txt"])
        .trim()
        .to_owned();
    let loose_blob = temp
        .path()
        .join(".git")
        .join("objects")
        .join(&clean_oid[..2])
        .join(&clean_oid[2..]);
    assert!(
        loose_blob.exists(),
        "expected loose blob at {}",
        loose_blob.display()
    );
    fs::remove_file(&loose_blob).expect("remove loose blob for clean file");

    temp.write("dirty.txt", "after\n");

    let repo = Repository::discover(temp.path()).expect("discover");
    let merge_base = repo.merge_base_with_head("main").expect("merge_base");
    let base = repo.commit(merge_base).expect("base commit");
    let working = repo.working_tree().expect("working tree");
    let changed = working
        .changed_files_from(&base)
        .expect("clean file should skip reading deleted loose blob");

    assert_eq!(changed, repo_paths(&["dirty.txt"]));
}

#[test]
fn racy_git_same_second_modification_is_detected() {
    let temp = TempRepo::new("racy-git");
    git(temp.path(), &["config", "core.trustCTime", "false"]);
    temp.write("racy.txt", "aaaa");
    temp.commit_all("base");

    let index_mtime = fs::metadata(temp.path().join(".git").join("index"))
        .expect("index metadata")
        .modified()
        .expect("index mtime");
    temp.write("racy.txt", "bbbb");
    let file = fs::OpenOptions::new()
        .write(true)
        .open(temp.path().join("racy.txt"))
        .expect("open racy.txt");
    file.set_modified(index_mtime).expect("match index mtime");

    let repo = Repository::discover(temp.path()).expect("discover");
    let changed = repo
        .changed_files_since("main")
        .expect("changed_files_since");
    assert_eq!(changed.files, repo_paths(&["racy.txt"]));
}

#[test]
fn detects_dirty_staged_untracked_deleted_renamed_and_reverted_files() {
    let temp = TempRepo::new("working-tree-states");
    let files = [
        ("clean.txt", "clean\n"),
        ("dirty.txt", "dirty v1\n"),
        ("staged.txt", "staged v1\n"),
        ("staged_reverted.txt", "reverted v1\n"),
        ("deleted.txt", "deleted v1\n"),
        ("renamed_old.txt", "renamed v1\n"),
    ];
    for &(path, body) in &files {
        temp.write(path, body);
        temp.backdate(path, 10);
    }
    temp.commit_all("base");

    temp.write("dirty.txt", "dirty v2\n");

    temp.write("staged.txt", "staged v2\n");
    temp.backdate("staged.txt", 5);
    git(temp.path(), &["add", "staged.txt"]);

    temp.write("staged_reverted.txt", "staged temporary\n");
    git(temp.path(), &["add", "staged_reverted.txt"]);
    temp.write("staged_reverted.txt", "reverted v1\n");

    fs::remove_file(temp.path().join("deleted.txt")).expect("delete file");
    git(temp.path(), &["mv", "renamed_old.txt", "renamed_new.txt"]);
    temp.write("untracked.txt", "brand new\n");

    let repo = Repository::discover(temp.path()).expect("discover");
    let changed = repo
        .changed_files_since("main")
        .expect("changed_files_since");
    assert_eq!(
        changed.files,
        repo_paths(&[
            "dirty.txt",
            "renamed_new.txt",
            "staged.txt",
            "untracked.txt"
        ])
    );
}

#[cfg(unix)]
#[test]
fn mode_change_falls_back_safely_without_false_positive_or_miss() {
    use std::os::unix::fs::PermissionsExt;

    let temp = TempRepo::new("mode-change");
    temp.write("script.sh", "#!/bin/sh\necho ok\n");
    temp.backdate("script.sh", 10);
    temp.commit_all("base");

    let script_path = temp.path().join("script.sh");
    let mut permissions = fs::metadata(&script_path).expect("metadata").permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&script_path, permissions).expect("chmod +x");

    let repo = Repository::discover(temp.path()).expect("discover");
    let unchanged = repo
        .changed_files_since("main")
        .expect("changed_files_since after chmod");
    assert!(unchanged.files.is_empty());

    temp.write("script.sh", "#!/bin/sh\necho changed\n");
    let changed = repo
        .changed_files_since("main")
        .expect("changed_files_since after edit");
    assert_eq!(changed.files, repo_paths(&["script.sh"]));
}
