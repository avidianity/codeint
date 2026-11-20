use std::fmt;
use std::path::Path;

use codeint_model::{RepoPath, SourceSet};
use globset::{GlobBuilder, GlobSet, GlobSetBuilder};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::Value;

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
    pub fn source_set(&self, path: &RepoPath, settled_by_rules: SourceSet) -> SourceSet {
        self.by_precedence
            .iter()
            .find(|(_, globs)| globs.is_match(path.as_str()))
            .map_or(settled_by_rules, |(source_set, _)| *source_set)
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
    #[error(
        "{CONFIGURATION_FILE_NAME} has an unknown key {key}; the keys allowed there are {}",
        allowed.join(", ")
    )]
    UnknownKey { key: KeyPath, allowed: Vec<String> },
    #[error(
        "{CONFIGURATION_FILE_NAME}: {key} must be {}, not {}",
        any_of(expected),
        found.with_article()
    )]
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
#[schemars(
    title = "codeint.json",
    description = "The configuration of codeint, read from codeint.json at the root of the repository."
)]
struct ConfigurationFile {
    #[serde(rename = "$schema", default)]
    #[schemars(
        description = "The address of this JSON schema, for editors; codeint does not read it."
    )]
    _schema: Option<String>,
    #[serde(default)]
    #[schemars(
        description = "Globs of repository-relative paths that put files into a Source set, whatever the Source set rules say. When globs of several Source sets match one file, the first of vendored, generated, test, and production wins."
    )]
    source_sets: SourceSetGlobs,
}

#[derive(Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct SourceSetGlobs {
    #[serde(default)]
    #[schemars(description = "Globs of files that belong to the Production Source set.")]
    production: Vec<String>,
    #[serde(default)]
    #[schemars(description = "Globs of files that belong to the Test Source set.")]
    test: Vec<String>,
    #[serde(default)]
    #[schemars(description = "Globs of files that belong to the Generated Source set.")]
    generated: Vec<String>,
    #[serde(default)]
    #[schemars(description = "Globs of files that belong to the Vendored Source set.")]
    vendored: Vec<String>,
}

pub fn read_configuration(repository_root: &Path) -> Result<Configuration, ConfigurationFileError> {
    match std::fs::read_to_string(repository_root.join(CONFIGURATION_FILE_NAME)) {
        Ok(text) => parse(&text),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Configuration::default()),
        Err(error) => Err(ConfigurationFileError::Unreadable(error)),
    }
}

pub fn configuration_schema() -> String {
    let schema = schemars::schema_for!(ConfigurationFile);
    format!("{:#}\n", schema.as_value())
}

fn parse(text: &str) -> Result<Configuration, ConfigurationFileError> {
    let value: Value = serde_json::from_str(text).map_err(ConfigurationFileError::NotJson)?;
    let schema = schemars::schema_for!(ConfigurationFile);
    shape::check_shape(&value, schema.as_value())?;
    let file: ConfigurationFile =
        serde_json::from_value(value).map_err(ConfigurationFileError::NotJson)?;
    Ok(Configuration {
        source_set_overrides: source_set_overrides(file.source_sets)?,
    })
}

fn source_set_overrides(
    globs: SourceSetGlobs,
) -> Result<SourceSetOverrides, ConfigurationFileError> {
    let SourceSetGlobs {
        production,
        test,
        generated,
        vendored,
    } = globs;
    let by_precedence = [
        (SourceSet::Vendored, "vendored", vendored),
        (SourceSet::Generated, "generated", generated),
        (SourceSet::Test, "test", test),
        (SourceSet::Production, "production", production),
    ]
    .into_iter()
    .filter(|(_, _, globs)| !globs.is_empty())
    .map(|(source_set, key, globs)| Ok((source_set, glob_set(key, &globs)?)))
    .collect::<Result<_, ConfigurationFileError>>()?;
    Ok(SourceSetOverrides { by_precedence })
}

fn glob_set(key: &str, globs: &[String]) -> Result<GlobSet, ConfigurationFileError> {
    let source_set_key = KeyPath::default()
        .child(KeySegment::Key("source_sets".to_owned()))
        .child(KeySegment::Key(key.to_owned()));
    let mut builder = GlobSetBuilder::new();
    for (index, glob) in globs.iter().enumerate() {
        let compiled = GlobBuilder::new(glob)
            .literal_separator(true)
            .backslash_escape(true)
            .build()
            .map_err(|source| ConfigurationFileError::InvalidGlob {
                key: source_set_key.child(KeySegment::Index(index)),
                source,
            })?;
        builder.add(compiled);
    }
    builder
        .build()
        .map_err(|source| ConfigurationFileError::InvalidGlob {
            key: source_set_key,
            source,
        })
}

impl KeyPath {
    fn child(&self, segment: KeySegment) -> KeyPath {
        let mut segments = self.0.clone();
        segments.push(segment);
        KeyPath(segments)
    }
}

impl fmt::Display for KeyPath {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (position, segment) in self.0.iter().enumerate() {
            match segment {
                KeySegment::Key(key) if position == 0 => write!(formatter, "{key}")?,
                KeySegment::Key(key) => write!(formatter, ".{key}")?,
                KeySegment::Index(index) => write!(formatter, "[{index}]")?,
            }
        }
        Ok(())
    }
}

impl JsonType {
    fn of(value: &Value) -> JsonType {
        match value {
            Value::Object(_) => JsonType::Object,
            Value::Array(_) => JsonType::Array,
            Value::String(_) => JsonType::String,
            Value::Number(number) if number.is_i64() || number.is_u64() => JsonType::Integer,
            Value::Number(_) => JsonType::Number,
            Value::Bool(_) => JsonType::Boolean,
            Value::Null => JsonType::Null,
        }
    }

    fn from_schema_name(name: &str) -> Option<JsonType> {
        match name {
            "object" => Some(JsonType::Object),
            "array" => Some(JsonType::Array),
            "string" => Some(JsonType::String),
            "number" => Some(JsonType::Number),
            "integer" => Some(JsonType::Integer),
            "boolean" => Some(JsonType::Boolean),
            "null" => Some(JsonType::Null),
            _ => None,
        }
    }

    fn accepts(self, value: &Value) -> bool {
        let found = JsonType::of(value);
        found == self || (self == JsonType::Number && found == JsonType::Integer)
    }

    fn with_article(self) -> &'static str {
        match self {
            JsonType::Object => "an object",
            JsonType::Array => "an array",
            JsonType::String => "a string",
            JsonType::Number => "a number",
            JsonType::Integer => "an integer",
            JsonType::Boolean => "a boolean",
            JsonType::Null => "null",
        }
    }
}

fn any_of(types: &[JsonType]) -> String {
    types
        .iter()
        .map(|json_type| json_type.with_article())
        .collect::<Vec<_>>()
        .join(" or ")
}
