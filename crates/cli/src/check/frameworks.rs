use std::collections::{BTreeMap, BTreeSet, HashSet};

use codeint_config::{ConfigFileKind, Configuration, SourceSetOverrides, display_package_key};
use codeint_git::Snapshot;
use codeint_model::{
    CodeModel, FrameworkProfileResult, FrameworkProfileSelection, LanguagePluginError, Manifest,
    Package, ReferenceRange, ReferenceRanges, RepoPath, SourceSet, Workspace,
};
use codeint_plugin_host::NamedLanguagePlugin;

use super::errors::CheckError;

#[derive(Clone, Default)]
pub(crate) struct ResolvedLanguageProfiles {
    by_package: BTreeMap<String, FrameworkProfileResult>,
}

impl ResolvedLanguageProfiles {
    pub(crate) fn active_profiles_union(&self) -> Vec<String> {
        let mut profiles = BTreeSet::new();
        for result in self.by_package.values() {
            profiles.extend(result.active_profiles.iter().cloned());
        }
        profiles.into_iter().collect()
    }

    pub(crate) fn active_profiles_by_package(&self) -> BTreeMap<String, Vec<String>> {
        self.by_package
            .iter()
            .filter(|(_, result)| !result.active_profiles.is_empty())
            .map(|(root, result)| {
                let unique: BTreeSet<String> = result.active_profiles.iter().cloned().collect();
                (
                    display_package_key(root).to_owned(),
                    unique.into_iter().collect(),
                )
            })
            .collect()
    }

    pub(crate) fn active_profiles_for_package(&self, package_root: &str) -> Vec<String> {
        self.by_package
            .get(package_root)
            .map(|result| {
                let unique: BTreeSet<String> = result.active_profiles.iter().cloned().collect();
                unique.into_iter().collect()
            })
            .unwrap_or_default()
    }

    pub(crate) fn entry_points(&self) -> Vec<RepoPath> {
        let entries: BTreeSet<RepoPath> = self.collect_profile_paths(|result| &result.entry_points);
        entries.into_iter().collect()
    }

    pub(crate) fn generated_paths(&self) -> HashSet<RepoPath> {
        self.collect_profile_paths(|result| &result.generated_paths)
    }

    fn collect_profile_paths<C>(&self, extract: fn(&FrameworkProfileResult) -> &[String]) -> C
    where
        C: FromIterator<RepoPath>,
    {
        self.by_package
            .values()
            .flat_map(|result| extract(result).iter())
            .filter_map(|path| RepoPath::new(path.as_str()).ok())
            .collect()
    }

    pub(crate) fn reference_ranges_for_scope(
        &self,
        named: &NamedLanguagePlugin<'_>,
        package_root: Option<&str>,
        config_file: ConfigFileKind,
    ) -> Result<(ReferenceRanges, Option<String>), CheckError> {
        match package_root {
            Some(root) => self.reference_ranges_for_package(named, root, config_file),
            None => self.reference_ranges_for_repository(named, config_file),
        }
    }

    fn reference_ranges_for_package(
        &self,
        named: &NamedLanguagePlugin<'_>,
        root: &str,
        config_file: ConfigFileKind,
    ) -> Result<(ReferenceRanges, Option<String>), CheckError> {
        let Some(result) = self.by_package.get(root) else {
            return Ok((base_reference_ranges(named, config_file)?, None));
        };
        let ranges = profile_or_base_ranges(named, &result.reference_ranges, config_file)?;
        Ok((ranges, result.reference_range_note.clone()))
    }

    fn reference_ranges_for_repository(
        &self,
        named: &NamedLanguagePlugin<'_>,
        config_file: ConfigFileKind,
    ) -> Result<(ReferenceRanges, Option<String>), CheckError> {
        let mut entries = self.by_package.values();
        let Some(first) = entries.next() else {
            return Ok((base_reference_ranges(named, config_file)?, None));
        };
        let uniform = !first.reference_ranges.is_empty()
            && entries.all(|other| {
                reference_ranges_equal(&first.reference_ranges, &other.reference_ranges)
            });
        let ranges = if uniform {
            ReferenceRanges::new(first.reference_ranges.clone())
                .map_err(CheckError::ReferenceRange)?
        } else {
            base_reference_ranges(named, config_file)?
        };
        Ok((ranges, self.combined_reference_range_note()))
    }

    fn combined_reference_range_note(&self) -> Option<String> {
        codeint_report::combine_reference_range_notes(
            self.by_package
                .values()
                .filter_map(|result| result.reference_range_note.as_deref()),
        )
    }
}

fn base_reference_ranges(
    named: &NamedLanguagePlugin<'_>,
    config_file: ConfigFileKind,
) -> Result<ReferenceRanges, CheckError> {
    named
        .plugin
        .reference_ranges()
        .map_err(|error| match error {
            LanguagePluginError::InvalidReferenceRange(invalid) => {
                CheckError::ReferenceRange(invalid)
            }
            other => CheckError::language_plugin(config_file, other),
        })
}

fn profile_or_base_ranges(
    named: &NamedLanguagePlugin<'_>,
    ranges: &[ReferenceRange],
    config_file: ConfigFileKind,
) -> Result<ReferenceRanges, CheckError> {
    if ranges.is_empty() {
        base_reference_ranges(named, config_file)
    } else {
        ReferenceRanges::new(ranges.to_vec()).map_err(CheckError::ReferenceRange)
    }
}

fn reference_ranges_equal(left: &[ReferenceRange], right: &[ReferenceRange]) -> bool {
    left.len() == right.len()
        && left.iter().zip(right).all(|(a, b)| {
            a.metric == b.metric
                && a.metric_version == b.metric_version
                && a.low.to_bits() == b.low.to_bits()
                && a.high.to_bits() == b.high.to_bits()
                && a.source == b.source
        })
}

pub(crate) fn validate_configured_framework_profiles(
    plugins: &[NamedLanguagePlugin<'_>],
    configuration: &Configuration,
) -> Result<(), CheckError> {
    let mut known = BTreeSet::new();
    for named in plugins {
        known.extend(named.plugin.known_framework_profiles());
    }
    let known: Vec<String> = known.into_iter().collect();
    configuration
        .framework_profiles
        .validate_known(configuration.config_file(), &known)?;
    Ok(())
}

pub(crate) fn resolve_all_language_profiles(
    tree: &Snapshot,
    plugins: &[NamedLanguagePlugin<'_>],
    models: &[CodeModel],
    manifests_by_plugin: &[Vec<Manifest>],
    workspace: &Workspace,
    configuration: &Configuration,
) -> Result<Vec<ResolvedLanguageProfiles>, CheckError> {
    let config_file = configuration.config_file();
    let files_by_package = group_paths_by_package(tree.files(), workspace);
    let mut resolved = Vec::with_capacity(plugins.len());
    for ((named, model), manifests) in plugins.iter().zip(models).zip(manifests_by_plugin) {
        let known_profiles = named.plugin.known_framework_profiles();
        if known_profiles.is_empty() || model.files.is_empty() {
            resolved.push(ResolvedLanguageProfiles::default());
            continue;
        }
        let active_packages = measured_packages(model, workspace);
        let mut by_package = BTreeMap::new();
        for package_root in active_packages {
            let package_files = files_by_package
                .get(&package_root)
                .map(Vec::as_slice)
                .unwrap_or(&[]);
            let package_manifests = manifests_for_package(manifests, workspace, &package_root);
            let selection = scope_selection_to_plugin(
                configuration
                    .framework_profiles
                    .selection_for_package(&package_root),
                &known_profiles,
            );
            let result = named
                .plugin
                .resolve_framework_profiles(
                    &package_root,
                    package_files,
                    package_manifests,
                    selection,
                )
                .map_err(|error| CheckError::language_plugin(config_file, error))?;
            by_package.insert(package_root, result);
        }
        resolved.push(ResolvedLanguageProfiles { by_package });
    }
    Ok(resolved)
}

pub(crate) fn apply_source_set_priority(
    model: &mut CodeModel,
    profiles: &ResolvedLanguageProfiles,
    overrides: &SourceSetOverrides,
) {
    let generated_paths = profiles.generated_paths();
    for file in &mut model.files {
        if generated_paths.contains(&file.path) && file.model.source_set != SourceSet::Vendored {
            file.model.source_set = SourceSet::Generated;
        }
        file.model.source_set = overrides.source_set(&file.path, file.model.source_set);
    }
}

pub(crate) fn package_root_key(package: Package<'_>) -> String {
    match package {
        Package::Root => String::new(),
        Package::Member(root) => root.as_str().to_owned(),
    }
}

fn group_paths_by_package(
    paths: &[RepoPath],
    workspace: &Workspace,
) -> BTreeMap<String, Vec<RepoPath>> {
    let mut grouped: BTreeMap<String, Vec<RepoPath>> = BTreeMap::new();
    for path in paths {
        grouped
            .entry(package_root_key(workspace.package_of(path)))
            .or_default()
            .push(path.clone());
    }
    grouped
}

fn measured_packages(model: &CodeModel, workspace: &Workspace) -> BTreeSet<String> {
    model
        .files
        .iter()
        .map(|file| package_root_key(workspace.package_of(&file.path)))
        .collect()
}

fn manifests_for_package(
    manifests: &[Manifest],
    workspace: &Workspace,
    package_root: &str,
) -> Vec<Manifest> {
    manifests
        .iter()
        .filter(|manifest| manifest_belongs_to_package(workspace, package_root, &manifest.path))
        .cloned()
        .collect()
}

fn manifest_belongs_to_package(
    workspace: &Workspace,
    package_root: &str,
    manifest_path: &str,
) -> bool {
    let Ok(repo_path) = RepoPath::new(manifest_path) else {
        return false;
    };
    package_root_key(workspace.package_of(&repo_path)) == package_root
}

fn scope_selection_to_plugin(
    selection: FrameworkProfileSelection,
    known_profiles: &[String],
) -> FrameworkProfileSelection {
    match selection {
        FrameworkProfileSelection::Auto => FrameworkProfileSelection::Auto,
        FrameworkProfileSelection::Disabled => FrameworkProfileSelection::Disabled,
        FrameworkProfileSelection::Exclude(excluded) => {
            let scoped: Vec<String> = excluded
                .into_iter()
                .filter(|name| known_profiles.contains(name))
                .collect();
            if scoped.is_empty() {
                FrameworkProfileSelection::Auto
            } else {
                FrameworkProfileSelection::Exclude(scoped)
            }
        }
        FrameworkProfileSelection::Only(only) => {
            let scoped: Vec<String> = only
                .into_iter()
                .filter(|name| known_profiles.contains(name))
                .collect();
            if scoped.is_empty() {
                FrameworkProfileSelection::Disabled
            } else {
                FrameworkProfileSelection::Only(scoped)
            }
        }
    }
}
