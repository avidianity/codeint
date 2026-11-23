use std::path::Path;

use codeint_config::{FindingsShown, PluginHostConfig, read_configuration};
use codeint_git::{CommitId, GitError, Repository, ResultCache};
use codeint_report::{BaseCommit, DiffReport, Head, HeadCommit};

use crate::check::{CheckError, LoadedPlugins};
use crate::commit_measurement::{CommitMeasurer, measure_all_languages_for_package};

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
    package: Option<&str>,
    result_cache: Option<&ResultCache>,
    plugin_host: &PluginHostConfig,
) -> Result<DiffReport, CheckError> {
    let repository = Repository::discover(folder)?;
    let configuration = read_configuration(repository.root())?;
    let plugins = LoadedPlugins::load(repository.root(), &configuration, plugin_host)?;
    let Sides {
        merge_base,
        head,
        head_commit,
    } = sides(&repository, &refs)?;
    let (selected, head_measurement) = match head_commit {
        Some(commit) => {
            if let Some(requested) = package {
                measure_all_languages_for_package(
                    &repository.commit(commit)?,
                    &plugins,
                    &configuration,
                    Some(requested),
                )?
            } else {
                let measurer =
                    CommitMeasurer::new(&repository, &configuration, &plugins, result_cache);
                (None, measurer.measure(commit)?)
            }
        }
        None => measure_all_languages_for_package(
            &repository.working_tree()?,
            &plugins,
            &configuration,
            package,
        )?,
    };
    let base_measurer = CommitMeasurer::for_package(
        &repository,
        &configuration,
        &plugins,
        result_cache,
        selected.as_ref().map(|pkg| pkg.as_str()),
    );
    let base_measurement = base_measurer.measure_selected_package(merge_base, selected.as_ref())?;
    Ok(DiffReport::compare(
        BaseCommit {
            git_ref: refs.base.to_owned(),
            merge_base: merge_base.to_string(),
        },
        head,
        base_measurement,
        head_measurement,
        findings,
        &configuration.budgets,
        &configuration.regression_limits,
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
