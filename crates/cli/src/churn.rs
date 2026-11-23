use std::path::Path;

use codeint_config::{ChurnWindow, FindingsShown, PluginHostConfig};
use codeint_git::ChurnRange;
use codeint_report::{ChurnHistoryInput, ChurnReport};

use crate::check::{CheckError, WorkspaceContext, measure_with};

pub(crate) fn churn(
    folder: &Path,
    window: ChurnWindow,
    findings: FindingsShown,
    plugin_host: &PluginHostConfig,
) -> Result<ChurnReport, CheckError> {
    let context = WorkspaceContext::load(folder, plugin_host)?;
    let plugins = context.plugins.as_named();
    let history = context.repository.file_churn(churn_range(window))?;
    let tree = context.repository.working_tree()?;
    let languages = measure_with(&tree, &plugins, &context.configuration)?;
    let input = ChurnHistoryInput {
        window,
        commits_analyzed: history.commits_analyzed,
        shallow: history.shallow,
        touches_by_file: history.touches_by_file,
    };
    Ok(ChurnReport::summarize(languages, &input, findings))
}

pub(crate) fn churn_range(window: ChurnWindow) -> ChurnRange {
    match window {
        ChurnWindow::Days(days) => ChurnRange::Days(days),
        ChurnWindow::Since(day) => ChurnRange::Since {
            unix_seconds: day.start_unix_seconds(),
        },
        ChurnWindow::Commits(count) => ChurnRange::Commits(count),
    }
}
