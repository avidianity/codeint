use std::path::Path;

use codeint_config::TrendRange;
use codeint_git::{CommitRange, ResultCache};
use codeint_report::TrendReport;

use crate::check::CheckError;

pub(crate) fn trend(
    folder: &Path,
    range: TrendRange,
    result_cache: Option<&ResultCache>,
) -> Result<TrendReport, CheckError> {
    let _ = (folder, range, result_cache, commit_range);
    todo!()
}

fn commit_range(range: TrendRange) -> CommitRange {
    let _ = range;
    todo!()
}
