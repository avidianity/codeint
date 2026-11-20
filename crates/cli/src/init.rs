use std::path::{Path, PathBuf};

use codeint_config::{DetectedLanguage, StarterConfigurationError};
use codeint_git::GitError;
use codeint_model::RepoPath;

use crate::check::NamedLanguagePlugin;

#[derive(Debug, thiserror::Error)]
pub(crate) enum InitError {
    #[error(transparent)]
    Git(#[from] GitError),
    #[error(transparent)]
    StarterConfiguration(#[from] StarterConfigurationError),
}

pub(crate) fn init(_folder: &Path) -> Result<PathBuf, InitError> {
    let _detected = detect_languages(&[], &[]);
    todo!()
}

fn detect_languages(
    _files: &[RepoPath],
    _plugins: &[NamedLanguagePlugin<'_>],
) -> Vec<DetectedLanguage> {
    todo!()
}
