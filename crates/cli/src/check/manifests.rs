use std::collections::{BTreeMap, HashSet};

use codeint_config::ConfigFileKind;
use codeint_git::Snapshot;
use codeint_model::{
    CodeModel, DeclaredDep, DependencyManifest, LanguageFacts, Manifest, ReferenceRanges, RepoPath,
    ResolveError, SourceFile, Workspace, choose_manifests, resolve_runtime_imports,
};
use codeint_plugin_host::{
    NamedLanguagePlugin, TYPESCRIPT_JAVASCRIPT_PLUGIN_NAME,
    manifests_declare_framework_reference_ranges, select_language_reference_ranges,
};
use codeint_report::MeasuredLanguage;

use super::errors::CheckError;
use super::package_filter::{SelectedPackage, package_root_matches_selected};

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
    repository_paths: &[RepoPath],
    config_file: ConfigFileKind,
) -> Result<MeasuredLanguage, CheckError> {
    let reference_ranges = select_reference_ranges(named, &model, &manifests)?;
    let has_imports = named.plugin.has_imports();
    if has_imports {
        let importers: Vec<&SourceFile> = model.files.iter().collect();
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
        model.runtime_imports =
            resolve_runtime_imports(named.plugin, repository_paths, manifests, &importers, &[])
                .map_err(|source| match source {
                    ResolveError::Plugin(error) => CheckError::language_plugin(config_file, error),
                    other => CheckError::Resolve {
                        plugin: named.name.to_owned(),
                        source: other,
                    },
                })?;
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
        has_imports,
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
        || manifests_declare_framework_reference_ranges(TYPESCRIPT_JAVASCRIPT_PLUGIN_NAME, &own)
    {
        return own;
    }
    manifests
        .iter()
        .filter(|manifest| manifest_root(&manifest.path).is_empty())
        .cloned()
        .collect()
}

pub(super) fn refresh_reference_ranges_for_package(
    languages: &mut [MeasuredLanguage],
    plugins: &[NamedLanguagePlugin<'_>],
    manifests_by_language: &BTreeMap<String, Vec<Manifest>>,
    workspace: &Workspace,
    selected: &SelectedPackage,
) -> Result<(), CheckError> {
    for language in languages {
        if language.plugin != TYPESCRIPT_JAVASCRIPT_PLUGIN_NAME {
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
    select_language_reference_ranges(named, model, manifests).map_err(CheckError::ReferenceRange)
}
