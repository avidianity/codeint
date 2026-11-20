use crate::{LanguagePlugin, RepoPath, SourceFile};

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
    _plugin: &dyn LanguagePlugin,
    _repository_paths: &[RepoPath],
    _importers: &[&SourceFile],
) -> Result<Vec<RuntimeImport>, ResolveError> {
    todo!()
}
