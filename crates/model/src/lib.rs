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
pub use language::{FileModel, SourceSet, Token};

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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SkippedFile {
    pub path: RepoPath,
    pub reason: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CodeModel {
    pub files: Vec<SourceFile>,
    pub skipped: Vec<SkippedFile>,
    pub declaration_only: Vec<RepoPath>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SourceSets {
    production: CodeModel,
    test: CodeModel,
    generated: CodeModel,
    vendored: CodeModel,
}

impl CodeModel {
    pub fn into_source_sets(self) -> SourceSets {
        let mut source_sets = SourceSets::default();
        for file in self.files {
            source_sets.get_mut(file.model.source_set).files.push(file);
        }
        source_sets
    }
}

impl SourceSets {
    pub fn get(&self, source_set: SourceSet) -> &CodeModel {
        match source_set {
            SourceSet::Production => &self.production,
            SourceSet::Test => &self.test,
            SourceSet::Generated => &self.generated,
            SourceSet::Vendored => &self.vendored,
        }
    }

    fn get_mut(&mut self, source_set: SourceSet) -> &mut CodeModel {
        match source_set {
            SourceSet::Production => &mut self.production,
            SourceSet::Test => &mut self.test,
            SourceSet::Generated => &mut self.generated,
            SourceSet::Vendored => &mut self.vendored,
        }
    }

    pub fn iter(&self) -> std::array::IntoIter<(SourceSet, &CodeModel), 4> {
        [
            SourceSet::Production,
            SourceSet::Test,
            SourceSet::Generated,
            SourceSet::Vendored,
        ]
        .map(|source_set| (source_set, self.get(source_set)))
        .into_iter()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum LowerError {
    #[error("cannot parse: {message}")]
    Unparsable { message: String },
    #[error("no Language plugin rule covers this file: {message}")]
    UnsupportedFile { message: String },
    #[error("the file has more source lines than the Code model can hold")]
    TooLarge,
    #[error("the file only declares types and holds no code to measure")]
    DeclarationOnly,
}

impl From<language::LowerError> for LowerError {
    fn from(error: language::LowerError) -> LowerError {
        match error {
            language::LowerError::Unparsable(message) => LowerError::Unparsable { message },
            language::LowerError::UnsupportedFile(message) => {
                LowerError::UnsupportedFile { message }
            }
            language::LowerError::TooLarge => LowerError::TooLarge,
            language::LowerError::DeclarationOnly => LowerError::DeclarationOnly,
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
