use std::collections::HashSet;

use crate::{LanguagePlugin, LanguagePluginError, Manifest, RepoPath, SourceFile, language};

#[cfg(test)]
mod tests;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ImportResolution {
    Resolved(Vec<RepoPath>),
    External(String),
    Builtin,
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
    #[error("the Language plugin resolved {specifier:?} in {importer} to an empty list of files")]
    EmptyResolution {
        importer: RepoPath,
        specifier: String,
    },
    #[error(
        "the Language plugin resolved {specifier:?} in {importer} to {target:?}, which is not a file of the repository"
    )]
    UnknownTarget {
        importer: RepoPath,
        specifier: String,
        target: String,
    },
    #[error("the Language plugin asked to read {path:?}, which is not a file of the repository")]
    UnknownManifest { path: String },
    #[error(transparent)]
    Plugin(#[from] LanguagePluginError),
}

pub fn choose_manifests(
    plugin: &dyn LanguagePlugin,
    repository_paths: &[RepoPath],
) -> Result<Vec<RepoPath>, ResolveError> {
    if !plugin.has_imports() {
        return Ok(Vec::new());
    }
    let known_paths: HashSet<&str> = repository_paths.iter().map(RepoPath::as_str).collect();
    plugin
        .manifest_paths(repository_paths)
        .into_iter()
        .map(|path| {
            if known_paths.contains(path.as_str()) {
                Ok(RepoPath(path))
            } else {
                Err(ResolveError::UnknownManifest { path })
            }
        })
        .collect()
}

pub fn resolve_runtime_imports(
    plugin: &dyn LanguagePlugin,
    repository_paths: &[RepoPath],
    manifests: Vec<Manifest>,
    importers: &[&SourceFile],
    active_profiles: &[String],
) -> Result<Vec<RuntimeImport>, ResolveError> {
    if !plugin.has_imports() {
        return Ok(Vec::new());
    }
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
    let resolutions =
        plugin.resolve_imports(repository_paths, manifests, requests, active_profiles)?;
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
            let resolution = validate_resolution(importer, specifier, resolution, &known_paths)?;
            Ok(RuntimeImport {
                importer: importer.clone(),
                specifier: specifier.clone(),
                resolution,
            })
        })
        .collect()
}

fn validate_resolution(
    importer: &RepoPath,
    specifier: &str,
    resolution: language::Resolution,
    known_paths: &HashSet<&str>,
) -> Result<ImportResolution, ResolveError> {
    match resolution {
        language::Resolution::Resolved(targets) => {
            validate_resolved_targets(importer, specifier, targets, known_paths)
                .map(ImportResolution::Resolved)
        }
        language::Resolution::External(package) => Ok(ImportResolution::External(package)),
        language::Resolution::Builtin => Ok(ImportResolution::Builtin),
        language::Resolution::Unresolved(reason) => Ok(ImportResolution::Unresolved { reason }),
    }
}

fn validate_resolved_targets(
    importer: &RepoPath,
    specifier: &str,
    targets: Vec<String>,
    known_paths: &HashSet<&str>,
) -> Result<Vec<RepoPath>, ResolveError> {
    if targets.is_empty() {
        return Err(ResolveError::EmptyResolution {
            importer: importer.clone(),
            specifier: specifier.to_owned(),
        });
    }
    let mut validated = Vec::with_capacity(targets.len());
    let mut seen_targets = HashSet::with_capacity(targets.len());
    for target in targets {
        if !known_paths.contains(target.as_str()) {
            return Err(ResolveError::UnknownTarget {
                importer: importer.clone(),
                specifier: specifier.to_owned(),
                target,
            });
        }
        if seen_targets.insert(target.clone()) {
            validated.push(RepoPath(target));
        }
    }
    Ok(validated)
}
