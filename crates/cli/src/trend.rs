use std::path::Path;

use codeint_config::{CalendarDay, TrendRange, read_configuration};
use codeint_git::{CommitRange, Repository, ResultCache};
use codeint_report::{TrendPoint, TrendReport};

use crate::check::CheckError;
use crate::commit_measurement::CommitMeasurer;

pub(crate) fn trend(
    folder: &Path,
    range: TrendRange,
    result_cache: Option<&ResultCache>,
) -> Result<TrendReport, CheckError> {
    let repository = Repository::discover(folder)?;
    let configuration = read_configuration(repository.root())?;
    let measurer = CommitMeasurer::new(&repository, &configuration, result_cache);
    let points = repository
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
    Ok(TrendReport::of(points))
}

fn commit_range(range: TrendRange) -> CommitRange {
    match range {
        TrendRange::Latest(count) => CommitRange::Latest(count),
        TrendRange::Since(day) => CommitRange::CommittedSince {
            unix_seconds: day.start_unix_seconds(),
        },
    }
}
