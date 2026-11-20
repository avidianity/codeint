use std::collections::HashSet;

use crate::{LanguagePlugin, RepoPath, SourceFile, language};

#[cfg(test)]
mod tests;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ImportResolution {
    Resolved(RepoPath),
    External,
    Unresolved { reason: String },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuntimeImport {
    pub importer: RepoPath,
    pub specifier: String,
    pub resolution: ImportResolution,
}

#[derive(Debug, thiserror::Error)]
pub enum ResolveError {
    #[error("the Language plugin answered {answered} of {asked} import requests")]
    WrongCount { asked: usize, answered: usize },
    #[error(
        "the Language plugin resolved {specifier:?} in {importer} to {target:?}, which is not a file of the repository"
    )]
    UnknownTarget {
        importer: RepoPath,
        specifier: String,
        target: String,
    },
}

pub fn resolve_runtime_imports(
    plugin: &dyn LanguagePlugin,
    repository_paths: &[RepoPath],
    importers: &[&SourceFile],
) -> Result<Vec<RuntimeImport>, ResolveError> {
    let pending: Vec<(&RepoPath, &String)> = importers
        .iter()
        .flat_map(|file| {
            file.model
                .runtime_imports
                .iter()
                .map(|specifier| (&file.path, specifier))
        })
        .collect();
    if pending.is_empty() {
        return Ok(Vec::new());
    }
    let requests = pending
        .iter()
        .map(|(importer, specifier)| language::ImportRequest {
            importer: importer.as_str().to_owned(),
            specifier: (*specifier).clone(),
        })
        .collect();
    let resolutions = plugin.resolve_imports(repository_paths, requests);
    if resolutions.len() != pending.len() {
        return Err(ResolveError::WrongCount {
            asked: pending.len(),
            answered: resolutions.len(),
        });
    }
    let known_paths: HashSet<&str> = repository_paths.iter().map(RepoPath::as_str).collect();
    pending
        .into_iter()
        .zip(resolutions)
        .map(|((importer, specifier), resolution)| {
            let resolution = match resolution {
                language::Resolution::Resolved(target) if known_paths.contains(target.as_str()) => {
                    ImportResolution::Resolved(RepoPath(target))
                }
                language::Resolution::Resolved(target) => {
                    return Err(ResolveError::UnknownTarget {
                        importer: importer.clone(),
                        specifier: specifier.clone(),
                        target,
                    });
                }
                language::Resolution::External => ImportResolution::External,
                language::Resolution::Unresolved(reason) => ImportResolution::Unresolved { reason },
            };
            Ok(RuntimeImport {
                importer: importer.clone(),
                specifier: specifier.clone(),
                resolution,
            })
        })
        .collect()
}
