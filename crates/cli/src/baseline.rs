use codeint_config::{Baseline, BaselineUpdateSummary};
use codeint_report::Report;

use crate::check::{CheckError, WorkspaceContext};

pub(crate) fn update_from_report(
    context: &WorkspaceContext,
    report: &Report,
    reset: bool,
) -> Result<BaselineUpdateSummary, CheckError> {
    let root = context.repository.root();
    let loaded = Baseline::load_for_update(root, reset)?;
    let (updated, mut summary) =
        Baseline::update(loaded.existing.as_ref(), &report.baseline_metrics(), reset);
    summary.replaced_invalid = loaded.replaced_invalid;
    updated.write_atomic(root)?;
    Ok(summary)
}
