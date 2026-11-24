use std::fmt;

use codeint_model::SourceSet;
use globset::{Glob, GlobBuilder, GlobSet, GlobSetBuilder};
use serde_json::Value;

pub(crate) const CONFIGURATION_FILE_NAME: &str = "codeint.json";
pub(crate) const CONFIGURATION_JSONC_FILE_NAME: &str = "codeint.jsonc";

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{message} at line {line} column {column}")]
pub struct JsoncError {
    pub message: String,
    pub line: usize,
    pub column: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ConfigFileKind {
    #[default]
    Json,
    Jsonc,
}

impl ConfigFileKind {
    pub fn file_name(self) -> &'static str {
        match self {
            ConfigFileKind::Json => CONFIGURATION_FILE_NAME,
            ConfigFileKind::Jsonc => CONFIGURATION_JSONC_FILE_NAME,
        }
    }
}

impl fmt::Display for ConfigFileKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.file_name())
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
    #[error(
        "both {CONFIGURATION_FILE_NAME} and {CONFIGURATION_JSONC_FILE_NAME} exist at the repository root; keep only one"
    )]
    AmbiguousConfigFiles,
    #[error("cannot read {file}")]
    Unreadable {
        file: ConfigFileKind,
        #[source]
        source: std::io::Error,
    },
    #[error("{file} is a broken symlink")]
    BrokenSymlink { file: ConfigFileKind },
    #[error("{file} is not valid JSON")]
    NotJson {
        file: ConfigFileKind,
        #[source]
        source: serde_json::Error,
    },
    #[error("{file} is not valid JSON")]
    InvalidJsonc {
        file: ConfigFileKind,
        #[source]
        source: JsoncError,
    },
    #[error(
        "{file} has an unknown key {key}; the keys allowed there are {}",
        allowed.join(", ")
    )]
    UnknownKey {
        file: ConfigFileKind,
        key: KeyPath,
        allowed: Vec<String>,
    },
    #[error(
        "--budget {}.{metric} names an unknown Metric {metric:?}; the Metrics allowed are {}",
        source_set.key(),
        allowed.join(", ")
    )]
    UnknownCliBudgetMetric {
        source_set: SourceSet,
        metric: String,
        allowed: Vec<String>,
    },
    #[error(
        "{file}: {key} must be {}, not {}",
        any_of(expected),
        found.with_article()
    )]
    WrongType {
        file: ConfigFileKind,
        key: KeyPath,
        expected: Vec<JsonType>,
        found: JsonType,
    },
    #[error("{file}: {key} must be between {minimum} and {maximum}, not {found}")]
    OutOfRange {
        file: ConfigFileKind,
        key: KeyPath,
        minimum: f64,
        maximum: f64,
        found: f64,
    },
    #[error(
        "{file}: {key} must be greater than {exclusive_minimum} and at most {maximum}, not {found}"
    )]
    OutOfExclusiveMinRange {
        file: ConfigFileKind,
        key: KeyPath,
        exclusive_minimum: f64,
        maximum: f64,
        found: f64,
    },
    #[error(
        "{file}: {key} must be one of {}, not {found}",
        allowed.join(", ")
    )]
    NotOneOf {
        file: ConfigFileKind,
        key: KeyPath,
        allowed: Vec<String>,
        found: Box<Value>,
    },
    #[error("{file}: {key} is not a valid glob")]
    InvalidGlob {
        file: ConfigFileKind,
        key: KeyPath,
        #[source]
        source: globset::Error,
    },
    #[error("{file}: {key} needs the key {missing}")]
    MissingKey {
        file: ConfigFileKind,
        key: KeyPath,
        missing: String,
    },
    #[error(
        "{file} names an unknown Language plugin {name} at {key}; the Language plugins are {}",
        known.join(", ")
    )]
    UnknownLanguagePlugin {
        file: ConfigFileKind,
        key: KeyPath,
        name: String,
        known: Vec<String>,
    },
    #[error("--source-set {}: {glob:?} is not a valid glob", source_set.key())]
    InvalidCliGlob {
        source_set: SourceSet,
        glob: String,
        #[source]
        source: globset::Error,
    },
    #[error("{file}: {key} must be a 64-character hexadecimal SHA-256 digest, not {found:?}")]
    InvalidSha256 {
        file: ConfigFileKind,
        key: KeyPath,
        found: String,
    },
    #[error("{file}: {key} must not be empty")]
    EmptyPluginField { file: ConfigFileKind, key: KeyPath },
    #[error("{file}: {key} for Plugin {name:?} must set either path or url")]
    MissingPluginSource {
        file: ConfigFileKind,
        key: KeyPath,
        name: String,
    },
    #[error("{file}: {key} for Plugin {name:?} must set either path or url, not both")]
    AmbiguousPluginSource {
        file: ConfigFileKind,
        key: KeyPath,
        name: String,
    },
    #[error("{file}: {key} for Plugin {name:?} must use an HTTPS URL (https://), not {url:?}")]
    NonHttpsPluginUrl {
        file: ConfigFileKind,
        key: KeyPath,
        name: String,
        url: String,
    },
    #[error(
        "{file} targets codeint v{config_version} via $schema, which is newer than this binary (v{binary_version})"
    )]
    NewerSchemaVersion {
        file: ConfigFileKind,
        config_version: String,
        binary_version: String,
    },
    #[error("{file}: {key} has invalid boundary zone name {name:?}")]
    InvalidBoundaryZoneName {
        file: ConfigFileKind,
        key: KeyPath,
        name: String,
    },
    #[error("{file}: {key} for boundary zone {zone:?} must list at least one glob pattern")]
    EmptyBoundaryZonePatterns {
        file: ConfigFileKind,
        key: KeyPath,
        zone: String,
    },
    #[error(
        "{file}: {key} references unknown boundary zone {zone:?}; known zones are {}",
        known.join(", ")
    )]
    UnknownBoundaryZone {
        file: ConfigFileKind,
        key: KeyPath,
        zone: String,
        known: Vec<String>,
    },
    #[error("{file}: {key} for boundary zone {from:?} must set either allow or deny, not both")]
    ConflictingBoundaryRule {
        file: ConfigFileKind,
        key: KeyPath,
        from: String,
    },
    #[error("{file}: {key} for boundary zone {from:?} must set either allow or deny")]
    MissingBoundaryRuleDirective {
        file: ConfigFileKind,
        key: KeyPath,
        from: String,
    },
    #[error("{file}: {key} mixes allow and deny directives for boundary zone {from:?}")]
    ConflictingBoundaryDirectives {
        file: ConfigFileKind,
        key: KeyPath,
        from: String,
    },
    #[error("{file}: {key} must not be empty")]
    EmptyDependencyIgnore { file: ConfigFileKind, key: KeyPath },
    #[error("{file}: {key} must not be empty")]
    EmptyCoverageReportPath { file: ConfigFileKind, key: KeyPath },
    #[error(
        "{file}: {key} must be a path relative to the repository root, not an absolute path ({path:?})"
    )]
    AbsoluteCoverageReportPath {
        file: ConfigFileKind,
        key: KeyPath,
        path: String,
    },
    #[error("{file}: {key} must stay inside the repository and must not contain \"..\" ({path:?})")]
    ParentTraversalCoverageReportPath {
        file: ConfigFileKind,
        key: KeyPath,
        path: String,
    },
    #[error("{file}: {key} is not supported; Test gap covers only the Production Source set")]
    TestGapBudgetOutsideProduction { file: ConfigFileKind, key: KeyPath },
    #[error(
        "--budget {}.test_gap is not supported; Test gap covers only the Production Source set",
        source_set.key()
    )]
    CliTestGapBudgetOutsideProduction { source_set: SourceSet },
    #[error("{file}: {key} is not supported; Test gap has no Regression check")]
    TestGapRegressionLimit { file: ConfigFileKind, key: KeyPath },
}

impl ConfigurationFileError {
    pub(crate) fn with_file_kind(mut self, file_kind: ConfigFileKind) -> Self {
        use ConfigurationFileError::*;
        match &mut self {
            Unreadable { file, .. }
            | BrokenSymlink { file }
            | NotJson { file, .. }
            | InvalidJsonc { file, .. }
            | UnknownKey { file, .. }
            | WrongType { file, .. }
            | OutOfRange { file, .. }
            | OutOfExclusiveMinRange { file, .. }
            | NotOneOf { file, .. }
            | MissingKey { file, .. }
            | UnknownLanguagePlugin { file, .. }
            | InvalidGlob { file, .. }
            | InvalidSha256 { file, .. }
            | EmptyPluginField { file, .. }
            | MissingPluginSource { file, .. }
            | AmbiguousPluginSource { file, .. }
            | NonHttpsPluginUrl { file, .. }
            | NewerSchemaVersion { file, .. }
            | InvalidBoundaryZoneName { file, .. }
            | EmptyBoundaryZonePatterns { file, .. }
            | UnknownBoundaryZone { file, .. }
            | ConflictingBoundaryRule { file, .. }
            | MissingBoundaryRuleDirective { file, .. }
            | ConflictingBoundaryDirectives { file, .. }
            | EmptyDependencyIgnore { file, .. }
            | EmptyCoverageReportPath { file, .. }
            | AbsoluteCoverageReportPath { file, .. }
            | ParentTraversalCoverageReportPath { file, .. }
            | TestGapBudgetOutsideProduction { file, .. }
            | TestGapRegressionLimit { file, .. } => *file = file_kind,
            AmbiguousConfigFiles
            | UnknownCliBudgetMetric { .. }
            | CliTestGapBudgetOutsideProduction { .. }
            | InvalidCliGlob { .. } => {}
        }
        self
    }
}

pub(crate) fn compile_glob(glob: &str) -> Result<Glob, globset::Error> {
    GlobBuilder::new(glob)
        .literal_separator(true)
        .backslash_escape(true)
        .build()
}

pub(crate) fn glob_set(key: &KeyPath, globs: &[String]) -> Result<GlobSet, ConfigurationFileError> {
    let mut builder = GlobSetBuilder::new();
    for (index, glob) in globs.iter().enumerate() {
        let compiled =
            compile_glob(glob).map_err(|source| ConfigurationFileError::InvalidGlob {
                file: ConfigFileKind::Json,
                key: key.child(KeySegment::Index(index)),
                source,
            })?;
        builder.add(compiled);
    }
    builder
        .build()
        .map_err(|source| ConfigurationFileError::InvalidGlob {
            file: ConfigFileKind::Json,
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
