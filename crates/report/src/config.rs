use codeint_config::{ConfigFormat, ResolvedConfiguration, ResolvedSetting};
use serde::Serialize;

use crate::ReportError;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ConfigReport {
    config_file: String,
    config_file_present: bool,
    settings: Vec<ResolvedSetting>,
}

impl ConfigReport {
    pub fn from_resolved(resolved: &ResolvedConfiguration) -> ConfigReport {
        ConfigReport {
            config_file: resolved.config_file.display().to_string(),
            config_file_present: resolved.config_file_present,
            settings: resolved.settings.clone(),
        }
    }

    pub fn render(&self, format: ConfigFormat) -> Result<String, ReportError> {
        match format {
            ConfigFormat::Text => Ok(self.format_lines()),
            ConfigFormat::Toon => crate::encode_toon(self),
            ConfigFormat::Json => crate::encode_json(self),
        }
    }

    fn format_lines(&self) -> String {
        let mut lines = Vec::with_capacity(self.settings.len() + 2);
        let state = if self.config_file_present {
            "loaded"
        } else {
            "not found; using defaults"
        };
        lines.push(format!("config_file: {} ({state})", self.config_file));
        lines.push("settings:".to_owned());
        for setting in &self.settings {
            let value = serde_json::to_string(&setting.value).unwrap_or_else(|_| "null".to_owned());
            lines.push(format!("  {} = {value} [{}]", setting.key, setting.source));
        }
        lines.join("\n")
    }
}
