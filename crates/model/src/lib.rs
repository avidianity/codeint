#![forbid(unsafe_code)]

use std::collections::BTreeMap;
use std::collections::btree_map::Entry;
use std::fmt;

mod metric;
mod runtime_imports;
mod source_set;
#[cfg(test)]
mod tests;

mod bindings {
    wit_bindgen::generate!({
        world: "language-plugin",
        path: "../../wit",
        additional_derives: [PartialEq, Eq],
        additional_derives_ignore: ["reference-range"],
    });
}

pub use bindings::exports::codeint::plugin::language;
pub use language::{FileModel, ReferenceRange, SourceSet, Token, TokenKind};
pub use metric::Metric;
pub use runtime_imports::{ImportResolution, ResolveError, RuntimeImport, resolve_runtime_imports};
pub use source_set::settle_source_set;

pub fn token_text_hash(text: &str) -> u64 {
    const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
    const FNV_PRIME: u64 = 0x0100_0000_01b3;
    text.bytes().fold(FNV_OFFSET_BASIS, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(FNV_PRIME)
    })
}

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
    pub submodules: Vec<RepoPath>,
    pub runtime_imports: Vec<RuntimeImport>,
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
        let source_set_of_importer: BTreeMap<RepoPath, SourceSet> = self
            .files
            .iter()
            .map(|file| (file.path.clone(), file.model.source_set))
            .collect();
        let mut source_sets = SourceSets::default();
        for file in self.files {
            source_sets.get_mut(file.model.source_set).files.push(file);
        }
        for import in self.runtime_imports {
            if let Some(&source_set) = source_set_of_importer.get(&import.importer) {
                source_sets.get_mut(source_set).runtime_imports.push(import);
            }
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

#[derive(Clone, Debug, Default)]
pub struct ReferenceRanges {
    by_metric: BTreeMap<String, ReferenceRange>,
}

#[derive(Debug, thiserror::Error)]
pub enum InvalidReferenceRange {
    #[error("the Language plugin supplies more than one Reference range for {metric}")]
    Duplicate { metric: String },
    #[error(
        "the Reference range for {metric} runs from {low} to {high}, but a Reference range runs from a lower to a higher ratio between 0 and 1"
    )]
    Bounds { metric: String, low: f64, high: f64 },
}

impl ReferenceRanges {
    pub fn new(ranges: Vec<ReferenceRange>) -> Result<ReferenceRanges, InvalidReferenceRange> {
        let mut by_metric = BTreeMap::new();
        for range in ranges {
            if !(0.0 <= range.low && range.low <= range.high && range.high <= 1.0) {
                return Err(InvalidReferenceRange::Bounds {
                    metric: range.metric,
                    low: range.low,
                    high: range.high,
                });
            }
            match by_metric.entry(range.metric.clone()) {
                Entry::Occupied(_) => {
                    return Err(InvalidReferenceRange::Duplicate {
                        metric: range.metric,
                    });
                }
                Entry::Vacant(vacant) => {
                    vacant.insert(range);
                }
            }
        }
        Ok(ReferenceRanges { by_metric })
    }

    pub fn for_metric(&self, metric: &str) -> Option<&ReferenceRange> {
        self.by_metric.get(metric)
    }
}

pub trait LanguagePlugin {
    fn extensions(&self) -> Vec<String>;

    fn lower(&self, path: &RepoPath, text: &str) -> Result<FileModel, LowerError>;

    fn resolve_imports(
        &self,
        repository_paths: &[RepoPath],
        requests: Vec<language::ImportRequest>,
    ) -> Vec<language::Resolution>;

    fn reference_ranges(&self) -> Result<ReferenceRanges, InvalidReferenceRange>;
}

impl<Plugin: language::Guest> LanguagePlugin for Plugin {
    fn extensions(&self) -> Vec<String> {
        Plugin::extensions()
    }

    fn lower(&self, path: &RepoPath, text: &str) -> Result<FileModel, LowerError> {
        Plugin::lower(path.as_str().to_owned(), text.to_owned()).map_err(LowerError::from)
    }

    fn resolve_imports(
        &self,
        repository_paths: &[RepoPath],
        requests: Vec<language::ImportRequest>,
    ) -> Vec<language::Resolution> {
        Plugin::resolve_imports(
            repository_paths
                .iter()
                .map(|path| path.as_str().to_owned())
                .collect(),
            requests,
        )
    }

    fn reference_ranges(&self) -> Result<ReferenceRanges, InvalidReferenceRange> {
        ReferenceRanges::new(Plugin::reference_ranges())
    }
}
