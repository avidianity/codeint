use std::collections::BTreeMap;

use codeint_model::{FrameworkProfileSelection, RepoPath};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::config_error::{ConfigFileKind, ConfigurationFileError, KeyPath, KeySegment};

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(untagged)]
pub(crate) enum FrameworkProfileRuleFile {
    Enabled(bool),
    List(Vec<String>),
    Object(FrameworkProfileObjectFile),
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct FrameworkProfileObjectFile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(description = "Auto-detect Framework profiles except the named profiles.")]
    pub(crate) exclude: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(description = "Activate only the named Framework profiles.")]
    pub(crate) only: Option<Vec<String>>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct PackageConfigFile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(
        description = "Framework profile selection for this Workspace package: false to disable all, true for auto-detection, a list of profile names (or {\"only\": [...]}), or {\"exclude\": [...]}."
    )]
    pub(crate) framework_profiles: Option<FrameworkProfileRuleFile>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrameworkProfilesConfig {
    global: FrameworkProfileSelection,
    packages: BTreeMap<String, FrameworkProfileSelection>,
}

impl Default for FrameworkProfilesConfig {
    fn default() -> FrameworkProfilesConfig {
        FrameworkProfilesConfig {
            global: FrameworkProfileSelection::Auto,
            packages: BTreeMap::new(),
        }
    }
}

impl FrameworkProfilesConfig {
    pub(crate) fn from_files(
        global: Option<FrameworkProfileRuleFile>,
        packages: BTreeMap<String, PackageConfigFile>,
    ) -> Result<FrameworkProfilesConfig, ConfigurationFileError> {
        let global_key = KeyPath::default().child(KeySegment::Key("framework_profiles".to_owned()));
        let global = match global {
            Some(rule) => parse_rule(&global_key, rule)?,
            None => FrameworkProfileSelection::Auto,
        };
        let packages_key = KeyPath::default().child(KeySegment::Key("packages".to_owned()));
        let mut parsed_packages = BTreeMap::new();
        for (raw_pkg, entry) in packages {
            let pkg_key = packages_key.child(KeySegment::Key(raw_pkg.clone()));
            let normalized = normalize_package_key(&raw_pkg).ok_or_else(|| {
                ConfigurationFileError::InvalidPackageConfigKey {
                    file: ConfigFileKind::Json,
                    key: pkg_key.clone(),
                    package: raw_pkg.clone(),
                }
            })?;
            if let Some(rule) = entry.framework_profiles {
                let rule_key = pkg_key.child(KeySegment::Key("framework_profiles".to_owned()));
                let selection = parse_rule(&rule_key, rule)?;
                if parsed_packages
                    .insert(normalized.clone(), selection)
                    .is_some()
                {
                    return Err(ConfigurationFileError::DuplicatePackageConfigKey {
                        file: ConfigFileKind::Json,
                        key: pkg_key,
                        package: display_package_key(&normalized).to_owned(),
                    });
                }
            }
        }
        Ok(FrameworkProfilesConfig {
            global,
            packages: parsed_packages,
        })
    }

    pub fn global(&self) -> &FrameworkProfileSelection {
        &self.global
    }

    pub fn packages(&self) -> &BTreeMap<String, FrameworkProfileSelection> {
        &self.packages
    }

    pub fn selection_for_package(&self, package_root: &str) -> FrameworkProfileSelection {
        self.packages
            .get(package_root)
            .cloned()
            .unwrap_or_else(|| self.global.clone())
    }

    pub fn validate_known(
        &self,
        config_file: ConfigFileKind,
        known_profiles: &[String],
    ) -> Result<(), ConfigurationFileError> {
        let global_key = KeyPath::default().child(KeySegment::Key("framework_profiles".to_owned()));
        validate_selection(config_file, &global_key, &self.global, known_profiles)?;
        let packages_key = KeyPath::default().child(KeySegment::Key("packages".to_owned()));
        for (package_root, selection) in &self.packages {
            let pkg_key = packages_key
                .child(KeySegment::Key(
                    display_package_key(package_root).to_owned(),
                ))
                .child(KeySegment::Key("framework_profiles".to_owned()));
            validate_selection(config_file, &pkg_key, selection, known_profiles)?;
        }
        Ok(())
    }

    pub(crate) fn resolved_global_value(&self) -> Value {
        selection_to_json(&self.global)
    }

    pub(crate) fn resolved_package_settings(&self) -> Vec<(String, Value)> {
        self.packages
            .iter()
            .map(|(root, selection)| {
                (
                    format!("packages.{}.framework_profiles", display_package_key(root)),
                    selection_to_json(selection),
                )
            })
            .collect()
    }
}

pub fn display_package_key(package_root: &str) -> &str {
    if package_root.is_empty() {
        "."
    } else {
        package_root
    }
}

fn selection_to_json(selection: &FrameworkProfileSelection) -> Value {
    match selection {
        FrameworkProfileSelection::Auto => json!("auto"),
        FrameworkProfileSelection::Disabled => json!(false),
        FrameworkProfileSelection::Exclude(names) => json!({ "exclude": names }),
        FrameworkProfileSelection::Only(names) => json!(names),
    }
}

fn parse_rule(
    key: &KeyPath,
    rule: FrameworkProfileRuleFile,
) -> Result<FrameworkProfileSelection, ConfigurationFileError> {
    match rule {
        FrameworkProfileRuleFile::Enabled(true) => Ok(FrameworkProfileSelection::Auto),
        FrameworkProfileRuleFile::Enabled(false) => Ok(FrameworkProfileSelection::Disabled),
        FrameworkProfileRuleFile::List(names) => {
            validate_non_empty_names(key, &names)?;
            Ok(FrameworkProfileSelection::Only(names))
        }
        FrameworkProfileRuleFile::Object(FrameworkProfileObjectFile {
            exclude: Some(_),
            only: Some(_),
        }) => Err(ConfigurationFileError::ConflictingFrameworkProfileRule {
            file: ConfigFileKind::Json,
            key: key.clone(),
        }),
        FrameworkProfileRuleFile::Object(FrameworkProfileObjectFile {
            exclude: None,
            only: None,
        }) => Err(ConfigurationFileError::EmptyFrameworkProfileObject {
            file: ConfigFileKind::Json,
            key: key.clone(),
        }),
        FrameworkProfileRuleFile::Object(FrameworkProfileObjectFile {
            exclude: Some(names),
            only: None,
        }) => {
            let exclude_key = key.child(KeySegment::Key("exclude".to_owned()));
            validate_non_empty_names(&exclude_key, &names)?;
            Ok(FrameworkProfileSelection::Exclude(names))
        }
        FrameworkProfileRuleFile::Object(FrameworkProfileObjectFile {
            exclude: None,
            only: Some(names),
        }) => {
            let only_key = key.child(KeySegment::Key("only".to_owned()));
            validate_non_empty_names(&only_key, &names)?;
            Ok(FrameworkProfileSelection::Only(names))
        }
    }
}

fn validate_non_empty_names(key: &KeyPath, names: &[String]) -> Result<(), ConfigurationFileError> {
    for (index, name) in names.iter().enumerate() {
        if name.trim().is_empty() {
            return Err(ConfigurationFileError::EmptyFrameworkProfileName {
                file: ConfigFileKind::Json,
                key: key.child(KeySegment::Index(index)),
            });
        }
    }
    Ok(())
}

fn validate_selection(
    config_file: ConfigFileKind,
    key: &KeyPath,
    selection: &FrameworkProfileSelection,
    known_profiles: &[String],
) -> Result<(), ConfigurationFileError> {
    let (names, list_key) = match selection {
        FrameworkProfileSelection::Auto | FrameworkProfileSelection::Disabled => return Ok(()),
        FrameworkProfileSelection::Exclude(names) => {
            (names, key.child(KeySegment::Key("exclude".to_owned())))
        }
        FrameworkProfileSelection::Only(names) => (names, key.clone()),
    };
    for (index, name) in names.iter().enumerate() {
        if !known_profiles.iter().any(|known| known == name) {
            return Err(ConfigurationFileError::UnknownFrameworkProfile {
                file: config_file,
                key: list_key.child(KeySegment::Index(index)),
                name: name.clone(),
                known: known_profiles.to_vec(),
            });
        }
    }
    Ok(())
}

fn normalize_package_key(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed.contains('\\') || trimmed.starts_with('/') {
        return None;
    }
    let mut normalized = trimmed;
    while let Some(stripped) = normalized.strip_prefix("./") {
        normalized = stripped;
    }
    normalized = normalized.trim_end_matches('/');
    if normalized == "." || (normalized.is_empty() && trimmed.trim_end_matches('/') == ".") {
        return Some(String::new());
    }
    RepoPath::new(normalized)
        .ok()
        .map(|path| path.as_str().to_owned())
}
