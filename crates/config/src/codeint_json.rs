#[cfg(test)]
use std::path::PathBuf;

#[cfg(test)]
use serde_json::{Value, json};

#[cfg(test)]
use crate::config_error::{CONFIGURATION_FILE_NAME, ConfigFileKind, ConfigurationFileError};
#[cfg(test)]
use crate::resolved::ConfigOverrides;

mod discovery;
mod merge;
mod schema;
mod shape;
mod starter;
#[cfg(test)]
mod tests;
mod types;
mod validation;

#[cfg(test)]
use discovery::{
    OFFICIAL_SCHEMA_PREFIX, OFFICIAL_SCHEMA_SUFFIX, check_schema_version, parse_with_overrides,
};

#[cfg(test)]
pub(crate) use discovery::parse_jsonc;
pub use discovery::{
    ConfigurationInspection, DiscoveredConfigFile, SchemaValueSpan, SchemaVersionStatus,
    discover_config_file, inspect_configuration, is_broken_symlink, read_configuration,
    read_resolved_configuration, schema_value_span,
};
pub use schema::{Budget, OutputFormat, PluginKind, configuration_schema};
pub use starter::{
    DetectedLanguage, StarterBoundaries, StarterBoundaryZone, StarterConfigurationError,
    published_schema_address, write_adoption_starter_configuration, write_adoption_zero_budgets,
    write_starter_configuration,
};
pub use types::{
    Budgets, Configuration, DEPENDENCY_METRIC_KEYS, DuplicationConfig, EntryPoints,
    HAZARDS_METRIC_KEY, LanguageCoverage, Languages, MinSimilarity, PluginDeclaration,
    PluginLimits, PluginSource, Sha256Digest, SourceSetOverrides, TEST_GAP_METRIC_KEY,
};

#[cfg(test)]
pub(crate) fn parse(text: &str) -> Result<Configuration, ConfigurationFileError> {
    parse_with_overrides(
        ConfigFileKind::Json,
        PathBuf::from(CONFIGURATION_FILE_NAME),
        true,
        Some(text),
        &ConfigOverrides::default(),
    )
    .map(|resolved| resolved.configuration)
}

#[cfg(test)]
mod schema_version_tests {
    use super::*;

    fn schema_json(url: &str) -> Value {
        serde_json::json!({ "$schema": url })
    }

    #[test]
    fn rejects_newer_patch_minor_and_major_schema_versions() {
        for newer in ["0.4.1", "0.5.0", "1.0.0"] {
            let url = format!("{OFFICIAL_SCHEMA_PREFIX}{newer}{OFFICIAL_SCHEMA_SUFFIX}");
            let err = check_schema_version(&schema_json(&url), "0.4.0").unwrap_err();
            assert!(matches!(
                err,
                ConfigurationFileError::NewerSchemaVersion {
                    ref config_version,
                    ref binary_version,
                    ..
                } if config_version == newer && binary_version == "0.4.0"
            ));
        }
    }

    #[test]
    fn accepts_equal_older_missing_and_custom_schemas() {
        for ok in ["0.4.0", "0.3.9", "0.1.0"] {
            let url = format!("{OFFICIAL_SCHEMA_PREFIX}{ok}{OFFICIAL_SCHEMA_SUFFIX}");
            assert!(check_schema_version(&schema_json(&url), "0.4.0").is_ok());
        }
        assert!(check_schema_version(&serde_json::json!({}), "0.4.0").is_ok());
        assert!(
            check_schema_version(&schema_json("./schema/codeint.schema.json"), "0.4.0").is_ok()
        );
    }
}
