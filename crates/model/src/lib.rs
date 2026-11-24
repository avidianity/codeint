#![forbid(unsafe_code)]

use std::borrow::Borrow;
use std::collections::btree_map::Entry;
use std::collections::{BTreeMap, HashSet};
use std::fmt;

mod metric;
mod runtime_imports;
pub mod scan;
mod source_set;
#[cfg(test)]
mod tests;
mod workspace;

mod bindings {
    wit_bindgen::generate!({
        world: "language-plugin",
        path: "../../wit",
        additional_derives: [PartialEq, Eq],
        additional_derives_ignore: ["reference-range"],
    });
}

pub use bindings::exports::codeint::plugin::language;
pub use language::{
    DeclaredDep, DepKind, FileModel, FunctionCoverage, HazardLine, Manifest, ReferenceRange,
    SourceSet, Token, TokenKind,
};
pub use metric::{
    ExternalFinding, ExternalMetricMeasurement, Metric, MetricPlugin, MetricPluginError,
};
pub use runtime_imports::{
    ImportResolution, ResolveError, RuntimeImport, choose_manifests, resolve_runtime_imports,
};
pub use source_set::settle_source_set;
pub use workspace::{Package, Workspace, WorkspaceError};

pub fn token_text_hash(text: &str) -> u64 {
    StableHasher::new().write_bytes(text.as_bytes()).finish()
}

#[derive(Clone, Copy, Debug)]
pub struct StableHasher(u64);

impl StableHasher {
    const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
    const FNV_PRIME: u64 = 0x0100_0000_01b3;

    pub fn new() -> StableHasher {
        StableHasher(StableHasher::FNV_OFFSET_BASIS)
    }

    pub fn write_bytes(self, bytes: &[u8]) -> StableHasher {
        StableHasher(bytes.iter().fold(self.0, |hash, byte| {
            (hash ^ u64::from(*byte)).wrapping_mul(StableHasher::FNV_PRIME)
        }))
    }

    pub fn write_u64(self, value: u64) -> StableHasher {
        self.write_bytes(&value.to_le_bytes())
    }

    pub fn finish(self) -> u64 {
        self.0
    }
}

impl Default for StableHasher {
    fn default() -> StableHasher {
        StableHasher::new()
    }
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

    pub fn file_name(&self) -> &str {
        self.0.rsplit('/').next().unwrap_or(&self.0)
    }

    pub fn extension(&self) -> Option<&str> {
        match self.file_name().rsplit_once('.') {
            Some((stem, extension)) if !stem.is_empty() => Some(extension),
            _ => None,
        }
    }
}

impl Borrow<str> for RepoPath {
    fn borrow(&self) -> &str {
        &self.0
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

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct DependencyManifest {
    pub package_root: String,
    pub path: RepoPath,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ManifestUsedDep {
    pub package_root: String,
    pub name: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LanguageFacts {
    pub language: String,
    pub has_coverage: bool,
    pub has_hazard_rules: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CodeModel {
    pub files: Vec<SourceFile>,
    pub skipped: Vec<SkippedFile>,
    pub declaration_only: Vec<RepoPath>,
    pub workspace: Workspace,
    pub runtime_imports: Vec<RuntimeImport>,
    pub declared_dependencies: Vec<DeclaredDep>,
    pub dependency_manifests: Vec<DependencyManifest>,
    pub manifest_used_dependencies: Vec<ManifestUsedDep>,
    pub languages: Vec<LanguageFacts>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SourceSets {
    production: CodeModel,
    test: CodeModel,
    generated: CodeModel,
    vendored: CodeModel,
}

impl CodeModel {
    pub fn merge(models: impl IntoIterator<Item = CodeModel>) -> CodeModel {
        let mut merged = CodeModel::default();
        let mut languages: BTreeMap<String, LanguageFacts> = BTreeMap::new();
        for model in models {
            merged.files.extend(model.files);
            merged.skipped.extend(model.skipped);
            merged.declaration_only.extend(model.declaration_only);
            merged.runtime_imports.extend(model.runtime_imports);
            merged.workspace.join(model.workspace);
            merged
                .declared_dependencies
                .extend(model.declared_dependencies);
            merged
                .dependency_manifests
                .extend(model.dependency_manifests);
            merged
                .manifest_used_dependencies
                .extend(model.manifest_used_dependencies);
            for facts in model.languages {
                languages
                    .entry(facts.language.clone())
                    .and_modify(|existing| {
                        existing.has_coverage |= facts.has_coverage;
                        existing.has_hazard_rules |= facts.has_hazard_rules;
                    })
                    .or_insert(facts);
            }
        }
        merged
            .files
            .sort_by(|left, right| left.path.cmp(&right.path));
        merged
            .skipped
            .sort_by(|left, right| left.path.cmp(&right.path));
        merged.declaration_only.sort();
        merged.dependency_manifests.sort();
        merged.dependency_manifests.dedup();
        merged.manifest_used_dependencies.sort();
        merged.manifest_used_dependencies.dedup();
        merged.languages = languages.into_values().collect();
        merged
    }

    pub fn into_source_sets(self) -> SourceSets {
        let source_set_of_importer: BTreeMap<RepoPath, SourceSet> = self
            .files
            .iter()
            .map(|file| (file.path.clone(), file.model.source_set))
            .collect();
        let mut source_sets = SourceSets::default();
        for source_set in [
            &mut source_sets.production,
            &mut source_sets.test,
            &mut source_sets.generated,
            &mut source_sets.vendored,
        ] {
            source_set.workspace = self.workspace.clone();
            source_set.languages = self.languages.clone();
        }
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
    #[error("Plugin {plugin:?} exceeded its memory cap of {memory_mb} MiB")]
    MemoryLimitExceeded { plugin: String, memory_mb: u64 },
    #[error("Plugin {plugin:?} exceeded its time limit of {timeout_ms} ms")]
    TimeLimitExceeded { plugin: String, timeout_ms: u64 },
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

#[derive(Debug, thiserror::Error)]
pub enum LanguagePluginError {
    #[error("the Language plugin returned an invalid entry point path {path:?}")]
    InvalidEntryPointPath { path: String },
    #[error(
        "the Language plugin returned entry point {path:?}, which is not a file of the repository"
    )]
    UnknownEntryPoint { path: String },
    #[error(
        "the Language plugin declared a dependency with an invalid package root {package_root:?}"
    )]
    InvalidDependencyPackageRoot { package_root: String },
    #[error(
        "the Language plugin declared a dependency with an empty name in package root {package_root:?}"
    )]
    EmptyDependencyName { package_root: String },
    #[error("Plugin {plugin:?} exceeded its time limit of {timeout_ms} ms")]
    TimeLimitExceeded { plugin: String, timeout_ms: u64 },
    #[error("Plugin {plugin:?} exceeded its memory cap of {memory_mb} MiB")]
    MemoryLimitExceeded { plugin: String, memory_mb: u64 },
    #[error("Language plugin {plugin:?} failed during {call}: {reason}")]
    Call {
        plugin: String,
        call: &'static str,
        reason: String,
    },
    #[error(
        "Language plugin {plugin:?} returned a Hazard line with unlisted rule {rule:?} in {path}"
    )]
    UnlistedHazardRule {
        plugin: String,
        path: RepoPath,
        rule: String,
    },
}

pub fn validate_declared_dependencies(
    deps: Vec<DeclaredDep>,
) -> Result<Vec<DeclaredDep>, LanguagePluginError> {
    for dep in &deps {
        if !dep.package_root.is_empty() && RepoPath::new(dep.package_root.as_str()).is_err() {
            return Err(LanguagePluginError::InvalidDependencyPackageRoot {
                package_root: dep.package_root.clone(),
            });
        }
        if dep.name.trim().is_empty() {
            return Err(LanguagePluginError::EmptyDependencyName {
                package_root: dep.package_root.clone(),
            });
        }
    }
    Ok(deps)
}

pub fn validate_entry_points(
    repository_paths: &[RepoPath],
    entries: Vec<String>,
) -> Result<Vec<RepoPath>, LanguagePluginError> {
    let known: HashSet<&str> = repository_paths.iter().map(RepoPath::as_str).collect();
    entries
        .into_iter()
        .map(|path| {
            let repo_path = RepoPath::new(path.clone())
                .map_err(|_| LanguagePluginError::InvalidEntryPointPath { path: path.clone() })?;
            if !known.contains(repo_path.as_str()) {
                return Err(LanguagePluginError::UnknownEntryPoint { path });
            }
            Ok(repo_path)
        })
        .collect()
}

pub trait LanguagePlugin: Sync {
    fn extensions(&self) -> Vec<String>;

    fn lower(&self, path: &RepoPath, text: &str) -> Result<FileModel, LowerError>;

    fn manifest_paths(&self, repository_paths: &[RepoPath]) -> Vec<String>;

    fn workspace_packages(&self, manifests: Vec<Manifest>)
    -> Result<Vec<RepoPath>, WorkspaceError>;

    fn resolve_imports(
        &self,
        repository_paths: &[RepoPath],
        manifests: Vec<Manifest>,
        requests: Vec<language::ImportRequest>,
    ) -> Vec<language::Resolution>;

    fn declared_dependencies(
        &self,
        manifests: Vec<Manifest>,
    ) -> Result<Vec<DeclaredDep>, LanguagePluginError>;

    fn entry_points(
        &self,
        repository_paths: &[RepoPath],
        manifests: Vec<Manifest>,
    ) -> Result<Vec<RepoPath>, LanguagePluginError>;

    fn reference_ranges(&self) -> Result<ReferenceRanges, InvalidReferenceRange>;

    fn has_hazard_rules(&self) -> bool;

    fn hazard_rules(&self) -> Vec<String>;
}

impl<Plugin: language::Guest + Sync> LanguagePlugin for Plugin {
    fn extensions(&self) -> Vec<String> {
        Plugin::extensions()
    }

    fn lower(&self, path: &RepoPath, text: &str) -> Result<FileModel, LowerError> {
        Plugin::lower(path.as_str().to_owned(), text.to_owned()).map_err(LowerError::from)
    }

    fn manifest_paths(&self, repository_paths: &[RepoPath]) -> Vec<String> {
        Plugin::manifest_paths(owned_paths(repository_paths))
    }

    fn workspace_packages(
        &self,
        manifests: Vec<Manifest>,
    ) -> Result<Vec<RepoPath>, WorkspaceError> {
        Plugin::workspace_packages(manifests)?
            .into_iter()
            .map(|root| Ok(RepoPath::new(root)?))
            .collect()
    }

    fn resolve_imports(
        &self,
        repository_paths: &[RepoPath],
        manifests: Vec<Manifest>,
        requests: Vec<language::ImportRequest>,
    ) -> Vec<language::Resolution> {
        Plugin::resolve_imports(owned_paths(repository_paths), manifests, requests)
    }

    fn declared_dependencies(
        &self,
        manifests: Vec<Manifest>,
    ) -> Result<Vec<DeclaredDep>, LanguagePluginError> {
        validate_declared_dependencies(Plugin::declared_dependencies(manifests))
    }

    fn entry_points(
        &self,
        repository_paths: &[RepoPath],
        manifests: Vec<Manifest>,
    ) -> Result<Vec<RepoPath>, LanguagePluginError> {
        validate_entry_points(
            repository_paths,
            Plugin::entry_points(owned_paths(repository_paths), manifests),
        )
    }

    fn reference_ranges(&self) -> Result<ReferenceRanges, InvalidReferenceRange> {
        ReferenceRanges::new(Plugin::reference_ranges())
    }

    fn has_hazard_rules(&self) -> bool {
        Plugin::has_hazard_rules()
    }

    fn hazard_rules(&self) -> Vec<String> {
        Plugin::hazard_rules()
    }
}

fn owned_paths(repository_paths: &[RepoPath]) -> Vec<String> {
    repository_paths
        .iter()
        .map(|path| path.as_str().to_owned())
        .collect()
}
