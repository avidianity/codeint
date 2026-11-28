use std::collections::{BTreeSet, HashSet};
use std::path::Path;

use codeint_config::{Baseline, FindingsShown, PluginHostConfig};
use codeint_git::{CommitId, GitError, Repository, ResultCache, Snapshot};
use codeint_model::RepoPath;
use codeint_report::{
    BaseCommit, CommitMeasurement, DiffReport, DiffReportOptions, Head, HeadCommit,
};

use crate::check::{
    CheckError, PreparedOwnership, SelectedPackage, WorkspaceContext, WorkspaceRef,
};
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

struct MeasuredHead {
    selected: Option<SelectedPackage>,
    measurement: CommitMeasurement,
    new_files: Option<CommitMeasurement>,
    changed_files: BTreeSet<String>,
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
    let context = WorkspaceContext::load(folder, plugin_host)?;
    diff_with_workspace(
        context.as_ref(),
        refs,
        findings,
        package,
        owner,
        result_cache,
    )
}

pub(crate) fn diff_with_workspace(
    workspace: WorkspaceRef<'_>,
    refs: Refs<'_>,
    findings: FindingsShown,
    package: Option<&str>,
    owner: Option<&str>,
    result_cache: Option<&ResultCache>,
) -> Result<DiffReport, CheckError> {
    let ownership = PreparedOwnership::load(workspace.repository.root(), owner)?;
    let Sides {
        merge_base,
        head,
        head_commit,
    } = sides(workspace.repository, &refs)?;
    let check_new_file_budgets = !workspace.configuration.new_file_budgets.is_empty();
    let check_per_file = workspace.configuration.regression_limits.per_file();
    let MeasuredHead {
        selected,
        measurement: head_measurement,
        new_files: new_files_measurement,
        changed_files,
    } = measure_head(
        workspace,
        merge_base,
        head_commit,
        package,
        result_cache,
        check_new_file_budgets,
        check_per_file,
    )?;
    let base_measurer = CommitMeasurer::for_package(
        workspace.repository,
        workspace.configuration,
        workspace.plugins,
        result_cache,
        selected.as_ref().map(|pkg| pkg.as_str()),
    );
    let base_measurement = base_measurer.measure_selected_package(merge_base, selected.as_ref())?;
    let baseline = Baseline::load_optional(workspace.repository.root())?;
    let mut report = DiffReport::compare_with_options(
        BaseCommit {
            git_ref: refs.base.to_owned(),
            merge_base: merge_base.to_string(),
        },
        head,
        base_measurement,
        head_measurement,
        DiffReportOptions {
            findings,
            budgets: &workspace.configuration.budgets,
            new_file_budgets: &workspace.configuration.new_file_budgets,
            new_files: new_files_measurement,
            changed_files,
            limits: &workspace.configuration.regression_limits,
            ownership: ownership.options(),
            baseline: baseline.as_ref(),
            include_test_gap_note: !workspace.configuration.coverage_reports.is_empty(),
        },
    );
    if let Some(baseline) = baseline.as_ref() {
        let mismatches = baseline.version_mismatches(&report.head_baseline_metrics());
        if !mismatches.is_empty() {
            return Err(CheckError::BaselineVersionMismatch {
                mismatches,
                supports_ignore_baseline: false,
            });
        }
    }
    crate::ratchet::enforce_if_present(
        workspace.repository.root(),
        &workspace.configuration.ratchet.path,
        package.is_some(),
        &mut report,
        DiffReport::set_ratchet_note,
        DiffReport::apply_ratchet,
    )?;
    Ok(report)
}

fn measure_head(
    context: WorkspaceRef<'_>,
    merge_base: CommitId,
    head_commit: Option<CommitId>,
    package: Option<&str>,
    result_cache: Option<&ResultCache>,
    check_new_file_budgets: bool,
    check_per_file: bool,
) -> Result<MeasuredHead, CheckError> {
    if !check_new_file_budgets && !check_per_file {
        let (selected, measurement) =
            measure_head_without_new_files(context, head_commit, package, result_cache)?;
        return Ok(MeasuredHead {
            selected,
            measurement,
            new_files: None,
            changed_files: BTreeSet::new(),
        });
    }
    let head_snapshot = open_head_snapshot(context.repository, head_commit)?;
    let base_snapshot = context.repository.commit(merge_base)?;
    let changed_files = collect_changed_files(&base_snapshot, &head_snapshot, check_per_file)?;
    if let Some(measured) = measure_head_if_new_files(
        context,
        &base_snapshot,
        &head_snapshot,
        package,
        check_new_file_budgets,
        &changed_files,
    )? {
        return Ok(measured);
    }
    let (selected, measurement) =
        measure_head_without_new_files(context, head_commit, package, result_cache)?;
    Ok(MeasuredHead {
        selected,
        measurement,
        new_files: None,
        changed_files,
    })
}

fn collect_changed_files(
    base_snapshot: &Snapshot,
    head_snapshot: &Snapshot,
    enabled: bool,
) -> Result<BTreeSet<String>, GitError> {
    if !enabled {
        return Ok(BTreeSet::new());
    }
    Ok(head_snapshot
        .changed_files_from(base_snapshot)?
        .into_iter()
        .map(|path| path.to_string())
        .collect())
}

fn measure_head_if_new_files(
    context: WorkspaceRef<'_>,
    base_snapshot: &Snapshot,
    head_snapshot: &Snapshot,
    package: Option<&str>,
    enabled: bool,
    changed_files: &BTreeSet<String>,
) -> Result<Option<MeasuredHead>, CheckError> {
    if !enabled {
        return Ok(None);
    }
    let new_files = newly_added_files(base_snapshot, head_snapshot);
    if new_files.is_empty() {
        return Ok(None);
    }
    let (selected, measurement, new_files) = measure_all_languages_with_new_files(
        head_snapshot,
        &new_files,
        context.plugins,
        context.configuration,
        package,
    )?;
    Ok(Some(MeasuredHead {
        selected,
        measurement,
        new_files,
        changed_files: changed_files.clone(),
    }))
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

fn newly_added_files(base_snapshot: &Snapshot, head_snapshot: &Snapshot) -> HashSet<RepoPath> {
    let base_files: HashSet<&RepoPath> = base_snapshot.files().iter().collect();
    head_snapshot
        .files()
        .iter()
        .filter(|path| !base_files.contains(path))
        .cloned()
        .collect()
}

fn measure_head_without_new_files(
    context: WorkspaceRef<'_>,
    head_commit: Option<CommitId>,
    package: Option<&str>,
    result_cache: Option<&ResultCache>,
) -> Result<(Option<SelectedPackage>, CommitMeasurement), CheckError> {
    let Some(commit) = head_commit else {
        return measure_all_languages_for_package(
            &context.repository.working_tree()?,
            context.plugins,
            context.configuration,
            package,
        );
    };
    if let Some(requested) = package {
        measure_all_languages_for_package(
            &context.repository.commit(commit)?,
            context.plugins,
            context.configuration,
            Some(requested),
        )
    } else {
        let measurer = CommitMeasurer::new(
            context.repository,
            context.configuration,
            context.plugins,
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
