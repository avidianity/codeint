use std::io::Write;
use std::path::{Path, PathBuf};

use codeint_model::RepoPath;

use super::{CONFIGURATION_FILE_NAME, ConfigurationFile, LanguageGlobs};

#[cfg(test)]
mod tests;

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
    format!(
        "https://raw.githubusercontent.com/avidianity/codeint/v{}/schema/codeint.schema.json",
        env!("CARGO_PKG_VERSION")
    )
}

pub fn write_starter_configuration(
    repository_root: &Path,
    languages: &[DetectedLanguage],
) -> Result<PathBuf, StarterConfigurationError> {
    let path = repository_root.join(CONFIGURATION_FILE_NAME);
    let text = starter_configuration_text(languages);
    let unwritable = |source| StarterConfigurationError::Unwritable {
        path: path.clone(),
        source,
    };
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .map_err(|source| {
            if source.kind() == std::io::ErrorKind::AlreadyExists {
                StarterConfigurationError::AlreadyExists { path: path.clone() }
            } else {
                unwritable(source)
            }
        })?;
    file.write_all(text.as_bytes())
        .and_then(|()| file.flush())
        .map_err(unwritable)?;
    Ok(path)
}

fn starter_configuration_text(languages: &[DetectedLanguage]) -> String {
    let starter = ConfigurationFile {
        _schema: Some(published_schema_address()),
        source_sets: Default::default(),
        budgets: Default::default(),
        regression_limits: Default::default(),
        plugins: Vec::new(),
        plugin_limits: Default::default(),
        boundaries: None,
        duplication: Default::default(),
        languages: (!languages.is_empty()).then(|| {
            languages
                .iter()
                .map(|language| LanguageGlobs {
                    plugin: language.plugin.clone(),
                    include: language
                        .include
                        .iter()
                        .map(|path| globset::escape(path.as_str()))
                        .collect(),
                    exclude: Vec::new(),
                })
                .collect()
        }),
    };
    let json = serde_json::to_value(&starter)
        .expect("the configuration types serialize to JSON without fallible keys");
    format!("{json:#}\n")
}
