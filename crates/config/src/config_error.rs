use std::fmt;

use codeint_model::{Metric, SourceSet};
use globset::{Glob, GlobBuilder, GlobSet, GlobSetBuilder};
use serde_json::Value;

pub(crate) const CONFIGURATION_FILE_NAME: &str = "codeint.json";

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{message} at line {line} column {column}")]
pub struct JsoncError {
    pub message: String,
    pub line: usize,
    pub column: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigSyntax {
    Json,
    Toml,
    Yaml,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ConfigFileKind {
    #[default]
    Json,
    Jsonc,
    Toml,
    Yaml,
    Yml,
    DotCodeintJson,
    DotCodeintJsonc,
    DotCodeintToml,
    DotCodeintYaml,
    DotCodeintYml,
}

impl ConfigFileKind {
    pub const ALL: [ConfigFileKind; 10] = [
        ConfigFileKind::Json,
        ConfigFileKind::Jsonc,
        ConfigFileKind::Toml,
        ConfigFileKind::Yaml,
        ConfigFileKind::Yml,
        ConfigFileKind::DotCodeintJson,
        ConfigFileKind::DotCodeintJsonc,
        ConfigFileKind::DotCodeintToml,
        ConfigFileKind::DotCodeintYaml,
        ConfigFileKind::DotCodeintYml,
    ];

    pub fn file_name(self) -> &'static str {
        match self {
            ConfigFileKind::Json => CONFIGURATION_FILE_NAME,
            ConfigFileKind::Jsonc => "codeint.jsonc",
            ConfigFileKind::Toml => "codeint.toml",
            ConfigFileKind::Yaml => "codeint.yaml",
            ConfigFileKind::Yml => "codeint.yml",
            ConfigFileKind::DotCodeintJson => ".codeint/config.json",
            ConfigFileKind::DotCodeintJsonc => ".codeint/config.jsonc",
            ConfigFileKind::DotCodeintToml => ".codeint/config.toml",
            ConfigFileKind::DotCodeintYaml => ".codeint/config.yaml",
            ConfigFileKind::DotCodeintYml => ".codeint/config.yml",
        }
    }

    pub fn syntax(self) -> ConfigSyntax {
        match self {
            ConfigFileKind::Json
            | ConfigFileKind::Jsonc
            | ConfigFileKind::DotCodeintJson
            | ConfigFileKind::DotCodeintJsonc => ConfigSyntax::Json,
            ConfigFileKind::Toml | ConfigFileKind::DotCodeintToml => ConfigSyntax::Toml,
            ConfigFileKind::Yaml
            | ConfigFileKind::Yml
            | ConfigFileKind::DotCodeintYaml
            | ConfigFileKind::DotCodeintYml => ConfigSyntax::Yaml,
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
    #[error("{}", format_ambiguous_config_files(files))]
    AmbiguousConfigFiles { files: Vec<ConfigFileKind> },
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
    #[error("{file} is not valid TOML: {message}")]
    InvalidToml {
        file: ConfigFileKind,
        message: String,
    },
    #[error("{file} is not valid YAML: {message}")]
    InvalidYaml {
        file: ConfigFileKind,
        message: String,
    },
    #[error(
        "{file}: {key} uses a TOML date-time value, which codeint configuration does not support"
    )]
    UnsupportedTomlDatetime { file: ConfigFileKind, key: KeyPath },
    #[error(
        "{file} specifies conflicting schema URLs in {first_label} ({first_url:?}) and {second_label} ({second_url:?})"
    )]
    ConflictingSchemaDirectives {
        file: ConfigFileKind,
        first_label: &'static str,
        first_url: String,
        second_label: &'static str,
        second_url: String,
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
    #[error(
        "{file}: {key} for boundary zone {from:?} must set either allow_external or deny_external, not both"
    )]
    ConflictingExternalBoundaryRule {
        file: ConfigFileKind,
        key: KeyPath,
        from: String,
    },
    #[error(
        "{file}: {key} for boundary zone {from:?} must set at least one of allow, deny, allow_external, or deny_external"
    )]
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
    #[error(
        "{file}: {key} mixes allow_external and deny_external directives for boundary zone {from:?}"
    )]
    ConflictingExternalBoundaryDirectives {
        file: ConfigFileKind,
        key: KeyPath,
        from: String,
    },
    #[error("{file}: {key} must not be empty")]
    EmptyBoundaryExternalPattern { file: ConfigFileKind, key: KeyPath },
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
    #[error(
        "{file}: {key} is not supported; {} applies to the Production Source set only",
        metric.display_name()
    )]
    MetricBudgetOutsideProduction {
        file: ConfigFileKind,
        key: KeyPath,
        metric: Metric,
    },
    #[error(
        "--budget {}.{} is not supported; {} applies to the Production Source set only",
        source_set.key(),
        metric.key(),
        metric.display_name()
    )]
    CliMetricBudgetOutsideProduction {
        source_set: SourceSet,
        metric: Metric,
    },
    #[error(
        "{file}: {key} is not supported; {} applies to the Production Source set only",
        metric.display_name()
    )]
    MetricRegressionLimitOutsideProduction {
        file: ConfigFileKind,
        key: KeyPath,
        metric: Metric,
    },
    #[error("{file}: {key} for SQL dialect {dialect:?} must list at least one glob pattern")]
    EmptySqlDialectInclude {
        file: ConfigFileKind,
        key: KeyPath,
        dialect: String,
    },
    #[error("{file}: {key} must not be empty")]
    EmptySqlDialectPattern { file: ConfigFileKind, key: KeyPath },
    #[error("{file}: {key} must not be empty")]
    EmptyHazardRuleName { file: ConfigFileKind, key: KeyPath },
    #[error("{}", format_unknown_framework_profile(*file, key, name, known))]
    UnknownFrameworkProfile {
        file: ConfigFileKind,
        key: KeyPath,
        name: String,
        known: Vec<String>,
    },
    #[error("{file}: {key} must not be empty")]
    EmptyFrameworkProfileName { file: ConfigFileKind, key: KeyPath },
    #[error("{file}: {key} must set either exclude or only, not both")]
    ConflictingFrameworkProfileRule { file: ConfigFileKind, key: KeyPath },
    #[error("{file}: {key} must set either exclude or only")]
    EmptyFrameworkProfileObject { file: ConfigFileKind, key: KeyPath },
    #[error("{file}: {key} has invalid Workspace package path {package:?}")]
    InvalidPackageConfigKey {
        file: ConfigFileKind,
        key: KeyPath,
        package: String,
    },
    #[error("{file}: {key} duplicates Workspace package path {package:?}")]
    DuplicatePackageConfigKey {
        file: ConfigFileKind,
        key: KeyPath,
        package: String,
    },
    #[error("{file}: {key} must not be empty")]
    EmptyRatchetPath { file: ConfigFileKind, key: KeyPath },
    #[error(
        "{file}: {key} must be a path relative to the repository root, not an absolute path ({path:?})"
    )]
    AbsoluteRatchetPath {
        file: ConfigFileKind,
        key: KeyPath,
        path: String,
    },
    #[error("{file}: {key} must stay inside the repository and must not contain \"..\" ({path:?})")]
    ParentTraversalRatchetPath {
        file: ConfigFileKind,
        key: KeyPath,
        path: String,
    },
}

fn format_ambiguous_config_files(files: &[ConfigFileKind]) -> String {
    let listed = files
        .iter()
        .map(|file| file.file_name())
        .collect::<Vec<_>>()
        .join(", ");
    format!("multiple configuration files exist ({listed}); keep only one")
}

impl ConfigurationFileError {
    pub(crate) fn with_file_kind(mut self, file_kind: ConfigFileKind) -> Self {
        use ConfigurationFileError::*;
        match &mut self {
            Unreadable { file, .. }
            | BrokenSymlink { file }
            | NotJson { file, .. }
            | InvalidJsonc { file, .. }
            | InvalidToml { file, .. }
            | InvalidYaml { file, .. }
            | UnsupportedTomlDatetime { file, .. }
            | ConflictingSchemaDirectives { file, .. }
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
            | ConflictingExternalBoundaryRule { file, .. }
            | MissingBoundaryRuleDirective { file, .. }
            | ConflictingBoundaryDirectives { file, .. }
            | ConflictingExternalBoundaryDirectives { file, .. }
            | EmptyBoundaryExternalPattern { file, .. }
            | EmptyDependencyIgnore { file, .. }
            | EmptyCoverageReportPath { file, .. }
            | AbsoluteCoverageReportPath { file, .. }
            | ParentTraversalCoverageReportPath { file, .. }
            | TestGapBudgetOutsideProduction { file, .. }
            | TestGapRegressionLimit { file, .. }
            | MetricBudgetOutsideProduction { file, .. }
            | MetricRegressionLimitOutsideProduction { file, .. }
            | EmptySqlDialectInclude { file, .. }
            | EmptySqlDialectPattern { file, .. }
            | EmptyHazardRuleName { file, .. }
            | UnknownFrameworkProfile { file, .. }
            | EmptyFrameworkProfileName { file, .. }
            | ConflictingFrameworkProfileRule { file, .. }
            | EmptyFrameworkProfileObject { file, .. }
            | InvalidPackageConfigKey { file, .. }
            | DuplicatePackageConfigKey { file, .. }
            | EmptyRatchetPath { file, .. }
            | AbsoluteRatchetPath { file, .. }
            | ParentTraversalRatchetPath { file, .. } => *file = file_kind,
            AmbiguousConfigFiles { .. }
            | UnknownCliBudgetMetric { .. }
            | CliTestGapBudgetOutsideProduction { .. }
            | CliMetricBudgetOutsideProduction { .. }
            | InvalidCliGlob { .. } => {}
        }
        self
    }
}

pub(crate) fn is_absolute_repo_path(path: &str) -> bool {
    if std::path::Path::new(path).is_absolute() || path.starts_with('/') || path.starts_with('\\') {
        return true;
    }
    let bytes = path.as_bytes();
    bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':'
}

pub(crate) fn contains_parent_traversal(path: &str) -> bool {
    path.split(['/', '\\']).any(|segment| segment == "..")
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
    pub(crate) fn nested(parent: &str, child: &str) -> KeyPath {
        KeyPath(vec![
            KeySegment::Key(parent.to_owned()),
            KeySegment::Key(child.to_owned()),
        ])
    }

    pub(crate) fn child(&self, segment: KeySegment) -> KeyPath {
        let mut segments = self.0.clone();
        segments.push(segment);
        KeyPath(segments)
    }

    pub fn first_key(&self) -> Option<&str> {
        match self.0.first() {
            Some(KeySegment::Key(key)) => Some(key.as_str()),
            _ => None,
        }
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

fn format_unknown_framework_profile(
    file: ConfigFileKind,
    key: &KeyPath,
    name: &str,
    known: &[String],
) -> String {
    if known.is_empty() {
        format!(
            "{file} names an unknown Framework profile {name:?} at {key}; no loaded Language plugin defines Framework profiles"
        )
    } else {
        format!(
            "{file} names an unknown Framework profile {name:?} at {key}; the known Framework profiles are {}",
            known.join(", ")
        )
    }
}
