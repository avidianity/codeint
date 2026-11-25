use std::collections::HashSet;
use std::path::Path;

use codeint_config::{FindingsShown, PluginHostConfig};
use codeint_git::{CommitId, GitError, Repository, ResultCache, Snapshot};
use codeint_model::RepoPath;
use codeint_report::{
    BaseCommit, CommitMeasurement, DiffReport, DiffReportOptions, Head, HeadCommit,
};

use crate::check::{CheckError, PreparedRun, SelectedPackage, WorkspaceContext};
use crate::commit_measurement::{
    CommitMeasurer, measure_all_languages_for_package, measure_all_languages_with_new_files,
};

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
    owner: Option<&str>,
    result_cache: Option<&ResultCache>,
    plugin_host: &PluginHostConfig,
) -> Result<DiffReport, CheckError> {
    let run = PreparedRun::load(folder, owner, plugin_host)?;
    let Sides {
        merge_base,
        head,
        head_commit,
    } = sides(&run.context.repository, &refs)?;
    let check_new_file_budgets = !run.context.configuration.new_file_budgets.is_empty();
    let (selected, head_measurement, new_files_measurement) = measure_head(
        &run.context,
        merge_base,
        head_commit,
        package,
        result_cache,
        check_new_file_budgets,
    )?;
    let base_measurer = CommitMeasurer::for_package(
        &run.context.repository,
        &run.context.configuration,
        &run.context.plugins,
        result_cache,
        selected.as_ref().map(|pkg| pkg.as_str()),
    );
    let base_measurement = base_measurer.measure_selected_package(merge_base, selected.as_ref())?;
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
            budgets: &run.context.configuration.budgets,
            new_file_budgets: &run.context.configuration.new_file_budgets,
            new_files: new_files_measurement,
            limits: &run.context.configuration.regression_limits,
            ownership: run.ownership(),
            include_test_gap_note: !run.context.configuration.coverage_reports.is_empty(),
        },
    ))
}

fn measure_head(
    context: &WorkspaceContext,
    merge_base: CommitId,
    head_commit: Option<CommitId>,
    package: Option<&str>,
    result_cache: Option<&ResultCache>,
    check_new_file_budgets: bool,
) -> Result<
    (
        Option<SelectedPackage>,
        CommitMeasurement,
        Option<CommitMeasurement>,
    ),
    CheckError,
> {
    if check_new_file_budgets {
        let head_snapshot = open_head_snapshot(&context.repository, head_commit)?;
        let new_files = newly_added_files(&context.repository, merge_base, &head_snapshot)?;
        if !new_files.is_empty() {
            return measure_all_languages_with_new_files(
                &head_snapshot,
                &new_files,
                &context.plugins,
                &context.configuration,
                package,
            );
        }
    }
    let (selected, head_measurement) =
        measure_head_without_new_files(context, head_commit, package, result_cache)?;
    Ok((selected, head_measurement, None))
}

fn open_head_snapshot(
    repository: &Repository,
    head_commit: Option<CommitId>,
) -> Result<Snapshot, GitError> {
    match head_commit {
        None => repository.working_tree(),
        Some(commit) => repository.commit(commit),
    }
}

fn newly_added_files(
    repository: &Repository,
    merge_base: CommitId,
    head_snapshot: &Snapshot,
) -> Result<HashSet<RepoPath>, GitError> {
    let base_snapshot = repository.commit(merge_base)?;
    let base_files: HashSet<&RepoPath> = base_snapshot.files().iter().collect();
    Ok(head_snapshot
        .files()
        .iter()
        .filter(|path| !base_files.contains(path))
        .cloned()
        .collect())
}

fn measure_head_without_new_files(
    context: &WorkspaceContext,
    head_commit: Option<CommitId>,
    package: Option<&str>,
    result_cache: Option<&ResultCache>,
) -> Result<(Option<SelectedPackage>, CommitMeasurement), CheckError> {
    let Some(commit) = head_commit else {
        return measure_all_languages_for_package(
            &context.repository.working_tree()?,
            &context.plugins,
            &context.configuration,
            package,
        );
    };
    if let Some(requested) = package {
        measure_all_languages_for_package(
            &context.repository.commit(commit)?,
            &context.plugins,
            &context.configuration,
            Some(requested),
        )
    } else {
        let measurer = CommitMeasurer::new(
            &context.repository,
            &context.configuration,
            &context.plugins,
            result_cache,
        );
        Ok((None, measurer.measure(commit)?))
    }
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
