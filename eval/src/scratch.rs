use std::path::{Path, PathBuf};
use std::process::Command;

use tempfile::TempDir;

use crate::command::{run_checked, stdout_text};
use crate::error::{EvaluationError, io_error};

const FOLDER_PREFIX: &str = "codeint-eval-";

fn scratch_base(scratch_root: Option<&Path>) -> PathBuf {
    scratch_root.map_or_else(std::env::temp_dir, Path::to_path_buf)
}

pub(crate) struct ScratchFolder {
    folder: TempDir,
}

impl ScratchFolder {
    pub(crate) fn outside(
        protected_repositories: &[PathBuf],
        scratch_root: Option<&Path>,
    ) -> Result<ScratchFolder, EvaluationError> {
        let root = scratch_base(scratch_root);
        let folder = tempfile::Builder::new()
            .prefix(&format!("{FOLDER_PREFIX}{}-", std::process::id()))
            .tempdir_in(&root)
            .map_err(io_error(root))?;
        let scratch = folder
            .path()
            .canonicalize()
            .map_err(io_error(folder.path()))?;
        for repository in protected_repositories {
            let Ok(repository) = repository.canonicalize() else {
                continue;
            };
            if scratch.starts_with(&repository) {
                return Err(EvaluationError::ScratchInsideRepository {
                    scratch,
                    repository,
                });
            }
        }
        Ok(ScratchFolder { folder })
    }

    pub(crate) fn path(&self) -> &Path {
        self.folder.path()
    }
}

fn git(checkout: &Path) -> Command {
    let mut command = Command::new("git");
    command.arg("-C").arg(checkout);
    command
}

const FETCH_ATTEMPTS: usize = 3;

fn fetch_attempts_for(url: &str) -> usize {
    if url.starts_with("file://") {
        1
    } else {
        FETCH_ATTEMPTS
    }
}

fn fetch_with_retry(checkout: &Path, url: &str, commit: &str) -> Result<(), EvaluationError> {
    let attempts = fetch_attempts_for(url);
    let mut last_error = None;
    for attempt in 1..=attempts {
        match run_checked(
            git(checkout).args(["fetch", "--quiet", "--depth", "1", "--", url, commit]),
        ) {
            Ok(_) => return Ok(()),
            Err(error) => {
                last_error = Some(error);
                if attempt < attempts {
                    std::thread::sleep(std::time::Duration::from_secs(2 * attempt as u64));
                }
            }
        }
    }
    Err(last_error.expect("at least one fetch attempt ran"))
}

pub(crate) fn clone_at_commit(
    scratch: &ScratchFolder,
    url: &str,
    commit: &str,
) -> Result<PathBuf, EvaluationError> {
    let checkout = scratch.path().join("checkout");
    run_checked(Command::new("git").args(["init", "--quiet"]).arg(&checkout))?;
    fetch_with_retry(&checkout, url, commit)?;
    run_checked(git(&checkout).args(["checkout", "--quiet", "--detach", "FETCH_HEAD"]))?;
    let head = stdout_text(git(&checkout).args(["rev-parse", "HEAD"]))?;
    if head != commit {
        return Err(EvaluationError::Command {
            command: format!("git fetch {url} {commit}"),
            detail: format!("checked out {head} instead of the pinned commit"),
        });
    }
    Ok(checkout)
}

pub(crate) fn tracked_files(checkout: &Path) -> Result<Vec<String>, EvaluationError> {
    let output = run_checked(git(checkout).args(["ls-files", "-z"]))?;
    Ok(output
        .stdout
        .split(|byte| *byte == 0)
        .filter(|path| !path.is_empty())
        .filter_map(|path| std::str::from_utf8(path).ok())
        .map(str::to_owned)
        .collect())
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct StaleSweep {
    pub removed: u64,
    pub failures: Vec<String>,
}

fn owner_process(folder_name: &str) -> Option<u32> {
    let (owner, suffix) = folder_name.strip_prefix(FOLDER_PREFIX)?.split_once('-')?;
    if suffix.is_empty() || owner.is_empty() || !owner.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    owner.parse().ok()
}

fn listing_names_process(listing: &str, process_id: u32) -> bool {
    listing.contains(&format!("\"{process_id}\""))
}

fn process_is_alive(process_id: u32) -> bool {
    let identifier = process_id.to_string();
    let output = if cfg!(windows) {
        Command::new("tasklist")
            .args(["/NH", "/FO", "CSV", "/FI"])
            .arg(format!("PID eq {identifier}"))
            .output()
    } else {
        Command::new("ps")
            .args(["-o", "pid=", "-p"])
            .arg(&identifier)
            .output()
    };
    match output {
        Err(_) => true,
        Ok(output) if cfg!(windows) => {
            listing_names_process(&String::from_utf8_lossy(&output.stdout), process_id)
        }
        Ok(output) => output.status.success(),
    }
}

pub fn sweep_stale_scratch_folders(scratch_root: Option<&Path>) -> StaleSweep {
    let root = scratch_base(scratch_root);
    let mut sweep = StaleSweep::default();
    let entries = match std::fs::read_dir(&root) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return sweep,
        Err(error) => {
            sweep
                .failures
                .push(format!("cannot read {}: {error}", root.display()));
            return sweep;
        }
    };
    for entry in entries.flatten() {
        let Some(owner) = entry.file_name().to_str().and_then(owner_process) else {
            continue;
        };
        let is_folder = entry.file_type().is_ok_and(|kind| kind.is_dir());
        if !is_folder || owner == std::process::id() || process_is_alive(owner) {
            continue;
        }
        match std::fs::remove_dir_all(entry.path()) {
            Ok(()) => sweep.removed += 1,
            Err(error) => sweep
                .failures
                .push(format!("cannot remove {}: {error}", entry.path().display())),
        }
    }
    sweep
}

#[cfg(test)]
mod tests;
