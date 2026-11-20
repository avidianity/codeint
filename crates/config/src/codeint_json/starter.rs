use std::path::{Path, PathBuf};

use codeint_model::RepoPath;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DetectedLanguage {
    pub plugin: String,
    pub include: Vec<RepoPath>,
}

#[derive(Debug, thiserror::Error)]
pub enum StarterConfigurationError {
    #[error("{} already exists, and codeint init never replaces it", path.display())]
    AlreadyExists { path: PathBuf },
    #[error("cannot write {}", path.display())]
    Unwritable {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

pub fn published_schema_address() -> String {
    todo!()
}

pub fn write_starter_configuration(
    _repository_root: &Path,
    _languages: &[DetectedLanguage],
) -> Result<PathBuf, StarterConfigurationError> {
    todo!()
}
