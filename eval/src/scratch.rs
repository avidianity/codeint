use std::path::{Path, PathBuf};
use std::process::Command;

use tempfile::TempDir;

use crate::command::{run_checked, stdout_text};
use crate::error::{EvaluationError, io_error};

pub(crate) struct ScratchFolder {
    folder: TempDir,
}

impl ScratchFolder {
    pub(crate) fn outside(
        codeint_repository: &Path,
        scratch_root: Option<&Path>,
    ) -> Result<ScratchFolder, EvaluationError> {
        let root = scratch_root.map_or_else(std::env::temp_dir, Path::to_path_buf);
        let folder = tempfile::Builder::new()
            .prefix("codeint-eval-")
            .tempdir_in(&root)
            .map_err(io_error(root))?;
        let scratch = folder
            .path()
            .canonicalize()
            .map_err(io_error(folder.path()))?;
        let repository = codeint_repository
            .canonicalize()
            .map_err(io_error(codeint_repository))?;
        if scratch.starts_with(&repository) {
            return Err(EvaluationError::ScratchInsideRepository {
                scratch,
                repository,
            });
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

pub(crate) fn clone_at_commit(
    scratch: &ScratchFolder,
    url: &str,
    commit: &str,
) -> Result<PathBuf, EvaluationError> {
    let checkout = scratch.path().join("checkout");
    run_checked(Command::new("git").args(["init", "--quiet"]).arg(&checkout))?;
    run_checked(git(&checkout).args(["fetch", "--quiet", "--depth", "1", url, commit]))?;
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
