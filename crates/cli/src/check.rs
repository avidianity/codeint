use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::path::{Path, PathBuf};

use codeint_config::{
    Codeowners, ConfigFileKind, Configuration, DependenciesConfig, EntryPoints, FindingsShown,
    LanguageCoverage, OwnerFilter, PluginHostConfig, read_configuration,
};
use codeint_git::{Repository, Snapshot};
use codeint_metrics::{DependencyHygiene, OrphanFiles};
use codeint_model::{
    CodeModel, LanguagePlugin, Manifest, MetricPlugin, RepoPath, SourceFile, SourceSet, Workspace,
};
use codeint_plugin_host::{
    NamedLanguagePlugin, SQL_PLUGIN_NAME, WasmLanguagePlugin, WasmMetricPlugin,
    built_in_language_descriptors, built_in_language_names, built_in_language_plugins,
    load_language_plugins_with_limits, load_metric_plugins_with_limits,
};
use codeint_report::{
    LanguageSchema, MeasuredLanguage, OwnershipOptions, RejectedStatementEntry, Report,
    ReportExtras, ReportOptions, SqlSchemaSummary,
};

mod coverage;
mod errors;
mod frameworks;
mod lowering;
mod manifests;
mod package_filter;
mod sql;
#[cfg(test)]
mod tests;

pub(crate) use coverage::{CoverageReportDigest, hash_configured_coverage_reports};
pub(crate) use errors::CheckError;
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
        sql::validate_disabled_hazard_rules(&named, configuration)?;
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
        self.measure_working_tree_model_for_package(None)
    }

    pub(crate) fn measure_working_tree_model_for_package(
        &self,
        package: Option<&str>,
    ) -> Result<CodeModel, CheckError> {
        let plugins = self.plugins.as_named();
        let tree = self.repository.working_tree()?;
        let mut languages = measure_with(&tree, &plugins, &self.configuration)?;
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
    coverage_reports: &[PathBuf],
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
        CheckRequest {
            folder,
            findings,
            package,
            cli_coverage_reports: coverage_reports,
            ownership: run.ownership(),
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
    sql::validate_disabled_hazard_rules(plugins, &configuration)?;
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
        },
    )
}

struct CheckRequest<'a> {
    folder: &'a Path,
    findings: FindingsShown,
    package: Option<&'a str>,
    cli_coverage_reports: &'a [PathBuf],
    ownership: OwnershipOptions<'a>,
}

pub(crate) struct MeasuredSnapshot {
    pub(crate) languages: Vec<MeasuredLanguage>,
    pub(crate) entry_points_by_language: BTreeMap<String, Vec<RepoPath>>,
    pub(crate) framework_profiles_by_language:
        BTreeMap<String, frameworks::ResolvedLanguageProfiles>,
    pub(crate) file_line_counts: BTreeMap<RepoPath, usize>,
    pub(crate) rejected_statements: Vec<RejectedStatementEntry>,
    pub(crate) sql_schemas: Vec<SqlSchemaSummary>,
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
        if let Some(measured) =
            OrphanFiles::measure_filtered(&language.model, entry_points, |path| {
                include(language, path)
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

fn check_loaded(
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
    apply_selected_package(&mut snapshot, plugins, selected.as_ref())?;
    let report = Report::summarize_with_extras(
        snapshot.languages,
        tree.submodules().to_vec(),
        ReportOptions {
            findings: request.findings,
            budgets: &configuration.budgets,
            metric_plugins,
            boundaries: configuration.boundaries.as_ref(),
            dependencies: &configuration.dependencies,
            dependency_hygiene: Some(&dependency_hygiene),
            dependency_hygiene_by_language: Some(&dependency_hygiene_by_language),
            duplication: configuration.duplication,
            orphan_files: orphan_files.as_ref(),
            orphan_files_by_language: Some(&orphan_files_by_language),
            ownership: request.ownership,
            coverage_reports_named,
            unmatched_coverage_paths: &coverage_outcome.unmatched_paths,
        },
        ReportExtras::default()
            .with_disabled_hazard_rules(configuration.hazards.disabled_rules_set())
            .with_rejected_statements(snapshot.rejected_statements)
            .with_sql_schemas(snapshot.sql_schemas),
    )
    .map_err(|error| CheckError::metric_plugin(configuration.config_file(), error))?;
    ensure_test_gap_budget_present(configuration, &report)?;
    Ok(report)
}

fn apply_selected_package(
    snapshot: &mut MeasuredSnapshot,
    plugins: &[NamedLanguagePlugin<'_>],
    selected: Option<&(SelectedPackage, Workspace)>,
) -> Result<(), CheckError> {
    let Some((selected_pkg, workspace)) = selected else {
        return Ok(());
    };
    filter_languages_to_package(&mut snapshot.languages, selected_pkg);
    sql::filter_sql_extras_to_package(
        &mut snapshot.rejected_statements,
        &mut snapshot.sql_schemas,
        workspace,
        selected_pkg,
    );
    manifests::refresh_profiles_for_package(
        &mut snapshot.languages,
        plugins,
        &snapshot.framework_profiles_by_language,
        selected_pkg,
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
    let names: Vec<&str> = plugins.iter().map(|plugin| plugin.name).collect();
    let coverage = configuration.languages.coverage_of(&names)?;
    measure(tree, plugins, &coverage, configuration)
}

fn measure(
    tree: &Snapshot,
    plugins: &[NamedLanguagePlugin<'_>],
    coverage: &[LanguageCoverage<'_>],
    configuration: &Configuration,
) -> Result<MeasuredSnapshot, CheckError> {
    let config_file = configuration.config_file();
    let covered_files = lowering::assign_files_to_plugins(tree, plugins, coverage, config_file)?;
    let sql_dialects =
        sql::resolve_sql_dialects(plugins, &covered_files, &configuration.sql, config_file)?;
    let manifests_by_plugin = read_manifests(tree, plugins)?;
    let plugin_members = manifests::find_plugin_workspaces(plugins, &manifests_by_plugin)?;
    let workspace = Workspace::new(plugin_members.iter().flatten().cloned());
    let outcomes =
        lowering::lower_covered_files(tree, plugins, &covered_files, &sql_dialects, config_file)?;
    let mut collected = lowering::collect_plugin_models(plugins.len(), workspace.clone(), outcomes);
    let profiles_by_plugin = frameworks::resolve_all_language_profiles(
        tree,
        plugins,
        &collected.models,
        &manifests_by_plugin,
        &workspace,
        configuration,
    )?;
    for (model, profiles) in collected.models.iter_mut().zip(&profiles_by_plugin) {
        frameworks::apply_source_set_priority(model, profiles, &configuration.source_set_overrides);
    }
    let sql_schemas = plugins
        .iter()
        .position(|named| named.name == SQL_PLUGIN_NAME)
        .filter(|&index| !collected.models[index].files.is_empty())
        .map(|index| {
            sql::apply_sql_schema_pass(&mut collected.models[index], &collected.sql_parsed)
        })
        .unwrap_or_default();
    let mut snapshot = resolve_measured_snapshot(SnapshotInputs {
        tree,
        plugins,
        models: collected.models,
        manifests_by_plugin,
        plugin_members: &plugin_members,
        profiles_by_plugin,
        configured_entry_points: &configuration.entry_points,
        config_file,
    })?;
    snapshot.file_line_counts = collected.file_line_counts;
    snapshot.rejected_statements = collected.rejected_statements;
    snapshot.sql_schemas = sql_schemas;
    Ok(snapshot)
}

struct SnapshotInputs<'a> {
    tree: &'a Snapshot,
    plugins: &'a [NamedLanguagePlugin<'a>],
    models: Vec<CodeModel>,
    manifests_by_plugin: Vec<Vec<Manifest>>,
    plugin_members: &'a [Vec<RepoPath>],
    profiles_by_plugin: Vec<frameworks::ResolvedLanguageProfiles>,
    configured_entry_points: &'a EntryPoints,
    config_file: ConfigFileKind,
}

fn resolve_measured_snapshot(inputs: SnapshotInputs<'_>) -> Result<MeasuredSnapshot, CheckError> {
    let SnapshotInputs {
        tree,
        plugins,
        models,
        manifests_by_plugin,
        plugin_members,
        profiles_by_plugin,
        configured_entry_points,
        config_file,
    } = inputs;
    let mut languages = Vec::with_capacity(plugins.len());
    let mut entry_points_by_language = BTreeMap::new();
    let mut framework_profiles_by_language = BTreeMap::new();
    for ((((named, model), manifests), members), profiles) in plugins
        .iter()
        .zip(models)
        .zip(manifests_by_plugin)
        .zip(plugin_members)
        .zip(profiles_by_plugin)
    {
        let manifest_entries = if named.plugin.has_imports() {
            named
                .plugin
                .entry_points(tree.files(), manifests.clone())
                .map_err(|error| CheckError::language_plugin(config_file, error))?
        } else {
            Vec::new()
        };
        let combined_entries = if named.plugin.has_imports() {
            combine_entry_points(
                manifest_entries,
                profiles.entry_points(),
                &model.files,
                configured_entry_points,
            )
        } else {
            Vec::new()
        };
        let measured = manifests::finalize_language_model(
            named,
            model,
            manifests,
            members,
            &profiles,
            tree.files(),
            config_file,
        )?;
        entry_points_by_language.insert(named.name.to_owned(), combined_entries);
        framework_profiles_by_language.insert(named.name.to_owned(), profiles);
        languages.push(measured);
    }
    Ok(MeasuredSnapshot {
        languages,
        entry_points_by_language,
        framework_profiles_by_language,
        file_line_counts: BTreeMap::new(),
        rejected_statements: Vec::new(),
        sql_schemas: Vec::new(),
    })
}

fn combine_entry_points(
    manifest_entries: Vec<RepoPath>,
    framework_entries: Vec<RepoPath>,
    files: &[SourceFile],
    configured: &EntryPoints,
) -> Vec<RepoPath> {
    let mut entries: BTreeSet<RepoPath> = manifest_entries
        .into_iter()
        .chain(framework_entries)
        .collect();
    for file in files {
        let marked_production =
            file.model.program_start && file.model.source_set == SourceSet::Production;
        let configured_match = !configured.is_empty() && configured.matches(&file.path);
        if marked_production || configured_match {
            entries.insert(file.path.clone());
        }
    }
    entries.into_iter().collect()
}
