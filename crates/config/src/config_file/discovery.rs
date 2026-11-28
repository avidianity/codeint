use std::path::{Path, PathBuf};

use jsonc_parser::errors::ParseError;
use jsonc_parser::{parse_to_serde_value, parse_to_value};
use serde_json::Value;

use super::formats::{parse_toml_value, parse_yaml_value};
use super::merge::parse_value_with_overrides;
use super::schema_directive::{
    JSONC_PARSE_OPTIONS, OFFICIAL_SCHEMA_PREFIX, OFFICIAL_SCHEMA_SUFFIX, resolve_schema_url,
};
use super::types::Configuration;
use crate::config_error::{
    CONFIGURATION_FILE_NAME, ConfigFileKind, ConfigSyntax, ConfigurationFileError, JsoncError,
};
use crate::resolved::{ConfigOverrides, ResolvedConfiguration};

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
        kind: ConfigFileKind,
        path: PathBuf,
        schema: SchemaVersionStatus,
        configuration: Box<Configuration>,
    },
    Invalid {
        kind: Option<ConfigFileKind>,
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
        Err(error)
            if matches!(
                error.kind(),
                std::io::ErrorKind::NotFound | std::io::ErrorKind::NotADirectory
            ) =>
        {
            Ok(false)
        }
        Err(source) => Err(ConfigurationFileError::Unreadable { file, source }),
    }
}

pub fn discover_config_file(
    repository_root: &Path,
) -> Result<Option<DiscoveredConfigFile>, ConfigurationFileError> {
    let mut found = Vec::new();
    for kind in ConfigFileKind::ALL {
        let path = repository_root.join(kind.file_name());
        if path_exists_including_symlinks(&path, kind)? {
            found.push(DiscoveredConfigFile { kind, path });
        }
    }
    match found.len() {
        0 => Ok(None),
        1 => Ok(found.into_iter().next()),
        _ => Err(ConfigurationFileError::AmbiguousConfigFiles {
            files: found.into_iter().map(|entry| entry.kind).collect(),
        }),
    }
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

fn parse_config_document(
    text: &str,
    file: ConfigFileKind,
) -> Result<Value, ConfigurationFileError> {
    match file.syntax() {
        ConfigSyntax::Json => parse_jsonc_value(text, file),
        ConfigSyntax::Toml => parse_toml_value(text, file),
        ConfigSyntax::Yaml => parse_yaml_value(text, file),
    }
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
                kind: None,
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
                kind: Some(discovered.kind),
                path: discovered.path,
                schema: None,
                error,
            };
        }
    };
    let value = match parse_config_document(&text, discovered.kind) {
        Ok(value) => value,
        Err(error) => {
            return ConfigurationInspection::Invalid {
                kind: Some(discovered.kind),
                path: discovered.path,
                schema: None,
                error,
            };
        }
    };
    let schema_url = match resolve_schema_url(discovered.kind, &text, &value) {
        Ok(url) => url,
        Err(error) => {
            return ConfigurationInspection::Invalid {
                kind: Some(discovered.kind),
                path: discovered.path,
                schema: None,
                error,
            };
        }
    };
    let schema = inspect_schema_url(schema_url.as_deref(), env!("CARGO_PKG_VERSION"));
    match parse_value_without_schema_version(discovered.kind, value) {
        Ok(configuration) => ConfigurationInspection::Valid {
            kind: discovered.kind,
            path: discovered.path,
            schema,
            configuration: Box::new(configuration),
        },
        Err(error) => ConfigurationInspection::Invalid {
            kind: Some(discovered.kind),
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
            let value = parse_config_document(text, config_file_kind)?;
            let schema_url = resolve_schema_url(config_file_kind, text, &value)?;
            check_schema_url(schema_url.as_deref(), env!("CARGO_PKG_VERSION"))
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

fn inspect_schema_url(schema_url: Option<&str>, binary_version: &str) -> SchemaVersionStatus {
    let Some(schema_url) = schema_url else {
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

fn check_schema_url(
    schema_url: Option<&str>,
    binary_version: &str,
) -> Result<(), ConfigurationFileError> {
    if let SchemaVersionStatus::NewerThanBinary {
        config_version,
        binary_version,
        ..
    } = inspect_schema_url(schema_url, binary_version)
    {
        return Err(ConfigurationFileError::NewerSchemaVersion {
            file: ConfigFileKind::Json,
            config_version,
            binary_version,
        });
    }
    Ok(())
}

#[cfg(test)]
pub(super) fn check_schema_version(
    value: &Value,
    binary_version: &str,
) -> Result<(), ConfigurationFileError> {
    let schema_url = value.get("$schema").and_then(Value::as_str);
    check_schema_url(schema_url, binary_version)
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
