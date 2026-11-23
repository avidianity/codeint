use std::path::Path;

use codeint_config::{ConfigOverrides, ConfigurationFileError, read_resolved_configuration};
use codeint_git::{GitError, Repository};
use codeint_report::ConfigReport;

#[derive(Debug, thiserror::Error)]
pub(crate) enum ConfigCommandError {
    #[error(transparent)]
    Git(#[from] GitError),
    #[error(transparent)]
    Configuration(#[from] ConfigurationFileError),
}

pub(crate) fn resolved_config(
    folder: &Path,
    overrides: &ConfigOverrides,
) -> Result<ConfigReport, ConfigCommandError> {
    let repository = Repository::discover(folder)?;
    let resolved = read_resolved_configuration(repository.root(), overrides)?;
    Ok(ConfigReport::from_resolved(&resolved))
}
