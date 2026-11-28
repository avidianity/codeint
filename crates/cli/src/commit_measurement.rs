use std::collections::{BTreeMap, HashSet};

use codeint_config::Configuration;
use codeint_git::{CommitId, Repository, ResultCache, Snapshot};
use codeint_metrics::DependencyHygiene;
use codeint_model::{CodeModel, Metric, RepoPath};
use codeint_plugin_host::{PLUGIN_INTERFACE_VERSION, built_in_language_names};
use codeint_report::{CommitMeasurement, CommitMeasurementOptions};
use serde::Serialize;

use crate::check::{
    CheckError, LoadedPlugins, LoweringCache, MeasuredSnapshot, SelectedPackage,
    compute_dependency_hygiene, compute_orphan_files, compute_orphan_files_for_subset,
    external_language_plugin_versions, filter_languages_to_package, measure_snapshot_with_cache,
    select_workspace_package, workspace_of_languages,
};
use crate::describe;

const CODEINT_VERSION: &str = env!("CARGO_PKG_VERSION");
const MEASUREMENT_CACHE_VERSION: u32 = 9;

#[derive(Serialize)]
struct MeasurementSettings<'configuration> {
    codeint_version: &'static str,
    cache_version: u32,
    plugin_interface_version: String,
    metric_versions: BTreeMap<String, u32>,
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

pub(crate) enum SnapshotRef<'a> {
    Borrowed(&'a Snapshot),
    Owned(Snapshot),
}

impl<'a> SnapshotRef<'a> {
    pub(crate) fn get_or_open<E>(
        snapshot: Option<&'a Snapshot>,
        open: impl FnOnce() -> Result<Snapshot, E>,
    ) -> Result<Self, E> {
        match snapshot {
            Some(opened) => Ok(Self::Borrowed(opened)),
            None => Ok(Self::Owned(open()?)),
        }
    }

    pub(crate) fn as_snapshot(&self) -> &Snapshot {
        match self {
            Self::Borrowed(snapshot) => snapshot,
            Self::Owned(snapshot) => snapshot,
        }
    }
}

pub(crate) struct CommitMeasurer<'run> {
    repository: &'run Repository,
    configuration: &'run Configuration,
    plugins: &'run LoadedPlugins,
    settings: MeasurementSettings<'run>,
    result_cache: Option<&'run ResultCache>,
    lowering_cache: Option<&'run LoweringCache>,
}

impl<'run> CommitMeasurer<'run> {
    pub(crate) fn new(
        repository: &'run Repository,
        configuration: &'run Configuration,
        plugins: &'run LoadedPlugins,
        result_cache: Option<&'run ResultCache>,
        lowering_cache: Option<&'run LoweringCache>,
    ) -> CommitMeasurer<'run> {
        let mut language_plugins: Vec<PluginIdentity> = built_in_language_names()
            .into_iter()
            .map(|name| PluginIdentity {
                name: name.to_owned(),
                version: CODEINT_VERSION.to_owned(),
            })
            .collect();
        let mut external_language_plugins: Vec<(&str, String)> =
            external_language_plugin_versions(configuration)
                .into_iter()
                .collect();
        external_language_plugins.sort_by(|left, right| left.0.cmp(right.0));
        for (name, version) in external_language_plugins {
            language_plugins.push(PluginIdentity {
                name: name.to_owned(),
                version,
            });
        }
        let metric_plugins = plugins.metric_plugins();
        let metric_versions = Metric::versions_with_plugins(&metric_plugins);
        CommitMeasurer {
            repository,
            configuration,
            plugins,
            settings: MeasurementSettings {
                codeint_version: CODEINT_VERSION,
                cache_version: MEASUREMENT_CACHE_VERSION,
                plugin_interface_version: PLUGIN_INTERFACE_VERSION.to_owned(),
                metric_versions,
                language_plugins,
                configuration,
                package: None,
            },
            result_cache,
            lowering_cache,
        }
    }

    pub(crate) fn with_package(mut self, package: Option<&'run str>) -> CommitMeasurer<'run> {
        self.settings.package = package;
        self
    }

    pub(crate) fn measure(&self, commit: CommitId) -> Result<CommitMeasurement, CheckError> {
        self.measure_with(commit, None, None)
    }

    pub(crate) fn measure_with(
        &self,
        commit: CommitId,
        selected: Option<&SelectedPackage>,
        snapshot: Option<&Snapshot>,
    ) -> Result<CommitMeasurement, CheckError> {
        if let Some(cached) = self
            .result_cache
            .and_then(|cache| cache.read(commit, &self.settings))
        {
            return Ok(cached);
        }
        let tree = SnapshotRef::get_or_open(snapshot, || self.repository.commit(commit))?;
        let measurement = measure_all_languages_selected(
            tree.as_snapshot(),
            self.plugins,
            self.configuration,
            selected,
            self.lowering_cache,
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
    lowering_cache: Option<&LoweringCache>,
) -> Result<(Option<SelectedPackage>, CommitMeasurement), CheckError> {
    let named = plugins.as_named();
    let snapshot = measure_snapshot_with_cache(tree, &named, configuration, lowering_cache)?;
    let selected = resolve_package(&snapshot, package)?;
    let measurement =
        measure_snapshot_to_commit(snapshot, plugins, configuration, selected.as_ref())?;
    Ok((selected, measurement))
}

pub(crate) fn measure_all_languages_with_new_files(
    tree: &Snapshot,
    new_files: &HashSet<RepoPath>,
    plugins: &LoadedPlugins,
    configuration: &Configuration,
    package: Option<&str>,
    lowering_cache: Option<&LoweringCache>,
) -> Result<
    (
        Option<SelectedPackage>,
        CommitMeasurement,
        Option<CommitMeasurement>,
    ),
    CheckError,
> {
    let named = plugins.as_named();
    let snapshot = measure_snapshot_with_cache(tree, &named, configuration, lowering_cache)?;
    let selected = resolve_package(&snapshot, package)?;
    let new_files_measurement = measure_new_files_subset(
        &snapshot,
        new_files,
        plugins,
        configuration,
        selected.as_ref(),
    )?;
    let measurement =
        measure_snapshot_to_commit(snapshot, plugins, configuration, selected.as_ref())?;
    Ok((selected, measurement, new_files_measurement))
}

fn resolve_package(
    snapshot: &MeasuredSnapshot,
    package: Option<&str>,
) -> Result<Option<SelectedPackage>, CheckError> {
    let Some(requested) = package else {
        return Ok(None);
    };
    let workspace = workspace_of_languages(&snapshot.languages);
    Ok(Some(select_workspace_package(&workspace, requested)?))
}

fn measure_new_files_subset(
    snapshot: &MeasuredSnapshot,
    new_files: &HashSet<RepoPath>,
    plugins: &LoadedPlugins,
    configuration: &Configuration,
    selected: Option<&SelectedPackage>,
) -> Result<Option<CommitMeasurement>, CheckError> {
    let orphan_files = compute_orphan_files_for_subset(snapshot, selected, new_files);
    let models: Vec<CodeModel> = snapshot
        .languages
        .iter()
        .map(|language| filter_model_to_new_files(&language.model, new_files, selected))
        .filter(|model| !model.files.is_empty())
        .collect();
    if models.is_empty() {
        return Ok(None);
    }
    let metric_plugins = plugins.metric_plugins();
    let disabled_hazard_rules = configuration.hazards.disabled_rules_set();
    let empty_hygiene = DependencyHygiene::default();
    let measurement = CommitMeasurement::of_languages(
        models,
        CommitMeasurementOptions {
            metric_plugins: &metric_plugins,
            boundaries: configuration.boundaries.as_ref(),
            dependencies: Some(&configuration.dependencies),
            duplication: configuration.duplication,
            orphan_files: orphan_files.as_ref(),
            dependency_hygiene: Some(&empty_hygiene),
            disabled_hazard_rules: Some(&disabled_hazard_rules),
        },
    )
    .map_err(|error| CheckError::metric_plugin(configuration.config_file(), error))?;
    Ok(Some(measurement))
}

fn filter_model_to_new_files(
    model: &CodeModel,
    new_files: &HashSet<RepoPath>,
    selected: Option<&SelectedPackage>,
) -> CodeModel {
    let matches = |path: &RepoPath| {
        new_files.contains(path)
            && selected.is_none_or(|pkg| pkg.matches(model.workspace.package_of(path)))
    };
    let files: Vec<_> = model
        .files
        .iter()
        .filter(|file| matches(&file.path))
        .cloned()
        .collect();
    if files.is_empty() {
        return CodeModel::default();
    }
    let runtime_imports = model
        .runtime_imports
        .iter()
        .filter(|import| matches(&import.importer))
        .cloned()
        .collect();
    CodeModel {
        files,
        workspace: model.workspace.clone(),
        runtime_imports,
        languages: model.languages.clone(),
        ..CodeModel::default()
    }
}

fn measure_all_languages_selected(
    tree: &Snapshot,
    plugins: &LoadedPlugins,
    configuration: &Configuration,
    selected: Option<&SelectedPackage>,
    lowering_cache: Option<&LoweringCache>,
) -> Result<CommitMeasurement, CheckError> {
    let named = plugins.as_named();
    let snapshot = measure_snapshot_with_cache(tree, &named, configuration, lowering_cache)?;
    measure_snapshot_to_commit(snapshot, plugins, configuration, selected)
}

fn measure_snapshot_to_commit(
    mut snapshot: MeasuredSnapshot,
    plugins: &LoadedPlugins,
    configuration: &Configuration,
    selected: Option<&SelectedPackage>,
) -> Result<CommitMeasurement, CheckError> {
    let metric_plugins = plugins.metric_plugins();
    let manifest_warnings = std::mem::take(&mut snapshot.manifest_warnings);
    let (orphan_files, _) = compute_orphan_files(&snapshot, selected);
    let (dependency_hygiene, _) =
        compute_dependency_hygiene(&snapshot, &configuration.dependencies, selected);
    if let Some(selected) = selected {
        filter_languages_to_package(&mut snapshot.languages, selected);
    }
    let models = snapshot
        .languages
        .into_iter()
        .map(|language| language.model)
        .collect();
    let disabled_hazard_rules = configuration.hazards.disabled_rules_set();
    CommitMeasurement::of_languages(
        models,
        CommitMeasurementOptions {
            metric_plugins: &metric_plugins,
            boundaries: configuration.boundaries.as_ref(),
            dependencies: Some(&configuration.dependencies),
            duplication: configuration.duplication,
            orphan_files: orphan_files.as_ref(),
            dependency_hygiene: Some(&dependency_hygiene),
            disabled_hazard_rules: Some(&disabled_hazard_rules),
        },
    )
    .map(|measurement| measurement.with_manifest_warnings(manifest_warnings))
    .map_err(|error| CheckError::metric_plugin(configuration.config_file(), error))
}
