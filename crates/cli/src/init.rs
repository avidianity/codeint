use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use codeint_config::{
    BoundaryPreset, DetectedLanguage, StarterBoundaries, StarterConfigurationError,
    write_starter_configuration,
};
use codeint_git::{GitError, Repository, Snapshot};
use codeint_model::{InvalidRepoPath, RepoPath, SourceSet, settle_source_set};
use codeint_plugin_host::{NamedLanguagePlugin, built_in_language_plugins};

use crate::check::{CheckError, find_workspace, read_manifests};

#[derive(Debug, thiserror::Error)]
pub(crate) enum InitError {
    #[error(transparent)]
    Git(#[from] GitError),
    #[error(transparent)]
    StarterConfiguration(#[from] StarterConfigurationError),
}

pub(crate) fn init(folder: &Path) -> Result<PathBuf, InitError> {
    let repository = Repository::discover(folder)?;
    let tree = repository.working_tree()?;
    let plugins = built_in_language_plugins();
    let languages = detect_languages(tree.files(), plugins)?;
    let boundaries = discover_starter_boundaries(&tree, plugins)?;
    Ok(write_starter_configuration(
        repository.root(),
        &languages,
        boundaries.as_ref(),
    )?)
}

fn discover_starter_boundaries(
    tree: &Snapshot,
    plugins: &[NamedLanguagePlugin<'_>],
) -> Result<Option<StarterBoundaries>, GitError> {
    let workspace_roots = discover_workspace_roots(tree, plugins)?;
    if let Some(workspace_boundaries) = StarterBoundaries::from_workspace_packages(&workspace_roots)
    {
        return Ok(Some(workspace_boundaries));
    }
    let source_files = import_capable_production_files(tree.files(), plugins);
    Ok(BoundaryPreset::detect(&source_files).map(StarterBoundaries::Preset))
}

fn discover_workspace_roots(
    tree: &Snapshot,
    plugins: &[NamedLanguagePlugin<'_>],
) -> Result<Vec<RepoPath>, GitError> {
    let manifests = match read_manifests(tree, plugins) {
        Ok(manifests) => manifests,
        Err(CheckError::Git(error)) => return Err(error),
        Err(_) => return Ok(Vec::new()),
    };
    let workspace = match find_workspace(plugins, &manifests) {
        Ok(workspace) => workspace,
        Err(CheckError::Git(error)) => return Err(error),
        Err(_) => return Ok(Vec::new()),
    };
    Ok(workspace.member_roots().cloned().collect())
}

fn import_capable_production_files(
    files: &[RepoPath],
    plugins: &[NamedLanguagePlugin<'_>],
) -> Vec<RepoPath> {
    files
        .iter()
        .filter(|path| is_import_capable_production_file(path, plugins))
        .cloned()
        .collect()
}

fn is_import_capable_production_file(path: &RepoPath, plugins: &[NamedLanguagePlugin<'_>]) -> bool {
    if settle_source_set(path, "", SourceSet::Production) != SourceSet::Production {
        return false;
    }
    path.extension().is_some_and(|extension| {
        plugins.iter().any(|named| {
            named.plugin.has_imports()
                && named
                    .plugin
                    .extensions()
                    .iter()
                    .any(|claimed| claimed == extension)
        })
    })
}

fn detect_languages(
    files: &[RepoPath],
    plugins: &[NamedLanguagePlugin<'_>],
) -> Result<Vec<DetectedLanguage>, GitError> {
    let mut detected = Vec::new();
    for named in plugins {
        let extensions = named.plugin.extensions();
        let top_level_paths: BTreeSet<RepoPath> = files
            .iter()
            .filter(|path| {
                path.extension()
                    .is_some_and(|extension| extensions.iter().any(|claimed| claimed == extension))
            })
            .map(top_level_path)
            .collect::<Result<_, InvalidRepoPath>>()?;
        if !top_level_paths.is_empty() {
            detected.push(DetectedLanguage {
                plugin: named.name.to_owned(),
                include: top_level_paths.into_iter().collect(),
            });
        }
    }
    Ok(detected)
}

fn top_level_path(path: &RepoPath) -> Result<RepoPath, InvalidRepoPath> {
    let first_segment = path.as_str().split('/').next().unwrap_or(path.as_str());
    RepoPath::new(first_segment)
}
