use std::path::Path;

use codeint_config::{
    ConfigOverrides, Configuration, ConfigurationFileError, PluginKind, read_resolved_configuration,
};
use codeint_git::{GitError, Repository};
use codeint_plugin_host::{built_in_language_framework_profiles, built_in_language_names};
use codeint_report::ConfigReport;

#[derive(Debug, thiserror::Error)]
pub(crate) enum ConfigCommandError {
    #[error(transparent)]
    Git(#[from] GitError),
    #[error(transparent)]
    Configuration(#[from] ConfigurationFileError),
}

pub(crate) fn resolved_config(
    folder: &Path,
    overrides: &ConfigOverrides,
) -> Result<ConfigReport, ConfigCommandError> {
    let repository = Repository::discover(folder)?;
    let resolved = read_resolved_configuration(repository.root(), overrides)?;
    validate_resolved_configuration(&resolved.configuration, overrides)?;
    Ok(ConfigReport::from_resolved(&resolved))
}

fn validate_resolved_configuration(
    configuration: &Configuration,
    overrides: &ConfigOverrides,
) -> Result<(), ConfigurationFileError> {
    let language_names = known_language_plugin_names(configuration);
    configuration.languages.coverage_of(&language_names)?;
    let owned_metric_keys = configured_metric_keys(configuration);
    let metric_keys: Vec<&str> = owned_metric_keys.iter().map(String::as_str).collect();
    configuration.validate_cli_budget_metrics(&overrides.budgets, &metric_keys)?;
    configuration.validate_metrics(&metric_keys)?;
    let has_external_languages = configuration
        .plugins
        .iter()
        .any(|plugin| plugin.kind == PluginKind::Language);
    if !has_external_languages {
        let mut known_profiles: Vec<String> = built_in_language_framework_profiles()
            .into_values()
            .flatten()
            .collect();
        known_profiles.sort();
        known_profiles.dedup();
        configuration
            .framework_profiles
            .validate_known(configuration.config_file(), &known_profiles)?;
    }
    Ok(())
}

fn known_language_plugin_names(configuration: &Configuration) -> Vec<&str> {
    let mut names: Vec<&str> = built_in_language_names();
    for plugin in &configuration.plugins {
        if plugin.kind == PluginKind::Language && !names.contains(&plugin.name.as_str()) {
            names.push(plugin.name.as_str());
        }
    }
    names
}

fn configured_metric_keys(configuration: &Configuration) -> Vec<String> {
    let mut keys = Vec::new();
    for plugin in &configuration.plugins {
        if plugin.kind == PluginKind::Metric {
            keys.push(plugin.name.clone());
            let snake = plugin.name.replace('-', "_");
            if snake != plugin.name {
                keys.push(snake);
            }
        }
    }
    keys
}
