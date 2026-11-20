#![forbid(unsafe_code)]

use std::fmt;

#[cfg(test)]
mod tests;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RepoPath(String);

#[derive(Debug, thiserror::Error)]
#[error("{path:?} is not a repository-relative path with forward slashes")]
pub struct InvalidRepoPath {
    path: String,
}

impl RepoPath {
    pub fn new(_path: impl Into<String>) -> Result<RepoPath, InvalidRepoPath> {
        todo!()
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn extension(&self) -> Option<&str> {
        todo!()
    }
}

impl fmt::Display for RepoPath {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileModel {
    pub source_lines: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceFile {
    pub path: RepoPath,
    pub model: FileModel,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CodeModel {
    pub files: Vec<SourceFile>,
}

#[derive(Debug, thiserror::Error)]
pub enum LowerError {
    #[error("cannot parse: {message}")]
    Unparsable { message: String },
}

pub trait LanguagePlugin {
    fn extensions(&self) -> Vec<String>;

    fn lower(&self, path: &RepoPath, text: &str) -> Result<FileModel, LowerError>;
}
