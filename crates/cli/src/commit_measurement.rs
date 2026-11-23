use codeint_config::Configuration;
use codeint_git::{CommitId, Repository, ResultCache, Snapshot};
use codeint_report::CommitMeasurement;
use serde::Serialize;

use crate::check::{
    BUILT_IN_LANGUAGE_PLUGINS, CheckError, LoadedPlugins, SelectedPackage,
    filter_languages_to_package, measure_with, select_workspace_package, workspace_of_languages,
};
use crate::describe;

const CODEINT_VERSION: &str = env!("CARGO_PKG_VERSION");
const MEASUREMENT_SCHEMA_VERSION: u32 = 2;

#[derive(Serialize)]
struct MeasurementSettings<'configuration> {
    schema_version: u32,
    codeint_version: &'static str,
    language_plugins: Vec<PluginIdentity>,
    configuration: &'configuration Configuration,
    #[serde(skip_serializing_if = "Option::is_none")]
    package: Option<&'configuration str>,
}

#[derive(Serialize)]
struct PluginIdentity {
    name: String,
    version: String,
}

pub(crate) struct CommitMeasurer<'run> {
    repository: &'run Repository,
    configuration: &'run Configuration,
    plugins: &'run LoadedPlugins,
    settings: MeasurementSettings<'run>,
    result_cache: Option<&'run ResultCache>,
}

impl<'run> CommitMeasurer<'run> {
    pub(crate) fn new(
        repository: &'run Repository,
        configuration: &'run Configuration,
        plugins: &'run LoadedPlugins,
        result_cache: Option<&'run ResultCache>,
    ) -> CommitMeasurer<'run> {
        CommitMeasurer::for_package(repository, configuration, plugins, result_cache, None)
    }

    pub(crate) fn for_package(
        repository: &'run Repository,
        configuration: &'run Configuration,
        plugins: &'run LoadedPlugins,
        result_cache: Option<&'run ResultCache>,
        package: Option<&'run str>,
    ) -> CommitMeasurer<'run> {
        let mut language_plugins: Vec<PluginIdentity> = BUILT_IN_LANGUAGE_PLUGINS
            .iter()
            .map(|named| PluginIdentity {
                name: named.name.to_owned(),
                version: CODEINT_VERSION.to_owned(),
            })
            .collect();
        for declared in &configuration.plugins {
            language_plugins.push(PluginIdentity {
                name: declared.name.clone(),
                version: declared.sha256.to_hex(),
            });
        }
        CommitMeasurer {
            repository,
            configuration,
            plugins,
            settings: MeasurementSettings {
                schema_version: MEASUREMENT_SCHEMA_VERSION,
                codeint_version: CODEINT_VERSION,
                language_plugins,
                configuration,
                package,
            },
            result_cache,
        }
    }

    pub(crate) fn measure(&self, commit: CommitId) -> Result<CommitMeasurement, CheckError> {
        self.measure_with_package(commit, None)
    }

    pub(crate) fn measure_selected_package(
        &self,
        commit: CommitId,
        selected: Option<&SelectedPackage>,
    ) -> Result<CommitMeasurement, CheckError> {
        self.measure_with_package(commit, selected)
    }

    fn measure_with_package(
        &self,
        commit: CommitId,
        selected: Option<&SelectedPackage>,
    ) -> Result<CommitMeasurement, CheckError> {
        if let Some(cached) = self
            .result_cache
            .and_then(|cache| cache.read(commit, &self.settings))
        {
            return Ok(cached);
        }
        let measurement = measure_all_languages_selected(
            &self.repository.commit(commit)?,
            self.plugins,
            self.configuration,
            selected,
        )?;
        if let Some(cache) = self.result_cache
            && let Err(error) = cache.write(commit, &self.settings, &measurement)
        {
            eprintln!(
                "codeint: {}; the next run measures commit {commit} again",
                describe(&error)
            );
        }
        Ok(measurement)
    }
}

pub(crate) fn measure_all_languages_for_package(
    tree: &Snapshot,
    plugins: &LoadedPlugins,
    configuration: &Configuration,
    package: Option<&str>,
) -> Result<(Option<SelectedPackage>, CommitMeasurement), CheckError> {
    let named = plugins.as_named();
    let metric_plugins = plugins.metric_plugins();
    let mut languages = measure_with(tree, &named, configuration)?;
    let selected = if let Some(requested) = package {
        let workspace = workspace_of_languages(&languages);
        let selected = select_workspace_package(&workspace, requested)?;
        filter_languages_to_package(&mut languages, &selected);
        Some(selected)
    } else {
        None
    };
    let models = languages
        .into_iter()
        .map(|language| language.model)
        .collect();
    let measurement = CommitMeasurement::of_languages_with_options(
        models,
        &metric_plugins,
        configuration.boundaries.as_ref(),
        &configuration.dependencies,
    )?;
    Ok((selected, measurement))
}

fn measure_all_languages_selected(
    tree: &Snapshot,
    plugins: &LoadedPlugins,
    configuration: &Configuration,
    selected: Option<&SelectedPackage>,
) -> Result<CommitMeasurement, CheckError> {
    let named = plugins.as_named();
    let metric_plugins = plugins.metric_plugins();
    let mut languages = measure_with(tree, &named, configuration)?;
    if let Some(selected) = selected {
        filter_languages_to_package(&mut languages, selected);
    }
    let models = languages
        .into_iter()
        .map(|language| language.model)
        .collect();
    Ok(CommitMeasurement::of_languages_with_options(
        models,
        &metric_plugins,
        configuration.boundaries.as_ref(),
        &configuration.dependencies,
    )?)
}
