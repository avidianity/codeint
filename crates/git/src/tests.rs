use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime};

use codeint_model::RepoPath;

use crate::history::SECONDS_PER_DAY;
use crate::{ChurnRange, CommitId, CommitRange, Repository, ResultCache};

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

fn cache_json_files(folder: &Path) -> Vec<PathBuf> {
    fs::read_dir(folder)
        .expect("read cache folder")
        .map(|entry| entry.expect("dir entry").path())
        .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("json"))
        .collect()
}

fn set_mtime(path: &Path, time: SystemTime) {
    let file = fs::OpenOptions::new()
        .write(true)
        .open(path)
        .expect("open for mtime");
    file.set_modified(time).expect("set_modified");
}

fn seed_cache_entry(cache_dir: &Path, name: &str, age_seconds: u64) -> PathBuf {
    let path = cache_dir.join(name);
    fs::write(&path, "{}").expect("write cache entry");
    set_mtime(&path, SystemTime::now() - Duration::from_secs(age_seconds));
    path
}

fn seed_filler_cache_entries(cache_dir: &Path, count: usize, age_seconds: u64) {
    let modified = SystemTime::now() - Duration::from_secs(age_seconds);
    for index in 0..count {
        let filler = cache_dir.join(format!("filler-{index:05}.json"));
        fs::write(&filler, "{}").expect("write filler");
        set_mtime(&filler, modified);
    }
}

#[test]
fn result_cache_prunes_entries_untouched_for_more_than_thirty_days_and_refreshes_on_hit() {
    let temp = TempRepo::new("result-cache-age-prune");
    temp.write("a.txt", "one\n");
    temp.commit_all("first");
    temp.write("a.txt", "two\n");
    temp.commit_all("second");
    temp.write("a.txt", "three\n");
    temp.commit_all("third");

    let repo = Repository::discover(temp.path()).expect("discover");
    let first = repo.resolve("HEAD~2").expect("first commit");
    let second = repo.resolve("HEAD~1").expect("second commit");
    let third = repo.resolve("HEAD").expect("third commit");

    let cache_dir = temp.path().join(".cache");
    let cache = ResultCache::in_folder(cache_dir.clone());
    cache.write(first, &"settings", &1u32).expect("write first");
    cache
        .write(second, &"settings", &2u32)
        .expect("write second");

    let thirty_one_days = Duration::from_secs(31 * 24 * 60 * 60);
    let stale_time = SystemTime::now() - thirty_one_days;
    for path in cache_json_files(&cache_dir) {
        set_mtime(&path, stale_time);
    }

    assert_eq!(cache.read::<u32>(second, &"settings"), Some(2));

    cache.write(third, &"settings", &3u32).expect("write third");

    assert_eq!(cache.read::<u32>(first, &"settings"), None);
    assert_eq!(cache.read::<u32>(second, &"settings"), Some(2));
    assert_eq!(cache.read::<u32>(third, &"settings"), Some(3));
    assert_eq!(cache_json_files(&cache_dir).len(), 2);
}

#[test]
fn result_cache_enforces_five_thousand_entry_cap_evicting_oldest_touched() {
    let temp = TempRepo::new("result-cache-cap-prune");
    temp.write("a.txt", "one\n");
    temp.commit_all("first");
    temp.write("a.txt", "two\n");
    temp.commit_all("second");

    let repo = Repository::discover(temp.path()).expect("discover");
    let first = repo.resolve("HEAD~1").expect("first commit");
    let second = repo.resolve("HEAD").expect("second commit");

    let cache_dir = temp.path().join(".cache");
    fs::create_dir_all(&cache_dir).expect("create cache dir");
    let cache = ResultCache::in_folder(cache_dir.clone());

    let oldest_path = seed_cache_entry(
        &cache_dir,
        "0000000000000000000000000000000000000000-oldest.json",
        3600,
    );

    cache.write(first, &"settings", &1u32).expect("write first");
    let first_path = cache_json_files(&cache_dir)
        .into_iter()
        .find(|path| path != &oldest_path)
        .expect("first cache file");
    set_mtime(&first_path, SystemTime::now() - Duration::from_secs(3500));

    seed_filler_cache_entries(&cache_dir, 4_998, 1800);
    assert_eq!(cache_json_files(&cache_dir).len(), 5_000);

    assert_eq!(cache.read::<u32>(first, &"settings"), Some(1));

    cache
        .write(second, &"settings", &2u32)
        .expect("write second");

    assert_eq!(cache_json_files(&cache_dir).len(), 5_000);
    assert!(!oldest_path.exists(), "oldest untouched entry was evicted");
    assert_eq!(cache.read::<u32>(first, &"settings"), Some(1));
    assert_eq!(cache.read::<u32>(second, &"settings"), Some(2));
}

#[test]
fn result_cache_prune_ignores_json_subdirectories_and_symlinks() {
    let temp = TempRepo::new("result-cache-ignore-non-files");
    temp.write("a.txt", "one\n");
    temp.commit_all("first");

    let repo = Repository::discover(temp.path()).expect("discover");
    let first = repo.resolve("HEAD").expect("first commit");

    let cache_dir = temp.path().join(".cache");
    fs::create_dir_all(&cache_dir).expect("create cache dir");
    let cache = ResultCache::in_folder(cache_dir.clone());

    let subdir = cache_dir.join("nested.json");
    fs::create_dir(&subdir).expect("create json subdir");

    #[cfg(unix)]
    let symlink_path = {
        let external_target = temp.path().join("external.json");
        fs::write(&external_target, "{}").expect("write symlink target");
        let thirty_one_days = Duration::from_secs(31 * 24 * 60 * 60);
        set_mtime(&external_target, SystemTime::now() - thirty_one_days);
        let link = cache_dir.join("linked.json");
        std::os::unix::fs::symlink(&external_target, &link).expect("create json symlink");
        link
    };

    let oldest_path = seed_cache_entry(
        &cache_dir,
        "0000000000000000000000000000000000000000-oldest.json",
        3600,
    );
    seed_filler_cache_entries(&cache_dir, 4_998, 1800);

    cache.write(first, &"settings", &1u32).expect("write first");

    assert!(subdir.is_dir(), "json subdirectory should remain untouched");
    #[cfg(unix)]
    assert!(
        fs::symlink_metadata(&symlink_path).is_ok(),
        "json symlink should remain untouched"
    );
    assert!(
        oldest_path.exists(),
        "non-file json entries should not count toward the 5,000 entry cap"
    );
    assert_eq!(cache.read::<u32>(first, &"settings"), Some(1));
}

fn seeded_cache(label: &str) -> (TempRepo, PathBuf, ResultCache, crate::CommitId) {
    let temp = TempRepo::seed(label, &[("a.txt", "one\n")], "first");
    let repo = Repository::discover(temp.path()).expect("discover");
    let head = repo.resolve("HEAD").expect("first commit");
    let cache_dir = temp.path().join(".cache");
    fs::create_dir_all(&cache_dir).expect("create cache dir");
    let cache = ResultCache::in_folder(cache_dir.clone());
    (temp, cache_dir, cache, head)
}

#[test]
fn result_cache_prune_evicts_future_dated_entries_while_preserving_small_clock_skew() {
    let (_temp, cache_dir, cache, first) = seeded_cache("result-cache-future-mtime");
    let future_path = seed_cache_entry(&cache_dir, "future.json", 0);
    set_mtime(
        &future_path,
        SystemTime::now() + Duration::from_secs(365 * 24 * 60 * 60),
    );
    let jitter_path = seed_cache_entry(&cache_dir, "jitter.json", 0);
    set_mtime(&jitter_path, SystemTime::now() + Duration::from_secs(60));
    seed_filler_cache_entries(&cache_dir, 4_999, 1800);

    cache.write(first, &"settings", &1u32).expect("write first");

    assert!(
        !future_path.exists(),
        "future-dated entry beyond clock skew tolerance must be evicted"
    );
    assert!(
        jitter_path.exists(),
        "entry within clock skew tolerance should remain"
    );
    assert_eq!(cache_json_files(&cache_dir).len(), 5_000);
}

#[test]
fn result_cache_prune_removes_stale_temporary_files_and_keeps_fresh_ones() {
    let (_temp, cache_dir, cache, first) = seeded_cache("result-cache-stale-temp");
    let stale_temp = seed_cache_entry(&cache_dir, ".writing-99-1-0.tmp", 90 * 24 * 60 * 60);
    let future_temp = seed_cache_entry(&cache_dir, ".writing-99-2-0.tmp", 0);
    set_mtime(
        &future_temp,
        SystemTime::now() + Duration::from_secs(365 * 24 * 60 * 60),
    );
    let fresh_temp = seed_cache_entry(&cache_dir, ".writing-current.tmp", 0);

    cache.write(first, &"settings", &1u32).expect("write first");

    assert!(
        !stale_temp.exists(),
        "stale orphaned temporary file must be removed"
    );
    assert!(
        !future_temp.exists(),
        "future-dated temporary file must be removed"
    );
    assert!(
        fresh_temp.exists(),
        "fresh in-flight temporary file must remain untouched"
    );
}

#[test]
fn parallel_commit_reader_streams_blobs_identically_across_threads() {
    let temp = TempRepo::new("parallel-commit-reader");
    for index in 0..32usize {
        temp.write(
            &format!("dir_{}/file_{index:02}.txt", index % 4),
            &format!("blob body {index}\nline two {index}\n"),
        );
    }
    temp.commit_all("initial");

    let repo = Repository::discover(temp.path()).expect("discover");
    let head = repo.resolve("HEAD").expect("resolve HEAD");
    let commit_snapshot = repo.commit(head).expect("commit snapshot");
    let working_snapshot = repo.working_tree().expect("working tree");

    for path in commit_snapshot.files() {
        assert!(commit_snapshot.blob_id(path).is_some());
        assert!(working_snapshot.blob_id(path).is_none());
    }

    let expected: Vec<(RepoPath, Vec<u8>)> = commit_snapshot
        .files()
        .iter()
        .map(|path| (path.clone(), commit_snapshot.read(path).expect("read")))
        .collect();

    let reader = commit_snapshot.parallel_reader();
    let files = commit_snapshot.files();
    std::thread::scope(|scope| {
        let mut handles = Vec::new();
        for _ in 0..4 {
            handles.push(scope.spawn(move || {
                let local = reader.thread_local();
                files
                    .iter()
                    .map(|path| {
                        assert!(local.blob_id(path).is_some());
                        (path.clone(), local.read(path).expect("parallel read"))
                    })
                    .collect::<Vec<_>>()
            }));
        }
        for handle in handles {
            let actual = handle.join().expect("worker thread");
            assert_eq!(actual, expected);
        }
    });
}
