use std::collections::BTreeSet;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::config_error::{ConfigFileKind, ConfigurationFileError, KeyPath, KeySegment};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HazardsConfig {
    pub disabled_rules: Vec<String>,
}

impl HazardsConfig {
    pub fn is_disabled(&self, rule: &str) -> bool {
        self.disabled_rules.iter().any(|disabled| disabled == rule)
    }

    pub fn disabled_rules_set(&self) -> BTreeSet<String> {
        self.disabled_rules.iter().cloned().collect()
    }

    pub(crate) fn from_file(
        file: HazardsConfigFile,
    ) -> Result<HazardsConfig, ConfigurationFileError> {
        let base_key = KeyPath::nested("hazards", "disabled_rules");
        if let Some((index, _)) = file
            .disabled_rules
            .iter()
            .enumerate()
            .find(|(_, entry)| entry.trim().is_empty())
        {
            return Err(ConfigurationFileError::EmptyHazardRuleName {
                file: ConfigFileKind::Json,
                key: base_key.child(KeySegment::Index(index)),
            });
        }
        Ok(HazardsConfig {
            disabled_rules: file.disabled_rules,
        })
    }
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct HazardsConfigFile {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[schemars(
        description = "Names of Hazard line rules turned off across the repository. A turned-off rule adds no Hazard line and no Finding."
    )]
    pub(crate) disabled_rules: Vec<String>,
}

pub(crate) fn is_hazards_unset(file: &HazardsConfigFile) -> bool {
    file.disabled_rules.is_empty()
}
