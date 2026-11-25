use std::path::{Path, PathBuf};

use codeint_config::{
    PluginHostConfig, Ratchet, RatchetFileError, RatchetVersionMismatch, RatchetViolation,
};
use codeint_report::{RATCHET_PACKAGE_SKIPPED_NOTE, RatchetUpdateReport};

use crate::check::{CheckError, WorkspaceContext, measure_whole_repo_for_ratchet};

pub(crate) fn enforce_if_present<T, FNote, FApply>(
    repository_root: &Path,
    ratchet_path: &str,
    package_scoped: bool,
    target: &mut T,
    set_note: FNote,
    apply: FApply,
) -> Result<(), CheckError>
where
    FNote: FnOnce(&mut T, &'static str),
    FApply: FnOnce(&mut T, &Ratchet) -> Result<(), Vec<RatchetVersionMismatch>>,
{
    if package_scoped {
        if Ratchet::exists(repository_root, ratchet_path)? {
            set_note(target, RATCHET_PACKAGE_SKIPPED_NOTE);
        }
        return Ok(());
    }
    if let Some(ratchet) = Ratchet::load_optional(repository_root, ratchet_path)? {
        apply(target, &ratchet).map_err(|mismatches| CheckError::RatchetVersionMismatch {
            path: ratchet_path.to_owned(),
            mismatches,
        })?;
    }
    Ok(())
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum RatchetError {
    #[error(transparent)]
    Check(#[from] CheckError),
    #[error(transparent)]
    RatchetFile(#[from] RatchetFileError),
    #[error(
        "refusing to loosen Ratchet ceilings in {path:?} for unchanged Metric versions: {}",
        format_violations(regressions)
    )]
    LooseningRefused {
        path: String,
        regressions: Vec<RatchetViolation>,
    },
}

pub(crate) fn update(
    folder: &Path,
    reset: bool,
    coverage_reports: &[PathBuf],
    plugin_host: &PluginHostConfig,
) -> Result<RatchetUpdateReport, RatchetError> {
    let context = WorkspaceContext::load(folder, plugin_host)?;
    let report = measure_whole_repo_for_ratchet(&context, folder, coverage_reports)?;
    let measured = report.ratchet_metrics();
    let ratchet_path = &context.configuration.ratchet.path;
    let existing = Ratchet::load_optional(context.repository.root(), ratchet_path)?;
    let (updated, summary) = Ratchet::update(existing.as_ref(), &measured, reset, ratchet_path)
        .map_err(|regressions| RatchetError::LooseningRefused {
            path: ratchet_path.clone(),
            regressions,
        })?;
    updated.write_atomic(context.repository.root(), ratchet_path)?;
    Ok(RatchetUpdateReport::from_summary(summary))
}

fn format_violations(violations: &[RatchetViolation]) -> String {
    violations
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(", ")
}
