use std::path::{Path, PathBuf};

use codeint_config::{Baseline, BaselineUpdateSummary, PluginHostConfig};

use crate::check::{CheckError, WorkspaceContext, measure_whole_repo};

pub(crate) fn update(
    folder: &Path,
    reset: bool,
    coverage_reports: &[PathBuf],
    plugin_host: &PluginHostConfig,
) -> Result<BaselineUpdateSummary, CheckError> {
    let context = WorkspaceContext::load(folder, plugin_host)?;
    let report = measure_whole_repo(&context, folder, coverage_reports)?;
    let root = context.repository.root();
    let existing = Baseline::load_optional(root)?;
    let (updated, summary) = Baseline::update(existing.as_ref(), &report.baseline_metrics(), reset);
    updated.write_atomic(root)?;
    Ok(summary)
}
