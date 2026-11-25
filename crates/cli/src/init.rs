use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use codeint_config::{DetectedLanguage, StarterConfigurationError, write_starter_configuration};
use codeint_git::{GitError, Repository};
use codeint_model::{InvalidRepoPath, RepoPath};
use codeint_plugin_host::{NamedLanguagePlugin, built_in_language_plugins};

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
    let languages = detect_languages(tree.files(), built_in_language_plugins())?;
    Ok(write_starter_configuration(repository.root(), &languages)?)
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
