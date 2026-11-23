use std::fmt;

use globset::{GlobBuilder, GlobSet, GlobSetBuilder};
use serde_json::Value;

pub(crate) const CONFIGURATION_FILE_NAME: &str = "codeint.json";

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
    #[error(
        "{CONFIGURATION_FILE_NAME}: {key} must be between {minimum} and {maximum}, not {found}"
    )]
    OutOfRange {
        key: KeyPath,
        minimum: f64,
        maximum: f64,
        found: f64,
    },
    #[error(
        "{CONFIGURATION_FILE_NAME}: {key} must be one of {}, not {found}",
        allowed.join(", ")
    )]
    NotOneOf {
        key: KeyPath,
        allowed: Vec<String>,
        found: Value,
    },
    #[error("{CONFIGURATION_FILE_NAME}: {key} needs the key {missing}")]
    MissingKey { key: KeyPath, missing: String },
    #[error(
        "{CONFIGURATION_FILE_NAME} names an unknown Language plugin {name} at {key}; the Language plugins are {}",
        known.join(", ")
    )]
    UnknownLanguagePlugin {
        key: KeyPath,
        name: String,
        known: Vec<String>,
    },
    #[error("{CONFIGURATION_FILE_NAME}: {key} is not a valid glob")]
    InvalidGlob {
        key: KeyPath,
        #[source]
        source: globset::Error,
    },
    #[error(
        "{CONFIGURATION_FILE_NAME}: {key} must be a 64-character hexadecimal SHA-256 digest, not {found:?}"
    )]
    InvalidSha256 { key: KeyPath, found: String },
    #[error("{CONFIGURATION_FILE_NAME}: {key} must not be empty")]
    EmptyPluginField { key: KeyPath },
    #[error("{CONFIGURATION_FILE_NAME}: {key} for Plugin {name:?} must set either path or url")]
    MissingPluginSource { key: KeyPath, name: String },
    #[error(
        "{CONFIGURATION_FILE_NAME}: {key} for Plugin {name:?} must set either path or url, not both"
    )]
    AmbiguousPluginSource { key: KeyPath, name: String },
    #[error(
        "{CONFIGURATION_FILE_NAME}: {key} for Plugin {name:?} must use an HTTPS URL (https://), not {url:?}"
    )]
    NonHttpsPluginUrl {
        key: KeyPath,
        name: String,
        url: String,
    },
    #[error(
        "{CONFIGURATION_FILE_NAME} targets codeint v{config_version} via $schema, which is newer than this binary (v{binary_version})"
    )]
    NewerSchemaVersion {
        config_version: String,
        binary_version: String,
    },
    #[error("{CONFIGURATION_FILE_NAME}: {key} has invalid boundary zone name {name:?}")]
    InvalidBoundaryZoneName { key: KeyPath, name: String },
    #[error(
        "{CONFIGURATION_FILE_NAME}: {key} for boundary zone {zone:?} must list at least one glob pattern"
    )]
    EmptyBoundaryZonePatterns { key: KeyPath, zone: String },
    #[error(
        "{CONFIGURATION_FILE_NAME}: {key} references unknown boundary zone {zone:?}; known zones are {}",
        known.join(", ")
    )]
    UnknownBoundaryZone {
        key: KeyPath,
        zone: String,
        known: Vec<String>,
    },
    #[error(
        "{CONFIGURATION_FILE_NAME}: {key} for boundary zone {from:?} must set either allow or deny, not both"
    )]
    ConflictingBoundaryRule { key: KeyPath, from: String },
    #[error(
        "{CONFIGURATION_FILE_NAME}: {key} for boundary zone {from:?} must set either allow or deny"
    )]
    MissingBoundaryRuleDirective { key: KeyPath, from: String },
    #[error(
        "{CONFIGURATION_FILE_NAME}: {key} mixes allow and deny directives for boundary zone {from:?}"
    )]
    ConflictingBoundaryDirectives { key: KeyPath, from: String },
}

pub(crate) fn glob_set(key: &KeyPath, globs: &[String]) -> Result<GlobSet, ConfigurationFileError> {
    let mut builder = GlobSetBuilder::new();
    for (index, glob) in globs.iter().enumerate() {
        let compiled = GlobBuilder::new(glob)
            .literal_separator(true)
            .backslash_escape(true)
            .build()
            .map_err(|source| ConfigurationFileError::InvalidGlob {
                key: key.child(KeySegment::Index(index)),
                source,
            })?;
        builder.add(compiled);
    }
    builder
        .build()
        .map_err(|source| ConfigurationFileError::InvalidGlob {
            key: key.clone(),
            source,
        })
}

impl KeyPath {
    pub(crate) fn child(&self, segment: KeySegment) -> KeyPath {
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
    pub(crate) fn of(value: &Value) -> JsonType {
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

    pub(crate) fn from_schema_name(name: &str) -> Option<JsonType> {
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

    pub(crate) fn accepts(self, value: &Value) -> bool {
        let found = JsonType::of(value);
        found == self || (self == JsonType::Number && found == JsonType::Integer)
    }

    pub(crate) fn with_article(self) -> &'static str {
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
