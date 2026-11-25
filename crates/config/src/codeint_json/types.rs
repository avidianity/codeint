use std::path::PathBuf;

use codeint_model::{Metric, RepoPath, SourceSet};
use globset::GlobSet;
use serde::Serialize;

use super::schema::{
    Budget, ConfigurationFile, DEFAULT_MIN_SIMILARITY, LanguageGlobs, NewFileBudgetsFile,
    PluginKind, SourceSetBudgets, SourceSetGlobs,
};
use crate::RegressionLimits;
use crate::boundaries::Boundaries;
use crate::config_error::{ConfigFileKind, ConfigurationFileError, KeyPath, KeySegment, glob_set};
use crate::dependencies::DependenciesConfig;
use crate::hazards::HazardsConfig;
use crate::sql::SqlConfig;

pub const DEPENDENCY_METRIC_KEYS: [&str; 3] = [
    Metric::UnusedDependencies.key(),
    Metric::UnlistedDependencies.key(),
    Metric::DevInProductionDependencies.key(),
];

pub const TEST_GAP_METRIC_KEY: &str = Metric::TestGap.key();
pub const HAZARDS_METRIC_KEY: &str = Metric::Hazards.key();

pub(super) const DEFAULT_PLUGIN_MEMORY_MB: u64 = 64;
pub(super) const DEFAULT_PLUGIN_TIMEOUT_MS: u64 = 5_000;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MinSimilarity(f64);

impl MinSimilarity {
    pub const DEFAULT: MinSimilarity = MinSimilarity(DEFAULT_MIN_SIMILARITY);

    pub fn new(value: f64) -> Option<MinSimilarity> {
        (value.is_finite() && value > 0.0 && value <= 1.0).then_some(MinSimilarity(value))
    }

    pub fn ratio(self) -> f64 {
        self.0
    }
}

impl Default for MinSimilarity {
    fn default() -> MinSimilarity {
        MinSimilarity::DEFAULT
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DuplicationConfig {
    pub near_clones: bool,
    pub min_similarity: MinSimilarity,
}

impl Default for DuplicationConfig {
    fn default() -> DuplicationConfig {
        DuplicationConfig {
            near_clones: false,
            min_similarity: MinSimilarity::DEFAULT,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PluginLimits {
    pub memory_mb: u64,
    pub timeout_ms: u64,
}

impl Default for PluginLimits {
    fn default() -> PluginLimits {
        PluginLimits {
            memory_mb: DEFAULT_PLUGIN_MEMORY_MB,
            timeout_ms: DEFAULT_PLUGIN_TIMEOUT_MS,
        }
    }
}

impl PluginLimits {
    pub fn memory_bytes(self) -> usize {
        usize::try_from(self.memory_mb.saturating_mul(1024 * 1024)).unwrap_or(usize::MAX)
    }
}

#[derive(Debug, Default)]
pub struct Configuration {
    pub source_set_overrides: SourceSetOverrides,
    pub budgets: Budgets,
    pub new_file_budgets: Budgets,
    pub languages: Languages,
    pub regression_limits: RegressionLimits,
    pub plugins: Vec<PluginDeclaration>,
    pub plugin_limits: PluginLimits,
    pub boundaries: Option<Boundaries>,
    pub dependencies: DependenciesConfig,
    pub duplication: DuplicationConfig,
    pub entry_points: EntryPoints,
    pub coverage_reports: Vec<String>,
    pub sql: SqlConfig,
    pub hazards: HazardsConfig,
    pub(super) config_file: ConfigFileKind,
    pub(super) effective: ConfigurationFile,
}

impl Configuration {
    pub fn config_file(&self) -> ConfigFileKind {
        self.config_file
    }
}

impl Serialize for Configuration {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.effective.serialize(serializer)
    }
}

#[derive(Debug, Default)]
pub struct EntryPoints {
    globs: Option<GlobSet>,
}

impl EntryPoints {
    pub fn is_empty(&self) -> bool {
        self.globs.is_none()
    }

    pub fn matches(&self, path: &RepoPath) -> bool {
        self.globs
            .as_ref()
            .is_some_and(|globs| globs.is_match(path.as_str()))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sha256Digest([u8; 32]);

impl Sha256Digest {
    pub fn parse_hex(hex: &str) -> Option<Sha256Digest> {
        let bytes = hex.as_bytes();
        if bytes.len() != 64 {
            return None;
        }
        let mut digest = [0u8; 32];
        let (pairs, _) = bytes.as_chunks::<2>();
        for (slot, pair) in digest.iter_mut().zip(pairs) {
            let high = hex_nibble(pair[0])?;
            let low = hex_nibble(pair[1])?;
            *slot = (high << 4) | low;
        }
        Some(Sha256Digest(digest))
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    pub fn to_hex(&self) -> String {
        const HEX: &[u8; 16] = b"0123456789abcdef";
        let mut out = String::with_capacity(64);
        for &byte in &self.0 {
            out.push(HEX[(byte >> 4) as usize] as char);
            out.push(HEX[(byte & 0x0f) as usize] as char);
        }
        out
    }
}

fn hex_nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PluginSource {
    Path(PathBuf),
    HttpsUrl(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginDeclaration {
    pub name: String,
    pub kind: PluginKind,
    pub source: PluginSource,
    pub sha256: Sha256Digest,
}

#[derive(Debug, Default)]
pub struct Budgets {
    pub(super) set: Vec<(SourceSet, String, Budget)>,
    pub(super) dependency_set: Vec<(String, Budget)>,
}

impl Budgets {
    pub fn is_empty(&self) -> bool {
        self.set.is_empty() && self.dependency_set.is_empty()
    }

    pub fn budget(&self, source_set: SourceSet, metric: Metric) -> Option<Budget> {
        self.budget_for_key(source_set, metric.key())
    }

    pub fn budget_for_key(&self, source_set: SourceSet, metric_key: &str) -> Option<Budget> {
        self.set
            .iter()
            .find(|(set, measured, _)| *set == source_set && measured == metric_key)
            .map(|(_, _, budget)| *budget)
    }

    pub fn production_test_gap(&self) -> Option<f64> {
        self.budget_for_key(SourceSet::Production, TEST_GAP_METRIC_KEY)
            .map(Budget::ratio)
    }

    pub fn dependency_budget(&self, metric_key: &str) -> Option<Budget> {
        self.dependency_set
            .iter()
            .find(|(measured, _)| measured == metric_key)
            .map(|(_, budget)| *budget)
    }
}

#[derive(Debug, Default)]
pub struct Languages {
    config_file: ConfigFileKind,
    entries: Option<Vec<LanguageEntry>>,
}

#[derive(Debug)]
struct LanguageEntry {
    plugin_key: KeyPath,
    plugin: String,
    include: GlobSet,
    exclude: GlobSet,
}

#[derive(Debug)]
pub struct LanguageCoverage<'languages> {
    entries: Option<Vec<&'languages LanguageEntry>>,
}

impl Languages {
    pub fn coverage_of(
        &self,
        plugin_names: &[&str],
    ) -> Result<Vec<LanguageCoverage<'_>>, ConfigurationFileError> {
        let Some(entries) = &self.entries else {
            return Ok(plugin_names
                .iter()
                .map(|_| LanguageCoverage { entries: None })
                .collect());
        };
        if let Some(unknown) = entries
            .iter()
            .find(|entry| !plugin_names.contains(&entry.plugin.as_str()))
        {
            return Err(ConfigurationFileError::UnknownLanguagePlugin {
                file: self.config_file,
                key: unknown.plugin_key.clone(),
                name: unknown.plugin.clone(),
                known: plugin_names.iter().map(|name| (*name).to_owned()).collect(),
            });
        }
        Ok(plugin_names
            .iter()
            .map(|name| LanguageCoverage {
                entries: Some(
                    entries
                        .iter()
                        .filter(|entry| entry.plugin == *name)
                        .collect(),
                ),
            })
            .collect())
    }
}

impl LanguageCoverage<'_> {
    pub fn covers_all(&self) -> bool {
        self.entries.is_none()
    }

    pub fn covers(&self, path: &RepoPath) -> bool {
        let Some(entries) = &self.entries else {
            return true;
        };
        let path_and_folders_above: Vec<&str> = path
            .as_str()
            .match_indices('/')
            .map(|(end, _)| &path.as_str()[..end])
            .chain([path.as_str()])
            .collect();
        let matches_any = |globs: &GlobSet| {
            path_and_folders_above
                .iter()
                .any(|path| globs.is_match(path))
        };
        entries
            .iter()
            .any(|entry| matches_any(&entry.include) && !matches_any(&entry.exclude))
    }
}

#[derive(Debug, Default)]
pub struct SourceSetOverrides {
    by_precedence: Vec<(SourceSet, GlobSet)>,
}

impl SourceSetOverrides {
    pub fn source_set(&self, path: &RepoPath, settled_by_rules: SourceSet) -> SourceSet {
        self.by_precedence
            .iter()
            .find(|(_, globs)| globs.is_match(path.as_str()))
            .map_or(settled_by_rules, |(source_set, _)| *source_set)
    }
}

pub(super) fn entry_points(globs: &[String]) -> Result<EntryPoints, ConfigurationFileError> {
    if globs.is_empty() {
        return Ok(EntryPoints::default());
    }
    let key = KeyPath::default().child(KeySegment::Key("entry_points".to_owned()));
    Ok(EntryPoints {
        globs: Some(glob_set(&key, globs)?),
    })
}

pub(super) fn budgets(file: SourceSetBudgets) -> Budgets {
    let entries = file.into_entries();
    Budgets {
        set: entries.set,
        dependency_set: entries.dependency_set,
    }
}

pub(super) fn new_file_budgets(file: NewFileBudgetsFile) -> Budgets {
    Budgets {
        set: file.into_entries(),
        dependency_set: Vec::new(),
    }
}

pub(super) fn languages(
    config_file: ConfigFileKind,
    entries: Option<Vec<LanguageGlobs>>,
) -> Result<Languages, ConfigurationFileError> {
    let Some(entries) = entries else {
        return Ok(Languages {
            config_file,
            entries: None,
        });
    };
    let languages_key = KeyPath::default().child(KeySegment::Key("languages".to_owned()));
    let entries = entries
        .into_iter()
        .enumerate()
        .map(|(index, entry)| {
            let entry_key = languages_key.child(KeySegment::Index(index));
            let (plugin, include, exclude) = entry.into_parts();
            Ok(LanguageEntry {
                plugin_key: entry_key.child(KeySegment::Key("plugin".to_owned())),
                plugin,
                include: glob_set(
                    &entry_key.child(KeySegment::Key("include".to_owned())),
                    &include,
                )?,
                exclude: glob_set(
                    &entry_key.child(KeySegment::Key("exclude".to_owned())),
                    &exclude,
                )?,
            })
        })
        .collect::<Result<_, ConfigurationFileError>>()?;
    Ok(Languages {
        config_file,
        entries: Some(entries),
    })
}

pub(super) fn source_set_overrides(
    globs: SourceSetGlobs,
) -> Result<SourceSetOverrides, ConfigurationFileError> {
    let by_precedence = globs
        .into_by_precedence()
        .into_iter()
        .filter(|(_, _, globs)| !globs.is_empty())
        .map(|(source_set, key, globs)| {
            let key = KeyPath::default()
                .child(KeySegment::Key("source_sets".to_owned()))
                .child(KeySegment::Key(key.to_owned()));
            Ok((source_set, glob_set(&key, &globs)?))
        })
        .collect::<Result<_, ConfigurationFileError>>()?;
    Ok(SourceSetOverrides { by_precedence })
}
