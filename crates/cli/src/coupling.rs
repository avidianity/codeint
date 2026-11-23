use std::collections::BTreeSet;
use std::path::Path;

use codeint_config::{CouplingSettings, FindingsShown, PluginHostConfig};
use codeint_model::{CodeModel, RepoPath, SourceSet};
use codeint_report::{CouplingHistoryInput, CouplingReport};

use crate::check::{CheckError, WorkspaceContext};
use crate::churn::churn_range;

pub(crate) fn coupling(
    folder: &Path,
    settings: CouplingSettings,
    findings: FindingsShown,
    plugin_host: &PluginHostConfig,
) -> Result<CouplingReport, CheckError> {
    let context = WorkspaceContext::load(folder, plugin_host)?;
    let model = context.measure_working_tree_model()?;
    let allowed_paths = selected_source_paths(&model, settings.include_tests);
    let history = context.repository.co_change_coupling(
        churn_range(settings.window),
        settings.max_commit_files,
        &allowed_paths,
    )?;
    let input = CouplingHistoryInput {
        window: settings.window,
        commits_analyzed: history.commits_analyzed,
        commits_excluded_by_bulk_limit: history.commits_excluded_by_bulk_limit,
        max_commit_files: settings.max_commit_files,
        min_co_changes: settings.min_co_changes,
        min_ratio: settings.min_ratio.value(),
        include_tests: settings.include_tests,
        shallow: history.shallow,
        touches_by_file: history.touches_by_file,
        co_changes: history.co_changes,
    };
    Ok(CouplingReport::summarize(&model, &input, findings))
}

fn selected_source_paths(model: &CodeModel, include_tests: bool) -> BTreeSet<RepoPath> {
    model
        .files
        .iter()
        .filter(|file| match file.model.source_set {
            SourceSet::Production => true,
            SourceSet::Test => include_tests,
            SourceSet::Generated | SourceSet::Vendored => false,
        })
        .map(|file| file.path.clone())
        .collect()
}
