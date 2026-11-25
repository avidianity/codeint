use std::path::{Path, PathBuf};

use jsonc_parser::errors::ParseError;
use jsonc_parser::{
    CollectOptions, ParseOptions, parse_to_ast, parse_to_serde_value, parse_to_value,
};
use serde_json::Value;

use super::merge::parse_value_with_overrides;
use super::types::Configuration;
use crate::config_error::{
    CONFIGURATION_FILE_NAME, CONFIGURATION_JSONC_FILE_NAME, ConfigFileKind, ConfigurationFileError,
    JsoncError,
};
use crate::resolved::{ConfigOverrides, ResolvedConfiguration};

pub(super) const OFFICIAL_SCHEMA_PREFIX: &str =
    "https://raw.githubusercontent.com/avidianity/codeint/v";
pub(super) const OFFICIAL_SCHEMA_SUFFIX: &str = "/schema/codeint.schema.json";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SchemaVersionStatus {
    Missing,
    Compatible {
        url: String,
        version: Option<String>,
    },
    NewerThanBinary {
        url: String,
        config_version: String,
        binary_version: String,
    },
}

#[derive(Debug)]
pub enum ConfigurationInspection {
    Missing {
        path: PathBuf,
    },
    Valid {
        path: PathBuf,
        schema: SchemaVersionStatus,
        configuration: Box<Configuration>,
    },
    Invalid {
        path: PathBuf,
        schema: Option<SchemaVersionStatus>,
        error: ConfigurationFileError,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredConfigFile {
    pub kind: ConfigFileKind,
    pub path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchemaValueSpan {
    pub span: std::ops::Range<usize>,
    pub value: String,
}

pub fn read_configuration(repository_root: &Path) -> Result<Configuration, ConfigurationFileError> {
    read_resolved_configuration(repository_root, &ConfigOverrides::default())
        .map(|resolved| resolved.configuration)
}

fn path_exists_including_symlinks(
    path: &Path,
    file: ConfigFileKind,
) -> Result<bool, ConfigurationFileError> {
    match std::fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(source) => Err(ConfigurationFileError::Unreadable { file, source }),
    }
}

pub fn discover_config_file(
    repository_root: &Path,
) -> Result<Option<DiscoveredConfigFile>, ConfigurationFileError> {
    let json_path = repository_root.join(CONFIGURATION_FILE_NAME);
    let jsonc_path = repository_root.join(CONFIGURATION_JSONC_FILE_NAME);
    let has_json = path_exists_including_symlinks(&json_path, ConfigFileKind::Json)?;
    let has_jsonc = path_exists_including_symlinks(&jsonc_path, ConfigFileKind::Jsonc)?;
    match (has_json, has_jsonc) {
        (true, true) => Err(ConfigurationFileError::AmbiguousConfigFiles),
        (true, false) => Ok(Some(DiscoveredConfigFile {
            kind: ConfigFileKind::Json,
            path: json_path,
        })),
        (false, true) => Ok(Some(DiscoveredConfigFile {
            kind: ConfigFileKind::Jsonc,
            path: jsonc_path,
        })),
        (false, false) => Ok(None),
    }
}

pub fn schema_value_span(text: &str) -> Option<SchemaValueSpan> {
    let without_bom = text.strip_prefix('\u{FEFF}').unwrap_or(text);
    let bom_len = text.len() - without_bom.len();
    let parsed = parse_to_ast(
        without_bom,
        &CollectOptions::default(),
        &JSONC_PARSE_OPTIONS,
    )
    .ok()?;
    let value = parsed.value?;
    let object = value.as_object()?;
    let property = object
        .properties
        .iter()
        .rev()
        .find(|property| property.name.as_str() == "$schema")?;
    let jsonc_parser::ast::Value::StringLit(schema) = &property.value else {
        return None;
    };
    Some(SchemaValueSpan {
        span: (schema.range.start + bom_len)..(schema.range.end + bom_len),
        value: schema.value.to_string(),
    })
}

fn read_config_file(discovered: &DiscoveredConfigFile) -> Result<String, ConfigurationFileError> {
    let file = discovered.kind;
    match std::fs::read_to_string(&discovered.path) {
        Ok(text) => Ok(text),
        Err(error)
            if error.kind() == std::io::ErrorKind::NotFound
                && is_broken_symlink(&discovered.path) =>
        {
            Err(ConfigurationFileError::BrokenSymlink { file })
        }
        Err(source) => Err(ConfigurationFileError::Unreadable { file, source }),
    }
}

const JSONC_PARSE_OPTIONS: ParseOptions = ParseOptions {
    allow_comments: true,
    allow_trailing_commas: true,
    allow_loose_object_property_names: false,
    allow_missing_commas: false,
    allow_single_quoted_strings: false,
    allow_hexadecimal_numbers: false,
    allow_unary_plus_numbers: false,
    allow_bare_decimal_point_numbers: false,
    allow_non_finite_numbers: false,
    allow_extended_string_escapes: false,
};

fn jsonc_error_from_parse_error(error: &ParseError) -> JsoncError {
    JsoncError {
        message: error.kind().to_string(),
        line: error.line_display(),
        column: error.column_display(),
    }
}

pub(crate) fn parse_jsonc(input: &str) -> Result<Value, JsoncError> {
    let without_bom = input.strip_prefix('\u{FEFF}').unwrap_or(input);
    let value: Value = parse_to_serde_value(without_bom, &JSONC_PARSE_OPTIONS)
        .map_err(|error| jsonc_error_from_parse_error(&error))?;
    if value.is_null()
        && parse_to_value(without_bom, &JSONC_PARSE_OPTIONS)
            .map_err(|error| jsonc_error_from_parse_error(&error))?
            .is_none()
    {
        return Err(JsoncError {
            message: "EOF while parsing a value".to_owned(),
            line: 1,
            column: 1,
        });
    }
    Ok(value)
}

fn parse_jsonc_value(text: &str, file: ConfigFileKind) -> Result<Value, ConfigurationFileError> {
    parse_jsonc(text).map_err(|source| ConfigurationFileError::InvalidJsonc { file, source })
}

pub fn is_broken_symlink(path: &Path) -> bool {
    std::fs::symlink_metadata(path).is_ok_and(|meta| meta.file_type().is_symlink())
        && matches!(
            std::fs::metadata(path),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound
        )
}

pub fn read_resolved_configuration(
    repository_root: &Path,
    overrides: &ConfigOverrides,
) -> Result<ResolvedConfiguration, ConfigurationFileError> {
    match discover_config_file(repository_root)? {
        Some(discovered) => {
            let text = read_config_file(&discovered)?;
            parse_with_overrides(
                discovered.kind,
                discovered.path,
                true,
                Some(&text),
                overrides,
            )
        }
        None => parse_with_overrides(
            ConfigFileKind::Json,
            repository_root.join(CONFIGURATION_FILE_NAME),
            false,
            None,
            overrides,
        ),
    }
}

pub fn inspect_configuration(repository_root: &Path) -> ConfigurationInspection {
    let discovered = match discover_config_file(repository_root) {
        Ok(Some(discovered)) => discovered,
        Ok(None) => {
            return ConfigurationInspection::Missing {
                path: repository_root.join(CONFIGURATION_FILE_NAME),
            };
        }
        Err(error) => {
            return ConfigurationInspection::Invalid {
                path: repository_root.join(CONFIGURATION_FILE_NAME),
                schema: None,
                error,
            };
        }
    };
    let text = match read_config_file(&discovered) {
        Ok(text) => text,
        Err(error) => {
            return ConfigurationInspection::Invalid {
                path: discovered.path,
                schema: None,
                error,
            };
        }
    };
    let value = match parse_jsonc_value(&text, discovered.kind) {
        Ok(value) => value,
        Err(error) => {
            return ConfigurationInspection::Invalid {
                path: discovered.path,
                schema: None,
                error,
            };
        }
    };
    let schema = inspect_schema_version(&value, env!("CARGO_PKG_VERSION"));
    match parse_value_without_schema_version(discovered.kind, value) {
        Ok(configuration) => ConfigurationInspection::Valid {
            path: discovered.path,
            schema,
            configuration: Box::new(configuration),
        },
        Err(error) => ConfigurationInspection::Invalid {
            path: discovered.path,
            schema: Some(schema),
            error,
        },
    }
}

pub(super) fn parse_with_overrides(
    config_file_kind: ConfigFileKind,
    config_file: PathBuf,
    config_file_present: bool,
    text: Option<&str>,
    overrides: &ConfigOverrides,
) -> Result<ResolvedConfiguration, ConfigurationFileError> {
    let raw = match text {
        Some(text) => {
            let value = parse_jsonc_value(text, config_file_kind)?;
            check_schema_version(&value, env!("CARGO_PKG_VERSION"))
                .map_err(|error| error.with_file_kind(config_file_kind))?;
            Some(value)
        }
        None => None,
    };
    parse_value_with_overrides(
        config_file_kind,
        config_file,
        config_file_present,
        raw,
        overrides,
    )
    .map_err(|error| error.with_file_kind(config_file_kind))
}

fn parse_value_without_schema_version(
    config_file_kind: ConfigFileKind,
    value: Value,
) -> Result<Configuration, ConfigurationFileError> {
    parse_value_with_overrides(
        config_file_kind,
        PathBuf::from(config_file_kind.file_name()),
        true,
        Some(value),
        &ConfigOverrides::default(),
    )
    .map(|resolved| resolved.configuration)
    .map_err(|error| error.with_file_kind(config_file_kind))
}

fn inspect_schema_version(value: &Value, binary_version: &str) -> SchemaVersionStatus {
    let Some(schema_url) = value.get("$schema").and_then(Value::as_str) else {
        return SchemaVersionStatus::Missing;
    };
    let Some(config_version) = extract_official_schema_version(schema_url) else {
        return SchemaVersionStatus::Compatible {
            url: schema_url.to_owned(),
            version: None,
        };
    };
    let (Some(cfg), Some(bin)) = (parse_semver(config_version), parse_semver(binary_version))
    else {
        return SchemaVersionStatus::Compatible {
            url: schema_url.to_owned(),
            version: Some(config_version.to_owned()),
        };
    };
    if cfg > bin {
        return SchemaVersionStatus::NewerThanBinary {
            url: schema_url.to_owned(),
            config_version: config_version.to_owned(),
            binary_version: binary_version.to_owned(),
        };
    }
    SchemaVersionStatus::Compatible {
        url: schema_url.to_owned(),
        version: Some(config_version.to_owned()),
    }
}

pub(super) fn check_schema_version(
    value: &Value,
    binary_version: &str,
) -> Result<(), ConfigurationFileError> {
    if let SchemaVersionStatus::NewerThanBinary {
        config_version,
        binary_version,
        ..
    } = inspect_schema_version(value, binary_version)
    {
        return Err(ConfigurationFileError::NewerSchemaVersion {
            file: ConfigFileKind::Json,
            config_version,
            binary_version,
        });
    }
    Ok(())
}

fn extract_official_schema_version(url: &str) -> Option<&str> {
    url.strip_prefix(OFFICIAL_SCHEMA_PREFIX)?
        .strip_suffix(OFFICIAL_SCHEMA_SUFFIX)
}

fn parse_semver(raw: &str) -> Option<(u64, u64, u64)> {
    let core = raw.split(['-', '+']).next()?;
    let mut parts = core.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    let patch = parts.next()?.parse().ok()?;
    if parts.next().is_some() {
        return None;
    }
    Some((major, minor, patch))
}
