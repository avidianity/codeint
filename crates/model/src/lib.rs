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
mod tooling;
mod workspace;

mod bindings {
    wit_bindgen::generate!({
        world: "language-plugin",
        path: "../../wit",
        additional_derives: [PartialEq, Eq],
        additional_derives_ignore: ["reference-range", "framework-profile-result"],
    });
}

pub use bindings::exports::codeint::plugin::language;
pub use language::{
    DeclaredDep, DepKind, DependencyManifestOwner, FileDiagnostic, FileHazards, FileModel,
    FileScopeArtifact, FrameworkProfileResult, FrameworkProfileSelection, FunctionCoverage,
    HazardLine, Manifest, ManifestAnalysis, PackageDependency, PackageFrameworkProfiles,
    ReferenceRange, ScopeAnalysis, ScopeSummary, SourceSet, SummaryField, Token, TokenKind,
};
pub use metric::{
    ExternalFinding, ExternalMetricMeasurement, Metric, MetricCapability, MetricDescriptor,
    MetricKind, MetricPlugin, MetricPluginError, MetricScope, NewFindingsRule,
    STRUCTURAL_REPORT_KEYS, is_reserved_report_key,
};
pub use runtime_imports::{
    ImportResolution, ResolveError, RuntimeImport, choose_manifests,
    is_valid_external_package_name, resolve_runtime_imports,
};
pub use source_set::{
    STANDARD_GENERATED_MARKERS, contains_ascii_ignore_case, has_any_ascii_marker, has_folder_named,
    has_generated_header_marker, has_generated_header_marker_with_extra, path_file_name,
    settle_source_set,
};
pub use tooling::UniversalToolingPaths;
pub use workspace::{Package, SkippedManifest, Workspace, WorkspaceError};

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
        path_file_name(&self.0)
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

impl AsRef<str> for RepoPath {
    fn as_ref(&self) -> &str {
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
    pub has_imports: bool,
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
                        existing.has_imports |= facts.has_imports;
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

    pub fn is_tooling_file(&self, file: &SourceFile) -> bool {
        file.model.is_tooling_file
            || UniversalToolingPaths::from_model(self).is_universal_tooling_path(&file.path)
    }

    pub fn is_tooling_path(&self, path: &RepoPath) -> bool {
        self.files
            .iter()
            .find(|file| &file.path == path)
            .is_some_and(|file| file.model.is_tooling_file)
            || UniversalToolingPaths::from_model(self).is_universal_tooling_path(path)
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
    #[error(transparent)]
    Contract(#[from] LanguagePluginError),
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

impl ReferenceRange {
    pub fn is_stale_for(&self, current_metric_version: u32) -> bool {
        self.metric_version != current_metric_version
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

    pub fn iter(&self) -> impl Iterator<Item = &ReferenceRange> {
        self.by_metric.values()
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
    #[error("the Language plugin returned an invalid generated path {path:?}")]
    InvalidGeneratedPath { path: String },
    #[error(
        "the Language plugin returned generated path {path:?}, which is not a file of the package"
    )]
    UnknownGeneratedPath { path: String },
    #[error("the Language plugin returned an unlisted Framework profile {profile:?}")]
    UnlistedFrameworkProfile { profile: String },
    #[error("the Language plugin failed to resolve Framework profiles: {message}")]
    FrameworkProfile { message: String },
    #[error("the Language plugin returned an invalid Framework profile Reference range")]
    InvalidFrameworkReferenceRange(#[from] InvalidReferenceRange),
    #[error("the Language plugin returned an invalid Reference range")]
    InvalidReferenceRange(#[source] InvalidReferenceRange),
    #[error(
        "the Language plugin declared a dependency with an invalid package root {package_root:?}"
    )]
    InvalidDependencyPackageRoot { package_root: String },
    #[error(
        "the Language plugin declared a dependency with an empty name in package root {package_root:?}"
    )]
    EmptyDependencyName { package_root: String },
    #[error("the Language plugin returned an invalid workspace package root {path:?}")]
    InvalidWorkspacePackageRoot { path: String },
    #[error(
        "the Language plugin reported invalid manifest {path:?} which was not passed in: {message}"
    )]
    UnknownInvalidManifest { path: String, message: String },
    #[error("the Language plugin reported duplicate invalid manifest {path:?}")]
    DuplicateInvalidManifest { path: String },
    #[error("the Language plugin returned an invalid dependency manifest path {path:?}")]
    InvalidDependencyManifestPath { path: String },
    #[error(
        "the Language plugin returned an invalid root-dependency inheritance package root {package_root:?}"
    )]
    InvalidInheritancePackageRoot { package_root: String },
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
        "Language plugin {plugin:?} returned Hazard line {line} in {path}, which is outside the file's {line_count} lines"
    )]
    HazardLineOutOfRange {
        plugin: String,
        path: RepoPath,
        line: u32,
        line_count: usize,
    },
    #[error(
        "Language plugin {plugin:?} returned a Hazard line with unlisted rule {rule:?} in {path}"
    )]
    UnlistedHazardRule {
        plugin: String,
        path: RepoPath,
        rule: String,
    },
    #[error(
        "Language plugin {plugin:?} returned Flagged line {line} in {path}, which is outside the file's {line_count} lines"
    )]
    FlaggedLineOutOfRange {
        plugin: String,
        path: RepoPath,
        line: u32,
        line_count: usize,
    },
    #[error(
        "Language plugin {plugin:?} returned a Flagged line with unlisted rule {rule:?} in {path}"
    )]
    UnlistedFlaggedLineRule {
        plugin: String,
        path: RepoPath,
        rule: String,
    },
    #[error("Language plugin {plugin:?} returned an invalid extra-hazards path {path:?}")]
    InvalidScopeHazardPath { plugin: String, path: String },
    #[error(
        "Language plugin {plugin:?} returned extra-hazards path {path:?}, which was not in the analyzed artifacts"
    )]
    UnknownScopeHazardPath { plugin: String, path: String },
    #[error("Language plugin {plugin:?} returned invalid extra Hazard line {line} in {path}")]
    InvalidScopeHazardLine {
        plugin: String,
        path: RepoPath,
        line: u32,
    },
    #[error("Language plugin {plugin:?} returned an invalid scope summary: {reason}")]
    InvalidScopeSummary { plugin: String, reason: String },
}

#[derive(Clone, Copy)]
enum RuleLineKind {
    Flagged,
    Hazard,
}

trait RuleLine {
    const KIND: RuleLineKind;
    fn line(&self) -> u32;
    fn rule(&self) -> &str;
}

impl RuleLine for language::FlaggedLine {
    const KIND: RuleLineKind = RuleLineKind::Flagged;
    fn line(&self) -> u32 {
        self.line
    }
    fn rule(&self) -> &str {
        &self.rule
    }
}

impl RuleLine for HazardLine {
    const KIND: RuleLineKind = RuleLineKind::Hazard;
    fn line(&self) -> u32 {
        self.line
    }
    fn rule(&self) -> &str {
        &self.rule
    }
}

fn validate_rule_lines<T: RuleLine>(
    plugin_name: &str,
    path: &RepoPath,
    line_count: usize,
    entries: &[T],
    listed_rules: &HashSet<String>,
) -> Result<(), LanguagePluginError> {
    for entry in entries {
        let line_number = usize::try_from(entry.line()).unwrap_or(usize::MAX);
        if line_number < 1 || line_number > line_count {
            let plugin = plugin_name.to_owned();
            let path = path.clone();
            let line = entry.line();
            return Err(match T::KIND {
                RuleLineKind::Flagged => LanguagePluginError::FlaggedLineOutOfRange {
                    plugin,
                    path,
                    line,
                    line_count,
                },
                RuleLineKind::Hazard => LanguagePluginError::HazardLineOutOfRange {
                    plugin,
                    path,
                    line,
                    line_count,
                },
            });
        }
    }
    for entry in entries {
        if !listed_rules.contains(entry.rule()) {
            let plugin = plugin_name.to_owned();
            let path = path.clone();
            let rule = entry.rule().to_owned();
            return Err(match T::KIND {
                RuleLineKind::Flagged => {
                    LanguagePluginError::UnlistedFlaggedLineRule { plugin, path, rule }
                }
                RuleLineKind::Hazard => {
                    LanguagePluginError::UnlistedHazardRule { plugin, path, rule }
                }
            });
        }
    }
    Ok(())
}

pub fn validate_flagged_lines(
    plugin_name: &str,
    path: &RepoPath,
    line_count: usize,
    flagged_lines: &[language::FlaggedLine],
    listed_rules: &HashSet<String>,
) -> Result<(), LanguagePluginError> {
    validate_rule_lines(plugin_name, path, line_count, flagged_lines, listed_rules)
}

pub fn validate_scope_analysis(
    plugin_name: &str,
    artifacts: &[FileScopeArtifact],
    known_hazard_rules: &HashSet<String>,
    line_counts: Option<&BTreeMap<String, usize>>,
    analysis: ScopeAnalysis,
) -> Result<ScopeAnalysis, LanguagePluginError> {
    let artifact_paths: HashSet<&str> = artifacts.iter().map(|item| item.path.as_str()).collect();
    for entry in &analysis.extra_hazards {
        validate_scope_hazards(
            plugin_name,
            entry,
            &artifact_paths,
            known_hazard_rules,
            line_counts,
        )?;
    }
    for summary in &analysis.summaries {
        if summary.kind.trim().is_empty()
            || summary
                .fields
                .iter()
                .any(|field| field.key.trim().is_empty())
        {
            return Err(LanguagePluginError::InvalidScopeSummary {
                plugin: plugin_name.to_owned(),
                reason: "scope summary has an empty kind or field key".to_owned(),
            });
        }
    }
    Ok(analysis)
}

fn validate_scope_hazards(
    plugin_name: &str,
    entry: &FileHazards,
    artifact_paths: &HashSet<&str>,
    known_hazard_rules: &HashSet<String>,
    line_counts: Option<&BTreeMap<String, usize>>,
) -> Result<(), LanguagePluginError> {
    let repo_path = RepoPath::new(entry.path.as_str()).map_err(|_| {
        LanguagePluginError::InvalidScopeHazardPath {
            plugin: plugin_name.to_owned(),
            path: entry.path.clone(),
        }
    })?;
    if !artifact_paths.contains(entry.path.as_str()) {
        return Err(LanguagePluginError::UnknownScopeHazardPath {
            plugin: plugin_name.to_owned(),
            path: entry.path.clone(),
        });
    }
    if let Some(counts) = line_counts {
        let max_lines = counts.get(&entry.path).copied().unwrap_or(0);
        return validate_hazard_lines(
            plugin_name,
            &repo_path,
            max_lines,
            &entry.hazard_lines,
            known_hazard_rules,
        );
    }
    for hazard in &entry.hazard_lines {
        if hazard.line < 1 {
            return Err(LanguagePluginError::InvalidScopeHazardLine {
                plugin: plugin_name.to_owned(),
                path: repo_path.clone(),
                line: hazard.line,
            });
        }
        if !known_hazard_rules.contains(&hazard.rule) {
            return Err(LanguagePluginError::UnlistedHazardRule {
                plugin: plugin_name.to_owned(),
                path: repo_path.clone(),
                rule: hazard.rule.clone(),
            });
        }
    }
    Ok(())
}

pub fn validate_hazard_lines(
    plugin_name: &str,
    path: &RepoPath,
    line_count: usize,
    hazard_lines: &[HazardLine],
    listed_rules: &HashSet<String>,
) -> Result<(), LanguagePluginError> {
    validate_rule_lines(plugin_name, path, line_count, hazard_lines, listed_rules)
}

pub fn validate_workspace_packages(roots: Vec<String>) -> Result<Vec<RepoPath>, WorkspaceError> {
    roots
        .into_iter()
        .map(|root| Ok(RepoPath::new(root)?))
        .collect()
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

fn validate_known_paths(
    paths: &[String],
    known_files: &HashSet<&str>,
    invalid: fn(String) -> LanguagePluginError,
    unknown: fn(String) -> LanguagePluginError,
) -> Result<(), LanguagePluginError> {
    for path in paths {
        let repo_path = RepoPath::new(path.clone()).map_err(|_| invalid(path.clone()))?;
        if !known_files.contains(repo_path.as_str()) {
            return Err(unknown(path.clone()));
        }
    }
    Ok(())
}

pub fn validate_framework_profile_result(
    known_profiles: &[String],
    files: &[RepoPath],
    result: FrameworkProfileResult,
) -> Result<FrameworkProfileResult, LanguagePluginError> {
    let known_profile_set: HashSet<&str> = known_profiles.iter().map(String::as_str).collect();
    for profile in &result.active_profiles {
        if !known_profile_set.contains(profile.as_str()) {
            return Err(LanguagePluginError::UnlistedFrameworkProfile {
                profile: profile.clone(),
            });
        }
    }
    let known_files: HashSet<&str> = files.iter().map(RepoPath::as_str).collect();
    validate_known_paths(
        &result.entry_points,
        &known_files,
        |path| LanguagePluginError::InvalidEntryPointPath { path },
        |path| LanguagePluginError::UnknownEntryPoint { path },
    )?;
    validate_known_paths(
        &result.generated_paths,
        &known_files,
        |path| LanguagePluginError::InvalidGeneratedPath { path },
        |path| LanguagePluginError::UnknownGeneratedPath { path },
    )?;
    let _ = ReferenceRanges::new(result.reference_ranges.clone())?;
    Ok(result)
}

pub fn require_extension<'a>(
    path: &'a RepoPath,
    suffix: &str,
    language: &str,
) -> Result<&'a str, LowerError> {
    let raw = path.as_str();
    if raw.ends_with(suffix) {
        Ok(raw)
    } else {
        Err(LowerError::UnsupportedFile {
            message: format!("{raw} is not a {language} source file"),
        })
    }
}

pub fn empty_file_model(source_set: SourceSet) -> FileModel {
    FileModel {
        source_set,
        source_lines: 0,
        functions: Vec::new(),
        tokens: Vec::new(),
        flagged_lines: Vec::new(),
        hazard_lines: Vec::new(),
        runtime_imports: Vec::new(),
        program_start: false,
        is_tooling_file: false,
        diagnostics: Vec::new(),
        scope_artifact: None,
    }
}

pub fn empty_manifest_analysis() -> ManifestAnalysis {
    ManifestAnalysis {
        workspace_packages: Vec::new(),
        invalid_manifests: Vec::new(),
        dependency_manifests: Vec::new(),
        declared_dependencies: Vec::new(),
        manifest_used_dependencies: Vec::new(),
        root_dependency_inheritance: Vec::new(),
    }
}

pub fn empty_scope_analysis() -> ScopeAnalysis {
    ScopeAnalysis {
        extra_hazards: Vec::new(),
        summaries: Vec::new(),
    }
}

pub fn active_profiles_for_path<'a>(
    package_profiles: &'a [PackageFrameworkProfiles],
    path: &str,
) -> &'a [String] {
    let mut best: Option<(usize, &'a [String])> = None;
    for entry in package_profiles {
        let matches = if entry.package_root.is_empty() {
            true
        } else {
            path == entry.package_root
                || path
                    .strip_prefix(entry.package_root.as_str())
                    .is_some_and(|rest| rest.starts_with('/'))
        };
        if matches {
            let depth = entry.package_root.len();
            if best.is_none_or(|(best_depth, _)| depth >= best_depth) {
                best = Some((depth, entry.active_profiles.as_slice()));
            }
        }
    }
    best.map_or(&[], |(_, profiles)| profiles)
}

pub type ResolveBatchFn = fn(
    &[RepoPath],
    &[Manifest],
    &[language::ImportRequest],
    &[String],
) -> Vec<language::Resolution>;

pub fn resolve_imports_by_package_profiles(
    repository_paths: &[RepoPath],
    manifests: &[Manifest],
    requests: &[language::ImportRequest],
    package_profiles: &[PackageFrameworkProfiles],
    resolve_batch: ResolveBatchFn,
) -> Vec<language::Resolution> {
    if requests.is_empty() {
        return Vec::new();
    }
    let mut grouped: BTreeMap<&[String], Vec<usize>> = BTreeMap::new();
    for (index, request) in requests.iter().enumerate() {
        let active = active_profiles_for_path(package_profiles, &request.importer);
        grouped.entry(active).or_default().push(index);
    }
    if grouped.len() == 1 {
        let active = grouped.keys().next().copied().unwrap_or(&[]);
        return resolve_batch(repository_paths, manifests, requests, active);
    }
    let mut ordered: Vec<Option<language::Resolution>> = vec![None; requests.len()];
    for (active, indices) in grouped {
        let batch_requests: Vec<language::ImportRequest> = indices
            .iter()
            .map(|&index| requests[index].clone())
            .collect();
        let batch_results = resolve_batch(repository_paths, manifests, &batch_requests, active);
        for (index, resolution) in indices.into_iter().zip(batch_results) {
            ordered[index] = Some(resolution);
        }
    }
    ordered
        .into_iter()
        .map(|item| item.unwrap_or_else(|| language::Resolution::Unresolved(String::new())))
        .collect()
}

pub fn validate_manifest_analysis(
    manifests: &[Manifest],
    analysis: ManifestAnalysis,
) -> Result<ManifestAnalysis, LanguagePluginError> {
    let passed_paths: HashSet<&str> = manifests.iter().map(|m| m.path.as_str()).collect();
    for root in &analysis.workspace_packages {
        if RepoPath::new(root.as_str()).is_err() {
            return Err(LanguagePluginError::InvalidWorkspacePackageRoot { path: root.clone() });
        }
    }
    let mut seen_invalid: HashSet<&str> = HashSet::with_capacity(analysis.invalid_manifests.len());
    for invalid in &analysis.invalid_manifests {
        if !passed_paths.contains(invalid.path.as_str()) {
            return Err(LanguagePluginError::UnknownInvalidManifest {
                path: invalid.path.clone(),
                message: invalid.message.clone(),
            });
        }
        if !seen_invalid.insert(invalid.path.as_str()) {
            return Err(LanguagePluginError::DuplicateInvalidManifest {
                path: invalid.path.clone(),
            });
        }
    }
    validate_dependency_owners(&analysis.dependency_manifests)?;
    let _ = validate_declared_dependencies(analysis.declared_dependencies.clone())?;
    validate_package_dependencies(&analysis.manifest_used_dependencies)?;
    validate_inheritance_roots(&analysis.root_dependency_inheritance)?;
    Ok(analysis)
}

fn validate_optional_package_root(package_root: &str) -> Result<(), LanguagePluginError> {
    if !package_root.is_empty() && RepoPath::new(package_root).is_err() {
        return Err(LanguagePluginError::InvalidDependencyPackageRoot {
            package_root: package_root.to_owned(),
        });
    }
    Ok(())
}

fn validate_dependency_owners(
    owners: &[DependencyManifestOwner],
) -> Result<(), LanguagePluginError> {
    for owner in owners {
        validate_optional_package_root(&owner.package_root)?;
        if RepoPath::new(owner.path.as_str()).is_err() {
            return Err(LanguagePluginError::InvalidDependencyManifestPath {
                path: owner.path.clone(),
            });
        }
    }
    Ok(())
}

fn validate_package_dependencies(deps: &[PackageDependency]) -> Result<(), LanguagePluginError> {
    for dep in deps {
        validate_optional_package_root(&dep.package_root)?;
        if dep.name.trim().is_empty() {
            return Err(LanguagePluginError::EmptyDependencyName {
                package_root: dep.package_root.clone(),
            });
        }
    }
    Ok(())
}

fn validate_inheritance_roots(roots: &[String]) -> Result<(), LanguagePluginError> {
    for root in roots {
        if !root.is_empty() && RepoPath::new(root.as_str()).is_err() {
            return Err(LanguagePluginError::InvalidInheritancePackageRoot {
                package_root: root.clone(),
            });
        }
    }
    Ok(())
}

pub fn analyze_manifests_single_pass<E: Into<WorkspaceError>>(
    manifests: Vec<Manifest>,
    discover_packages: impl Fn(&[Manifest]) -> Result<Vec<String>, E>,
    extract_declared: impl Fn(&[Manifest]) -> Vec<DeclaredDep>,
) -> Result<ManifestAnalysis, LanguagePluginError> {
    let mut remaining = manifests.clone();
    let mut invalid_manifests = Vec::new();
    let workspace_packages = loop {
        match discover_packages(&remaining).map_err(Into::into) {
            Ok(packages) => break packages,
            Err(WorkspaceError::Manifest { path, message }) => {
                let Some(pos) = remaining.iter().position(|m| m.path == path) else {
                    return Err(LanguagePluginError::UnknownInvalidManifest { path, message });
                };
                remaining.remove(pos);
                invalid_manifests.push(language::WorkspaceError { path, message });
            }
            Err(error) => {
                return Err(LanguagePluginError::Call {
                    plugin: String::new(),
                    call: "analyze-manifests",
                    reason: error.to_string(),
                });
            }
        }
    };
    let declared_dependencies = extract_declared(&remaining);
    validate_manifest_analysis(
        &manifests,
        ManifestAnalysis {
            workspace_packages,
            invalid_manifests,
            dependency_manifests: Vec::new(),
            declared_dependencies,
            manifest_used_dependencies: Vec::new(),
            root_dependency_inheritance: Vec::new(),
        },
    )
}

pub fn empty_framework_profile_result() -> FrameworkProfileResult {
    FrameworkProfileResult {
        active_profiles: Vec::new(),
        entry_points: Vec::new(),
        generated_paths: Vec::new(),
        reference_ranges: Vec::new(),
        reference_range_note: None,
    }
}

#[macro_export]
macro_rules! empty_v05_guest_methods {
    () => {
        fn known_dialects() -> ::std::vec::Vec<::std::string::String> {
            ::std::vec::Vec::new()
        }

        fn flagged_line_rules() -> ::std::vec::Vec<::std::string::String> {
            ::std::vec::Vec::new()
        }

        fn has_scope_analysis() -> bool {
            false
        }

        fn analyze_scope(
            _package_root: ::std::string::String,
            _dialect: ::core::option::Option<::std::string::String>,
            _artifacts: ::std::vec::Vec<$crate::language::FileScopeArtifact>,
        ) -> $crate::language::ScopeAnalysis {
            $crate::empty_scope_analysis()
        }
    };
}

#[macro_export]
macro_rules! empty_framework_profile_guest_methods {
    () => {
        fn known_framework_profiles() -> ::std::vec::Vec<::std::string::String> {
            ::std::vec::Vec::new()
        }

        fn resolve_framework_profiles(
            _package_root: ::std::string::String,
            _files: ::std::vec::Vec<::std::string::String>,
            _manifests: ::std::vec::Vec<$crate::language::Manifest>,
            _selection: $crate::language::FrameworkProfileSelection,
        ) -> ::std::result::Result<$crate::language::FrameworkProfileResult, ::std::string::String>
        {
            ::std::result::Result::Ok($crate::empty_framework_profile_result())
        }
    };
}

#[macro_export]
macro_rules! empty_framework_profile_plugin_methods {
    () => {};
}

#[macro_export]
macro_rules! impl_builtin_language_plugin {
    (
        $plugin:ty,
        $extensions:expr,
        lower_ext($lower:path, $suffix:literal, $label:literal),
        $($rest:tt)*
    ) => {
        $crate::impl_builtin_language_plugin!(
            @build
            $plugin,
            $extensions,
            { lower_ext($lower, $suffix, $label) },
            $($rest)*
        );
    };
    (
        $plugin:ty,
        $extensions:expr,
        $lower:path,
        $($rest:tt)*
    ) => {
        $crate::impl_builtin_language_plugin!(
            @build
            $plugin,
            $extensions,
            { direct($lower) },
            $($rest)*
        );
    };
    (
        @build
        $plugin:ty,
        $extensions:expr,
        { $($lower_spec:tt)* },
        $ranges:expr,
        flagged_lines($flagged:expr),
        imports($($imports:tt)*),
        frameworks($($frameworks:tt)*),
        hazards($($hazards:tt)*) $(,)?
    ) => {
        impl $crate::LanguagePlugin for $plugin {
            fn extensions(&self) -> ::std::vec::Vec<::std::string::String> {
                $extensions
                    .iter()
                    .map(::std::string::ToString::to_string)
                    .collect()
            }

            $crate::impl_builtin_language_plugin!(@lower $($lower_spec)*);

            fn flagged_line_rules(&self) -> ::std::vec::Vec<::std::string::String> {
                $flagged.map(::std::string::ToString::to_string).collect()
            }

            fn reference_ranges(
                &self,
            ) -> ::std::result::Result<$crate::ReferenceRanges, $crate::LanguagePluginError> {
                $crate::ReferenceRanges::new($ranges)
                    .map_err($crate::LanguagePluginError::InvalidReferenceRange)
            }

            $crate::impl_builtin_language_plugin!(@imports $($imports)*);
            $crate::impl_builtin_language_plugin!(@frameworks $($frameworks)*);
            $crate::impl_builtin_language_plugin!(@hazards $($hazards)*);
        }
    };
    (@lower lower_ext($lower:path, $suffix:literal, $label:literal)) => {
        fn lower(
            &self,
            path: &$crate::RepoPath,
            text: &str,
            _dialect: ::core::option::Option<&str>,
        ) -> ::std::result::Result<$crate::FileModel, $crate::LowerError> {
            let raw = $crate::require_extension(path, $suffix, $label)?;
            $lower(raw, text)
        }
    };
    (@lower direct($lower:path)) => {
        fn lower(
            &self,
            path: &$crate::RepoPath,
            text: &str,
            dialect: ::core::option::Option<&str>,
        ) -> ::std::result::Result<$crate::FileModel, $crate::LowerError> {
            $lower(self, path, text, dialect)
        }
    };
    (@imports none) => {};
    (@imports $manifest_paths:path, $workspace_packages:path, by_profile($resolve_batch:path), $declared_dependencies:path, $entry_points:path $(,)?) => {
        $crate::impl_builtin_language_plugin!(
            @imports
            $manifest_paths,
            $workspace_packages,
            |paths, manifests, requests, package_profiles| {
                $crate::resolve_imports_by_package_profiles(
                    paths,
                    manifests,
                    requests,
                    package_profiles,
                    $resolve_batch,
                )
            },
            $declared_dependencies,
            $entry_points,
        );
    };
    (@imports $manifest_paths:path, $workspace_packages:path, $resolve_imports:expr, $declared_dependencies:path, $entry_points:path $(,)?) => {
        fn has_imports(&self) -> bool {
            true
        }

        fn manifest_paths(
            &self,
            repository_paths: &[$crate::RepoPath],
        ) -> ::std::result::Result<::std::vec::Vec<::std::string::String>, $crate::LanguagePluginError>
        {
            ::std::result::Result::Ok($manifest_paths(repository_paths))
        }

        fn analyze_manifests(
            &self,
            manifests: ::std::vec::Vec<$crate::Manifest>,
        ) -> ::std::result::Result<$crate::ManifestAnalysis, $crate::LanguagePluginError> {
            $crate::analyze_manifests_single_pass(
                manifests,
                $workspace_packages,
                $declared_dependencies,
            )
        }

        fn resolve_imports(
            &self,
            repository_paths: &[$crate::RepoPath],
            manifests: ::std::vec::Vec<$crate::Manifest>,
            requests: ::std::vec::Vec<$crate::language::ImportRequest>,
            package_profiles: &[$crate::PackageFrameworkProfiles],
        ) -> ::std::result::Result<
            ::std::vec::Vec<$crate::language::Resolution>,
            $crate::LanguagePluginError,
        > {
            let resolve: fn(
                &[$crate::RepoPath],
                &[$crate::Manifest],
                &[$crate::language::ImportRequest],
                &[$crate::PackageFrameworkProfiles],
            ) -> ::std::vec::Vec<$crate::language::Resolution> = $resolve_imports;
            ::std::result::Result::Ok(resolve(
                repository_paths,
                &manifests,
                &requests,
                package_profiles,
            ))
        }

        fn entry_points(
            &self,
            repository_paths: &[$crate::RepoPath],
            manifests: ::std::vec::Vec<$crate::Manifest>,
        ) -> ::std::result::Result<::std::vec::Vec<$crate::RepoPath>, $crate::LanguagePluginError>
        {
            $crate::validate_entry_points(repository_paths, $entry_points(repository_paths, &manifests))
        }
    };
    (@frameworks none) => {};
    (@frameworks $module:ident) => {
        fn known_framework_profiles(&self) -> ::std::vec::Vec<::std::string::String> {
            $module::known_profiles()
        }

        fn resolve_framework_profiles(
            &self,
            package_root: &str,
            files: &[$crate::RepoPath],
            manifests: ::std::vec::Vec<$crate::Manifest>,
            selection: $crate::FrameworkProfileSelection,
        ) -> ::std::result::Result<$crate::FrameworkProfileResult, $crate::LanguagePluginError> {
            let result =
                $module::resolve_framework_profiles(package_root, files, &manifests, selection)
                    .map_err(|message| $crate::LanguagePluginError::FrameworkProfile { message })?;
            $crate::validate_framework_profile_result(
                &self.known_framework_profiles(),
                files,
                result,
            )
        }
    };
    (@hazards none) => {};
    (@hazards $rules:expr) => {
        fn has_hazard_rules(&self) -> bool {
            true
        }

        fn hazard_rules(&self) -> ::std::vec::Vec<::std::string::String> {
            $rules.iter().map(|rule| (*rule).to_owned()).collect()
        }
    };
}

pub trait LanguagePlugin: Sync {
    fn extensions(&self) -> Vec<String>;

    fn known_dialects(&self) -> Vec<String> {
        Vec::new()
    }

    fn lower(
        &self,
        path: &RepoPath,
        text: &str,
        dialect: Option<&str>,
    ) -> Result<FileModel, LowerError>;

    fn flagged_line_rules(&self) -> Vec<String> {
        Vec::new()
    }

    fn manifest_paths(
        &self,
        _repository_paths: &[RepoPath],
    ) -> Result<Vec<String>, LanguagePluginError> {
        Ok(Vec::new())
    }

    fn analyze_manifests(
        &self,
        _manifests: Vec<Manifest>,
    ) -> Result<ManifestAnalysis, LanguagePluginError> {
        Ok(empty_manifest_analysis())
    }

    fn resolve_imports(
        &self,
        _repository_paths: &[RepoPath],
        _manifests: Vec<Manifest>,
        _requests: Vec<language::ImportRequest>,
        _package_profiles: &[PackageFrameworkProfiles],
    ) -> Result<Vec<language::Resolution>, LanguagePluginError> {
        Ok(Vec::new())
    }

    fn entry_points(
        &self,
        _repository_paths: &[RepoPath],
        _manifests: Vec<Manifest>,
    ) -> Result<Vec<RepoPath>, LanguagePluginError> {
        Ok(Vec::new())
    }

    fn reference_ranges(&self) -> Result<ReferenceRanges, LanguagePluginError>;

    fn known_framework_profiles(&self) -> Vec<String> {
        Vec::new()
    }

    fn resolve_framework_profiles(
        &self,
        _package_root: &str,
        _files: &[RepoPath],
        _manifests: Vec<Manifest>,
        _selection: FrameworkProfileSelection,
    ) -> Result<FrameworkProfileResult, LanguagePluginError> {
        Ok(empty_framework_profile_result())
    }

    fn has_hazard_rules(&self) -> bool {
        false
    }

    fn hazard_rules(&self) -> Vec<String> {
        Vec::new()
    }

    fn has_imports(&self) -> bool {
        false
    }

    fn has_scope_analysis(&self) -> bool {
        false
    }

    fn analyze_scope(
        &self,
        _package_root: &str,
        _dialect: Option<&str>,
        _artifacts: Vec<FileScopeArtifact>,
    ) -> Result<ScopeAnalysis, LanguagePluginError> {
        Ok(empty_scope_analysis())
    }
}

impl<Plugin: language::Guest + Sync> LanguagePlugin for Plugin {
    fn extensions(&self) -> Vec<String> {
        Plugin::extensions()
    }

    fn known_dialects(&self) -> Vec<String> {
        Plugin::known_dialects()
    }

    fn lower(
        &self,
        path: &RepoPath,
        text: &str,
        dialect: Option<&str>,
    ) -> Result<FileModel, LowerError> {
        Plugin::lower(
            path.as_str().to_owned(),
            text.to_owned(),
            dialect.map(str::to_owned),
        )
        .map_err(LowerError::from)
    }

    fn flagged_line_rules(&self) -> Vec<String> {
        Plugin::flagged_line_rules()
    }

    fn manifest_paths(
        &self,
        repository_paths: &[RepoPath],
    ) -> Result<Vec<String>, LanguagePluginError> {
        Ok(Plugin::manifest_paths(owned_paths(repository_paths)))
    }

    fn analyze_manifests(
        &self,
        manifests: Vec<Manifest>,
    ) -> Result<ManifestAnalysis, LanguagePluginError> {
        let analysis = Plugin::analyze_manifests(manifests.clone());
        validate_manifest_analysis(&manifests, analysis)
    }

    fn resolve_imports(
        &self,
        repository_paths: &[RepoPath],
        manifests: Vec<Manifest>,
        requests: Vec<language::ImportRequest>,
        package_profiles: &[PackageFrameworkProfiles],
    ) -> Result<Vec<language::Resolution>, LanguagePluginError> {
        Ok(Plugin::resolve_imports(
            owned_paths(repository_paths),
            manifests,
            requests,
            package_profiles.to_vec(),
        ))
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

    fn reference_ranges(&self) -> Result<ReferenceRanges, LanguagePluginError> {
        ReferenceRanges::new(Plugin::reference_ranges())
            .map_err(LanguagePluginError::InvalidReferenceRange)
    }

    fn known_framework_profiles(&self) -> Vec<String> {
        Plugin::known_framework_profiles()
    }

    fn resolve_framework_profiles(
        &self,
        package_root: &str,
        files: &[RepoPath],
        manifests: Vec<Manifest>,
        selection: FrameworkProfileSelection,
    ) -> Result<FrameworkProfileResult, LanguagePluginError> {
        let result = Plugin::resolve_framework_profiles(
            package_root.to_owned(),
            owned_paths(files),
            manifests,
            selection,
        )
        .map_err(|message| LanguagePluginError::FrameworkProfile { message })?;
        validate_framework_profile_result(&Plugin::known_framework_profiles(), files, result)
    }

    fn has_hazard_rules(&self) -> bool {
        Plugin::has_hazard_rules()
    }

    fn hazard_rules(&self) -> Vec<String> {
        Plugin::hazard_rules()
    }

    fn has_imports(&self) -> bool {
        Plugin::has_imports()
    }

    fn has_scope_analysis(&self) -> bool {
        Plugin::has_scope_analysis()
    }

    fn analyze_scope(
        &self,
        package_root: &str,
        dialect: Option<&str>,
        artifacts: Vec<FileScopeArtifact>,
    ) -> Result<ScopeAnalysis, LanguagePluginError> {
        let analysis = Plugin::analyze_scope(
            package_root.to_owned(),
            dialect.map(str::to_owned),
            artifacts.clone(),
        );
        let listed: HashSet<String> = Plugin::hazard_rules().into_iter().collect();
        validate_scope_analysis("", &artifacts, &listed, None, analysis)
    }
}

fn owned_paths(repository_paths: &[RepoPath]) -> Vec<String> {
    repository_paths
        .iter()
        .map(|path| path.as_str().to_owned())
        .collect()
}
