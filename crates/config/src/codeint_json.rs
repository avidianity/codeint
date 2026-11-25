use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use codeint_model::{Metric, MetricScope, RepoPath, SourceSet};

use crate::RegressionLimits;
use crate::boundaries::{Boundaries, BoundariesFile};
use crate::config_error::{
    CONFIGURATION_FILE_NAME, CONFIGURATION_JSONC_FILE_NAME, ConfigFileKind, ConfigurationFileError,
    JsonType, JsoncError, KeyPath, KeySegment, compile_glob, glob_set,
};
use crate::dependencies::{DependenciesConfig, DependenciesFile};
use crate::hazards::{HazardsConfig, HazardsConfigFile};
use crate::regression_limits::{RegressionLimitSetting, ResolvedRegressionLimit};
use crate::resolved::{
    BudgetOverride, ConfigOverrides, ConfigProvenance, ResolvedConfiguration, ResolvedSetting,
    SourceSetOverride, resolve_with_provenance,
};
use crate::sql::{SqlConfig, SqlConfigFile};
use globset::GlobSet;
use jsonc_parser::errors::ParseError;
use jsonc_parser::{
    CollectOptions, ParseOptions, parse_to_ast, parse_to_serde_value, parse_to_value,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

pub const DEPENDENCY_METRIC_KEYS: [&str; 3] = [
    Metric::UnusedDependencies.key(),
    Metric::UnlistedDependencies.key(),
    Metric::DevInProductionDependencies.key(),
];

pub const TEST_GAP_METRIC_KEY: &str = Metric::TestGap.key();
pub const HAZARDS_METRIC_KEY: &str = Metric::Hazards.key();

mod shape;
mod starter;
#[cfg(test)]
mod tests;

pub use starter::{
    DetectedLanguage, StarterConfigurationError, published_schema_address,
    write_starter_configuration,
};

const DEFAULT_PLUGIN_MEMORY_MB: u64 = 64;
const DEFAULT_PLUGIN_TIMEOUT_MS: u64 = 5_000;
const DEFAULT_MIN_SIMILARITY: f64 = 0.85;

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
    config_file: ConfigFileKind,
    effective: ConfigurationFile,
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

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PluginKind {
    #[default]
    Language,
    Metric,
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

impl Configuration {
    pub fn config_file(&self) -> ConfigFileKind {
        self.config_file
    }

    pub fn validate_cli_budget_metrics(
        &self,
        overrides: &[BudgetOverride],
        external_metric_keys: &[&str],
    ) -> Result<(), ConfigurationFileError> {
        for entry in overrides {
            if let Some(metric) = Metric::from_key(&entry.metric) {
                if is_production_only_outside_production(metric, entry.source_set) {
                    return Err(cli_production_only_budget_error(entry.source_set, metric));
                }
                if metric.scope().applies_to_source_set(entry.source_set) {
                    continue;
                }
            } else if external_metric_keys.contains(&entry.metric.as_str()) {
                continue;
            }
            return Err(ConfigurationFileError::UnknownCliBudgetMetric {
                source_set: entry.source_set,
                metric: entry.metric.clone(),
                allowed: allowed_budget_metric_keys(entry.source_set, external_metric_keys),
            });
        }
        Ok(())
    }

    pub fn validate_metrics(
        &self,
        external_metric_keys: &[&str],
    ) -> Result<(), ConfigurationFileError> {
        for (source_set, metric_key, _) in &self.budgets.set {
            if let Some(metric) = Metric::from_key(metric_key) {
                if is_production_only_outside_production(metric, *source_set) {
                    return Err(production_only_budget_error(
                        self.config_file,
                        *source_set,
                        metric,
                    ));
                }
                if metric.scope().applies_to_source_set(*source_set) {
                    continue;
                }
            } else if external_metric_keys.contains(&metric_key.as_str()) {
                continue;
            }
            return Err(ConfigurationFileError::UnknownKey {
                file: self.config_file,
                key: budget_key_path(*source_set, metric_key),
                allowed: allowed_budget_metric_keys(*source_set, external_metric_keys),
            });
        }
        validate_regression_limit_keys(
            self.config_file,
            &self.regression_limits,
            external_metric_keys,
        )
    }
}

fn is_production_only_outside_production(metric: Metric, source_set: SourceSet) -> bool {
    metric.scope() == MetricScope::ProductionOnly
        && !metric.scope().applies_to_source_set(source_set)
}

fn budget_key_path(source_set: SourceSet, metric_key: &str) -> KeyPath {
    KeyPath::default()
        .child(KeySegment::Key("budgets".to_owned()))
        .child(KeySegment::Key(source_set.key().to_owned()))
        .child(KeySegment::Key(metric_key.to_owned()))
}

fn regression_limit_key_path(source_set: Option<SourceSet>, metric_key: &str) -> KeyPath {
    let mut path = KeyPath::default().child(KeySegment::Key("regression_limits".to_owned()));
    if let Some(source_set) = source_set {
        path = path.child(KeySegment::Key(source_set.key().to_owned()));
    }
    path.child(KeySegment::Key(metric_key.to_owned()))
}

fn production_only_budget_error(
    file: ConfigFileKind,
    source_set: SourceSet,
    metric: Metric,
) -> ConfigurationFileError {
    let key = budget_key_path(source_set, metric.key());
    match metric {
        Metric::TestGap => ConfigurationFileError::TestGapBudgetOutsideProduction { file, key },
        other => ConfigurationFileError::MetricBudgetOutsideProduction {
            file,
            key,
            metric: other,
        },
    }
}

fn cli_production_only_budget_error(
    source_set: SourceSet,
    metric: Metric,
) -> ConfigurationFileError {
    match metric {
        Metric::TestGap => ConfigurationFileError::CliTestGapBudgetOutsideProduction { source_set },
        other => ConfigurationFileError::CliMetricBudgetOutsideProduction {
            source_set,
            metric: other,
        },
    }
}

fn validate_regression_limit_keys(
    config_file: ConfigFileKind,
    limits: &RegressionLimits,
    external_metric_keys: &[&str],
) -> Result<(), ConfigurationFileError> {
    for (source_set, metric_key) in limits.metric_keys() {
        validate_regression_limit_key(config_file, source_set, metric_key, external_metric_keys)?;
    }
    Ok(())
}

fn validate_regression_limit_key(
    config_file: ConfigFileKind,
    source_set: Option<SourceSet>,
    metric_key: &str,
    external_metric_keys: &[&str],
) -> Result<(), ConfigurationFileError> {
    if let Some(metric) = Metric::from_key(metric_key) {
        if !metric.supports_diff_regression() {
            return Err(ConfigurationFileError::TestGapRegressionLimit {
                file: config_file,
                key: regression_limit_key_path(source_set, metric_key),
            });
        }
        let Some(set) = source_set else {
            return Ok(());
        };
        if is_production_only_outside_production(metric, set) {
            return Err(
                ConfigurationFileError::MetricRegressionLimitOutsideProduction {
                    file: config_file,
                    key: regression_limit_key_path(Some(set), metric_key),
                    metric,
                },
            );
        }
        if metric.scope().applies_to_source_set(set) {
            return Ok(());
        }
    } else if external_metric_keys.contains(&metric_key) {
        return Ok(());
    }
    Err(ConfigurationFileError::UnknownKey {
        file: config_file,
        key: regression_limit_key_path(source_set, metric_key),
        allowed: allowed_regression_limit_metric_keys(source_set, external_metric_keys),
    })
}

fn is_repository_wide_diff_metric(metric: Metric) -> bool {
    metric.scope() == MetricScope::RepositoryWide && metric.supports_diff_regression()
}

fn ordered_built_in_metrics(predicate: impl Fn(Metric) -> bool) -> Vec<Metric> {
    let mut built_in: Vec<Metric> = Metric::ALL
        .into_iter()
        .filter(|metric| predicate(*metric))
        .collect();
    move_metric_after(
        &mut built_in,
        Metric::CognitiveComplexity,
        Metric::ImportCycles,
    );
    move_metric_after(&mut built_in, Metric::TestGap, Metric::Hazards);
    built_in
}

fn move_metric_after(metrics: &mut Vec<Metric>, target: Metric, anchor: Metric) {
    if let (Some(target_pos), Some(anchor_pos)) = (
        metrics.iter().position(|metric| *metric == target),
        metrics.iter().position(|metric| *metric == anchor),
    ) && target_pos < anchor_pos
    {
        let metric = metrics.remove(target_pos);
        metrics.insert(anchor_pos, metric);
    }
}

fn allowed_budget_metric_keys(source_set: SourceSet, external_metric_keys: &[&str]) -> Vec<String> {
    ordered_built_in_metrics(|metric| metric.scope().applies_to_source_set(source_set))
        .into_iter()
        .map(|metric| metric.key().to_owned())
        .chain(external_metric_keys.iter().map(|key| (*key).to_owned()))
        .collect()
}

fn allowed_regression_limit_metric_keys(
    source_set: Option<SourceSet>,
    external_metric_keys: &[&str],
) -> Vec<String> {
    let built_in = match source_set {
        Some(source_set) => ordered_built_in_metrics(|metric| {
            metric.scope().applies_to_source_set(source_set) && metric.supports_diff_regression()
        }),
        None => ordered_built_in_metrics(|metric| {
            metric.scope() != MetricScope::RepositoryWide && metric.supports_diff_regression()
        }),
    };
    let mut allowed: Vec<String> = built_in
        .into_iter()
        .map(|metric| metric.key().to_owned())
        .chain(external_metric_keys.iter().map(|key| (*key).to_owned()))
        .collect();
    if source_set.is_none() {
        allowed.extend(
            Metric::ALL
                .into_iter()
                .filter(|metric| is_repository_wide_diff_metric(*metric))
                .map(|metric| metric.key().to_owned()),
        );
        allowed.extend(SourceSet::ALL.map(|set| set.key().to_owned()));
    }
    allowed
}

impl Serialize for Configuration {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.effective.serialize(serializer)
    }
}

#[derive(Debug, Default)]
pub struct Budgets {
    set: Vec<(SourceSet, String, Budget)>,
    dependency_set: Vec<(String, Budget)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, JsonSchema)]
#[serde(transparent)]
pub struct Budget(#[schemars(range(min = 0, max = 1))] f64);

impl<'de> Deserialize<'de> for Budget {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Budget, D::Error> {
        let value = f64::deserialize(deserializer)?;
        Ok(Budget(normalize_zero(value)))
    }
}

fn normalize_zero(value: f64) -> f64 {
    if value == 0.0 { 0.0 } else { value }
}

impl Budgets {
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

impl Budget {
    pub fn new(ratio: f64) -> Option<Budget> {
        (ratio.is_finite() && (0.0..=1.0).contains(&ratio)).then_some(Budget(normalize_zero(ratio)))
    }

    pub fn ratio(self) -> f64 {
        normalize_zero(self.0)
    }

    pub fn admits(self, ratio: f64) -> bool {
        ratio <= self.ratio()
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

#[derive(Debug, Clone, Default, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(
    title = "codeint.json",
    description = "The configuration of codeint, read from codeint.json at the root of the repository."
)]
struct ConfigurationFile {
    #[serde(rename = "$schema", default)]
    #[schemars(
        description = "The address of this JSON schema, for editors; codeint does not read it."
    )]
    _schema: Option<String>,
    #[serde(default)]
    #[schemars(
        description = "Globs of repository-relative paths that put files into a Source set, whatever the Source set rules say. When globs of several Source sets match one file, the first of vendored, generated, test, and production wins."
    )]
    source_sets: SourceSetGlobs,
    #[serde(default)]
    #[schemars(
        description = "The highest ratio each Metric may reach in each Source set; a Metric over its Budget fails the run with exit code 1. A ratio equal to its Budget passes. Codeint compares the ratio as the report shows it, rounded to 4 decimal places."
    )]
    budgets: SourceSetBudgets,
    #[serde(default)]
    #[schemars(
        description = "How much each Metric may get worse from the merge-base to the head side before codeint diff reports a Regression and exits with code 1. A key named after a Metric sets its limit in every Source set; a key named after a Source set sets limits for that Source set only. For each setting, the limit of the Source set wins over the limit of the Metric, and a setting left out keeps the default: a ratio rise of 0.01, and new Findings fail only with a numerator rise, except for Import cycles, where any new Finding fails."
    )]
    regression_limits: RegressionLimitsFile,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(
        description = "Which Language plugin measures which folders. Each entry names a Language plugin, the globs of the folders or files it measures, and the globs it leaves out. Several entries may name one Language plugin; it measures what any of them includes. Without this key, every Language plugin measures the whole repository; with it, a Language plugin measures only what its entries include."
    )]
    languages: Option<Vec<LanguageGlobs>>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[schemars(
        description = "External WebAssembly component Language and Metric plugins to load from local paths or HTTPS URLs. Each entry names the plugin, its optional kind (language by default, or metric), either its local path or its HTTPS url, and its required SHA-256 hex digest."
    )]
    plugins: Vec<PluginEntry>,
    #[serde(default, skip_serializing_if = "PluginLimitsFile::is_unset")]
    #[schemars(
        description = "Memory cap and time limit enforced on each WebAssembly Plugin. Defaults are 64 MiB and 5000 ms."
    )]
    plugin_limits: PluginLimitsFile,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(
        with = "BoundariesFile",
        description = "Architectural import boundaries defining named zones, optional public_files entry points, directional allow or deny rules, and optional strict_zones mode."
    )]
    boundaries: Option<BoundariesFile>,
    #[serde(default, skip_serializing_if = "DependenciesFile::is_unset")]
    #[schemars(
        description = "Dependency hygiene settings, including the allowlist of external package names excluded from unused, unlisted, and dev-in-production Findings."
    )]
    dependencies: DependenciesFile,
    #[serde(default, skip_serializing_if = "DuplicationConfigFile::is_unset")]
    #[schemars(
        description = "Settings for Duplication detection, including opt-in shingle-based near-clone detection. Exact Clone groups remain the default when omitted."
    )]
    duplication: DuplicationConfigFile,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[schemars(
        description = "Globs of repository-relative paths treated as additional Production entry points for Orphan files detection, combined with manifest-discovered entry points."
    )]
    entry_points: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[schemars(
        description = "Paths relative to the repository root of LCOV or Cobertura XML Coverage reports used to measure Test gap."
    )]
    coverage_reports: Vec<String>,
    #[serde(default, skip_serializing_if = "SqlConfigFile::is_unset")]
    #[schemars(description = "SQL settings, including Dialect mappings for folders or files.")]
    sql: SqlConfigFile,
    #[serde(default, skip_serializing_if = "crate::hazards::is_hazards_unset")]
    #[schemars(
        description = "Hazards Metric settings, including Hazard line rules turned off across the repository."
    )]
    hazards: HazardsConfigFile,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct DuplicationConfigFile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(
        with = "bool",
        description = "When true, detect near-duplicate code blocks using shingle similarity in addition to exact Clone groups; false by default."
    )]
    near_clones: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(
        with = "f64",
        range(max = 1),
        extend("exclusiveMinimum" = 0),
        description = "Minimum Jaccard shingle similarity in (0, 1] required for two blocks to match as a near-clone; 0.85 by default."
    )]
    min_similarity: Option<f64>,
}

impl DuplicationConfigFile {
    fn is_unset(&self) -> bool {
        self.near_clones.is_none() && self.min_similarity.is_none()
    }

    fn normalized(self) -> DuplicationConfigFile {
        DuplicationConfigFile {
            near_clones: self.near_clones.filter(|&enabled| enabled),
            min_similarity: self
                .min_similarity
                .filter(|&ratio| ratio != DEFAULT_MIN_SIMILARITY),
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct PluginLimitsFile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(
        with = "u64",
        range(min = 1, max = 4096),
        description = "Maximum linear memory in MiB that a WebAssembly Plugin may allocate; 64 by default."
    )]
    memory_mb: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(
        with = "u64",
        range(min = 1, max = 300000),
        description = "Maximum wall-clock time in milliseconds for each WebAssembly Plugin call; 5000 by default."
    )]
    timeout_ms: Option<u64>,
}

impl PluginLimitsFile {
    fn is_unset(&self) -> bool {
        self.memory_mb.is_none() && self.timeout_ms.is_none()
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct PluginEntry {
    #[schemars(description = "The name of the external Plugin.")]
    name: String,
    #[serde(default)]
    #[schemars(
        description = "Whether the external Plugin is a Language plugin or a Metric plugin; language by default."
    )]
    kind: PluginKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(
        description = "Path to the WebAssembly component file, relative to the repository root or absolute. Set either path or url, not both."
    )]
    path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(
        description = "HTTPS URL of the WebAssembly component file. Set either path or url, not both."
    )]
    url: Option<String>,
    #[schemars(
        description = "The required 64-character hexadecimal SHA-256 digest of the plugin file."
    )]
    sha256: String,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, JsonSchema)]
struct RegressionLimitsFile {
    #[serde(default, skip_serializing_if = "RegressionLimitSetting::is_unset")]
    #[schemars(description = "The Regression limit of Erosion in every Source set.")]
    erosion: RegressionLimitSetting,
    #[serde(default, skip_serializing_if = "RegressionLimitSetting::is_unset")]
    #[schemars(description = "The Regression limit of Cognitive complexity in every Source set.")]
    cognitive_complexity: RegressionLimitSetting,
    #[serde(default, skip_serializing_if = "RegressionLimitSetting::is_unset")]
    #[schemars(description = "The Regression limit of Duplication in every Source set.")]
    duplication: RegressionLimitSetting,
    #[serde(default, skip_serializing_if = "RegressionLimitSetting::is_unset")]
    #[schemars(description = "The Regression limit of Verbosity in every Source set.")]
    verbosity: RegressionLimitSetting,
    #[serde(default, skip_serializing_if = "RegressionLimitSetting::is_unset")]
    #[schemars(description = "The Regression limit of Import cycles in every Source set.")]
    import_cycles: RegressionLimitSetting,
    #[serde(default, skip_serializing_if = "RegressionLimitSetting::is_unset")]
    #[schemars(description = "The Regression limit of Boundaries in every Source set.")]
    boundaries: RegressionLimitSetting,
    #[serde(default, skip_serializing_if = "RegressionLimitSetting::is_unset")]
    #[schemars(description = "The Regression limit of Orphan files in the Production Source set.")]
    orphan_files: RegressionLimitSetting,
    #[serde(default, skip_serializing_if = "RegressionLimitSetting::is_unset")]
    #[schemars(description = "The Regression limit of Hazards in every Source set.")]
    hazards: RegressionLimitSetting,
    #[serde(default, skip_serializing_if = "RegressionLimitSetting::is_unset")]
    #[schemars(description = "The Regression limit of unused declared dependencies.")]
    unused_dependencies: RegressionLimitSetting,
    #[serde(default, skip_serializing_if = "RegressionLimitSetting::is_unset")]
    #[schemars(description = "The Regression limit of unlisted external dependencies.")]
    unlisted_dependencies: RegressionLimitSetting,
    #[serde(default, skip_serializing_if = "RegressionLimitSetting::is_unset")]
    #[schemars(description = "The Regression limit of dev dependencies imported in production.")]
    dev_in_production_dependencies: RegressionLimitSetting,
    #[serde(
        default,
        skip_serializing_if = "ProductionMetricRegressionLimits::is_unset"
    )]
    #[schemars(description = "The Regression limits of the Production Source set.")]
    production: ProductionMetricRegressionLimits,
    #[serde(default, skip_serializing_if = "MetricRegressionLimits::is_unset")]
    #[schemars(description = "The Regression limits of the Test Source set.")]
    test: MetricRegressionLimits,
    #[serde(default, skip_serializing_if = "MetricRegressionLimits::is_unset")]
    #[schemars(description = "The Regression limits of the Generated Source set.")]
    generated: MetricRegressionLimits,
    #[serde(default, skip_serializing_if = "MetricRegressionLimits::is_unset")]
    #[schemars(description = "The Regression limits of the Vendored Source set.")]
    vendored: MetricRegressionLimits,
    #[serde(flatten, default, skip_serializing_if = "BTreeMap::is_empty")]
    external: BTreeMap<String, RegressionLimitSetting>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, JsonSchema)]
struct ProductionMetricRegressionLimits {
    #[serde(flatten)]
    shared: MetricRegressionLimits,
    #[serde(default, skip_serializing_if = "RegressionLimitSetting::is_unset")]
    #[schemars(
        description = "The Regression limit of Orphan files. Applies to the Production Source set only."
    )]
    orphan_files: RegressionLimitSetting,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, JsonSchema)]
struct MetricRegressionLimits {
    #[serde(default, skip_serializing_if = "RegressionLimitSetting::is_unset")]
    #[schemars(description = "The Regression limit of Erosion in this Source set.")]
    erosion: RegressionLimitSetting,
    #[serde(default, skip_serializing_if = "RegressionLimitSetting::is_unset")]
    #[schemars(description = "The Regression limit of Cognitive complexity in this Source set.")]
    cognitive_complexity: RegressionLimitSetting,
    #[serde(default, skip_serializing_if = "RegressionLimitSetting::is_unset")]
    #[schemars(description = "The Regression limit of Duplication in this Source set.")]
    duplication: RegressionLimitSetting,
    #[serde(default, skip_serializing_if = "RegressionLimitSetting::is_unset")]
    #[schemars(description = "The Regression limit of Verbosity in this Source set.")]
    verbosity: RegressionLimitSetting,
    #[serde(default, skip_serializing_if = "RegressionLimitSetting::is_unset")]
    #[schemars(description = "The Regression limit of Import cycles in this Source set.")]
    import_cycles: RegressionLimitSetting,
    #[serde(default, skip_serializing_if = "RegressionLimitSetting::is_unset")]
    #[schemars(description = "The Regression limit of Boundaries in this Source set.")]
    boundaries: RegressionLimitSetting,
    #[serde(default, skip_serializing_if = "RegressionLimitSetting::is_unset")]
    #[schemars(description = "The Regression limit of Hazards in this Source set.")]
    hazards: RegressionLimitSetting,
    #[serde(flatten, default, skip_serializing_if = "BTreeMap::is_empty")]
    external: BTreeMap<String, RegressionLimitSetting>,
}

impl ProductionMetricRegressionLimits {
    fn is_unset(&self) -> bool {
        self.shared.is_unset() && self.orphan_files.is_unset()
    }

    fn by_metric(self) -> impl Iterator<Item = (String, RegressionLimitSetting)> {
        let ProductionMetricRegressionLimits {
            shared,
            orphan_files,
        } = self;
        shared.by_metric().chain(
            (!orphan_files.is_unset())
                .then(|| (Metric::OrphanFiles.key().to_owned(), orphan_files)),
        )
    }
}

impl MetricRegressionLimits {
    fn is_unset(&self) -> bool {
        [
            &self.erosion,
            &self.cognitive_complexity,
            &self.duplication,
            &self.verbosity,
            &self.import_cycles,
            &self.boundaries,
            &self.hazards,
        ]
        .into_iter()
        .chain(self.external.values())
        .all(RegressionLimitSetting::is_unset)
    }

    fn by_metric(self) -> impl Iterator<Item = (String, RegressionLimitSetting)> {
        let MetricRegressionLimits {
            erosion,
            cognitive_complexity,
            duplication,
            verbosity,
            import_cycles,
            boundaries,
            hazards,
            external,
        } = self;
        all_source_set_metric_pairs([
            erosion,
            cognitive_complexity,
            duplication,
            verbosity,
            import_cycles,
            boundaries,
            hazards,
        ])
        .chain(external)
        .filter(|(_, setting)| !setting.is_unset())
    }
}

fn all_source_set_metric_pairs<T>(values: [T; 7]) -> impl Iterator<Item = (String, T)> {
    [
        Metric::Erosion.key(),
        Metric::CognitiveComplexity.key(),
        Metric::Duplication.key(),
        Metric::Verbosity.key(),
        Metric::ImportCycles.key(),
        Metric::Boundaries.key(),
        Metric::Hazards.key(),
    ]
    .into_iter()
    .zip(values)
    .map(|(key, value)| (key.to_owned(), value))
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct SourceSetBudgets {
    #[serde(default)]
    #[schemars(description = "The Budgets of the Production Source set.")]
    production: ProductionMetricBudgets,
    #[serde(default)]
    #[schemars(description = "The Budgets of the Test Source set.")]
    test: MetricBudgets,
    #[serde(default)]
    #[schemars(description = "The Budgets of the Generated Source set.")]
    generated: MetricBudgets,
    #[serde(default)]
    #[schemars(description = "The Budgets of the Vendored Source set.")]
    vendored: MetricBudgets,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(
        with = "f64",
        range(min = 0, max = 1),
        description = "The highest share of declared dependencies that may be unused, from 0 to 1."
    )]
    unused_dependencies: Option<Budget>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(
        with = "f64",
        range(min = 0, max = 1),
        description = "The highest share of imported external packages that may be unlisted, from 0 to 1."
    )]
    unlisted_dependencies: Option<Budget>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(
        with = "f64",
        range(min = 0, max = 1),
        description = "The highest share of production-imported external packages that may be dev-only, from 0 to 1."
    )]
    dev_in_production_dependencies: Option<Budget>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, JsonSchema)]
struct ProductionMetricBudgets {
    #[serde(flatten)]
    common: SharedMetricBudgets,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(
        with = "f64",
        range(min = 0, max = 1),
        description = "The highest Orphan files ratio, from 0 to 1. Applies to the Production Source set only."
    )]
    orphan_files: Option<Budget>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(
        with = "f64",
        range(min = 0, max = 1),
        description = "The highest Test gap ratio, from 0 to 1. Applies to the Production Source set only."
    )]
    test_gap: Option<Budget>,
    #[serde(flatten, default, skip_serializing_if = "BTreeMap::is_empty")]
    external: BTreeMap<String, Budget>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, JsonSchema)]
struct MetricBudgets {
    #[serde(flatten)]
    common: SharedMetricBudgets,
    #[serde(flatten, default, skip_serializing_if = "BTreeMap::is_empty")]
    external: BTreeMap<String, Budget>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, JsonSchema)]
struct SharedMetricBudgets {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(
        with = "f64",
        range(min = 0, max = 1),
        description = "The highest Erosion ratio, from 0 to 1."
    )]
    erosion: Option<Budget>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(
        with = "f64",
        range(min = 0, max = 1),
        description = "The highest Cognitive complexity ratio, from 0 to 1."
    )]
    cognitive_complexity: Option<Budget>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(
        with = "f64",
        range(min = 0, max = 1),
        description = "The highest Duplication ratio, from 0 to 1."
    )]
    duplication: Option<Budget>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(
        with = "f64",
        range(min = 0, max = 1),
        description = "The highest Verbosity ratio, from 0 to 1."
    )]
    verbosity: Option<Budget>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(
        with = "f64",
        range(min = 0, max = 1),
        description = "The highest Import cycles ratio, from 0 to 1."
    )]
    import_cycles: Option<Budget>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(
        with = "f64",
        range(min = 0, max = 1),
        description = "The highest Boundaries ratio, from 0 to 1."
    )]
    boundaries: Option<Budget>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(
        with = "f64",
        range(min = 0, max = 1),
        description = "The highest Hazards ratio, from 0 to 1."
    )]
    hazards: Option<Budget>,
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct LanguageGlobs {
    #[schemars(description = "The name of the Language plugin, such as typescript-javascript.")]
    plugin: String,
    #[schemars(
        description = "Globs of repository-relative folders or files that the Language plugin measures; a folder covers every file below it."
    )]
    include: Vec<String>,
    #[serde(default)]
    #[schemars(
        description = "Globs of repository-relative folders or files inside the included ones that the Language plugin leaves out."
    )]
    exclude: Vec<String>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct SourceSetGlobs {
    #[serde(default)]
    #[schemars(description = "Globs of files that belong to the Production Source set.")]
    production: Vec<String>,
    #[serde(default)]
    #[schemars(description = "Globs of files that belong to the Test Source set.")]
    test: Vec<String>,
    #[serde(default)]
    #[schemars(description = "Globs of files that belong to the Generated Source set.")]
    generated: Vec<String>,
    #[serde(default)]
    #[schemars(description = "Globs of files that belong to the Vendored Source set.")]
    vendored: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SchemaVersionStatus {
    Missing,
    Compatible {
        url: String,
        version: Option<String>,
    },
    NewerThanBinary {
        url: String,
        config_version: String,
        binary_version: String,
    },
}

#[derive(Debug)]
pub enum ConfigurationInspection {
    Missing {
        path: PathBuf,
    },
    Valid {
        path: PathBuf,
        schema: SchemaVersionStatus,
        configuration: Box<Configuration>,
    },
    Invalid {
        path: PathBuf,
        schema: Option<SchemaVersionStatus>,
        error: ConfigurationFileError,
    },
}

pub fn read_configuration(repository_root: &Path) -> Result<Configuration, ConfigurationFileError> {
    read_resolved_configuration(repository_root, &ConfigOverrides::default())
        .map(|resolved| resolved.configuration)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredConfigFile {
    pub kind: ConfigFileKind,
    pub path: PathBuf,
}

fn path_exists_including_symlinks(
    path: &Path,
    file: ConfigFileKind,
) -> Result<bool, ConfigurationFileError> {
    match std::fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(source) => Err(ConfigurationFileError::Unreadable { file, source }),
    }
}

pub fn discover_config_file(
    repository_root: &Path,
) -> Result<Option<DiscoveredConfigFile>, ConfigurationFileError> {
    let json_path = repository_root.join(CONFIGURATION_FILE_NAME);
    let jsonc_path = repository_root.join(CONFIGURATION_JSONC_FILE_NAME);
    let has_json = path_exists_including_symlinks(&json_path, ConfigFileKind::Json)?;
    let has_jsonc = path_exists_including_symlinks(&jsonc_path, ConfigFileKind::Jsonc)?;
    match (has_json, has_jsonc) {
        (true, true) => Err(ConfigurationFileError::AmbiguousConfigFiles),
        (true, false) => Ok(Some(DiscoveredConfigFile {
            kind: ConfigFileKind::Json,
            path: json_path,
        })),
        (false, true) => Ok(Some(DiscoveredConfigFile {
            kind: ConfigFileKind::Jsonc,
            path: jsonc_path,
        })),
        (false, false) => Ok(None),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchemaValueSpan {
    pub span: std::ops::Range<usize>,
    pub value: String,
}

pub fn schema_value_span(text: &str) -> Option<SchemaValueSpan> {
    let without_bom = text.strip_prefix('\u{FEFF}').unwrap_or(text);
    let bom_len = text.len() - without_bom.len();
    let parsed = parse_to_ast(
        without_bom,
        &CollectOptions::default(),
        &JSONC_PARSE_OPTIONS,
    )
    .ok()?;
    let value = parsed.value?;
    let object = value.as_object()?;
    let property = object
        .properties
        .iter()
        .rev()
        .find(|property| property.name.as_str() == "$schema")?;
    let jsonc_parser::ast::Value::StringLit(schema) = &property.value else {
        return None;
    };
    Some(SchemaValueSpan {
        span: (schema.range.start + bom_len)..(schema.range.end + bom_len),
        value: schema.value.to_string(),
    })
}

fn read_config_file(discovered: &DiscoveredConfigFile) -> Result<String, ConfigurationFileError> {
    let file = discovered.kind;
    match std::fs::read_to_string(&discovered.path) {
        Ok(text) => Ok(text),
        Err(error)
            if error.kind() == std::io::ErrorKind::NotFound
                && is_broken_symlink(&discovered.path) =>
        {
            Err(ConfigurationFileError::BrokenSymlink { file })
        }
        Err(source) => Err(ConfigurationFileError::Unreadable { file, source }),
    }
}

const JSONC_PARSE_OPTIONS: ParseOptions = ParseOptions {
    allow_comments: true,
    allow_trailing_commas: true,
    allow_loose_object_property_names: false,
    allow_missing_commas: false,
    allow_single_quoted_strings: false,
    allow_hexadecimal_numbers: false,
    allow_unary_plus_numbers: false,
    allow_bare_decimal_point_numbers: false,
    allow_non_finite_numbers: false,
    allow_extended_string_escapes: false,
};

fn jsonc_error_from_parse_error(error: &ParseError) -> JsoncError {
    JsoncError {
        message: error.kind().to_string(),
        line: error.line_display(),
        column: error.column_display(),
    }
}

pub(crate) fn parse_jsonc(input: &str) -> Result<Value, JsoncError> {
    let without_bom = input.strip_prefix('\u{FEFF}').unwrap_or(input);
    let value: Value = parse_to_serde_value(without_bom, &JSONC_PARSE_OPTIONS)
        .map_err(|error| jsonc_error_from_parse_error(&error))?;
    if value.is_null()
        && parse_to_value(without_bom, &JSONC_PARSE_OPTIONS)
            .map_err(|error| jsonc_error_from_parse_error(&error))?
            .is_none()
    {
        return Err(JsoncError {
            message: "EOF while parsing a value".to_owned(),
            line: 1,
            column: 1,
        });
    }
    Ok(value)
}

fn parse_jsonc_value(text: &str, file: ConfigFileKind) -> Result<Value, ConfigurationFileError> {
    parse_jsonc(text).map_err(|source| ConfigurationFileError::InvalidJsonc { file, source })
}

pub fn is_broken_symlink(path: &Path) -> bool {
    std::fs::symlink_metadata(path).is_ok_and(|meta| meta.file_type().is_symlink())
        && matches!(
            std::fs::metadata(path),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound
        )
}

pub fn read_resolved_configuration(
    repository_root: &Path,
    overrides: &ConfigOverrides,
) -> Result<ResolvedConfiguration, ConfigurationFileError> {
    match discover_config_file(repository_root)? {
        Some(discovered) => {
            let text = read_config_file(&discovered)?;
            parse_with_overrides(
                discovered.kind,
                discovered.path,
                true,
                Some(&text),
                overrides,
            )
        }
        None => parse_with_overrides(
            ConfigFileKind::Json,
            repository_root.join(CONFIGURATION_FILE_NAME),
            false,
            None,
            overrides,
        ),
    }
}

pub fn inspect_configuration(repository_root: &Path) -> ConfigurationInspection {
    let discovered = match discover_config_file(repository_root) {
        Ok(Some(discovered)) => discovered,
        Ok(None) => {
            return ConfigurationInspection::Missing {
                path: repository_root.join(CONFIGURATION_FILE_NAME),
            };
        }
        Err(error) => {
            return ConfigurationInspection::Invalid {
                path: repository_root.join(CONFIGURATION_FILE_NAME),
                schema: None,
                error,
            };
        }
    };
    let text = match read_config_file(&discovered) {
        Ok(text) => text,
        Err(error) => {
            return ConfigurationInspection::Invalid {
                path: discovered.path,
                schema: None,
                error,
            };
        }
    };
    let value = match parse_jsonc_value(&text, discovered.kind) {
        Ok(value) => value,
        Err(error) => {
            return ConfigurationInspection::Invalid {
                path: discovered.path,
                schema: None,
                error,
            };
        }
    };
    let schema = inspect_schema_version(&value, env!("CARGO_PKG_VERSION"));
    match parse_value_without_schema_version(discovered.kind, value) {
        Ok(configuration) => ConfigurationInspection::Valid {
            path: discovered.path,
            schema,
            configuration: Box::new(configuration),
        },
        Err(error) => ConfigurationInspection::Invalid {
            path: discovered.path,
            schema: Some(schema),
            error,
        },
    }
}

pub fn configuration_schema() -> String {
    let schema = schemars::schema_for!(ConfigurationFile);
    format!("{:#}\n", schema.as_value())
}

#[cfg(test)]
pub(crate) fn parse(text: &str) -> Result<Configuration, ConfigurationFileError> {
    parse_with_overrides(
        ConfigFileKind::Json,
        PathBuf::from(CONFIGURATION_FILE_NAME),
        true,
        Some(text),
        &ConfigOverrides::default(),
    )
    .map(|resolved| resolved.configuration)
}

fn parse_with_overrides(
    config_file_kind: ConfigFileKind,
    config_file: PathBuf,
    config_file_present: bool,
    text: Option<&str>,
    overrides: &ConfigOverrides,
) -> Result<ResolvedConfiguration, ConfigurationFileError> {
    let raw = match text {
        Some(text) => {
            let value = parse_jsonc_value(text, config_file_kind)?;
            check_schema_version(&value, env!("CARGO_PKG_VERSION"))
                .map_err(|error| error.with_file_kind(config_file_kind))?;
            Some(value)
        }
        None => None,
    };
    parse_value_with_overrides(
        config_file_kind,
        config_file,
        config_file_present,
        raw,
        overrides,
    )
    .map_err(|error| error.with_file_kind(config_file_kind))
}

fn parse_value_without_schema_version(
    config_file_kind: ConfigFileKind,
    value: Value,
) -> Result<Configuration, ConfigurationFileError> {
    parse_value_with_overrides(
        config_file_kind,
        PathBuf::from(config_file_kind.file_name()),
        true,
        Some(value),
        &ConfigOverrides::default(),
    )
    .map(|resolved| resolved.configuration)
    .map_err(|error| error.with_file_kind(config_file_kind))
}

fn parse_configuration_file(
    config_file_kind: ConfigFileKind,
    raw: Option<&Value>,
) -> Result<ConfigurationFile, ConfigurationFileError> {
    let Some(value) = raw else {
        return Ok(ConfigurationFile::default());
    };
    reject_unsupported_metric_keys(value)?;
    let schema = schemars::schema_for!(ConfigurationFile);
    shape::check_shape(value, schema.as_value())?;
    serde_json::from_value(value.clone()).map_err(|source| ConfigurationFileError::NotJson {
        file: config_file_kind,
        source,
    })
}

fn reject_unsupported_metric_keys(root: &Value) -> Result<(), ConfigurationFileError> {
    let Some(root) = root.as_object() else {
        return Ok(());
    };
    if let Some(budgets) = root.get("budgets").and_then(Value::as_object) {
        reject_unsupported_budget_keys(budgets)?;
    }
    if let Some(limits) = root.get("regression_limits").and_then(Value::as_object) {
        reject_unsupported_regression_limit_keys(limits)?;
    }
    Ok(())
}

fn reject_unsupported_budget_keys(
    budgets: &serde_json::Map<String, Value>,
) -> Result<(), ConfigurationFileError> {
    for source_set in SourceSet::ALL {
        let Some(section) = budgets.get(source_set.key()).and_then(Value::as_object) else {
            continue;
        };
        for metric in Metric::ALL {
            if is_production_only_outside_production(metric, source_set)
                && section.contains_key(metric.key())
            {
                return Err(production_only_budget_error(
                    ConfigFileKind::Json,
                    source_set,
                    metric,
                ));
            }
        }
    }
    Ok(())
}

fn reject_unsupported_regression_limit_keys(
    limits: &serde_json::Map<String, Value>,
) -> Result<(), ConfigurationFileError> {
    for metric in Metric::ALL {
        if !metric.supports_diff_regression() && limits.contains_key(metric.key()) {
            return Err(ConfigurationFileError::TestGapRegressionLimit {
                file: ConfigFileKind::Json,
                key: regression_limit_key_path(None, metric.key()),
            });
        }
    }
    for source_set in SourceSet::ALL {
        let Some(section) = limits.get(source_set.key()).and_then(Value::as_object) else {
            continue;
        };
        reject_unsupported_source_set_regression_limit_keys(source_set, section)?;
    }
    Ok(())
}

fn reject_unsupported_source_set_regression_limit_keys(
    source_set: SourceSet,
    section: &serde_json::Map<String, Value>,
) -> Result<(), ConfigurationFileError> {
    for metric in Metric::ALL {
        if !section.contains_key(metric.key()) {
            continue;
        }
        if !metric.supports_diff_regression() {
            return Err(ConfigurationFileError::TestGapRegressionLimit {
                file: ConfigFileKind::Json,
                key: regression_limit_key_path(Some(source_set), metric.key()),
            });
        }
        if is_production_only_outside_production(metric, source_set) {
            return Err(
                ConfigurationFileError::MetricRegressionLimitOutsideProduction {
                    file: ConfigFileKind::Json,
                    key: regression_limit_key_path(Some(source_set), metric.key()),
                    metric,
                },
            );
        }
    }
    Ok(())
}

struct ParsedFeatureSections {
    boundaries: Option<Boundaries>,
    dependencies: DependenciesConfig,
    duplication: DuplicationConfig,
    entry_points: EntryPoints,
    coverage_reports: Vec<String>,
}

fn parse_feature_sections(
    raw: Option<&Value>,
    boundaries_file: Option<BoundariesFile>,
    dependencies_file: DependenciesFile,
    duplication_file: DuplicationConfigFile,
    entry_points_globs: &[String],
    coverage_report_paths: &[String],
    settings: &mut Vec<ResolvedSetting>,
) -> Result<ParsedFeatureSections, ConfigurationFileError> {
    let boundaries = boundaries_file.map(Boundaries::from_file).transpose()?;
    append_boundaries_settings(raw, boundaries.as_ref(), settings);
    let dependencies = DependenciesConfig::from_file(dependencies_file)?;
    append_dependencies_settings(raw, &dependencies, settings);
    let duplication = duplication_config(duplication_file)?;
    append_duplication_settings(raw, duplication, settings);
    let entry_points = entry_points(entry_points_globs)?;
    append_string_list_setting(raw, "entry_points", entry_points_globs, settings);
    let coverage_reports = coverage_reports(coverage_report_paths)?;
    append_string_list_setting(raw, "coverage_reports", &coverage_reports, settings);
    Ok(ParsedFeatureSections {
        boundaries,
        dependencies,
        duplication,
        entry_points,
        coverage_reports,
    })
}

fn parse_value_with_overrides(
    config_file_kind: ConfigFileKind,
    config_file: PathBuf,
    config_file_present: bool,
    raw: Option<Value>,
    overrides: &ConfigOverrides,
) -> Result<ResolvedConfiguration, ConfigurationFileError> {
    let file = parse_configuration_file(config_file_kind, raw.as_ref())?;
    let mut effective = file.without_schema_address();
    let mut settings = Vec::new();
    let source_set_overrides = merge_source_sets(
        raw.as_ref(),
        &mut effective.source_sets,
        &overrides.source_sets,
        &mut settings,
    )?;
    let budgets = merge_budgets(&mut effective.budgets, &overrides.budgets, &mut settings);
    let regression_limits = regression_limits(file.regression_limits);
    append_regression_limit_settings(&regression_limits, &mut settings);
    append_languages_setting(&effective.languages, &mut settings);
    let languages = languages(config_file_kind, file.languages)?;
    append_plugins_setting(raw.as_ref(), &effective.plugins, &mut settings);
    let plugins = plugins(file.plugins)?;
    let plugin_limits = merge_plugin_limits(&mut effective.plugin_limits, overrides, &mut settings);
    let features = parse_feature_sections(
        raw.as_ref(),
        file.boundaries,
        file.dependencies,
        file.duplication,
        &effective.entry_points,
        &effective.coverage_reports,
        &mut settings,
    )?;
    let (sql, hazards) =
        parse_sql_and_hazards(raw.as_ref(), file.sql, file.hazards, &mut settings)?;
    let configuration = Configuration {
        config_file: config_file_kind,
        effective,
        source_set_overrides,
        budgets,
        languages,
        regression_limits,
        plugins,
        plugin_limits,
        boundaries: features.boundaries,
        dependencies: features.dependencies,
        duplication: features.duplication,
        entry_points: features.entry_points,
        coverage_reports: features.coverage_reports,
        sql,
        hazards,
    };
    Ok(ResolvedConfiguration {
        config_file,
        config_file_present,
        configuration,
        settings,
    })
}

fn parse_sql_and_hazards(
    raw: Option<&Value>,
    sql_file: SqlConfigFile,
    hazards_file: HazardsConfigFile,
    settings: &mut Vec<ResolvedSetting>,
) -> Result<(SqlConfig, HazardsConfig), ConfigurationFileError> {
    let dialects_json = serde_json::to_value(&sql_file.dialects).unwrap_or_else(|_| json!([]));
    let sql = SqlConfig::from_file(sql_file)?;
    let sql_source = if has_json_path(raw, &["sql", "dialects"]) {
        ConfigProvenance::File
    } else {
        ConfigProvenance::Default
    };
    push_setting(settings, "sql.dialects", dialects_json, sql_source);

    let hazards = HazardsConfig::from_file(hazards_file)?;
    let hazards_source = if has_json_path(raw, &["hazards", "disabled_rules"]) {
        ConfigProvenance::File
    } else {
        ConfigProvenance::Default
    };
    push_setting(
        settings,
        "hazards.disabled_rules",
        json!(hazards.disabled_rules),
        hazards_source,
    );
    Ok((sql, hazards))
}

fn append_dependencies_settings(
    raw: Option<&Value>,
    dependencies: &DependenciesConfig,
    settings: &mut Vec<ResolvedSetting>,
) {
    let source = if has_json_path(raw, &["dependencies", "ignore"]) {
        ConfigProvenance::File
    } else {
        ConfigProvenance::Default
    };
    let ignored: Vec<&str> = dependencies.ignored().iter().map(String::as_str).collect();
    push_setting(settings, "dependencies.ignore", json!(ignored), source);
}

fn entry_points(globs: &[String]) -> Result<EntryPoints, ConfigurationFileError> {
    if globs.is_empty() {
        return Ok(EntryPoints::default());
    }
    let key = KeyPath::default().child(KeySegment::Key("entry_points".to_owned()));
    Ok(EntryPoints {
        globs: Some(glob_set(&key, globs)?),
    })
}

fn append_string_list_setting(
    raw: Option<&Value>,
    key: &str,
    entries: &[String],
    settings: &mut Vec<ResolvedSetting>,
) {
    let source = if has_json_path(raw, &[key]) {
        ConfigProvenance::File
    } else {
        ConfigProvenance::Default
    };
    push_setting(settings, key, json!(entries), source);
}

fn coverage_reports(reports: &[String]) -> Result<Vec<String>, ConfigurationFileError> {
    let reports_key = KeyPath::default().child(KeySegment::Key("coverage_reports".to_owned()));
    let mut validated = Vec::with_capacity(reports.len());
    for (index, entry) in reports.iter().enumerate() {
        let entry_key = reports_key.child(KeySegment::Index(index));
        if entry.trim().is_empty() {
            return Err(ConfigurationFileError::EmptyCoverageReportPath {
                file: ConfigFileKind::Json,
                key: entry_key,
            });
        }
        if is_absolute_coverage_report_path(entry) {
            return Err(ConfigurationFileError::AbsoluteCoverageReportPath {
                file: ConfigFileKind::Json,
                key: entry_key,
                path: entry.clone(),
            });
        }
        if contains_parent_traversal(entry) {
            return Err(ConfigurationFileError::ParentTraversalCoverageReportPath {
                file: ConfigFileKind::Json,
                key: entry_key,
                path: entry.clone(),
            });
        }
        validated.push(entry.clone());
    }
    Ok(validated)
}

fn is_absolute_coverage_report_path(path: &str) -> bool {
    if Path::new(path).is_absolute() || path.starts_with('/') || path.starts_with('\\') {
        return true;
    }
    let bytes = path.as_bytes();
    bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':'
}

fn contains_parent_traversal(path: &str) -> bool {
    path.split(['/', '\\']).any(|segment| segment == "..")
}

fn duplication_config(
    file: DuplicationConfigFile,
) -> Result<DuplicationConfig, ConfigurationFileError> {
    let min_similarity = match file.min_similarity {
        Some(value) => MinSimilarity::new(value).ok_or_else(|| {
            ConfigurationFileError::OutOfExclusiveMinRange {
                file: ConfigFileKind::Json,
                key: KeyPath::default()
                    .child(KeySegment::Key("duplication".to_owned()))
                    .child(KeySegment::Key("min_similarity".to_owned())),
                exclusive_minimum: 0.0,
                maximum: 1.0,
                found: value,
            }
        })?,
        None => MinSimilarity::DEFAULT,
    };
    Ok(DuplicationConfig {
        near_clones: file.near_clones.unwrap_or(false),
        min_similarity,
    })
}

fn append_duplication_settings(
    raw: Option<&Value>,
    duplication: DuplicationConfig,
    settings: &mut Vec<ResolvedSetting>,
) {
    let near_clones_source = if has_json_path(raw, &["duplication", "near_clones"]) {
        ConfigProvenance::File
    } else {
        ConfigProvenance::Default
    };
    push_setting(
        settings,
        "duplication.near_clones",
        json!(duplication.near_clones),
        near_clones_source,
    );
    let min_similarity_source = if has_json_path(raw, &["duplication", "min_similarity"]) {
        ConfigProvenance::File
    } else {
        ConfigProvenance::Default
    };
    push_setting(
        settings,
        "duplication.min_similarity",
        json!(duplication.min_similarity.ratio()),
        min_similarity_source,
    );
}

fn has_json_path(raw: Option<&Value>, path: &[&str]) -> bool {
    let mut current = match raw {
        Some(value) => value,
        None => return false,
    };
    for segment in path {
        match current.get(*segment) {
            Some(next) => current = next,
            None => return false,
        }
    }
    true
}

fn push_setting(
    settings: &mut Vec<ResolvedSetting>,
    key: impl Into<String>,
    value: Value,
    source: ConfigProvenance,
) {
    settings.push(ResolvedSetting {
        key: key.into(),
        value,
        source,
    });
}

fn merge_source_sets(
    raw: Option<&Value>,
    globs: &mut SourceSetGlobs,
    overrides: &[SourceSetOverride],
    settings: &mut Vec<ResolvedSetting>,
) -> Result<SourceSetOverrides, ConfigurationFileError> {
    for entry in overrides {
        compile_glob(&entry.glob).map_err(|source| ConfigurationFileError::InvalidCliGlob {
            source_set: entry.source_set,
            glob: entry.glob.clone(),
            source,
        })?;
    }
    let cli_by_set = group_source_set_overrides(overrides);
    let slots = [
        (SourceSet::Production, &mut globs.production),
        (SourceSet::Test, &mut globs.test),
        (SourceSet::Generated, &mut globs.generated),
        (SourceSet::Vendored, &mut globs.vendored),
    ];
    for (source_set, slot) in slots {
        let source = if let Some(cli_globs) = cli_by_set.get(&source_set) {
            *slot = cli_globs.clone();
            ConfigProvenance::Cli
        } else if has_json_path(raw, &["source_sets", source_set.key()]) {
            ConfigProvenance::File
        } else {
            ConfigProvenance::Default
        };
        push_setting(
            settings,
            format!("source_sets.{}", source_set.key()),
            json!(slot),
            source,
        );
    }
    source_set_overrides(globs.clone())
}

fn group_source_set_overrides(overrides: &[SourceSetOverride]) -> BTreeMap<SourceSet, Vec<String>> {
    let mut grouped: BTreeMap<SourceSet, Vec<String>> = BTreeMap::new();
    for entry in overrides {
        grouped
            .entry(entry.source_set)
            .or_default()
            .push(entry.glob.clone());
    }
    grouped
}

fn merge_budgets(
    file_budgets: &mut SourceSetBudgets,
    overrides: &[BudgetOverride],
    settings: &mut Vec<ResolvedSetting>,
) -> Budgets {
    let parsed_file = budgets(file_budgets.clone());
    let mut resolved: BTreeMap<(SourceSet, String), (Budget, ConfigProvenance)> = BTreeMap::new();
    for (source_set, metric, budget) in parsed_file.set {
        resolved.insert((source_set, metric), (budget, ConfigProvenance::File));
    }
    for entry in overrides {
        file_budgets.set(entry.source_set, &entry.metric, entry.budget);
        resolved.insert(
            (entry.source_set, entry.metric.clone()),
            (entry.budget, ConfigProvenance::Cli),
        );
    }
    append_budget_settings(&resolved, &parsed_file.dependency_set, settings);
    Budgets {
        set: resolved
            .into_iter()
            .map(|((source_set, metric), (budget, _))| (source_set, metric, budget))
            .collect(),
        dependency_set: parsed_file.dependency_set,
    }
}

fn append_budget_settings(
    resolved: &BTreeMap<(SourceSet, String), (Budget, ConfigProvenance)>,
    dependency_set: &[(String, Budget)],
    settings: &mut Vec<ResolvedSetting>,
) {
    if resolved.is_empty() && dependency_set.is_empty() {
        push_setting(settings, "budgets", json!({}), ConfigProvenance::Default);
        return;
    }
    for source_set in SourceSet::ALL {
        for ((_, metric), (budget, source)) in
            resolved.iter().filter(|((set, _), _)| *set == source_set)
        {
            push_setting(
                settings,
                format!("budgets.{}.{metric}", source_set.key()),
                json!(budget.ratio()),
                *source,
            );
        }
    }
    for (metric, budget) in dependency_set {
        push_setting(
            settings,
            format!("budgets.{metric}"),
            json!(budget.ratio()),
            ConfigProvenance::File,
        );
    }
}

fn push_resolved_regression_limit(
    settings: &mut Vec<ResolvedSetting>,
    prefix: &str,
    resolved: ResolvedRegressionLimit,
) {
    push_setting(
        settings,
        format!("{prefix}.ratio_rise"),
        json!(resolved.limit.ratio_rise.ratio()),
        resolved.ratio_rise_source,
    );
    push_setting(
        settings,
        format!("{prefix}.new_findings"),
        json!(resolved.limit.new_findings),
        resolved.new_findings_source,
    );
}

fn append_regression_limit_settings(
    limits: &RegressionLimits,
    settings: &mut Vec<ResolvedSetting>,
) {
    let extra_metric_keys = limits.extra_metric_keys();
    for source_set in SourceSet::ALL {
        let metric_keys = allowed_regression_limit_metric_keys(Some(source_set), &[]);
        for metric_key in metric_keys.iter().chain(&extra_metric_keys) {
            let resolved = limits.resolved_limit_for_key(source_set, metric_key);
            let prefix = format!("regression_limits.{}.{metric_key}", source_set.key());
            push_resolved_regression_limit(settings, &prefix, resolved);
        }
    }
    for metric in Metric::ALL
        .into_iter()
        .filter(|metric| is_repository_wide_diff_metric(*metric))
    {
        let metric_key = metric.key();
        let resolved = limits.resolved_limit_for_dependency(metric_key);
        let prefix = format!("regression_limits.{metric_key}");
        push_resolved_regression_limit(settings, &prefix, resolved);
    }
}

fn append_languages_setting(
    entries: &Option<Vec<LanguageGlobs>>,
    settings: &mut Vec<ResolvedSetting>,
) {
    match entries {
        Some(entries) => push_setting(
            settings,
            "languages",
            serde_json::to_value(entries).unwrap_or(Value::Null),
            ConfigProvenance::File,
        ),
        None => push_setting(
            settings,
            "languages",
            Value::Null,
            ConfigProvenance::Default,
        ),
    }
}

fn append_plugins_setting(
    raw: Option<&Value>,
    entries: &[PluginEntry],
    settings: &mut Vec<ResolvedSetting>,
) {
    let source = if has_json_path(raw, &["plugins"]) {
        ConfigProvenance::File
    } else {
        ConfigProvenance::Default
    };
    push_setting(
        settings,
        "plugins",
        serde_json::to_value(entries).unwrap_or_else(|_| json!([])),
        source,
    );
}

fn merge_plugin_limits(
    file: &mut PluginLimitsFile,
    overrides: &ConfigOverrides,
    settings: &mut Vec<ResolvedSetting>,
) -> PluginLimits {
    let (memory_mb, memory_source) = resolve_with_provenance(
        DEFAULT_PLUGIN_MEMORY_MB,
        file.memory_mb,
        overrides.plugin_memory_mb,
    );
    let (timeout_ms, timeout_source) = resolve_with_provenance(
        DEFAULT_PLUGIN_TIMEOUT_MS,
        file.timeout_ms,
        overrides.plugin_timeout_ms,
    );
    if let Some(value) = overrides.plugin_memory_mb {
        file.memory_mb = Some(value);
    }
    if let Some(value) = overrides.plugin_timeout_ms {
        file.timeout_ms = Some(value);
    }
    push_setting(
        settings,
        "plugin_limits.memory_mb",
        json!(memory_mb),
        memory_source,
    );
    push_setting(
        settings,
        "plugin_limits.timeout_ms",
        json!(timeout_ms),
        timeout_source,
    );
    PluginLimits {
        memory_mb,
        timeout_ms,
    }
}

fn append_boundaries_settings(
    raw: Option<&Value>,
    boundaries: Option<&Boundaries>,
    settings: &mut Vec<ResolvedSetting>,
) {
    let Some(boundaries) = boundaries else {
        push_setting(
            settings,
            "boundaries",
            Value::Null,
            ConfigProvenance::Default,
        );
        return;
    };
    let preset_source = if boundaries.preset().is_some() {
        ConfigProvenance::File
    } else {
        ConfigProvenance::Default
    };
    push_setting(
        settings,
        "boundaries.preset",
        serde_json::to_value(boundaries.preset()).unwrap_or(Value::Null),
        preset_source,
    );
    let strict_source = if has_json_path(raw, &["boundaries", "strict_zones"]) {
        ConfigProvenance::File
    } else {
        ConfigProvenance::Default
    };
    push_setting(
        settings,
        "boundaries.strict_zones",
        json!(boundaries.strict_zones()),
        strict_source,
    );
    let has_non_empty_source_sets = raw
        .and_then(|value| value.get("boundaries"))
        .and_then(|value| value.get("source_sets"))
        .and_then(Value::as_array)
        .is_some_and(|items| !items.is_empty());
    let source_sets_source = if has_non_empty_source_sets {
        ConfigProvenance::File
    } else {
        ConfigProvenance::Default
    };
    push_setting(
        settings,
        "boundaries.source_sets",
        json!(boundaries.source_set_keys()),
        source_sets_source,
    );
    let zones_source = if has_json_path(raw, &["boundaries", "zones"]) {
        ConfigProvenance::File
    } else {
        ConfigProvenance::Default
    };
    push_setting(
        settings,
        "boundaries.zones",
        boundaries.resolved_zones_json(),
        zones_source,
    );
    let rules_source = if has_json_path(raw, &["boundaries", "rules"]) {
        ConfigProvenance::File
    } else {
        ConfigProvenance::Default
    };
    push_setting(
        settings,
        "boundaries.rules",
        boundaries.resolved_rules_json(),
        rules_source,
    );
}

impl SourceSetBudgets {
    fn set(&mut self, source_set: SourceSet, metric: &str, budget: Budget) {
        match source_set {
            SourceSet::Production => self.production.set(metric, budget),
            SourceSet::Test => self.test.set(metric, budget),
            SourceSet::Generated => self.generated.set(metric, budget),
            SourceSet::Vendored => self.vendored.set(metric, budget),
        }
    }
}

impl SharedMetricBudgets {
    fn set_known(&mut self, metric: &str, budget: Budget) -> bool {
        match Metric::from_key(metric) {
            Some(Metric::Erosion) => self.erosion = Some(budget),
            Some(Metric::CognitiveComplexity) => self.cognitive_complexity = Some(budget),
            Some(Metric::Duplication) => self.duplication = Some(budget),
            Some(Metric::Verbosity) => self.verbosity = Some(budget),
            Some(Metric::ImportCycles) => self.import_cycles = Some(budget),
            Some(Metric::Boundaries) => self.boundaries = Some(budget),
            Some(Metric::Hazards) => self.hazards = Some(budget),
            _ => return false,
        }
        true
    }

    fn into_pairs(self) -> impl Iterator<Item = (String, Budget)> {
        all_source_set_metric_pairs([
            self.erosion,
            self.cognitive_complexity,
            self.duplication,
            self.verbosity,
            self.import_cycles,
            self.boundaries,
            self.hazards,
        ])
        .filter_map(|(metric, ratio)| Some((metric, ratio?)))
    }
}

impl ProductionMetricBudgets {
    fn set(&mut self, metric: &str, budget: Budget) {
        if self.common.set_known(metric, budget) {
            return;
        }
        match Metric::from_key(metric) {
            Some(Metric::OrphanFiles) => self.orphan_files = Some(budget),
            Some(Metric::TestGap) => self.test_gap = Some(budget),
            _ => {
                self.external.insert(metric.to_owned(), budget);
            }
        }
    }
}

impl MetricBudgets {
    fn set(&mut self, metric: &str, budget: Budget) {
        if !self.common.set_known(metric, budget) {
            self.external.insert(metric.to_owned(), budget);
        }
    }
}

fn plugins(entries: Vec<PluginEntry>) -> Result<Vec<PluginDeclaration>, ConfigurationFileError> {
    let plugins_key = KeyPath::default().child(KeySegment::Key("plugins".to_owned()));
    entries
        .into_iter()
        .enumerate()
        .map(|(index, entry)| {
            let entry_key = plugins_key.child(KeySegment::Index(index));
            if entry.name.trim().is_empty() {
                return Err(ConfigurationFileError::EmptyPluginField {
                    file: ConfigFileKind::Json,
                    key: entry_key.child(KeySegment::Key("name".to_owned())),
                });
            }
            let source = match (entry.path, entry.url) {
                (Some(path), None) => {
                    if path.trim().is_empty() {
                        return Err(ConfigurationFileError::EmptyPluginField {
                            file: ConfigFileKind::Json,
                            key: entry_key.child(KeySegment::Key("path".to_owned())),
                        });
                    }
                    PluginSource::Path(PathBuf::from(path))
                }
                (None, Some(url)) => {
                    let url_key = entry_key.child(KeySegment::Key("url".to_owned()));
                    if url.trim().is_empty() {
                        return Err(ConfigurationFileError::EmptyPluginField {
                            file: ConfigFileKind::Json,
                            key: url_key,
                        });
                    }
                    let normalized = normalize_https_url(&url).ok_or_else(|| {
                        ConfigurationFileError::NonHttpsPluginUrl {
                            file: ConfigFileKind::Json,
                            key: url_key,
                            name: entry.name.clone(),
                            url,
                        }
                    })?;
                    PluginSource::HttpsUrl(normalized)
                }
                (None, None) => {
                    return Err(ConfigurationFileError::MissingPluginSource {
                        file: ConfigFileKind::Json,
                        key: entry_key,
                        name: entry.name,
                    });
                }
                (Some(_), Some(_)) => {
                    return Err(ConfigurationFileError::AmbiguousPluginSource {
                        file: ConfigFileKind::Json,
                        key: entry_key,
                        name: entry.name,
                    });
                }
            };
            let sha256 = Sha256Digest::parse_hex(&entry.sha256).ok_or_else(|| {
                ConfigurationFileError::InvalidSha256 {
                    file: ConfigFileKind::Json,
                    key: entry_key.child(KeySegment::Key("sha256".to_owned())),
                    found: entry.sha256,
                }
            })?;
            Ok(PluginDeclaration {
                name: entry.name,
                kind: entry.kind,
                source,
                sha256,
            })
        })
        .collect()
}

fn normalize_https_url(url: &str) -> Option<String> {
    let prefix = url.get(..8)?;
    if !prefix.eq_ignore_ascii_case("https://") {
        return None;
    }
    let rest = &url[8..];
    if rest.is_empty() || rest.starts_with('/') || url.chars().any(char::is_whitespace) {
        return None;
    }
    Some(format!("https://{rest}"))
}

fn regression_limits(file: RegressionLimitsFile) -> RegressionLimits {
    let RegressionLimitsFile {
        erosion,
        cognitive_complexity,
        duplication,
        verbosity,
        import_cycles,
        boundaries,
        orphan_files,
        hazards,
        unused_dependencies,
        unlisted_dependencies,
        dev_in_production_dependencies,
        production,
        test,
        generated,
        vendored,
        external,
    } = file;
    let dependency_limits = [
        (
            Metric::UnusedDependencies.key().to_owned(),
            unused_dependencies,
        ),
        (
            Metric::UnlistedDependencies.key().to_owned(),
            unlisted_dependencies,
        ),
        (
            Metric::DevInProductionDependencies.key().to_owned(),
            dev_in_production_dependencies,
        ),
    ]
    .into_iter()
    .filter(|(_, setting)| !setting.is_unset());
    let every_source_set = ProductionMetricRegressionLimits {
        shared: MetricRegressionLimits {
            erosion,
            cognitive_complexity,
            duplication,
            verbosity,
            import_cycles,
            boundaries,
            hazards,
            external,
        },
        orphan_files,
    }
    .by_metric()
    .chain(dependency_limits)
    .collect();
    let production_limits = production
        .by_metric()
        .map(|(metric, setting)| (SourceSet::Production, metric, setting));
    let other_limits = [
        (SourceSet::Test, test),
        (SourceSet::Generated, generated),
        (SourceSet::Vendored, vendored),
    ]
    .into_iter()
    .flat_map(|(source_set, limits)| {
        limits
            .by_metric()
            .map(move |(metric, setting)| (source_set, metric, setting))
    });
    let one_source_set = production_limits.chain(other_limits).collect();
    RegressionLimits::new(every_source_set, one_source_set)
}

impl ConfigurationFile {
    fn without_schema_address(&self) -> ConfigurationFile {
        ConfigurationFile {
            _schema: None,
            duplication: self.duplication.clone().normalized(),
            ..self.clone()
        }
    }
}

fn budgets(file: SourceSetBudgets) -> Budgets {
    let SourceSetBudgets {
        production,
        test,
        generated,
        vendored,
        unused_dependencies,
        unlisted_dependencies,
        dev_in_production_dependencies,
    } = file;
    let dependency_set = [
        (
            Metric::UnusedDependencies.key().to_owned(),
            unused_dependencies,
        ),
        (
            Metric::UnlistedDependencies.key().to_owned(),
            unlisted_dependencies,
        ),
        (
            Metric::DevInProductionDependencies.key().to_owned(),
            dev_in_production_dependencies,
        ),
    ]
    .into_iter()
    .filter_map(|(metric, ratio)| Some((metric, ratio?)))
    .collect();
    let production_entries = production
        .common
        .into_pairs()
        .chain(
            production
                .orphan_files
                .map(|ratio| (Metric::OrphanFiles.key().to_owned(), ratio)),
        )
        .chain(
            production
                .test_gap
                .map(|ratio| (TEST_GAP_METRIC_KEY.to_owned(), ratio)),
        )
        .chain(production.external)
        .map(|(metric, ratio)| (SourceSet::Production, metric, ratio));
    let other_entries = [
        (SourceSet::Test, test),
        (SourceSet::Generated, generated),
        (SourceSet::Vendored, vendored),
    ]
    .into_iter()
    .flat_map(|(source_set, metric_budgets)| {
        metric_budgets
            .common
            .into_pairs()
            .chain(metric_budgets.external)
            .map(move |(metric, ratio)| (source_set, metric, ratio))
    });
    let set = production_entries.chain(other_entries).collect();
    Budgets {
        set,
        dependency_set,
    }
}

fn languages(
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
            Ok(LanguageEntry {
                plugin_key: entry_key.child(KeySegment::Key("plugin".to_owned())),
                plugin: entry.plugin,
                include: glob_set(
                    &entry_key.child(KeySegment::Key("include".to_owned())),
                    &entry.include,
                )?,
                exclude: glob_set(
                    &entry_key.child(KeySegment::Key("exclude".to_owned())),
                    &entry.exclude,
                )?,
            })
        })
        .collect::<Result<_, ConfigurationFileError>>()?;
    Ok(Languages {
        config_file,
        entries: Some(entries),
    })
}

fn source_set_overrides(
    globs: SourceSetGlobs,
) -> Result<SourceSetOverrides, ConfigurationFileError> {
    let SourceSetGlobs {
        production,
        test,
        generated,
        vendored,
    } = globs;
    let by_precedence = [
        (SourceSet::Vendored, "vendored", vendored),
        (SourceSet::Generated, "generated", generated),
        (SourceSet::Test, "test", test),
        (SourceSet::Production, "production", production),
    ]
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

const OFFICIAL_SCHEMA_PREFIX: &str = "https://raw.githubusercontent.com/avidianity/codeint/v";
const OFFICIAL_SCHEMA_SUFFIX: &str = "/schema/codeint.schema.json";

fn inspect_schema_version(value: &Value, binary_version: &str) -> SchemaVersionStatus {
    let Some(schema_url) = value.get("$schema").and_then(Value::as_str) else {
        return SchemaVersionStatus::Missing;
    };
    let Some(config_version) = extract_official_schema_version(schema_url) else {
        return SchemaVersionStatus::Compatible {
            url: schema_url.to_owned(),
            version: None,
        };
    };
    let (Some(cfg), Some(bin)) = (parse_semver(config_version), parse_semver(binary_version))
    else {
        return SchemaVersionStatus::Compatible {
            url: schema_url.to_owned(),
            version: Some(config_version.to_owned()),
        };
    };
    if cfg > bin {
        return SchemaVersionStatus::NewerThanBinary {
            url: schema_url.to_owned(),
            config_version: config_version.to_owned(),
            binary_version: binary_version.to_owned(),
        };
    }
    SchemaVersionStatus::Compatible {
        url: schema_url.to_owned(),
        version: Some(config_version.to_owned()),
    }
}

fn check_schema_version(value: &Value, binary_version: &str) -> Result<(), ConfigurationFileError> {
    if let SchemaVersionStatus::NewerThanBinary {
        config_version,
        binary_version,
        ..
    } = inspect_schema_version(value, binary_version)
    {
        return Err(ConfigurationFileError::NewerSchemaVersion {
            file: ConfigFileKind::Json,
            config_version,
            binary_version,
        });
    }
    Ok(())
}

fn extract_official_schema_version(url: &str) -> Option<&str> {
    url.strip_prefix(OFFICIAL_SCHEMA_PREFIX)?
        .strip_suffix(OFFICIAL_SCHEMA_SUFFIX)
}

fn parse_semver(raw: &str) -> Option<(u64, u64, u64)> {
    let core = raw.split(['-', '+']).next()?;
    let mut parts = core.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    let patch = parts.next()?.parse().ok()?;
    if parts.next().is_some() {
        return None;
    }
    Some((major, minor, patch))
}

#[cfg(test)]
mod schema_version_tests {
    use super::*;

    fn schema_json(url: &str) -> Value {
        serde_json::json!({ "$schema": url })
    }

    #[test]
    fn rejects_newer_patch_minor_and_major_schema_versions() {
        for newer in ["0.4.1", "0.5.0", "1.0.0"] {
            let url = format!("{OFFICIAL_SCHEMA_PREFIX}{newer}{OFFICIAL_SCHEMA_SUFFIX}");
            let err = check_schema_version(&schema_json(&url), "0.4.0").unwrap_err();
            assert!(matches!(
                err,
                ConfigurationFileError::NewerSchemaVersion {
                    ref config_version,
                    ref binary_version,
                    ..
                } if config_version == newer && binary_version == "0.4.0"
            ));
        }
    }

    #[test]
    fn accepts_equal_older_missing_and_custom_schemas() {
        for ok in ["0.4.0", "0.3.9", "0.1.0"] {
            let url = format!("{OFFICIAL_SCHEMA_PREFIX}{ok}{OFFICIAL_SCHEMA_SUFFIX}");
            assert!(check_schema_version(&schema_json(&url), "0.4.0").is_ok());
        }
        assert!(check_schema_version(&serde_json::json!({}), "0.4.0").is_ok());
        assert!(
            check_schema_version(&schema_json("./schema/codeint.schema.json"), "0.4.0").is_ok()
        );
    }
}
