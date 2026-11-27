use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use codeint_model::RepoPath;

use crate::{ChurnRange, CommitId, CommitRange, Repository};

static NEXT_REPO_ID: AtomicU64 = AtomicU64::new(0);

const MAY_10_UTC: i64 = 1_778_371_200;
const SECONDS_PER_DAY: i64 = 86_400;

struct TempRepo {
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

    fn path(&self) -> &Path {
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

    let repo = Repository::discover(temp.path()).expect("discover repo");
    let commits = repo
        .first_parent_history(CommitRange::CommittedSince {
            unix_seconds: MAY_10_UTC,
        })
        .expect("walk history");

    let ids: Vec<CommitId> = commits.iter().map(|entry| entry.commit).collect();
    assert_eq!(ids, vec![behind_skew, head]);
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

    let repo = Repository::discover(temp.path()).expect("discover repo");
    let commits = repo
        .first_parent_history(CommitRange::CommittedSince {
            unix_seconds: MAY_10_UTC,
        })
        .expect("walk history");

    let ids: Vec<CommitId> = commits.iter().map(|entry| entry.commit).collect();
    assert_eq!(ids, vec![reachable, head]);
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
