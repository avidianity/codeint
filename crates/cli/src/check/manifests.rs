use std::collections::{BTreeMap, HashSet};

use codeint_config::{ConfigFileKind, display_package_key};
use codeint_git::Snapshot;
use codeint_model::{
    CodeModel, DeclaredDep, DependencyManifest, LanguageFacts, LanguagePluginError, Manifest,
    ManifestUsedDep, PackageFrameworkProfiles, RepoPath, ResolveError, RuntimeImport,
    SkippedManifest, SourceFile, Workspace, WorkspaceError, choose_manifests,
    resolve_runtime_imports,
};
use codeint_plugin_host::NamedLanguagePlugin;
use codeint_report::MeasuredLanguage;

use super::control_characters::escape_control_characters;
use super::errors::CheckError;
use super::frameworks::{ResolvedLanguageProfiles, package_root_key};
use super::package_filter::SelectedPackage;

pub(crate) fn read_manifests(
    tree: &Snapshot,
    plugins: &[NamedLanguagePlugin<'_>],
    config_file: ConfigFileKind,
) -> Result<Vec<Vec<Manifest>>, CheckError> {
    plugins
        .iter()
        .map(|named| {
            if !named.plugin.has_imports() {
                return Ok(Vec::new());
            }
            choose_manifests(named.plugin, tree.files())
                .map_err(|source| match source {
                    ResolveError::Plugin(error) => CheckError::language_plugin(config_file, error),
                    other => CheckError::Resolve {
                        plugin: named.name.to_owned(),
                        source: other,
                    },
                })?
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

pub(crate) struct PluginManifestFacts {
    pub(crate) members: Vec<RepoPath>,
    pub(crate) valid_manifests: Vec<Manifest>,
    pub(crate) dependency_manifests: Vec<DependencyManifest>,
    pub(crate) declared_dependencies: Vec<DeclaredDep>,
    pub(crate) manifest_used_dependencies: Vec<ManifestUsedDep>,
    pub(crate) root_dependency_inheritance: Vec<String>,
}

pub(crate) struct WorkspaceDiscovery {
    pub(crate) plugins: Vec<PluginManifestFacts>,
    pub(crate) skipped: Vec<SkippedManifest>,
}

impl WorkspaceDiscovery {
    pub(crate) fn warnings(&self) -> Vec<String> {
        self.skipped.iter().map(skipped_manifest_warning).collect()
    }

    pub(crate) fn workspace(&self) -> Workspace {
        Workspace::new(
            self.plugins
                .iter()
                .flat_map(|facts| facts.members.iter().cloned()),
        )
        .with_skipped_manifests(self.skipped.iter().cloned())
    }
}

fn skipped_manifest_warning(skipped: &SkippedManifest) -> String {
    escape_control_characters(&format!(
        "Skipped the invalid {} manifest {}: {}. Files under it count toward the surrounding Workspace package.",
        skipped.plugin, skipped.path, skipped.reason
    ))
}

pub(crate) fn find_workspace(
    plugins: &[NamedLanguagePlugin<'_>],
    manifests_by_plugin: Vec<Vec<Manifest>>,
) -> Result<(Workspace, Vec<String>), CheckError> {
    let discovery = find_plugin_workspaces(plugins, manifests_by_plugin)?;
    Ok((discovery.workspace(), discovery.warnings()))
}

pub(super) fn find_plugin_workspaces(
    plugins: &[NamedLanguagePlugin<'_>],
    manifests_by_plugin: Vec<Vec<Manifest>>,
) -> Result<WorkspaceDiscovery, CheckError> {
    let mut discovery = WorkspaceDiscovery {
        plugins: Vec::with_capacity(plugins.len()),
        skipped: Vec::new(),
    };
    for (named, manifests) in plugins.iter().zip(manifests_by_plugin) {
        if !named.plugin.has_imports() {
            discovery.plugins.push(PluginManifestFacts {
                members: Vec::new(),
                valid_manifests: manifests,
                dependency_manifests: Vec::new(),
                declared_dependencies: Vec::new(),
                manifest_used_dependencies: Vec::new(),
                root_dependency_inheritance: Vec::new(),
            });
            continue;
        }
        let found = find_valid_plugin_workspace(named, manifests, &mut discovery.skipped)?;
        discovery.plugins.push(found);
    }
    Ok(discovery)
}

fn analyze_plugin_manifests(
    named: &NamedLanguagePlugin<'_>,
    manifests: &[Manifest],
) -> Result<codeint_model::ManifestAnalysis, CheckError> {
    match named.plugin.analyze_manifests(manifests.to_vec()) {
        Ok(analysis) => Ok(analysis),
        Err(LanguagePluginError::UnknownInvalidManifest { path, message }) => {
            Err(WorkspaceError::Manifest { path, message }.into())
        }
        Err(LanguagePluginError::InvalidWorkspacePackageRoot { path }) => {
            match RepoPath::new(path.clone()) {
                Err(err) => Err(WorkspaceError::InvalidPackageRoot(err).into()),
                Ok(_) => Err(WorkspaceError::PluginFailed {
                    plugin: named.name.to_owned(),
                    message: format!("invalid workspace package root {path:?}"),
                }
                .into()),
            }
        }
        Err(error) => Err(WorkspaceError::PluginFailed {
            plugin: named.name.to_owned(),
            message: error.to_string(),
        }
        .into()),
    }
}

fn find_valid_plugin_workspace(
    named: &NamedLanguagePlugin<'_>,
    mut manifests: Vec<Manifest>,
    skipped: &mut Vec<SkippedManifest>,
) -> Result<PluginManifestFacts, CheckError> {
    let analysis = analyze_plugin_manifests(named, &manifests)?;
    for invalid in analysis.invalid_manifests {
        if let Some(position) = manifests.iter().position(|m| m.path == invalid.path) {
            manifests.remove(position);
        }
        let reason = invalid
            .message
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        skipped.push(SkippedManifest {
            path: invalid.path,
            plugin: named.name.to_owned(),
            reason: escape_control_characters(reason.trim_end_matches('.')),
        });
    }
    let members = analysis
        .workspace_packages
        .into_iter()
        .map(|root| Ok(RepoPath::new(root)?))
        .collect::<Result<Vec<RepoPath>, WorkspaceError>>()?;
    let dependency_manifests = analysis
        .dependency_manifests
        .into_iter()
        .map(|owner| {
            let path = RepoPath::new(owner.path)?;
            Ok(DependencyManifest {
                package_root: owner.package_root,
                path,
            })
        })
        .collect::<Result<Vec<DependencyManifest>, WorkspaceError>>()?;
    let manifest_used_dependencies = analysis
        .manifest_used_dependencies
        .into_iter()
        .map(|dep| ManifestUsedDep {
            package_root: dep.package_root,
            name: dep.name,
        })
        .collect();
    Ok(PluginManifestFacts {
        members,
        valid_manifests: manifests,
        dependency_manifests,
        declared_dependencies: analysis.declared_dependencies,
        manifest_used_dependencies,
        root_dependency_inheritance: analysis.root_dependency_inheritance,
    })
}

pub(super) struct FinalizeLanguageInputs<'a> {
    pub(super) named: &'a NamedLanguagePlugin<'a>,
    pub(super) model: CodeModel,
    pub(super) manifest_facts: PluginManifestFacts,
    pub(super) profiles: &'a ResolvedLanguageProfiles,
    pub(super) repository_paths: &'a [RepoPath],
    pub(super) config_file: ConfigFileKind,
}

pub(super) fn finalize_language_model(
    inputs: FinalizeLanguageInputs<'_>,
) -> Result<MeasuredLanguage, CheckError> {
    let FinalizeLanguageInputs {
        named,
        mut model,
        manifest_facts,
        profiles,
        repository_paths,
        config_file,
    } = inputs;
    let (reference_ranges, reference_range_note) =
        profiles.reference_ranges_for_scope(named, None, config_file)?;
    let has_imports = named.plugin.has_imports();
    if has_imports {
        model.dependency_manifests = filter_active_dependency_manifests(
            manifest_facts.dependency_manifests,
            &manifest_facts.members,
            &manifest_facts.declared_dependencies,
            &model.files,
        );
        let mut manifest_used = manifest_facts.manifest_used_dependencies;
        manifest_used.sort();
        manifest_used.dedup();
        model.manifest_used_dependencies = manifest_used;
        model.declared_dependencies = manifest_facts.declared_dependencies;
        let mut inheritance = manifest_facts.root_dependency_inheritance;
        inheritance.sort();
        inheritance.dedup();
        model.root_dependency_inheritance = inheritance;
        model.runtime_imports = resolve_imports_with_profiles(
            named,
            &model,
            repository_paths,
            manifest_facts.valid_manifests,
            profiles,
            config_file,
        )?;
    }
    if !model.files.is_empty() {
        model.languages = vec![LanguageFacts {
            language: named.name.to_owned(),
            has_coverage: false,
            has_hazard_rules: named.plugin.has_hazard_rules(),
            has_imports,
        }];
    }
    Ok(MeasuredLanguage {
        plugin: named.name.to_owned(),
        model,
        reference_ranges,
        reference_range_note,
        has_imports,
        framework_profiles: profiles.active_profiles_union(),
        framework_profiles_by_package: profiles.active_profiles_by_package(),
    })
}

fn resolve_imports_with_profiles(
    named: &NamedLanguagePlugin<'_>,
    model: &CodeModel,
    repository_paths: &[RepoPath],
    manifests: Vec<Manifest>,
    profiles: &ResolvedLanguageProfiles,
    config_file: ConfigFileKind,
) -> Result<Vec<RuntimeImport>, CheckError> {
    if model.files.is_empty() {
        return Ok(Vec::new());
    }
    let mut package_roots: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for file in &model.files {
        let pkg_root = package_root_key(model.workspace.package_of(&file.path));
        package_roots
            .entry(pkg_root.clone())
            .or_insert_with(|| profiles.active_profiles_for_package(&pkg_root));
    }
    let package_profiles: Vec<PackageFrameworkProfiles> = package_roots
        .into_iter()
        .map(|(package_root, active_profiles)| PackageFrameworkProfiles {
            package_root,
            active_profiles,
        })
        .collect();
    let importers: Vec<&SourceFile> = model.files.iter().collect();
    resolve_runtime_imports(
        named.plugin,
        repository_paths,
        manifests,
        &importers,
        &package_profiles,
    )
    .map_err(|source| match source {
        ResolveError::Plugin(error) => CheckError::language_plugin(config_file, error),
        other => CheckError::Resolve {
            plugin: named.name.to_owned(),
            source: other,
        },
    })
}

fn filter_active_dependency_manifests(
    dependency_manifests: Vec<DependencyManifest>,
    plugin_members: &[RepoPath],
    declared: &[DeclaredDep],
    files: &[SourceFile],
) -> Vec<DependencyManifest> {
    let declared_roots: HashSet<&str> = declared
        .iter()
        .map(|dep| dep.package_root.as_str())
        .collect();
    let member_roots: HashSet<&str> = plugin_members.iter().map(RepoPath::as_str).collect();
    let mut collected: Vec<DependencyManifest> = dependency_manifests
        .into_iter()
        .filter(|manifest| {
            let root = manifest.package_root.as_str();
            declared_roots.contains(root)
                || member_roots.contains(root)
                || owns_any_file(root, files)
        })
        .collect();
    collected.sort();
    collected.dedup();
    collected
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

pub(super) fn refresh_profiles_for_package(
    languages: &mut [MeasuredLanguage],
    plugins: &[NamedLanguagePlugin<'_>],
    profiles_by_language: &BTreeMap<String, ResolvedLanguageProfiles>,
    selected: &SelectedPackage,
    config_file: ConfigFileKind,
) -> Result<(), CheckError> {
    let selected_root = match selected {
        SelectedPackage::Root => "",
        SelectedPackage::Member(root) => root.as_str(),
    };
    let selected_display = display_package_key(selected_root);
    for language in languages {
        let Some(named) = plugins.iter().find(|plugin| plugin.name == language.plugin) else {
            continue;
        };
        let Some(profiles) = profiles_by_language.get(&language.plugin) else {
            continue;
        };
        let (reference_ranges, reference_range_note) =
            profiles.reference_ranges_for_scope(named, Some(selected_root), config_file)?;
        language.reference_ranges = reference_ranges;
        language.reference_range_note = reference_range_note;
        let active = profiles.active_profiles_for_package(selected_root);
        language.framework_profiles = active.clone();
        language.framework_profiles_by_package.clear();
        if !active.is_empty() {
            language
                .framework_profiles_by_package
                .insert(selected_display.to_owned(), active);
        }
    }
    Ok(())
}
