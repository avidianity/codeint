use std::collections::BTreeMap;
use std::path::Path;
use std::string::FromUtf8Error;

use codeint_config::{
    Configuration, ConfigurationFileError, FindingsShown, LanguageCoverage, PluginHostConfig,
    SourceSetOverrides, read_configuration,
};
use codeint_git::{GitError, Repository, Snapshot};
use codeint_lang_dart::Dart;
use codeint_lang_php::Php;
use codeint_lang_python::Python;
use codeint_lang_rust::Rust;
use codeint_lang_ts::{TypeScriptAndJavaScript, vue_reference_ranges};
use codeint_model::{
    CodeModel, InvalidReferenceRange, LanguagePlugin, LowerError, Manifest, MetricPlugin,
    MetricPluginError, ReferenceRanges, RepoPath, ResolveError, SkippedFile, SourceFile, Workspace,
    WorkspaceError, choose_manifests, resolve_runtime_imports, settle_source_set,
};
use codeint_plugin_host::{
    PluginHostError, WasmLanguagePlugin, WasmMetricPlugin, load_language_plugins_with_limits,
    load_metric_plugins_with_limits,
};
use codeint_report::{MeasuredLanguage, Report};
use rayon::prelude::{IntoParallelIterator, IntoParallelRefIterator, ParallelIterator};

#[cfg(test)]
mod tests;

#[derive(Debug, thiserror::Error)]
pub(crate) enum CheckError {
    #[error(transparent)]
    Git(#[from] GitError),
    #[error(transparent)]
    Configuration(#[from] ConfigurationFileError),
    #[error("cannot resolve runtime imports")]
    Resolve(#[source] ResolveError),
    #[error("a Language plugin supplies an invalid Reference range")]
    ReferenceRange(#[source] InvalidReferenceRange),
    #[error(transparent)]
    Workspace(#[from] WorkspaceError),
    #[error(transparent)]
    PluginHost(#[from] PluginHostError),
    #[error(transparent)]
    MetricPlugin(#[from] MetricPluginError),
    #[error("{}", format_extension_conflict(first, second, extension, path.as_ref()))]
    ExtensionConflict {
        first: String,
        second: String,
        extension: String,
        path: Option<RepoPath>,
    },
}

fn format_extension_conflict(
    first: &str,
    second: &str,
    extension: &str,
    path: Option<&RepoPath>,
) -> String {
    match path {
        Some(path) => format!(
            "Language plugins {first} and {second} both claim extension .{extension} for {path}; assign separate directories in the languages setting of codeint.json"
        ),
        None => format!(
            "Language plugins {first} and {second} both claim extension .{extension}; assign separate directories in the languages setting of codeint.json"
        ),
    }
}

pub(crate) struct NamedLanguagePlugin<'plugin> {
    pub(crate) name: &'plugin str,
    pub(crate) plugin: &'plugin dyn LanguagePlugin,
}

pub(crate) const BUILT_IN_LANGUAGE_PLUGINS: [NamedLanguagePlugin<'static>; 5] = [
    NamedLanguagePlugin {
        name: "typescript-javascript",
        plugin: &TypeScriptAndJavaScript,
    },
    NamedLanguagePlugin {
        name: "rust",
        plugin: &Rust,
    },
    NamedLanguagePlugin {
        name: "php",
        plugin: &Php,
    },
    NamedLanguagePlugin {
        name: "dart",
        plugin: &Dart,
    },
    NamedLanguagePlugin {
        name: "python",
        plugin: &Python,
    },
];

pub(crate) struct LoadedPlugins {
    external_languages: Vec<(String, WasmLanguagePlugin)>,
    external_metrics: Vec<(String, WasmMetricPlugin)>,
}

impl LoadedPlugins {
    pub(crate) fn load(
        repository_root: &Path,
        configuration: &Configuration,
        plugin_host: &PluginHostConfig,
    ) -> Result<LoadedPlugins, CheckError> {
        let reserved: Vec<&str> = BUILT_IN_LANGUAGE_PLUGINS
            .iter()
            .map(|plugin| plugin.name)
            .collect();
        let external_languages = load_language_plugins_with_limits(
            repository_root,
            &configuration.plugins,
            &reserved,
            plugin_host,
            configuration.plugin_limits,
        )?;
        let external_metrics = load_metric_plugins_with_limits(
            repository_root,
            &configuration.plugins,
            &reserved,
            plugin_host,
            configuration.plugin_limits,
        )?;
        let metric_keys: Vec<&str> = external_metrics
            .iter()
            .map(|(_, plugin)| plugin.key())
            .collect();
        configuration.validate_metrics(&metric_keys)?;
        Ok(LoadedPlugins {
            external_languages,
            external_metrics,
        })
    }

    pub(crate) fn as_named(&self) -> Vec<NamedLanguagePlugin<'_>> {
        BUILT_IN_LANGUAGE_PLUGINS
            .iter()
            .map(|named| NamedLanguagePlugin {
                name: named.name,
                plugin: named.plugin,
            })
            .chain(
                self.external_languages
                    .iter()
                    .map(|(name, plugin)| NamedLanguagePlugin {
                        name: name.as_str(),
                        plugin: plugin as &dyn LanguagePlugin,
                    }),
            )
            .collect()
    }

    pub(crate) fn metric_plugins(&self) -> Vec<&dyn MetricPlugin> {
        self.external_metrics
            .iter()
            .map(|(_, plugin)| plugin as &dyn MetricPlugin)
            .collect()
    }
}

pub(crate) fn check(
    folder: &Path,
    findings: FindingsShown,
    plugin_host: &PluginHostConfig,
) -> Result<Report, CheckError> {
    let repository = Repository::discover(folder)?;
    let configuration = read_configuration(repository.root())?;
    let loaded = LoadedPlugins::load(repository.root(), &configuration, plugin_host)?;
    let plugins = loaded.as_named();
    let metric_plugins = loaded.metric_plugins();
    check_loaded(
        &repository,
        &configuration,
        &plugins,
        &metric_plugins,
        findings,
    )
}

#[cfg(test)]
fn check_with(
    folder: &Path,
    plugins: &[NamedLanguagePlugin<'_>],
    findings: FindingsShown,
) -> Result<Report, CheckError> {
    let repository = Repository::discover(folder)?;
    let configuration = read_configuration(repository.root())?;
    configuration.validate_metrics(&[])?;
    check_loaded(&repository, &configuration, plugins, &[], findings)
}

fn check_loaded(
    repository: &Repository,
    configuration: &Configuration,
    plugins: &[NamedLanguagePlugin<'_>],
    metric_plugins: &[&dyn MetricPlugin],
    findings: FindingsShown,
) -> Result<Report, CheckError> {
    let tree = repository.working_tree()?;
    let languages = measure_with(&tree, plugins, configuration)?;
    Ok(Report::summarize_with_metrics_and_boundaries(
        languages,
        tree.submodules().to_vec(),
        findings,
        &configuration.budgets,
        metric_plugins,
        configuration.boundaries.as_ref(),
    )?)
}

pub(crate) fn measure_with(
    tree: &Snapshot,
    plugins: &[NamedLanguagePlugin<'_>],
    configuration: &Configuration,
) -> Result<Vec<MeasuredLanguage>, CheckError> {
    let names: Vec<&str> = plugins.iter().map(|plugin| plugin.name).collect();
    let coverage = configuration.languages.coverage_of(&names)?;
    measure(
        tree,
        plugins,
        &coverage,
        &configuration.source_set_overrides,
    )
}

fn measure(
    tree: &Snapshot,
    plugins: &[NamedLanguagePlugin<'_>],
    coverage: &[LanguageCoverage<'_>],
    overrides: &SourceSetOverrides,
) -> Result<Vec<MeasuredLanguage>, CheckError> {
    let covered_files = assign_files_to_plugins(tree, plugins, coverage)?;
    let manifests_by_plugin = read_manifests(tree, plugins)?;
    let workspace = find_workspace(plugins, &manifests_by_plugin)?;
    let mut models: Vec<CodeModel> = plugins
        .iter()
        .map(|_| CodeModel {
            workspace: workspace.clone(),
            ..CodeModel::default()
        })
        .collect();
    let outcomes = if let Some(reader) = tree.working_tree_reader() {
        covered_files
            .par_iter()
            .map(|&(path, index)| {
                let bytes = reader.read(path)?;
                Ok((
                    index,
                    lower_bytes(bytes, path, plugins[index].plugin, overrides),
                ))
            })
            .collect::<Result<Vec<_>, CheckError>>()?
    } else {
        let loaded = covered_files
            .iter()
            .map(|&(path, index)| Ok((path, index, tree.read(path)?)))
            .collect::<Result<Vec<_>, CheckError>>()?;
        loaded
            .into_par_iter()
            .map(|(path, index, bytes)| {
                (
                    index,
                    lower_bytes(bytes, path, plugins[index].plugin, overrides),
                )
            })
            .collect()
    };
    for (index, outcome) in outcomes {
        let model = &mut models[index];
        match outcome {
            LoweredFile::Measured(file) => model.files.push(file),
            LoweredFile::DeclarationOnly(path) => model.declaration_only.push(path),
            LoweredFile::Skipped(skipped) => model.skipped.push(skipped),
        }
    }
    plugins
        .iter()
        .zip(models)
        .zip(manifests_by_plugin)
        .map(|((named, mut model), manifests)| {
            let importers: Vec<&SourceFile> = model.files.iter().collect();
            let reference_ranges = select_reference_ranges(named, &model, &manifests)?;
            model.runtime_imports =
                resolve_runtime_imports(named.plugin, tree.files(), manifests, &importers)
                    .map_err(CheckError::Resolve)?;
            Ok(MeasuredLanguage {
                plugin: named.name.to_owned(),
                model,
                reference_ranges,
            })
        })
        .collect()
}

fn select_reference_ranges(
    named: &NamedLanguagePlugin<'_>,
    model: &CodeModel,
    manifests: &[Manifest],
) -> Result<ReferenceRanges, CheckError> {
    if named.name == "typescript-javascript"
        && model
            .files
            .iter()
            .any(|f| f.path.as_str().ends_with(".vue"))
        && let Some(ranges) = vue_reference_ranges(manifests)
    {
        return ReferenceRanges::new(ranges).map_err(CheckError::ReferenceRange);
    }
    named
        .plugin
        .reference_ranges()
        .map_err(CheckError::ReferenceRange)
}

fn assign_files_to_plugins<'tree>(
    tree: &'tree Snapshot,
    plugins: &[NamedLanguagePlugin<'_>],
    coverage: &[LanguageCoverage<'_>],
) -> Result<Vec<(&'tree RepoPath, usize)>, CheckError> {
    let mut plugins_by_extension: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for (index, named) in plugins.iter().enumerate() {
        for extension in named.plugin.extensions() {
            let claimants = plugins_by_extension.entry(extension).or_default();
            if !claimants.contains(&index) {
                claimants.push(index);
            }
        }
    }
    for (extension, claimants) in &plugins_by_extension {
        if claimants.len() < 2 {
            continue;
        }
        let unrestricted: Vec<usize> = claimants
            .iter()
            .copied()
            .filter(|&index| coverage[index].covers_all())
            .collect();
        if unrestricted.len() >= 2 {
            let (first, second) = first_two_plugin_names(plugins, &unrestricted);
            return Err(CheckError::ExtensionConflict {
                first,
                second,
                extension: extension.clone(),
                path: None,
            });
        }
    }
    let mut covered_files = Vec::new();
    for path in tree.files() {
        let Some(extension) = path.extension() else {
            continue;
        };
        let Some(claimants) = plugins_by_extension.get(extension) else {
            continue;
        };
        let matching: Vec<usize> = claimants
            .iter()
            .copied()
            .filter(|&index| coverage[index].covers(path))
            .collect();
        match matching.as_slice() {
            [] => {}
            [index] => covered_files.push((path, *index)),
            many => {
                let (first, second) = first_two_plugin_names(plugins, many);
                return Err(CheckError::ExtensionConflict {
                    first,
                    second,
                    extension: extension.to_owned(),
                    path: Some(path.clone()),
                });
            }
        }
    }
    Ok(covered_files)
}

fn first_two_plugin_names(
    plugins: &[NamedLanguagePlugin<'_>],
    indices: &[usize],
) -> (String, String) {
    let mut names: Vec<&str> = indices.iter().map(|&index| plugins[index].name).collect();
    names.sort_unstable();
    (names[0].to_owned(), names[1].to_owned())
}

fn read_manifests(
    tree: &Snapshot,
    plugins: &[NamedLanguagePlugin<'_>],
) -> Result<Vec<Vec<Manifest>>, CheckError> {
    plugins
        .iter()
        .map(|named| {
            choose_manifests(named.plugin, tree.files())
                .map_err(CheckError::Resolve)?
                .into_iter()
                .map(|path| {
                    Ok(Manifest {
                        text: String::from_utf8_lossy(&tree.read(&path)?).into_owned(),
                        path: path.to_string(),
                    })
                })
                .collect()
        })
        .collect()
}

fn find_workspace(
    plugins: &[NamedLanguagePlugin<'_>],
    manifests_by_plugin: &[Vec<Manifest>],
) -> Result<Workspace, CheckError> {
    let mut member_roots = Vec::new();
    for (named, manifests) in plugins.iter().zip(manifests_by_plugin) {
        member_roots.extend(named.plugin.workspace_packages(manifests.clone())?);
    }
    Ok(Workspace::new(member_roots))
}

enum LoweredFile {
    Measured(SourceFile),
    DeclarationOnly(RepoPath),
    Skipped(SkippedFile),
}

fn lower_bytes(
    bytes: Vec<u8>,
    path: &RepoPath,
    plugin: &dyn LanguagePlugin,
    overrides: &SourceSetOverrides,
) -> LoweredFile {
    let text = match String::from_utf8(bytes) {
        Ok(text) => text,
        Err(error) => return LoweredFile::Skipped(not_utf8(path.clone(), &error)),
    };
    match plugin.lower(path, &text) {
        Ok(mut file) => {
            file.source_set =
                overrides.source_set(path, settle_source_set(path, &text, file.source_set));
            LoweredFile::Measured(SourceFile {
                path: path.clone(),
                model: file,
            })
        }
        Err(LowerError::DeclarationOnly) => LoweredFile::DeclarationOnly(path.clone()),
        Err(reason) => LoweredFile::Skipped(SkippedFile {
            path: path.clone(),
            reason: reason.to_string(),
        }),
    }
}

fn not_utf8(path: RepoPath, error: &FromUtf8Error) -> SkippedFile {
    SkippedFile {
        path,
        reason: format!("not valid UTF-8: {}", error.utf8_error()),
    }
}
