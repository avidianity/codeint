use std::path::Path;

use codeint_config::{FindingsShown, PluginHostConfig};
use codeint_git::{CommitId, GitError, Repository, ResultCache};
use codeint_report::{BaseCommit, DiffReport, DiffReportOptions, Head, HeadCommit};

use crate::check::{CheckError, PreparedRun};
use crate::commit_measurement::{CommitMeasurer, measure_all_languages};

pub(crate) struct Refs<'a> {
    pub(crate) base: &'a str,
    pub(crate) head: Option<&'a str>,
}

struct Sides {
    merge_base: CommitId,
    head: Head,
    head_commit: Option<CommitId>,
}

pub(crate) fn diff(
    folder: &Path,
    refs: Refs<'_>,
    findings: FindingsShown,
    owner: Option<&str>,
    result_cache: Option<&ResultCache>,
    plugin_host: &PluginHostConfig,
) -> Result<DiffReport, CheckError> {
    let run = PreparedRun::load(folder, owner, plugin_host)?;
    let Sides {
        merge_base,
        head,
        head_commit,
    } = sides(&run.repository, &refs)?;
    let measurer = CommitMeasurer::new(
        &run.repository,
        &run.configuration,
        &run.loaded,
        result_cache,
    );
    let base_measurement = measurer.measure(merge_base)?;
    let head_measurement = match head_commit {
        Some(commit) => measurer.measure(commit)?,
        None => measure_all_languages(
            &run.repository.working_tree()?,
            &run.loaded,
            &run.configuration,
        )?,
    };
    Ok(DiffReport::compare_with_options(
        BaseCommit {
            git_ref: refs.base.to_owned(),
            merge_base: merge_base.to_string(),
        },
        head,
        base_measurement,
        head_measurement,
        DiffReportOptions {
            findings,
            budgets: &run.configuration.budgets,
            limits: &run.configuration.regression_limits,
            ownership: run.ownership(),
        },
    ))
}

fn sides(repository: &Repository, refs: &Refs<'_>) -> Result<Sides, GitError> {
    match refs.head {
        None => Ok(Sides {
            merge_base: repository.merge_base_with_head(refs.base)?,
            head: Head::WorkingTree,
            head_commit: None,
        }),
        Some(head_ref) => {
            let merge_base = repository.merge_base(refs.base, head_ref)?;
            let head_commit = repository.resolve(head_ref)?;
            Ok(Sides {
                merge_base,
                head: Head::Commit(HeadCommit {
                    git_ref: head_ref.to_owned(),
                    commit: head_commit.to_string(),
                }),
                head_commit: Some(head_commit),
            })
        }
    }
}
