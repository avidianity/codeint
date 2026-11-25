use std::collections::{BTreeMap, HashSet};

use codeint_config::{ConfigFileKind, display_package_key};
use codeint_git::Snapshot;
use codeint_model::{
    CodeModel, DeclaredDep, DependencyManifest, LanguageFacts, Manifest, RepoPath, ResolveError,
    RuntimeImport, SourceFile, Workspace, choose_manifests, resolve_runtime_imports,
};
use codeint_plugin_host::NamedLanguagePlugin;
use codeint_report::MeasuredLanguage;

use super::errors::CheckError;
use super::frameworks::{ResolvedLanguageProfiles, package_root_key};
use super::package_filter::SelectedPackage;

pub(crate) fn read_manifests(
    tree: &Snapshot,
    plugins: &[NamedLanguagePlugin<'_>],
) -> Result<Vec<Vec<Manifest>>, CheckError> {
    plugins
        .iter()
        .map(|named| {
            if !named.plugin.has_imports() {
                return Ok(Vec::new());
            }
            choose_manifests(named.plugin, tree.files())
                .map_err(|source| CheckError::Resolve {
                    plugin: named.name.to_owned(),
                    source,
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

pub(crate) fn find_workspace(
    plugins: &[NamedLanguagePlugin<'_>],
    manifests_by_plugin: &[Vec<Manifest>],
) -> Result<Workspace, CheckError> {
    let member_roots = find_plugin_workspaces(plugins, manifests_by_plugin)?
        .into_iter()
        .flatten();
    Ok(Workspace::new(member_roots))
}

pub(super) fn find_plugin_workspaces(
    plugins: &[NamedLanguagePlugin<'_>],
    manifests_by_plugin: &[Vec<Manifest>],
) -> Result<Vec<Vec<RepoPath>>, CheckError> {
    plugins
        .iter()
        .zip(manifests_by_plugin)
        .map(|(named, manifests)| {
            if !named.plugin.has_imports() {
                return Ok(Vec::new());
            }
            Ok(named.plugin.workspace_packages(manifests.clone())?)
        })
        .collect()
}

pub(super) fn finalize_language_model(
    named: &NamedLanguagePlugin<'_>,
    mut model: CodeModel,
    manifests: Vec<Manifest>,
    plugin_members: &[RepoPath],
    profiles: &ResolvedLanguageProfiles,
    repository_paths: &[RepoPath],
    config_file: ConfigFileKind,
) -> Result<MeasuredLanguage, CheckError> {
    let (reference_ranges, reference_range_note) =
        profiles.reference_ranges_for_scope(named, None)?;
    let has_imports = named.plugin.has_imports();
    if has_imports {
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
        model.manifest_used_dependencies =
            crate::package_scripts::extract_manifest_used_dependencies(
                &manifests,
                plugin_members,
                &declared_dependencies,
            );
        model.declared_dependencies = declared_dependencies;
        model.runtime_imports = resolve_imports_with_profiles(
            named,
            &model,
            repository_paths,
            manifests,
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
    let mut grouped: BTreeMap<Vec<String>, Vec<&SourceFile>> = BTreeMap::new();
    for file in &model.files {
        let pkg_root = package_root_key(model.workspace.package_of(&file.path));
        let active = profiles.active_profiles_for_package(&pkg_root);
        grouped.entry(active).or_default().push(file);
    }
    let map_error = |source| match source {
        ResolveError::Plugin(error) => CheckError::language_plugin(config_file, error),
        other => CheckError::Resolve {
            plugin: named.name.to_owned(),
            source: other,
        },
    };
    if grouped.len() <= 1 {
        let (active_profiles, importers) = match grouped.into_iter().next() {
            Some(entry) => entry,
            None => return Ok(Vec::new()),
        };
        return resolve_runtime_imports(
            named.plugin,
            repository_paths,
            manifests,
            &importers,
            &active_profiles,
        )
        .map_err(map_error);
    }
    let mut by_importer: BTreeMap<RepoPath, Vec<RuntimeImport>> = BTreeMap::new();
    for (active_profiles, importers) in grouped {
        let resolved = resolve_runtime_imports(
            named.plugin,
            repository_paths,
            manifests.clone(),
            &importers,
            &active_profiles,
        )
        .map_err(map_error)?;
        for import in resolved {
            by_importer
                .entry(import.importer.clone())
                .or_default()
                .push(import);
        }
    }
    let mut ordered = Vec::new();
    for file in &model.files {
        if let Some(imports) = by_importer.remove(&file.path) {
            ordered.extend(imports);
        }
    }
    Ok(ordered)
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

pub(super) fn refresh_profiles_for_package(
    languages: &mut [MeasuredLanguage],
    plugins: &[NamedLanguagePlugin<'_>],
    profiles_by_language: &BTreeMap<String, ResolvedLanguageProfiles>,
    selected: &SelectedPackage,
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
            profiles.reference_ranges_for_scope(named, Some(selected_root))?;
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
