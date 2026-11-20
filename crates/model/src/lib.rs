#![forbid(unsafe_code)]

use std::fmt;

#[cfg(test)]
mod tests;

mod bindings {
    wit_bindgen::generate!({
        world: "language-plugin",
        path: "../../wit",
        additional_derives: [PartialEq, Eq],
    });
}

pub use bindings::exports::codeint::plugin::language;
pub use language::FileModel;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RepoPath(String);

#[derive(Debug, thiserror::Error)]
#[error("{path:?} is not a repository-relative path with forward slashes")]
pub struct InvalidRepoPath {
    path: String,
}

impl RepoPath {
    pub fn new(path: impl Into<String>) -> Result<RepoPath, InvalidRepoPath> {
        let path = path.into();
        let is_relative_with_forward_slashes = !path.contains('\\')
            && path
                .split('/')
                .all(|segment| !matches!(segment, "" | "." | ".."));
        if is_relative_with_forward_slashes {
            Ok(RepoPath(path))
        } else {
            Err(InvalidRepoPath { path })
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn extension(&self) -> Option<&str> {
        let file_name = self.0.rsplit('/').next().unwrap_or(&self.0);
        match file_name.rsplit_once('.') {
            Some((stem, extension)) if !stem.is_empty() => Some(extension),
            _ => None,
        }
    }
}

impl fmt::Display for RepoPath {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
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
    #[error("no Language plugin rule covers this file: {message}")]
    UnsupportedFile { message: String },
    #[error("the file has more source lines than the Code model can hold")]
    TooLarge,
}

impl From<language::LowerError> for LowerError {
    fn from(error: language::LowerError) -> LowerError {
        match error {
            language::LowerError::Unparsable(message) => LowerError::Unparsable { message },
            language::LowerError::UnsupportedFile(message) => {
                LowerError::UnsupportedFile { message }
            }
            language::LowerError::TooLarge => LowerError::TooLarge,
        }
    }
}

pub trait LanguagePlugin {
    fn extensions(&self) -> Vec<String>;

    fn lower(&self, path: &RepoPath, text: &str) -> Result<FileModel, LowerError>;
}

impl<Plugin: language::Guest> LanguagePlugin for Plugin {
    fn extensions(&self) -> Vec<String> {
        Plugin::extensions()
    }

    fn lower(&self, path: &RepoPath, text: &str) -> Result<FileModel, LowerError> {
        Plugin::lower(path.as_str().to_owned(), text.to_owned()).map_err(LowerError::from)
    }
}
