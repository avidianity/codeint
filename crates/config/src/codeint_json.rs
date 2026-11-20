use std::path::Path;

use codeint_model::{RepoPath, SourceSet};
use globset::GlobSet;
use schemars::JsonSchema;
use serde::Deserialize;

mod shape;

pub const CONFIGURATION_FILE_NAME: &str = "codeint.json";

#[derive(Debug, Default)]
pub struct Configuration {
    pub source_set_overrides: SourceSetOverrides,
}

#[derive(Debug, Default)]
pub struct SourceSetOverrides {
    by_precedence: Vec<(SourceSet, GlobSet)>,
}

impl SourceSetOverrides {
    pub fn source_set(&self, path: &RepoPath, assigned_by_rules: SourceSet) -> SourceSet {
        todo!("{path:?} {assigned_by_rules:?} {:?}", self.by_precedence)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JsonType {
    Object,
    Array,
    String,
    Number,
    Integer,
    Boolean,
    Null,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeySegment {
    Key(String),
    Index(usize),
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct KeyPath(Vec<KeySegment>);

#[derive(Debug, thiserror::Error)]
pub enum ConfigurationFileError {
    #[error("cannot read {CONFIGURATION_FILE_NAME}")]
    Unreadable(#[source] std::io::Error),
    #[error("{CONFIGURATION_FILE_NAME} is not valid JSON")]
    NotJson(#[source] serde_json::Error),
    #[error("{CONFIGURATION_FILE_NAME} has an unknown key {key}")]
    UnknownKey { key: KeyPath, allowed: Vec<String> },
    #[error("{CONFIGURATION_FILE_NAME}: {key} must be {expected:?}, not {found:?}")]
    WrongType {
        key: KeyPath,
        expected: Vec<JsonType>,
        found: JsonType,
    },
    #[error("{CONFIGURATION_FILE_NAME}: {key} is not a valid glob")]
    InvalidGlob {
        key: KeyPath,
        #[source]
        source: globset::Error,
    },
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ConfigurationFile {
    #[serde(rename = "$schema", default)]
    schema: Option<String>,
    #[serde(default)]
    source_sets: SourceSetGlobs,
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct SourceSetGlobs {
    #[serde(default)]
    production: Vec<String>,
    #[serde(default)]
    test: Vec<String>,
    #[serde(default)]
    generated: Vec<String>,
    #[serde(default)]
    vendored: Vec<String>,
}

pub fn read_configuration(repository_root: &Path) -> Result<Configuration, ConfigurationFileError> {
    todo!("{repository_root:?} {:?}", parse as fn(&str) -> _)
}

pub fn configuration_schema() -> String {
    todo!("{}", schemars::schema_for!(ConfigurationFile).as_value())
}

fn parse(text: &str) -> Result<Configuration, ConfigurationFileError> {
    let value: serde_json::Value =
        serde_json::from_str(text).map_err(ConfigurationFileError::NotJson)?;
    shape::check_shape(
        &value,
        schemars::schema_for!(ConfigurationFile).as_value(),
        &KeyPath::default(),
    )?;
    let file: ConfigurationFile =
        serde_json::from_value(value).map_err(ConfigurationFileError::NotJson)?;
    let SourceSetGlobs {
        production,
        test,
        generated,
        vendored,
    } = file.source_sets;
    todo!(
        "{:?} {production:?} {test:?} {generated:?} {vendored:?}",
        file.schema
    )
}

impl std::fmt::Display for KeyPath {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{:?}", self.0)
    }
}
