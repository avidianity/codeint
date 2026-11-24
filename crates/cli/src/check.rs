use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::fmt;
use std::path::Path;
use std::string::FromUtf8Error;

use codeint_config::{
    Codeowners, CodeownersError, ConfigFileKind, Configuration, ConfigurationFileError,
    DependenciesConfig, EntryPoints, FindingsShown, LanguageCoverage, OwnerFilter,
    PluginHostConfig, SourceSetOverrides, read_configuration,
};
use codeint_git::{GitError, Repository, Snapshot};
use codeint_lang_dart::Dart;
use codeint_lang_php::Php;
use codeint_lang_python::Python;
use codeint_lang_rust::Rust;
use codeint_lang_ts::{TypeScriptAndJavaScript, react_reference_ranges, vue_reference_ranges};
use codeint_metrics::{DependencyHygiene, OrphanFiles};
use codeint_model::{
    CodeModel, DeclaredDep, DependencyManifest, InvalidReferenceRange, LanguagePlugin,
    LanguagePluginError, LowerError, Manifest, MetricPlugin, MetricPluginError, Package,
    ReferenceRanges, RepoPath, ResolveError, SkippedFile, SourceFile, Workspace, WorkspaceError,
    choose_manifests, resolve_runtime_imports, settle_source_set,
};
use codeint_plugin_host::{
    PluginHostError, WasmLanguagePlugin, WasmMetricPlugin, load_language_plugins_with_limits,
    load_metric_plugins_with_limits,
};
use codeint_report::{LanguageSchema, MeasuredLanguage, OwnershipOptions, Report, ReportOptions};
use rayon::prelude::{IntoParallelIterator, IntoParallelRefIterator, ParallelIterator};

#[cfg(test)]
mod tests;

#[derive(Debug)]
pub(crate) struct ConfigScopedError<E> {
    pub(crate) config_file: ConfigFileKind,
    pub(crate) error: E,
}

impl<E: fmt::Display> fmt::Display for ConfigScopedError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.error.fmt(formatter)
    }
}

impl<E: std::error::Error + 'static> std::error::Error for ConfigScopedError<E> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.error.source()
    }
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum CheckError {
    #[error(transparent)]
    Git(#[from] GitError),
    #[error(transparent)]
    Configuration(#[from] ConfigurationFileError),
    #[error(transparent)]
    Codeowners(#[from] CodeownersError),
    #[error("cannot resolve runtime imports")]
    Resolve(#[source] ResolveError),
    #[error("a Language plugin supplies an invalid Reference range")]
    ReferenceRange(#[source] InvalidReferenceRange),
    #[error(transparent)]
    Workspace(#[from] WorkspaceError),
    #[error(transparent)]
    PluginHost(ConfigScopedError<PluginHostError>),
    #[error(transparent)]
    LanguagePlugin(ConfigScopedError<LanguagePluginError>),
    #[error(transparent)]
    MetricPlugin(ConfigScopedError<MetricPluginError>),
    #[error(
        "{}",
        format_extension_conflict(first, second, extension, path.as_ref(), *config_file)
    )]
    ExtensionConflict {
        config_file: ConfigFileKind,
        first: String,
        second: String,
        extension: String,
        path: Option<RepoPath>,
    },
    #[error("{}", format_unknown_workspace_package(requested, available))]
    UnknownWorkspacePackage {
        requested: String,
        available: Vec<String>,
    },
}

impl CheckError {
    pub(crate) fn plugin_host(config_file: ConfigFileKind, error: PluginHostError) -> CheckError {
        CheckError::PluginHost(ConfigScopedError { config_file, error })
    }

    pub(crate) fn language_plugin(
        config_file: ConfigFileKind,
        error: LanguagePluginError,
    ) -> CheckError {
        CheckError::LanguagePlugin(ConfigScopedError { config_file, error })
    }

    pub(crate) fn metric_plugin(
        config_file: ConfigFileKind,
        error: MetricPluginError,
    ) -> CheckError {
        CheckError::MetricPlugin(ConfigScopedError { config_file, error })
    }
}

fn format_unknown_workspace_package(requested: &str, available: &[String]) -> String {
    if available.is_empty() {
        format!(
            "Workspace package {requested:?} was not found; no Workspace packages were discovered in this repository"
        )
    } else {
        format!(
            "Workspace package {requested:?} was not found; discovered Workspace packages: {}",
            available.join(", ")
        )
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum SelectedPackage {
    Root,
    Member(RepoPath),
}

impl SelectedPackage {
    pub(crate) fn as_str(&self) -> &str {
        match self {
            SelectedPackage::Root => ".",
            SelectedPackage::Member(root) => root.as_str(),
        }
    }

    pub(crate) fn matches(&self, package: Package<'_>) -> bool {
        match (self, package) {
            (SelectedPackage::Root, Package::Root) => true,
            (SelectedPackage::Member(selected), Package::Member(actual)) => selected == actual,
            _ => false,
        }
    }
}

pub(crate) fn select_workspace_package(
    workspace: &Workspace,
    requested: &str,
) -> Result<SelectedPackage, CheckError> {
    let available: Vec<String> = workspace.member_roots().map(ToString::to_string).collect();
    if !workspace.has_members() {
        return Err(CheckError::UnknownWorkspacePackage {
            requested: requested.to_owned(),
            available,
        });
    }
    let normalized = normalize_package_path(requested);
    if normalized == "." {
        return Ok(SelectedPackage::Root);
    }
    if let Ok(candidate) = RepoPath::new(normalized)
        && workspace.member_roots().any(|root| root == &candidate)
    {
        return Ok(SelectedPackage::Member(candidate));
    }
    Err(CheckError::UnknownWorkspacePackage {
        requested: requested.to_owned(),
        available,
    })
}

fn normalize_package_path(raw: &str) -> String {
    let replaced = raw.replace('\\', "/");
    let mut trimmed = replaced.as_str();
    while let Some(stripped) = trimmed.strip_prefix("./") {
        trimmed = stripped;
    }
    trimmed = trimmed.trim_end_matches('/');
    if trimmed.is_empty() && !raw.is_empty() && replaced.trim_end_matches('/') == "." {
        return ".".to_owned();
    }
    trimmed.to_owned()
}

pub(crate) fn workspace_of_languages(languages: &[MeasuredLanguage]) -> Workspace {
    let mut workspace = Workspace::default();
    for language in languages {
        workspace.join(language.model.workspace.clone());
    }
    workspace
}

pub(crate) fn filter_model_to_package(model: &mut CodeModel, selected: &SelectedPackage) {
    let workspace = std::mem::take(&mut model.workspace);
    model
        .files
        .retain(|file| selected.matches(workspace.package_of(&file.path)));
    model
        .skipped
        .retain(|file| selected.matches(workspace.package_of(&file.path)));
    model
        .declaration_only
        .retain(|path| selected.matches(workspace.package_of(path)));
    model
        .runtime_imports
        .retain(|import| selected.matches(workspace.package_of(&import.importer)));
    model
        .dependency_manifests
        .retain(|manifest| selected.matches(workspace.package_of(&manifest.path)));
    let retained_roots: HashSet<String> = model
        .dependency_manifests
        .iter()
        .map(|manifest| manifest.package_root.clone())
        .collect();
    model.declared_dependencies.retain(|dep| {
        retained_roots.contains(&dep.package_root)
            || matches_selected_package_root(selected, &dep.package_root)
    });
    model.manifest_used_dependencies.retain(|dep| {
        retained_roots.contains(&dep.package_root)
            || matches_selected_package_root(selected, &dep.package_root)
    });
}

fn matches_selected_package_root(selected: &SelectedPackage, package_root: &str) -> bool {
    match selected {
        SelectedPackage::Root => package_root.is_empty(),
        SelectedPackage::Member(root) => package_root == root.as_str(),
    }
}

fn package_root_matches_selected(
    workspace: &Workspace,
    selected: &SelectedPackage,
    package_root: &str,
) -> bool {
    let actual = if package_root.is_empty() {
        Package::Root
    } else if let Ok(synthetic) = RepoPath::new(format!("{package_root}/manifest")) {
        workspace.package_of(&synthetic)
    } else {
        return matches_selected_package_root(selected, package_root);
    };
    selected.matches(actual)
}

pub(crate) fn filter_languages_to_package(
    languages: &mut [MeasuredLanguage],
    selected: &SelectedPackage,
) {
    for language in languages {
        filter_model_to_package(&mut language.model, selected);
    }
}

fn format_extension_conflict(
    first: &str,
    second: &str,
    extension: &str,
    path: Option<&RepoPath>,
    config_file: ConfigFileKind,
) -> String {
    match path {
        Some(path) => format!(
            "Language plugins {first} and {second} both claim extension .{extension} for {path}; assign separate directories in the languages setting of {config_file}"
        ),
        None => format!(
            "Language plugins {first} and {second} both claim extension .{extension}; assign separate directories in the languages setting of {config_file}"
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

pub(crate) fn built_in_languages() -> Vec<LanguageSchema> {
    BUILT_IN_LANGUAGE_PLUGINS
        .iter()
        .map(|named| LanguageSchema {
            name: named.name.to_owned(),
            extensions: named.plugin.extensions(),
        })
        .collect()
}

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
        let config_file = configuration.config_file();
        let external_languages = load_language_plugins_with_limits(
            repository_root,
            &configuration.plugins,
            &reserved,
            plugin_host,
            configuration.plugin_limits,
        )
        .map_err(|error| CheckError::plugin_host(config_file, error))?;
        let external_metrics = load_metric_plugins_with_limits(
            repository_root,
            &configuration.plugins,
            &reserved,
            plugin_host,
            configuration.plugin_limits,
        )
        .map_err(|error| CheckError::plugin_host(config_file, error))?;
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

pub(crate) struct WorkspaceContext {
    pub(crate) repository: Repository,
    pub(crate) configuration: Configuration,
    pub(crate) plugins: LoadedPlugins,
}

impl WorkspaceContext {
    pub(crate) fn load(
        folder: &Path,
        plugin_host: &PluginHostConfig,
    ) -> Result<WorkspaceContext, CheckError> {
        let repository = Repository::discover(folder)?;
        let configuration = read_configuration(repository.root())?;
        let plugins = LoadedPlugins::load(repository.root(), &configuration, plugin_host)?;
        Ok(WorkspaceContext {
            repository,
            configuration,
            plugins,
        })
    }

    pub(crate) fn measure_working_tree_model(&self) -> Result<CodeModel, CheckError> {
        let plugins = self.plugins.as_named();
        let tree = self.repository.working_tree()?;
        let languages = measure_with(&tree, &plugins, &self.configuration)?;
        Ok(CodeModel::merge(
            languages.into_iter().map(|language| language.model),
        ))
    }
}

pub(crate) struct PreparedRun {
    pub(crate) context: WorkspaceContext,
    pub(crate) codeowners: Option<Codeowners>,
    pub(crate) owner_filter: Option<OwnerFilter>,
}

impl PreparedRun {
    pub(crate) fn load(
        folder: &Path,
        owner_pattern: Option<&str>,
        plugin_host: &PluginHostConfig,
    ) -> Result<PreparedRun, CheckError> {
        let context = WorkspaceContext::load(folder, plugin_host)?;
        let codeowners = Codeowners::load(context.repository.root())?;
        let owner_filter = owner_pattern.map(OwnerFilter::new).transpose()?;
        Ok(PreparedRun {
            context,
            codeowners,
            owner_filter,
        })
    }

    pub(crate) fn ownership(&self) -> OwnershipOptions<'_> {
        OwnershipOptions {
            codeowners: self.codeowners.as_ref(),
            owner_filter: self.owner_filter.as_ref(),
        }
    }
}

pub(crate) fn check(
    folder: &Path,
    findings: FindingsShown,
    package: Option<&str>,
    owner: Option<&str>,
    plugin_host: &PluginHostConfig,
) -> Result<Report, CheckError> {
    let run = PreparedRun::load(folder, owner, plugin_host)?;
    let plugins = run.context.plugins.as_named();
    let metric_plugins = run.context.plugins.metric_plugins();
    check_loaded(
        &run.context.repository,
        &run.context.configuration,
        &plugins,
        &metric_plugins,
        findings,
        package,
        run.ownership(),
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
    check_loaded(
        &repository,
        &configuration,
        plugins,
        &[],
        findings,
        None,
        OwnershipOptions::default(),
    )
}

pub(crate) struct MeasuredSnapshot {
    pub(crate) languages: Vec<MeasuredLanguage>,
    pub(crate) entry_points_by_language: BTreeMap<String, Vec<RepoPath>>,
    pub(crate) manifests_by_language: BTreeMap<String, Vec<Manifest>>,
}

pub(crate) fn compute_orphan_files(
    snapshot: &MeasuredSnapshot,
    selected: Option<&SelectedPackage>,
) -> (Option<OrphanFiles>, BTreeMap<String, OrphanFiles>) {
    let mut by_language = BTreeMap::new();
    for language in &snapshot.languages {
        let entry_points = snapshot
            .entry_points_by_language
            .get(&language.plugin)
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        if let Some(measured) =
            OrphanFiles::measure_filtered(&language.model, entry_points, |path| {
                selected.is_none_or(|pkg| pkg.matches(language.model.workspace.package_of(path)))
            })
        {
            by_language.insert(language.plugin.clone(), measured);
        }
    }
    let total = OrphanFiles::merge(by_language.values().cloned());
    (total, by_language)
}

pub(crate) fn compute_dependency_hygiene(
    snapshot: &MeasuredSnapshot,
    dependencies: &DependenciesConfig,
    selected: Option<&SelectedPackage>,
) -> (DependencyHygiene, BTreeMap<String, DependencyHygiene>) {
    let ignored = dependencies.ignored();
    let mut by_language = BTreeMap::new();
    for language in &snapshot.languages {
        let measured = DependencyHygiene::measure_filtered(&language.model, ignored, |root| {
            selected.is_none_or(|pkg| {
                package_root_matches_selected(&language.model.workspace, pkg, root)
            })
        });
        by_language.insert(language.plugin.clone(), measured);
    }
    let total = DependencyHygiene::merge(by_language.values().cloned());
    (total, by_language)
}

fn check_loaded(
    repository: &Repository,
    configuration: &Configuration,
    plugins: &[NamedLanguagePlugin<'_>],
    metric_plugins: &[&dyn MetricPlugin],
    findings: FindingsShown,
    package: Option<&str>,
    ownership: OwnershipOptions<'_>,
) -> Result<Report, CheckError> {
    let tree = repository.working_tree()?;
    let mut snapshot = measure_snapshot(&tree, plugins, configuration)?;
    let selected = if let Some(requested) = package {
        let workspace = workspace_of_languages(&snapshot.languages);
        Some((select_workspace_package(&workspace, requested)?, workspace))
    } else {
        None
    };
    let selected_ref = selected.as_ref().map(|(pkg, _)| pkg);
    let (orphan_files, orphan_files_by_language) = compute_orphan_files(&snapshot, selected_ref);
    let (dependency_hygiene, dependency_hygiene_by_language) =
        compute_dependency_hygiene(&snapshot, &configuration.dependencies, selected_ref);
    if let Some((selected_pkg, workspace)) = &selected {
        filter_languages_to_package(&mut snapshot.languages, selected_pkg);
        refresh_reference_ranges_for_package(
            &mut snapshot.languages,
            plugins,
            &snapshot.manifests_by_language,
            workspace,
            selected_pkg,
        )?;
    }
    Report::summarize_with_options(
        snapshot.languages,
        tree.submodules().to_vec(),
        ReportOptions {
            findings,
            budgets: &configuration.budgets,
            metric_plugins,
            boundaries: configuration.boundaries.as_ref(),
            dependencies: &configuration.dependencies,
            dependency_hygiene: Some(&dependency_hygiene),
            dependency_hygiene_by_language: Some(&dependency_hygiene_by_language),
            duplication: configuration.duplication,
            orphan_files: orphan_files.as_ref(),
            orphan_files_by_language: Some(&orphan_files_by_language),
            ownership,
        },
    )
    .map_err(|error| CheckError::metric_plugin(configuration.config_file(), error))
}

pub(crate) fn measure_with(
    tree: &Snapshot,
    plugins: &[NamedLanguagePlugin<'_>],
    configuration: &Configuration,
) -> Result<Vec<MeasuredLanguage>, CheckError> {
    Ok(measure_snapshot(tree, plugins, configuration)?.languages)
}

pub(crate) fn measure_snapshot(
    tree: &Snapshot,
    plugins: &[NamedLanguagePlugin<'_>],
    configuration: &Configuration,
) -> Result<MeasuredSnapshot, CheckError> {
    let names: Vec<&str> = plugins.iter().map(|plugin| plugin.name).collect();
    let coverage = configuration.languages.coverage_of(&names)?;
    measure(
        tree,
        plugins,
        &coverage,
        &configuration.source_set_overrides,
        &configuration.entry_points,
        configuration.config_file(),
    )
}

fn measure(
    tree: &Snapshot,
    plugins: &[NamedLanguagePlugin<'_>],
    coverage: &[LanguageCoverage<'_>],
    overrides: &SourceSetOverrides,
    configured_entry_points: &EntryPoints,
    config_file: ConfigFileKind,
) -> Result<MeasuredSnapshot, CheckError> {
    let covered_files = assign_files_to_plugins(tree, plugins, coverage, config_file)?;
    let manifests_by_plugin = read_manifests(tree, plugins)?;
    let plugin_members = find_plugin_workspaces(plugins, &manifests_by_plugin)?;
    let workspace = Workspace::new(plugin_members.iter().flatten().cloned());
    let outcomes = lower_covered_files(tree, plugins, &covered_files, overrides)?;
    let models = collect_plugin_models(plugins.len(), workspace, outcomes);
    resolve_measured_snapshot(
        tree,
        plugins,
        models,
        manifests_by_plugin,
        &plugin_members,
        configured_entry_points,
        config_file,
    )
}

fn lower_covered_files(
    tree: &Snapshot,
    plugins: &[NamedLanguagePlugin<'_>],
    covered_files: &[(&RepoPath, usize)],
    overrides: &SourceSetOverrides,
) -> Result<Vec<(usize, LoweredFile)>, CheckError> {
    if let Some(reader) = tree.working_tree_reader() {
        return covered_files
            .par_iter()
            .map(|&(path, index)| {
                let bytes = reader.read(path)?;
                Ok((
                    index,
                    lower_bytes(bytes, path, plugins[index].plugin, overrides),
                ))
            })
            .collect();
    }
    let loaded = covered_files
        .iter()
        .map(|&(path, index)| Ok((path, index, tree.read(path)?)))
        .collect::<Result<Vec<_>, CheckError>>()?;
    Ok(loaded
        .into_par_iter()
        .map(|(path, index, bytes)| {
            (
                index,
                lower_bytes(bytes, path, plugins[index].plugin, overrides),
            )
        })
        .collect())
}

fn collect_plugin_models(
    plugin_count: usize,
    workspace: Workspace,
    outcomes: Vec<(usize, LoweredFile)>,
) -> Vec<CodeModel> {
    let mut models: Vec<CodeModel> = (0..plugin_count)
        .map(|_| CodeModel {
            workspace: workspace.clone(),
            ..CodeModel::default()
        })
        .collect();
    for (index, outcome) in outcomes {
        let model = &mut models[index];
        match outcome {
            LoweredFile::Measured(file) => model.files.push(file),
            LoweredFile::DeclarationOnly(path) => model.declaration_only.push(path),
            LoweredFile::Skipped(skipped) => model.skipped.push(skipped),
        }
    }
    models
}

fn resolve_measured_snapshot(
    tree: &Snapshot,
    plugins: &[NamedLanguagePlugin<'_>],
    models: Vec<CodeModel>,
    manifests_by_plugin: Vec<Vec<Manifest>>,
    plugin_members: &[Vec<RepoPath>],
    configured_entry_points: &EntryPoints,
    config_file: ConfigFileKind,
) -> Result<MeasuredSnapshot, CheckError> {
    let mut languages = Vec::with_capacity(plugins.len());
    let mut entry_points_by_language = BTreeMap::new();
    let mut manifests_by_language = BTreeMap::new();
    for (((named, model), manifests), members) in plugins
        .iter()
        .zip(models)
        .zip(manifests_by_plugin)
        .zip(plugin_members)
    {
        let manifest_entries = named
            .plugin
            .entry_points(tree.files(), manifests.clone())
            .map_err(|error| CheckError::language_plugin(config_file, error))?;
        let combined_entries =
            combine_entry_points(manifest_entries, &model.files, configured_entry_points);
        let retained_manifests = if named.name == "typescript-javascript" {
            manifests.clone()
        } else {
            Vec::new()
        };
        let measured =
            finalize_language_model(named, model, manifests, members, tree.files(), config_file)?;
        entry_points_by_language.insert(named.name.to_owned(), combined_entries);
        manifests_by_language.insert(named.name.to_owned(), retained_manifests);
        languages.push(measured);
    }
    Ok(MeasuredSnapshot {
        languages,
        entry_points_by_language,
        manifests_by_language,
    })
}

fn combine_entry_points(
    manifest_entries: Vec<RepoPath>,
    files: &[SourceFile],
    configured: &EntryPoints,
) -> Vec<RepoPath> {
    let mut entries: BTreeSet<RepoPath> = manifest_entries.into_iter().collect();
    if !configured.is_empty() {
        for file in files {
            if configured.matches(&file.path) {
                entries.insert(file.path.clone());
            }
        }
    }
    entries.into_iter().collect()
}

fn finalize_language_model(
    named: &NamedLanguagePlugin<'_>,
    mut model: CodeModel,
    manifests: Vec<Manifest>,
    plugin_members: &[RepoPath],
    repository_paths: &[RepoPath],
    config_file: ConfigFileKind,
) -> Result<MeasuredLanguage, CheckError> {
    let importers: Vec<&SourceFile> = model.files.iter().collect();
    let reference_ranges = select_reference_ranges(named, &model, &manifests)?;
    let declared_dependencies = named
        .plugin
        .declared_dependencies(manifests.clone())
        .map_err(|error| CheckError::language_plugin(config_file, error))?;
    model.dependency_manifests = collect_dependency_manifests(
        &manifests,
        plugin_members,
        &declared_dependencies,
        &model.files,
    );
    model.manifest_used_dependencies = crate::package_scripts::extract_manifest_used_dependencies(
        &manifests,
        plugin_members,
        &declared_dependencies,
    );
    model.declared_dependencies = declared_dependencies;
    model.runtime_imports =
        resolve_runtime_imports(named.plugin, repository_paths, manifests, &importers)
            .map_err(CheckError::Resolve)?;
    Ok(MeasuredLanguage {
        plugin: named.name.to_owned(),
        model,
        reference_ranges,
    })
}

fn collect_dependency_manifests(
    manifests: &[Manifest],
    plugin_members: &[RepoPath],
    declared: &[DeclaredDep],
    files: &[SourceFile],
) -> Vec<DependencyManifest> {
    let declared_roots: HashSet<&str> = declared
        .iter()
        .map(|dep| dep.package_root.as_str())
        .collect();
    let member_roots: HashSet<&str> = plugin_members.iter().map(RepoPath::as_str).collect();
    let mut collected: Vec<DependencyManifest> = manifests
        .iter()
        .filter(|manifest| should_include_manifest(manifest, &declared_roots, &member_roots, files))
        .filter_map(|manifest| {
            let repo_path = RepoPath::new(&manifest.path).ok()?;
            Some(DependencyManifest {
                package_root: manifest_root(&manifest.path).to_owned(),
                path: repo_path,
            })
        })
        .collect();
    collected.sort();
    collected.dedup();
    collected
}

fn should_include_manifest(
    manifest: &Manifest,
    declared_roots: &HashSet<&str>,
    member_roots: &HashSet<&str>,
    files: &[SourceFile],
) -> bool {
    if !is_dependency_manifest_path(&manifest.path) || is_virtual_cargo_workspace(manifest) {
        return false;
    }
    let root = manifest_root(&manifest.path);
    if declared_roots.contains(root) || member_roots.contains(root) {
        return true;
    }
    owns_any_file(root, files)
}

fn is_virtual_cargo_workspace(manifest: &Manifest) -> bool {
    let is_cargo = manifest.path == "Cargo.toml" || manifest.path.ends_with("/Cargo.toml");
    if !is_cargo {
        return false;
    }
    toml::from_str::<toml::Value>(&manifest.text)
        .ok()
        .is_some_and(|doc| doc.get("workspace").is_some() && doc.get("package").is_none())
}

fn owns_any_file(root: &str, files: &[SourceFile]) -> bool {
    if root.is_empty() {
        return !files.is_empty();
    }
    let prefix = format!("{root}/");
    files
        .iter()
        .any(|file| file.path.as_str().starts_with(&prefix))
}

fn is_dependency_manifest_path(path: &str) -> bool {
    let ignored_dir = path.split('/').any(|segment| {
        matches!(
            segment,
            "node_modules" | "target" | "vendor" | ".venv" | "venv" | ".dart_tool"
        )
    });
    if ignored_dir {
        return false;
    }
    let file_name = path.rsplit_once('/').map_or(path, |(_, name)| name);
    let is_ts_config = file_name.starts_with("tsconfig") && file_name.ends_with(".json");
    !is_ts_config && file_name != "pnpm-workspace.yaml"
}

fn manifest_root(path: &str) -> &str {
    path.rsplit_once('/').map_or("", |(dir, _)| dir)
}

fn scoped_manifests_for_package(
    manifests: &[Manifest],
    workspace: &Workspace,
    selected: &SelectedPackage,
) -> Vec<Manifest> {
    let own: Vec<Manifest> = manifests
        .iter()
        .filter(|manifest| {
            package_root_matches_selected(workspace, selected, manifest_root(&manifest.path))
        })
        .cloned()
        .collect();
    if matches!(selected, SelectedPackage::Root)
        || vue_reference_ranges(&own).is_some()
        || react_reference_ranges(&own).is_some()
    {
        return own;
    }
    manifests
        .iter()
        .filter(|manifest| manifest_root(&manifest.path).is_empty())
        .cloned()
        .collect()
}

fn refresh_reference_ranges_for_package(
    languages: &mut [MeasuredLanguage],
    plugins: &[NamedLanguagePlugin<'_>],
    manifests_by_language: &BTreeMap<String, Vec<Manifest>>,
    workspace: &Workspace,
    selected: &SelectedPackage,
) -> Result<(), CheckError> {
    for language in languages {
        if language.plugin != "typescript-javascript" {
            continue;
        }
        let Some(named) = plugins.iter().find(|plugin| plugin.name == language.plugin) else {
            continue;
        };
        let scoped = manifests_by_language
            .get(&language.plugin)
            .map(|manifests| scoped_manifests_for_package(manifests, workspace, selected))
            .unwrap_or_default();
        language.reference_ranges = select_reference_ranges(named, &language.model, &scoped)?;
    }
    Ok(())
}

fn select_reference_ranges(
    named: &NamedLanguagePlugin<'_>,
    model: &CodeModel,
    manifests: &[Manifest],
) -> Result<ReferenceRanges, CheckError> {
    if named.name == "typescript-javascript" {
        let has_vue = model
            .files
            .iter()
            .any(|f| f.path.as_str().ends_with(".vue"));
        let has_jsx_or_tsx = model
            .files
            .iter()
            .any(|f| f.path.as_str().ends_with(".tsx") || f.path.as_str().ends_with(".jsx"));
        let react_ranges = has_jsx_or_tsx
            .then(|| react_reference_ranges(manifests))
            .flatten();
        if has_vue {
            if react_ranges.is_none()
                && let Some(ranges) = vue_reference_ranges(manifests)
            {
                return ReferenceRanges::new(ranges).map_err(CheckError::ReferenceRange);
            }
        } else if let Some(ranges) = react_ranges {
            return ReferenceRanges::new(ranges).map_err(CheckError::ReferenceRange);
        }
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
    config_file: ConfigFileKind,
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
                config_file,
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
                    config_file,
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

pub(crate) fn read_manifests(
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

pub(crate) fn find_workspace(
    plugins: &[NamedLanguagePlugin<'_>],
    manifests_by_plugin: &[Vec<Manifest>],
) -> Result<Workspace, CheckError> {
    let member_roots = find_plugin_workspaces(plugins, manifests_by_plugin)?
        .into_iter()
        .flatten();
    Ok(Workspace::new(member_roots))
}

fn find_plugin_workspaces(
    plugins: &[NamedLanguagePlugin<'_>],
    manifests_by_plugin: &[Vec<Manifest>],
) -> Result<Vec<Vec<RepoPath>>, CheckError> {
    plugins
        .iter()
        .zip(manifests_by_plugin)
        .map(|(named, manifests)| Ok(named.plugin.workspace_packages(manifests.clone())?))
        .collect()
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
