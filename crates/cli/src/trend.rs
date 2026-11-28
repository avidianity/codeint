use std::path::Path;

use codeint_config::{CalendarDay, PluginHostConfig, TrendRange};
use codeint_git::{CommitRange, ResultCache};
use codeint_report::{TrendPoint, TrendReport};

use crate::check::{CheckError, WorkspaceContext, WorkspaceRef};
use crate::commit_measurement::CommitMeasurer;

pub(crate) fn trend(
    folder: &Path,
    range: TrendRange,
    result_cache: Option<&ResultCache>,
    plugin_host: &PluginHostConfig,
) -> Result<TrendReport, CheckError> {
    let context = WorkspaceContext::load(folder, plugin_host)?;
    trend_with_workspace(context.as_ref(), range, result_cache)
}

pub(crate) fn trend_with_workspace(
    workspace: WorkspaceRef<'_>,
    range: TrendRange,
    result_cache: Option<&ResultCache>,
) -> Result<TrendReport, CheckError> {
    let measurer = CommitMeasurer::new(
        workspace.repository,
        workspace.configuration,
        workspace.plugins,
        result_cache,
    );
    let points = workspace
        .repository
        .first_parent_history(commit_range(range))?
        .into_iter()
        .map(|history_commit| {
            Ok(TrendPoint {
                commit: history_commit.commit.to_string(),
                committed_on: CalendarDay::containing(history_commit.committed_at_unix_seconds),
                measurement: measurer.measure(history_commit.commit)?,
            })
        })
        .collect::<Result<Vec<_>, CheckError>>()?;
    Ok(TrendReport::of_with_test_gap_note(
        points,
        !workspace.configuration.coverage_reports.is_empty(),
    ))
}

fn commit_range(range: TrendRange) -> CommitRange {
    match range {
        TrendRange::Latest(count) => CommitRange::Latest(count),
        TrendRange::Since(day) => CommitRange::CommittedSince {
            unix_seconds: day.start_unix_seconds(),
        },
    }
}
