use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, SystemTime};

use codeint_model::RepoPath;

use crate::{Repository, ResultCache};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

struct TempRepo {
    root: PathBuf,
}

impl TempRepo {
    fn new(label: &str) -> TempRepo {
        let id = COUNTER.fetch_add(1, Ordering::Relaxed);
        let root =
            std::env::temp_dir().join(format!("codeint-git-{label}-{}-{id}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("create temp repo");
        git(&root, &["init", "-q", "-b", "main"]);
        TempRepo { root }
    }

    fn path(&self) -> &Path {
        &self.root
    }

    fn write(&self, relative: &str, body: &str) {
        let target = self.root.join(relative);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).expect("mkdir");
        }
        fs::write(target, body).expect("write");
    }

    fn backdate(&self, relative: &str, seconds_ago: u64) {
        let target = self.root.join(relative);
        let file = fs::OpenOptions::new()
            .write(true)
            .open(target)
            .expect("open for backdate");
        let past = SystemTime::now() - Duration::from_secs(seconds_ago);
        file.set_modified(past).expect("set_modified");
    }

    fn commit_all(&self, message: &str) {
        git(&self.root, &["add", "--all"]);
        git(&self.root, &["commit", "-q", "-m", message]);
    }
}

impl Drop for TempRepo {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn git(cwd: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .envs([
            ("GIT_CONFIG_GLOBAL", "/dev/null"),
            ("GIT_CONFIG_NOSYSTEM", "1"),
            ("GIT_AUTHOR_NAME", "Tester"),
            ("GIT_AUTHOR_EMAIL", "tester@example.invalid"),
            ("GIT_COMMITTER_NAME", "Tester"),
            ("GIT_COMMITTER_EMAIL", "tester@example.invalid"),
        ])
        .output()
        .expect("spawn git");
    assert!(
        output.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("utf8 stdout")
}

fn repo_paths(raw: &[&str]) -> Vec<RepoPath> {
    raw.iter()
        .map(|item| RepoPath::new((*item).to_owned()).expect("valid repo path"))
        .collect()
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

    let oldest_path = cache_dir.join("0000000000000000000000000000000000000000-oldest.json");
    fs::write(&oldest_path, "{}").expect("write oldest");
    set_mtime(&oldest_path, SystemTime::now() - Duration::from_secs(3600));

    cache.write(first, &"settings", &1u32).expect("write first");
    let first_path = cache_json_files(&cache_dir)
        .into_iter()
        .find(|path| path != &oldest_path)
        .expect("first cache file");
    set_mtime(&first_path, SystemTime::now() - Duration::from_secs(3500));

    let mid_time = SystemTime::now() - Duration::from_secs(1800);
    for index in 0..4_998usize {
        let filler = cache_dir.join(format!("filler-{index:05}.json"));
        fs::write(&filler, "{}").expect("write filler");
        set_mtime(&filler, mid_time);
    }
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
