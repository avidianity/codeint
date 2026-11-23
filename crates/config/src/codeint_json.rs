use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use codeint_model::{Metric, RepoPath, SourceSet};

use crate::RegressionLimits;
use crate::boundaries::{Boundaries, BoundariesFile};
use crate::config_error::{
    CONFIGURATION_FILE_NAME, ConfigurationFileError, JsonType, KeyPath, KeySegment, glob_set,
};
use crate::dependencies::{DependenciesConfig, DependenciesFile};
use crate::regression_limits::RegressionLimitSetting;

pub const DEPENDENCY_METRIC_KEYS: [&str; 3] = [
    "unused_dependencies",
    "unlisted_dependencies",
    "dev_in_production_dependencies",
];
use globset::GlobSet;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

mod shape;
mod starter;

pub use starter::{
    DetectedLanguage, StarterConfigurationError, published_schema_address,
    write_starter_configuration,
};

const DEFAULT_PLUGIN_MEMORY_MB: u64 = 64;
const DEFAULT_PLUGIN_TIMEOUT_MS: u64 = 5_000;

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
    effective: ConfigurationFile,
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
    pub fn validate_metrics(
        &self,
        external_metric_keys: &[&str],
    ) -> Result<(), ConfigurationFileError> {
        let allowed_metrics = || -> Vec<String> {
            Metric::BUILT_IN_KEYS
                .iter()
                .chain(["cognitive_complexity", "boundaries"].iter())
                .chain(external_metric_keys.iter())
                .map(|key| (*key).to_owned())
                .collect()
        };
        let is_known = |key: &str| {
            Metric::BUILT_IN_KEYS.contains(&key)
                || key == "cognitive_complexity"
                || key == "boundaries"
                || external_metric_keys.contains(&key)
        };
        for (source_set, metric_key, _) in &self.budgets.set {
            if !is_known(metric_key) {
                return Err(ConfigurationFileError::UnknownKey {
                    key: KeyPath::default()
                        .child(KeySegment::Key("budgets".to_owned()))
                        .child(KeySegment::Key(source_set.key().to_owned()))
                        .child(KeySegment::Key(metric_key.clone())),
                    allowed: allowed_metrics(),
                });
            }
        }
        validate_regression_limit_keys(&self.regression_limits, is_known, allowed_metrics)
    }
}

fn validate_regression_limit_keys(
    limits: &RegressionLimits,
    is_known: impl Fn(&str) -> bool,
    allowed_metrics: impl Fn() -> Vec<String>,
) -> Result<(), ConfigurationFileError> {
    for (source_set, metric_key) in limits.metric_keys() {
        let allowed_here = is_known(metric_key)
            || (source_set.is_none() && DEPENDENCY_METRIC_KEYS.contains(&metric_key));
        if !allowed_here {
            let mut path =
                KeyPath::default().child(KeySegment::Key("regression_limits".to_owned()));
            let mut allowed = allowed_metrics();
            if let Some(source_set) = source_set {
                path = path.child(KeySegment::Key(source_set.key().to_owned()));
            } else {
                allowed.extend(DEPENDENCY_METRIC_KEYS.into_iter().map(str::to_owned));
                allowed.extend(
                    ["production", "test", "generated", "vendored"]
                        .into_iter()
                        .map(str::to_owned),
                );
            }
            return Err(ConfigurationFileError::UnknownKey {
                key: path.child(KeySegment::Key(metric_key.to_owned())),
                allowed,
            });
        }
    }
    Ok(())
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

#[derive(Debug, Clone, Copy, PartialEq, Deserialize, Serialize, JsonSchema)]
#[serde(transparent)]
pub struct Budget(#[schemars(range(min = 0, max = 1))] f64);

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

    pub fn dependency_budget(&self, metric_key: &str) -> Option<Budget> {
        self.dependency_set
            .iter()
            .find(|(measured, _)| measured == metric_key)
            .map(|(_, budget)| *budget)
    }
}

impl Budget {
    pub fn ratio(self) -> f64 {
        self.0
    }

    pub fn admits(self, ratio: f64) -> bool {
        ratio <= self.0
    }
}

#[derive(Debug, Default)]
pub struct Languages {
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
    #[schemars(description = "The Regression limit of unused declared dependencies.")]
    unused_dependencies: RegressionLimitSetting,
    #[serde(default, skip_serializing_if = "RegressionLimitSetting::is_unset")]
    #[schemars(description = "The Regression limit of unlisted external dependencies.")]
    unlisted_dependencies: RegressionLimitSetting,
    #[serde(default, skip_serializing_if = "RegressionLimitSetting::is_unset")]
    #[schemars(description = "The Regression limit of dev dependencies imported in production.")]
    dev_in_production_dependencies: RegressionLimitSetting,
    #[serde(default, skip_serializing_if = "MetricRegressionLimits::is_unset")]
    #[schemars(description = "The Regression limits of the Production Source set.")]
    production: MetricRegressionLimits,
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
    #[serde(flatten, default, skip_serializing_if = "BTreeMap::is_empty")]
    external: BTreeMap<String, RegressionLimitSetting>,
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
            external,
        } = self;
        [
            (Metric::Erosion.key().to_owned(), erosion),
            ("cognitive_complexity".to_owned(), cognitive_complexity),
            (Metric::Duplication.key().to_owned(), duplication),
            (Metric::Verbosity.key().to_owned(), verbosity),
            (Metric::ImportCycles.key().to_owned(), import_cycles),
            ("boundaries".to_owned(), boundaries),
        ]
        .into_iter()
        .chain(external)
        .filter(|(_, setting)| !setting.is_unset())
    }
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct SourceSetBudgets {
    #[serde(default)]
    #[schemars(description = "The Budgets of the Production Source set.")]
    production: MetricBudgets,
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
struct MetricBudgets {
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
    #[serde(flatten, default, skip_serializing_if = "BTreeMap::is_empty")]
    external: BTreeMap<String, Budget>,
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

pub fn read_configuration(repository_root: &Path) -> Result<Configuration, ConfigurationFileError> {
    match std::fs::read_to_string(repository_root.join(CONFIGURATION_FILE_NAME)) {
        Ok(text) => parse(&text),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Configuration::default()),
        Err(error) => Err(ConfigurationFileError::Unreadable(error)),
    }
}

pub fn configuration_schema() -> String {
    let schema = schemars::schema_for!(ConfigurationFile);
    format!("{:#}\n", schema.as_value())
}

pub(crate) fn parse(text: &str) -> Result<Configuration, ConfigurationFileError> {
    let value: Value = serde_json::from_str(text).map_err(ConfigurationFileError::NotJson)?;
    check_schema_version(&value, env!("CARGO_PKG_VERSION"))?;
    let schema = schemars::schema_for!(ConfigurationFile);
    shape::check_shape(&value, schema.as_value())?;
    let file: ConfigurationFile =
        serde_json::from_value(value).map_err(ConfigurationFileError::NotJson)?;
    let effective = file.without_schema_address();
    let parsed_boundaries = file.boundaries.map(Boundaries::from_file).transpose()?;
    let parsed_dependencies = DependenciesConfig::from_file(file.dependencies)?;
    Ok(Configuration {
        effective,
        source_set_overrides: source_set_overrides(file.source_sets)?,
        budgets: budgets(file.budgets),
        languages: languages(file.languages)?,
        regression_limits: regression_limits(file.regression_limits),
        plugins: plugins(file.plugins)?,
        plugin_limits: plugin_limits(file.plugin_limits),
        boundaries: parsed_boundaries,
        dependencies: parsed_dependencies,
    })
}

fn plugin_limits(file: PluginLimitsFile) -> PluginLimits {
    PluginLimits {
        memory_mb: file.memory_mb.unwrap_or(DEFAULT_PLUGIN_MEMORY_MB),
        timeout_ms: file.timeout_ms.unwrap_or(DEFAULT_PLUGIN_TIMEOUT_MS),
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
                    key: entry_key.child(KeySegment::Key("name".to_owned())),
                });
            }
            let source = match (entry.path, entry.url) {
                (Some(path), None) => {
                    if path.trim().is_empty() {
                        return Err(ConfigurationFileError::EmptyPluginField {
                            key: entry_key.child(KeySegment::Key("path".to_owned())),
                        });
                    }
                    PluginSource::Path(PathBuf::from(path))
                }
                (None, Some(url)) => {
                    let url_key = entry_key.child(KeySegment::Key("url".to_owned()));
                    if url.trim().is_empty() {
                        return Err(ConfigurationFileError::EmptyPluginField { key: url_key });
                    }
                    let normalized = normalize_https_url(&url).ok_or_else(|| {
                        ConfigurationFileError::NonHttpsPluginUrl {
                            key: url_key,
                            name: entry.name.clone(),
                            url,
                        }
                    })?;
                    PluginSource::HttpsUrl(normalized)
                }
                (None, None) => {
                    return Err(ConfigurationFileError::MissingPluginSource {
                        key: entry_key,
                        name: entry.name,
                    });
                }
                (Some(_), Some(_)) => {
                    return Err(ConfigurationFileError::AmbiguousPluginSource {
                        key: entry_key,
                        name: entry.name,
                    });
                }
            };
            let sha256 = Sha256Digest::parse_hex(&entry.sha256).ok_or_else(|| {
                ConfigurationFileError::InvalidSha256 {
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
        ("unused_dependencies".to_owned(), unused_dependencies),
        ("unlisted_dependencies".to_owned(), unlisted_dependencies),
        (
            "dev_in_production_dependencies".to_owned(),
            dev_in_production_dependencies,
        ),
    ]
    .into_iter()
    .filter(|(_, setting)| !setting.is_unset());
    let every_source_set = MetricRegressionLimits {
        erosion,
        cognitive_complexity,
        duplication,
        verbosity,
        import_cycles,
        boundaries,
        external,
    }
    .by_metric()
    .chain(dependency_limits)
    .collect();
    let one_source_set = [
        (SourceSet::Production, production),
        (SourceSet::Test, test),
        (SourceSet::Generated, generated),
        (SourceSet::Vendored, vendored),
    ]
    .into_iter()
    .flat_map(|(source_set, limits)| {
        limits
            .by_metric()
            .map(move |(metric, setting)| (source_set, metric, setting))
    })
    .collect();
    RegressionLimits::new(every_source_set, one_source_set)
}

impl ConfigurationFile {
    fn without_schema_address(&self) -> ConfigurationFile {
        ConfigurationFile {
            _schema: None,
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
        ("unused_dependencies".to_owned(), unused_dependencies),
        ("unlisted_dependencies".to_owned(), unlisted_dependencies),
        (
            "dev_in_production_dependencies".to_owned(),
            dev_in_production_dependencies,
        ),
    ]
    .into_iter()
    .filter_map(|(metric, ratio)| Some((metric, ratio?)))
    .collect();
    let set = [
        (SourceSet::Production, production),
        (SourceSet::Test, test),
        (SourceSet::Generated, generated),
        (SourceSet::Vendored, vendored),
    ]
    .into_iter()
    .flat_map(|(source_set, metric_budgets)| {
        let MetricBudgets {
            erosion,
            cognitive_complexity,
            duplication,
            verbosity,
            import_cycles,
            boundaries,
            external,
        } = metric_budgets;
        [
            (Metric::Erosion.key().to_owned(), erosion),
            ("cognitive_complexity".to_owned(), cognitive_complexity),
            (Metric::Duplication.key().to_owned(), duplication),
            (Metric::Verbosity.key().to_owned(), verbosity),
            (Metric::ImportCycles.key().to_owned(), import_cycles),
            ("boundaries".to_owned(), boundaries),
        ]
        .into_iter()
        .filter_map(move |(metric, ratio)| Some((source_set, metric, ratio?)))
        .chain(
            external
                .into_iter()
                .map(move |(metric, ratio)| (source_set, metric, ratio)),
        )
    })
    .collect();
    Budgets {
        set,
        dependency_set,
    }
}

fn languages(entries: Option<Vec<LanguageGlobs>>) -> Result<Languages, ConfigurationFileError> {
    let Some(entries) = entries else {
        return Ok(Languages::default());
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

fn check_schema_version(value: &Value, binary_version: &str) -> Result<(), ConfigurationFileError> {
    let Some(schema_url) = value.get("$schema").and_then(Value::as_str) else {
        return Ok(());
    };
    let Some(config_version) = extract_official_schema_version(schema_url) else {
        return Ok(());
    };
    let (Some(cfg), Some(bin)) = (parse_semver(config_version), parse_semver(binary_version))
    else {
        return Ok(());
    };
    if cfg > bin {
        return Err(ConfigurationFileError::NewerSchemaVersion {
            config_version: config_version.to_owned(),
            binary_version: binary_version.to_owned(),
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
                    ref binary_version
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
