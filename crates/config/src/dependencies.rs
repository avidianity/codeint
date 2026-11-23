use std::collections::BTreeSet;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::config_error::{ConfigFileKind, ConfigurationFileError, KeyPath, KeySegment};

#[cfg(test)]
mod tests;

#[derive(Debug, Clone, Default, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct DependenciesFile {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[schemars(
        description = "Canonical external package names excluded from unused, unlisted, and dev-in-production dependency Findings."
    )]
    ignore: Vec<String>,
}

impl DependenciesFile {
    pub(crate) fn is_unset(&self) -> bool {
        self.ignore.is_empty()
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DependenciesConfig {
    ignore: BTreeSet<String>,
}

impl DependenciesConfig {
    pub(crate) fn from_file(
        file: DependenciesFile,
    ) -> Result<DependenciesConfig, ConfigurationFileError> {
        let ignore_key = KeyPath::default()
            .child(KeySegment::Key("dependencies".to_owned()))
            .child(KeySegment::Key("ignore".to_owned()));
        let mut ignore = BTreeSet::new();
        for (index, entry) in file.ignore.into_iter().enumerate() {
            let trimmed = entry.trim();
            if trimmed.is_empty() {
                return Err(ConfigurationFileError::EmptyDependencyIgnore {
                    file: ConfigFileKind::Json,
                    key: ignore_key.child(KeySegment::Index(index)),
                });
            }
            ignore.insert(trimmed.to_owned());
        }
        Ok(DependenciesConfig { ignore })
    }

    pub fn ignored(&self) -> &BTreeSet<String> {
        &self.ignore
    }

    pub fn is_ignored(&self, dependency: &str) -> bool {
        self.ignore.contains(dependency)
    }
}
