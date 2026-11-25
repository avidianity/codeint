use std::path::{Path, PathBuf};

use codeint_config::{PluginKind, PluginLimits};
use codeint_plugin_host::{ConformanceSource, ConformanceStatus, PluginHostError, run_conformance};
use codeint_report::{PluginTestCheck, PluginTestReport, PluginTestStatus};

#[derive(Debug, thiserror::Error)]
pub(crate) enum PluginTestError {
    #[error(transparent)]
    PluginHost(#[from] PluginHostError),
    #[error("cannot read source file {}", path.display())]
    Source {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

pub(crate) fn plugin_test(
    path: &Path,
    kind: PluginKind,
    source_paths: &[PathBuf],
    limits: PluginLimits,
) -> Result<PluginTestReport, PluginTestError> {
    let sources = source_paths
        .iter()
        .map(PathBuf::as_path)
        .map(read_source)
        .collect::<Result<Vec<_>, _>>()?;
    let conformance = run_conformance(kind, path, limits, &sources)?;
    let checks = conformance
        .checks
        .into_iter()
        .map(|check| PluginTestCheck {
            id: check.id.to_owned(),
            status: match check.status {
                ConformanceStatus::Pass => PluginTestStatus::Pass,
                ConformanceStatus::Fail => PluginTestStatus::Fail,
                ConformanceStatus::Skipped => PluginTestStatus::Skipped,
            },
            message: check.message,
            hint: check.hint,
        })
        .collect();
    let kind_name = match kind {
        PluginKind::Language => "language",
        PluginKind::Metric => "metric",
    };
    Ok(PluginTestReport::new(
        path.display().to_string(),
        kind_name,
        checks,
    ))
}

fn read_source(path: &Path) -> Result<ConformanceSource, PluginTestError> {
    let text = std::fs::read_to_string(path).map_err(|source| PluginTestError::Source {
        path: path.to_path_buf(),
        source,
    })?;
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("source")
        .to_owned();
    Ok(ConformanceSource { file_name, text })
}
