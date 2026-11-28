use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};

use codeint_config::{
    Baseline, Codeowners, ConfigFileKind, Configuration, DependenciesConfig, EntryPoints,
    FindingsShown, LanguageCoverage, OwnerFilter, PluginHostConfig, PluginKind, read_configuration,
};
use codeint_git::{Repository, Snapshot};
use codeint_metrics::{DependencyHygiene, OrphanFiles};
use codeint_model::{
    CodeModel, LanguagePlugin, Manifest, MetricPlugin, RepoPath, SourceFile, SourceSet, Workspace,
};
use codeint_plugin_host::{
    NamedLanguagePlugin, WasmLanguagePlugin, WasmMetricPlugin, built_in_language_descriptors,
    built_in_language_names, built_in_language_plugins, load_language_plugins_with_limits,
    load_metric_plugins_with_limits,
};
use codeint_report::{
    FileDiagnosticEntry, LanguageSchema, MeasuredLanguage, OwnershipOptions, Report, ReportExtras,
    ReportOptions, ScopeSummaryEntry,
};

mod control_characters;
mod coverage;
mod errors;
mod frameworks;
mod lowering;
mod manifests;
mod package_filter;
mod scope;
#[cfg(test)]
mod tests;

pub(crate) use errors::CheckError;
pub(crate) use lowering::{LoweringCache, VersionedLanguagePlugin};
pub(crate) use manifests::{find_workspace, read_manifests};
#[cfg(test)]
use package_filter::filter_model_to_package;
pub(crate) use package_filter::{
    SelectedPackage, filter_languages_to_package, select_workspace_package, workspace_of_languages,
};

pub(crate) fn built_in_languages() -> Vec<LanguageSchema> {
    built_in_language_descriptors()
        .into_iter()
        .map(|descriptor| LanguageSchema {
            name: descriptor.name,
            extensions: descriptor.extensions,
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
        let reserved = built_in_language_names();
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
        let loaded = LoadedPlugins {
            external_languages,
            external_metrics,
        };
        let named = loaded.as_named();
        scope::validate_configured_dialects(&named, configuration)?;
        scope::validate_disabled_hazard_rules(&named, configuration)?;
        frameworks::validate_configured_framework_profiles(&named, configuration)?;
        Ok(loaded)
    }

    pub(crate) fn as_named(&self) -> Vec<NamedLanguagePlugin<'_>> {
        built_in_language_plugins()
            .iter()
            .copied()
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

#[derive(Clone, Copy)]
pub(crate) struct WorkspaceRef<'a> {
    pub(crate) repository: &'a Repository,
    pub(crate) configuration: &'a Configuration,
    pub(crate) plugins: &'a LoadedPlugins,
}

impl<'a> WorkspaceRef<'a> {
    pub(crate) fn measure_working_tree_model(self) -> Result<CodeModel, CheckError> {
        self.measure_working_tree_model_for_package(None)
    }

    pub(crate) fn measure_working_tree_model_for_package(
        self,
        package: Option<&str>,
    ) -> Result<CodeModel, CheckError> {
        let plugins = self.plugins.as_named();
        let tree = self.repository.working_tree()?;
        let mut languages = measure_with(&tree, &plugins, self.configuration)?;
        let workspace = if let Some(requested) = package {
            let workspace = workspace_of_languages(&languages);
            let selected = select_workspace_package(&workspace, requested)?;
            filter_languages_to_package(&mut languages, &selected);
            Some(workspace)
        } else {
            None
        };
        let mut model = CodeModel::merge(languages.into_iter().map(|language| language.model));
        if let Some(workspace) = workspace {
            model.workspace = workspace;
        }
        Ok(model)
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

    pub(crate) fn as_ref(&self) -> WorkspaceRef<'_> {
        WorkspaceRef {
            repository: &self.repository,
            configuration: &self.configuration,
            plugins: &self.plugins,
        }
    }
}

pub(crate) struct PreparedOwnership {
    pub(crate) codeowners: Option<Codeowners>,
    pub(crate) owner_filter: Option<OwnerFilter>,
}

impl PreparedOwnership {
    pub(crate) fn load(
        repository_root: &Path,
        owner_pattern: Option<&str>,
    ) -> Result<PreparedOwnership, CheckError> {
        let codeowners = Codeowners::load(repository_root)?;
        let owner_filter = owner_pattern.map(OwnerFilter::new).transpose()?;
        Ok(PreparedOwnership {
            codeowners,
            owner_filter,
        })
    }

    pub(crate) fn options(&self) -> OwnershipOptions<'_> {
        OwnershipOptions {
            codeowners: self.codeowners.as_ref(),
            owner_filter: self.owner_filter.as_ref(),
        }
    }
}

pub(crate) struct CheckScope<'a> {
    pub(crate) findings: FindingsShown,
    pub(crate) package: Option<&'a str>,
    pub(crate) owner: Option<&'a str>,
    pub(crate) coverage_reports: &'a [PathBuf],
    pub(crate) ignore_baseline: bool,
}

pub(crate) fn check(
    folder: &Path,
    scope: CheckScope<'_>,
    plugin_host: &PluginHostConfig,
) -> Result<Report, CheckError> {
    let context = WorkspaceContext::load(folder, plugin_host)?;
    check_with_workspace(folder, scope, context.as_ref())
}

pub(crate) fn check_with_workspace(
    folder: &Path,
    scope: CheckScope<'_>,
    workspace: WorkspaceRef<'_>,
) -> Result<Report, CheckError> {
    let ownership = PreparedOwnership::load(workspace.repository.root(), scope.owner)?;
    let plugins = workspace.plugins.as_named();
    let metric_plugins = workspace.plugins.metric_plugins();
    let baseline = if scope.ignore_baseline {
        None
    } else {
        Baseline::load_optional(workspace.repository.root())?
    };
    check_loaded(
        workspace.repository,
        workspace.configuration,
        &plugins,
        &metric_plugins,
        CheckRequest {
            folder,
            findings: scope.findings,
            package: scope.package,
            cli_coverage_reports: scope.coverage_reports,
            ownership: ownership.options(),
            baseline: baseline.as_ref(),
        },
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
    scope::validate_configured_dialects(plugins, &configuration)?;
    scope::validate_disabled_hazard_rules(plugins, &configuration)?;
    frameworks::validate_configured_framework_profiles(plugins, &configuration)?;
    check_loaded(
        &repository,
        &configuration,
        plugins,
        &[],
        CheckRequest {
            folder,
            findings,
            package: None,
            cli_coverage_reports: &[],
            ownership: OwnershipOptions::default(),
            baseline: None,
        },
    )
}

struct CheckRequest<'a> {
    folder: &'a Path,
    findings: FindingsShown,
    package: Option<&'a str>,
    cli_coverage_reports: &'a [PathBuf],
    ownership: OwnershipOptions<'a>,
    baseline: Option<&'a Baseline>,
}

pub(crate) struct MeasuredSnapshot {
    pub(crate) languages: Vec<MeasuredLanguage>,
    pub(crate) entry_points_by_language: BTreeMap<String, Vec<RepoPath>>,
    pub(crate) explicit_entry_points_by_language: BTreeMap<String, Vec<RepoPath>>,
    pub(crate) framework_profiles_by_language:
        BTreeMap<String, frameworks::ResolvedLanguageProfiles>,
    pub(crate) file_line_counts: BTreeMap<RepoPath, usize>,
    pub(crate) file_diagnostics: Vec<FileDiagnosticEntry>,
    pub(crate) scope_summaries: Vec<ScopeSummaryEntry>,
    pub(crate) manifest_warnings: Vec<String>,
}

pub(crate) fn compute_orphan_files(
    snapshot: &MeasuredSnapshot,
    selected: Option<&SelectedPackage>,
) -> (Option<OrphanFiles>, BTreeMap<String, OrphanFiles>) {
    compute_orphan_files_matching(snapshot, |language, path| {
        selected.is_none_or(|pkg| pkg.matches(language.model.workspace.package_of(path)))
    })
}

pub(crate) fn compute_orphan_files_for_subset(
    snapshot: &MeasuredSnapshot,
    selected: Option<&SelectedPackage>,
    subset: &HashSet<RepoPath>,
) -> Option<OrphanFiles> {
    compute_orphan_files_matching(snapshot, |language, path| {
        subset.contains(path)
            && selected.is_none_or(|pkg| pkg.matches(language.model.workspace.package_of(path)))
    })
    .0
}

fn compute_orphan_files_matching(
    snapshot: &MeasuredSnapshot,
    include: impl Fn(&MeasuredLanguage, &RepoPath) -> bool,
) -> (Option<OrphanFiles>, BTreeMap<String, OrphanFiles>) {
    let mut by_language = BTreeMap::new();
    for language in &snapshot.languages {
        if !language.has_imports {
            continue;
        }
        let entry_points = snapshot
            .entry_points_by_language
            .get(&language.plugin)
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        let explicit_entry_points = snapshot
            .explicit_entry_points_by_language
            .get(&language.plugin)
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        if let Some(measured) = OrphanFiles::measure_filtered_with_explicit_entry_points(
            &language.model,
            entry_points,
            explicit_entry_points,
            |path| include(language, path),
        ) {
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
        if !language.has_imports {
            continue;
        }
        let measured = DependencyHygiene::measure_filtered(&language.model, ignored, |root| {
            selected.is_none_or(|pkg| {
                package_filter::package_root_matches_selected(&language.model.workspace, pkg, root)
            })
        });
        by_language.insert(language.plugin.clone(), measured);
    }
    let total = DependencyHygiene::merge(by_language.values().cloned());
    (total, by_language)
}

pub(crate) fn measure_whole_repo(
    context: &WorkspaceContext,
    folder: &Path,
    coverage_reports: &[PathBuf],
) -> Result<Report, CheckError> {
    let plugins = context.plugins.as_named();
    let metric_plugins = context.plugins.metric_plugins();
    measure_check_report(
        &context.repository,
        &context.configuration,
        &plugins,
        &metric_plugins,
        CheckRequest {
            folder,
            findings: FindingsShown::All,
            package: None,
            cli_coverage_reports: coverage_reports,
            ownership: OwnershipOptions::default(),
            baseline: None,
        },
    )
}

pub(crate) fn load_and_measure_whole_repo(
    folder: &Path,
    coverage_reports: &[PathBuf],
    plugin_host: &PluginHostConfig,
) -> Result<(WorkspaceContext, Report), CheckError> {
    let context = WorkspaceContext::load(folder, plugin_host)?;
    let report = measure_whole_repo(&context, folder, coverage_reports)?;
    Ok((context, report))
}

fn check_loaded(
    repository: &Repository,
    configuration: &Configuration,
    plugins: &[NamedLanguagePlugin<'_>],
    metric_plugins: &[&dyn MetricPlugin],
    request: CheckRequest<'_>,
) -> Result<Report, CheckError> {
    let package_scoped = request.package.is_some();
    let baseline = request.baseline;
    let mut report =
        measure_check_report(repository, configuration, plugins, metric_plugins, request)?;
    ensure_test_gap_budget_present(configuration, &report)?;
    ensure_baseline_versions_match(baseline, &report)?;
    apply_ratchet_to_check_report(
        repository.root(),
        configuration,
        package_scoped,
        &mut report,
    )?;
    Ok(report)
}

fn ensure_baseline_versions_match(
    baseline: Option<&Baseline>,
    report: &Report,
) -> Result<(), CheckError> {
    let mismatches = baseline
        .map(|baseline| baseline.version_mismatches(&report.baseline_metrics()))
        .unwrap_or_default();
    if mismatches.is_empty() {
        Ok(())
    } else {
        Err(CheckError::BaselineVersionMismatch {
            mismatches,
            supports_ignore_baseline: true,
        })
    }
}

fn apply_ratchet_to_check_report(
    repository_root: &Path,
    configuration: &Configuration,
    package_scoped: bool,
    report: &mut Report,
) -> Result<(), CheckError> {
    crate::ratchet::enforce_if_present(
        repository_root,
        &configuration.ratchet.path,
        package_scoped,
        report,
        Report::set_ratchet_note,
        Report::apply_ratchet,
    )
}

fn measure_check_report(
    repository: &Repository,
    configuration: &Configuration,
    plugins: &[NamedLanguagePlugin<'_>],
    metric_plugins: &[&dyn MetricPlugin],
    request: CheckRequest<'_>,
) -> Result<Report, CheckError> {
    let selected_coverage_paths = coverage::select_coverage_report_paths(
        request.folder,
        repository.root(),
        &configuration.coverage_reports,
        request.cli_coverage_reports,
    );
    let coverage_reports_named = !selected_coverage_paths.is_empty();
    let loaded_coverage =
        coverage::load_coverage_reports(repository.root(), &selected_coverage_paths)?;
    let tree = repository.working_tree()?;
    let mut snapshot = measure_snapshot(&tree, plugins, configuration)?;
    let coverage_outcome = coverage::apply_loaded_coverage_reports(
        &loaded_coverage,
        &mut snapshot.languages,
        &snapshot.file_line_counts,
    )?;
    let selected = package_filter::resolve_package_selection(&snapshot.languages, request.package)?;
    let selected_ref = selected.as_ref().map(|(pkg, _)| pkg);
    let (orphan_files, orphan_files_by_language) = compute_orphan_files(&snapshot, selected_ref);
    let (dependency_hygiene, dependency_hygiene_by_language) =
        compute_dependency_hygiene(&snapshot, &configuration.dependencies, selected_ref);
    apply_selected_package(
        &mut snapshot,
        plugins,
        selected.as_ref(),
        configuration.config_file(),
    )?;
    Report::summarize_with_options(
        snapshot.languages,
        tree.submodules().to_vec(),
        ReportOptions {
            findings: request.findings,
            budgets: &configuration.budgets,
            metric_plugins,
            boundaries: configuration.boundaries.as_ref(),
            dependencies: Some(&configuration.dependencies),
            dependency_hygiene: Some(&dependency_hygiene),
            dependency_hygiene_by_language: Some(&dependency_hygiene_by_language),
            duplication: configuration.duplication,
            orphan_files: orphan_files.as_ref(),
            orphan_files_by_language: Some(&orphan_files_by_language),
            ownership: request.ownership,
            coverage_reports_named,
            unmatched_coverage_paths: &coverage_outcome.unmatched_paths,
            baseline: request.baseline,
            extras: ReportExtras::default()
                .with_disabled_hazard_rules(configuration.hazards.disabled_rules_set())
                .with_file_diagnostics(snapshot.file_diagnostics)
                .with_scope_summaries(snapshot.scope_summaries)
                .with_manifest_warnings(snapshot.manifest_warnings),
        },
    )
    .map_err(|error| CheckError::metric_plugin(configuration.config_file(), error))
}

fn apply_selected_package(
    snapshot: &mut MeasuredSnapshot,
    plugins: &[NamedLanguagePlugin<'_>],
    selected: Option<&(SelectedPackage, Workspace)>,
    config_file: ConfigFileKind,
) -> Result<(), CheckError> {
    let Some((selected_pkg, workspace)) = selected else {
        return Ok(());
    };
    filter_languages_to_package(&mut snapshot.languages, selected_pkg);
    scope::filter_extras_to_package(
        &mut snapshot.file_diagnostics,
        &mut snapshot.scope_summaries,
        workspace,
        selected_pkg,
    );
    manifests::refresh_profiles_for_package(
        &mut snapshot.languages,
        plugins,
        &snapshot.framework_profiles_by_language,
        selected_pkg,
        config_file,
    )
}

fn ensure_test_gap_budget_present(
    configuration: &Configuration,
    report: &Report,
) -> Result<(), CheckError> {
    if let Some(budget) = configuration.budgets.production_test_gap()
        && !report.has_production_test_gap()
    {
        return Err(CheckError::TestGapBudgetWithoutCoverage { budget });
    }
    Ok(())
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
    measure_snapshot_with_cache(tree, plugins, configuration, None)
}

pub(crate) fn measure_snapshot_with_cache(
    tree: &Snapshot,
    plugins: &[NamedLanguagePlugin<'_>],
    configuration: &Configuration,
    lowering_cache: Option<&LoweringCache>,
) -> Result<MeasuredSnapshot, CheckError> {
    let external_versions = external_language_plugin_versions(configuration);
    let versioned: Vec<VersionedLanguagePlugin<'_>> = plugins
        .iter()
        .copied()
        .map(|named| VersionedLanguagePlugin {
            version: external_versions
                .get(named.name)
                .map(String::as_str)
                .unwrap_or(env!("CARGO_PKG_VERSION")),
            named,
        })
        .collect();
    measure_snapshot_with_versions(tree, &versioned, configuration, lowering_cache)
}

fn measure_snapshot_with_versions(
    tree: &Snapshot,
    plugins: &[VersionedLanguagePlugin<'_>],
    configuration: &Configuration,
    lowering_cache: Option<&LoweringCache>,
) -> Result<MeasuredSnapshot, CheckError> {
    let names: Vec<&str> = plugins.iter().map(|plugin| plugin.named.name).collect();
    let coverage = configuration.languages.coverage_of(&names)?;
    measure(tree, plugins, &coverage, configuration, lowering_cache)
}

pub(crate) fn external_language_plugin_versions(
    configuration: &Configuration,
) -> HashMap<&str, String> {
    configuration
        .plugins
        .iter()
        .filter(|declared| declared.kind == PluginKind::Language)
        .map(|declared| (declared.name.as_str(), declared.sha256.to_hex()))
        .collect()
}

fn measure(
    tree: &Snapshot,
    plugins: &[VersionedLanguagePlugin<'_>],
    coverage: &[LanguageCoverage<'_>],
    configuration: &Configuration,
    lowering_cache: Option<&LoweringCache>,
) -> Result<MeasuredSnapshot, CheckError> {
    let named_plugins: Vec<NamedLanguagePlugin<'_>> =
        plugins.iter().map(|plugin| plugin.named).collect();
    let config_file = configuration.config_file();
    let covered_files =
        lowering::assign_files_to_plugins(tree, &named_plugins, coverage, config_file)?;
    let file_dialects = scope::resolve_file_dialects(
        &named_plugins,
        &covered_files,
        &configuration.dialects,
        config_file,
    )?;
    let discovery = manifests::find_plugin_workspaces(
        &named_plugins,
        read_manifests(tree, &named_plugins, config_file)?,
    )?;
    let workspace = discovery.workspace();
    let manifest_warnings = discovery.warnings();
    let manifest_facts_by_plugin = discovery.plugins;
    let manifests_by_plugin: Vec<Vec<Manifest>> = manifest_facts_by_plugin
        .iter()
        .map(|facts| facts.valid_manifests.clone())
        .collect();
    let outcomes = lowering::lower_covered_files(
        tree,
        plugins,
        &covered_files,
        &file_dialects,
        config_file,
        lowering_cache,
    )?;
    let mut collected = lowering::collect_plugin_models(plugins.len(), workspace.clone(), outcomes);
    let profiles_by_plugin = frameworks::resolve_all_language_profiles(
        tree,
        &named_plugins,
        &collected.models,
        &manifests_by_plugin,
        &workspace,
        configuration,
    )?;
    for (model, profiles) in collected.models.iter_mut().zip(&profiles_by_plugin) {
        frameworks::apply_source_set_priority(model, profiles, &configuration.source_set_overrides);
    }
    let scope_summaries = scope::apply_scope_analysis(
        &named_plugins,
        &mut collected.models,
        &collected.file_dialects,
        &collected.file_line_counts,
        config_file,
    )?;
    let mut snapshot = resolve_measured_snapshot(SnapshotInputs {
        tree,
        plugins: &named_plugins,
        models: collected.models,
        manifest_facts_by_plugin,
        profiles_by_plugin,
        configured_entry_points: &configuration.entry_points,
        config_file,
    })?;
    snapshot.file_line_counts = collected.file_line_counts;
    snapshot.file_diagnostics = collected.file_diagnostics;
    snapshot.scope_summaries = scope_summaries;
    snapshot.manifest_warnings = manifest_warnings;
    Ok(snapshot)
}

struct SnapshotInputs<'a> {
    tree: &'a Snapshot,
    plugins: &'a [NamedLanguagePlugin<'a>],
    models: Vec<CodeModel>,
    manifest_facts_by_plugin: Vec<manifests::PluginManifestFacts>,
    profiles_by_plugin: Vec<frameworks::ResolvedLanguageProfiles>,
    configured_entry_points: &'a EntryPoints,
    config_file: ConfigFileKind,
}

fn resolve_measured_snapshot(inputs: SnapshotInputs<'_>) -> Result<MeasuredSnapshot, CheckError> {
    let SnapshotInputs {
        tree,
        plugins,
        models,
        manifest_facts_by_plugin,
        profiles_by_plugin,
        configured_entry_points,
        config_file,
    } = inputs;
    let mut languages = Vec::with_capacity(plugins.len());
    let mut entry_points_by_language = BTreeMap::new();
    let mut explicit_entry_points_by_language = BTreeMap::new();
    let mut framework_profiles_by_language = BTreeMap::new();
    for (((named, model), manifest_facts), profiles) in plugins
        .iter()
        .zip(models)
        .zip(manifest_facts_by_plugin)
        .zip(profiles_by_plugin)
    {
        let manifest_entries = if named.plugin.has_imports() {
            named
                .plugin
                .entry_points(tree.files(), manifest_facts.valid_manifests.clone())
                .map_err(|error| CheckError::language_plugin(config_file, error))?
        } else {
            Vec::new()
        };
        let (discovered_entries, explicit_entries) = if named.plugin.has_imports() {
            (
                combine_discovered_entry_points(
                    manifest_entries,
                    profiles.entry_points(),
                    &model.files,
                ),
                collect_configured_entry_points(&model.files, configured_entry_points),
            )
        } else {
            (Vec::new(), Vec::new())
        };
        let measured = manifests::finalize_language_model(manifests::FinalizeLanguageInputs {
            named,
            model,
            manifest_facts,
            profiles: &profiles,
            repository_paths: tree.files(),
            config_file,
        })?;
        entry_points_by_language.insert(named.name.to_owned(), discovered_entries);
        explicit_entry_points_by_language.insert(named.name.to_owned(), explicit_entries);
        framework_profiles_by_language.insert(named.name.to_owned(), profiles);
        languages.push(measured);
    }
    Ok(MeasuredSnapshot {
        languages,
        entry_points_by_language,
        explicit_entry_points_by_language,
        framework_profiles_by_language,
        file_line_counts: BTreeMap::new(),
        file_diagnostics: Vec::new(),
        scope_summaries: Vec::new(),
        manifest_warnings: Vec::new(),
    })
}

fn combine_discovered_entry_points(
    manifest_entries: Vec<RepoPath>,
    framework_entries: Vec<RepoPath>,
    files: &[SourceFile],
) -> Vec<RepoPath> {
    let mut entries: BTreeSet<RepoPath> = manifest_entries
        .into_iter()
        .chain(framework_entries)
        .collect();
    for file in files {
        if file.model.program_start && file.model.source_set == SourceSet::Production {
            entries.insert(file.path.clone());
        }
    }
    entries.into_iter().collect()
}

fn collect_configured_entry_points(
    files: &[SourceFile],
    configured: &EntryPoints,
) -> Vec<RepoPath> {
    if configured.is_empty() {
        return Vec::new();
    }
    let mut entries = BTreeSet::new();
    for file in files {
        if configured.matches(&file.path) {
            entries.insert(file.path.clone());
        }
    }
    entries.into_iter().collect()
}
