use std::collections::BTreeMap;

use codeint_model::{Metric, SourceSet};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::RegressionLimits;
use crate::boundaries::BoundariesFile;
use crate::dependencies::DependenciesFile;
use crate::framework_profiles::{FrameworkProfileRuleFile, PackageConfigFile};
use crate::hazards::HazardsConfigFile;
use crate::ratchet::RatchetConfigFile;
use crate::regression_limits::RegressionLimitSetting;
use crate::sql::SqlConfigFile;

pub(super) const DEFAULT_MIN_SIMILARITY: f64 = 0.85;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum OutputFormat {
    #[default]
    Toon,
    Text,
    Json,
    Sarif,
    Markdown,
}

impl std::fmt::Display for OutputFormat {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            OutputFormat::Toon => "toon",
            OutputFormat::Text => "text",
            OutputFormat::Json => "json",
            OutputFormat::Sarif => "sarif",
            OutputFormat::Markdown => "markdown",
        })
    }
}

#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize, JsonSchema, clap::ValueEnum,
)]
#[serde(rename_all = "snake_case")]
pub enum PluginKind {
    #[default]
    Language,
    Metric,
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

pub fn configuration_schema() -> String {
    let schema = schemars::schema_for!(ConfigurationFile);
    format!("{:#}\n", schema.as_value())
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(
    title = "codeint.json",
    description = "The configuration of codeint, read from codeint.json, codeint.jsonc, .codeint/config.json, or .codeint/config.jsonc at the root of the repository."
)]
pub(super) struct ConfigurationFile {
    #[serde(rename = "$schema", default)]
    #[schemars(
        description = "The address of this JSON schema, for editors; codeint does not read it."
    )]
    _schema: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(
        with = "OutputFormat",
        description = "Default output format for check, diff, trend, report, impact, trace, churn, and coupling: toon, text, json, sarif, or markdown. Omitted defaults to toon. CLI --format always overrides this setting."
    )]
    format: Option<OutputFormat>,
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
    #[serde(default, skip_serializing_if = "NewFileBudgetsFile::is_unset")]
    #[schemars(
        description = "The highest ratio each Metric may reach across newly added files in codeint diff, per Source set; a Metric over its new-file Budget fails codeint diff with exit code 1. A ratio equal to its Budget passes. Codeint compares the ratio as the report shows it, rounded to 4 decimal places."
    )]
    new_file_budgets: NewFileBudgetsFile,
    #[serde(default)]
    #[schemars(
        description = "How much each Metric may get worse from the merge-base to the head side before codeint diff reports a Regression and exits with code 1. A key named after a Metric sets its limit in every Source set; a key named after a Source set sets limits for that Source set only. For each setting, the limit of the Source set wins over the limit of the Metric, and a setting left out keeps the default: a ratio rise of 0.01, and numerator_rise (direct numerator rise), except for Import cycles, Boundaries, Orphan files, Hazards, and Dependency hygiene, where any_new_finding (plus numerator rise) applies."
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
        description = "Architectural import boundaries defining named zones, optional public_files entry points, directional internal allow or deny rules, external package allow_external or deny_external glob rules, and optional strict_zones mode."
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(
        with = "FrameworkProfileRuleFile",
        description = "Global Framework profile selection: false to disable all, true for auto-detection, a list of profile names (or {\"only\": [...]}), or {\"exclude\": [...]}. Omitted uses automatic detection."
    )]
    framework_profiles: Option<FrameworkProfileRuleFile>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    #[schemars(
        description = "Per-Workspace-package configuration overrides keyed by repository-relative package root (or \".\" for the repository root)."
    )]
    packages: BTreeMap<String, PackageConfigFile>,
    #[serde(default, skip_serializing_if = "crate::ratchet::is_ratchet_unset")]
    #[schemars(
        description = "Ratchet settings, including an optional repository-relative path for the Ratchet file (.codeint/ratchet.json by default)."
    )]
    ratchet: RatchetConfigFile,
}

pub(super) struct ConfigurationFileSections {
    pub(super) new_file_budgets: NewFileBudgetsFile,
    pub(super) regression_limits: RegressionLimitsFile,
    pub(super) languages: Option<Vec<LanguageGlobs>>,
    pub(super) plugins: Vec<PluginEntry>,
    pub(super) boundaries: Option<BoundariesFile>,
    pub(super) dependencies: DependenciesFile,
    pub(super) duplication: DuplicationConfigFile,
    pub(super) sql: SqlConfigFile,
    pub(super) hazards: HazardsConfigFile,
    pub(super) framework_profiles: Option<FrameworkProfileRuleFile>,
    pub(super) packages: BTreeMap<String, PackageConfigFile>,
    pub(super) ratchet: RatchetConfigFile,
}

impl ConfigurationFile {
    pub(super) fn starter(
        schema_address: String,
        languages: Option<Vec<LanguageGlobs>>,
    ) -> ConfigurationFile {
        ConfigurationFile {
            _schema: Some(schema_address),
            languages,
            ..Default::default()
        }
    }

    pub(super) fn adoption_starter(
        schema_address: String,
        languages: Option<Vec<LanguageGlobs>>,
    ) -> ConfigurationFile {
        let mut starter = ConfigurationFile::starter(schema_address, languages);
        starter.regression_limits.per_file = Some(true);
        starter
    }

    pub(super) fn without_schema_address(&self) -> ConfigurationFile {
        ConfigurationFile {
            _schema: None,
            format: self.format.filter(|&format| format != OutputFormat::Toon),
            regression_limits: self.regression_limits.clone().normalized(),
            duplication: self.duplication.clone().normalized(),
            ..self.clone()
        }
    }

    pub(super) fn format(&self) -> Option<OutputFormat> {
        self.format
    }

    pub(super) fn into_sections(self) -> ConfigurationFileSections {
        ConfigurationFileSections {
            new_file_budgets: self.new_file_budgets,
            regression_limits: self.regression_limits,
            languages: self.languages,
            plugins: self.plugins,
            boundaries: self.boundaries,
            dependencies: self.dependencies,
            duplication: self.duplication,
            sql: self.sql,
            hazards: self.hazards,
            framework_profiles: self.framework_profiles,
            packages: self.packages,
            ratchet: self.ratchet,
        }
    }

    pub(super) fn source_sets_mut(&mut self) -> &mut SourceSetGlobs {
        &mut self.source_sets
    }

    pub(super) fn budgets_mut(&mut self) -> &mut SourceSetBudgets {
        &mut self.budgets
    }

    pub(super) fn languages(&self) -> &Option<Vec<LanguageGlobs>> {
        &self.languages
    }

    pub(super) fn plugins(&self) -> &[PluginEntry] {
        &self.plugins
    }

    pub(super) fn plugin_memory_mb(&self) -> Option<u64> {
        self.plugin_limits.memory_mb
    }

    pub(super) fn plugin_timeout_ms(&self) -> Option<u64> {
        self.plugin_limits.timeout_ms
    }

    pub(super) fn set_plugin_memory_mb(&mut self, value: u64) {
        self.plugin_limits.memory_mb = Some(value);
    }

    pub(super) fn set_plugin_timeout_ms(&mut self, value: u64) {
        self.plugin_limits.timeout_ms = Some(value);
    }

    pub(super) fn entry_points(&self) -> &[String] {
        &self.entry_points
    }

    pub(super) fn coverage_reports(&self) -> &[String] {
        &self.coverage_reports
    }
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct DuplicationConfigFile {
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

    pub(super) fn into_parts(self) -> (Option<bool>, Option<f64>) {
        (self.near_clones, self.min_similarity)
    }
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct PluginLimitsFile {
    #[schemars(
        with = "u64",
        range(min = 1, max = 4096),
        description = "Maximum linear memory in MiB that a WebAssembly Plugin may allocate; 64 by default."
    )]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    memory_mb: Option<u64>,
    #[schemars(
        with = "u64",
        range(min = 1, max = 300000),
        description = "Maximum wall-clock time in milliseconds for each WebAssembly Plugin call; 5000 by default."
    )]
    #[serde(default, skip_serializing_if = "Option::is_none")]
    timeout_ms: Option<u64>,
}

impl PluginLimitsFile {
    fn is_unset(&self) -> bool {
        self.memory_mb.is_none() && self.timeout_ms.is_none()
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct PluginEntry {
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

impl PluginEntry {
    pub(super) fn into_parts(self) -> (String, PluginKind, Option<String>, Option<String>, String) {
        (self.name, self.kind, self.path, self.url, self.sha256)
    }
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, JsonSchema)]
pub(super) struct RegressionLimitsFile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(
        with = "bool",
        description = "When true, codeint diff compares file-level Metric numerators and Findings for every modified file and reports a Regression if any modified file worsens, even when another file shrank; false by default so moving an unchanged function between files passes."
    )]
    per_file: Option<bool>,
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

fn dependency_metric_pairs<T>(
    unused: T,
    unlisted: T,
    dev_in_production: T,
) -> impl Iterator<Item = (String, T)> {
    [
        (Metric::UnusedDependencies.key(), unused),
        (Metric::UnlistedDependencies.key(), unlisted),
        (Metric::DevInProductionDependencies.key(), dev_in_production),
    ]
    .into_iter()
    .map(|(key, value)| (key.to_owned(), value))
}

impl RegressionLimitsFile {
    fn normalized(mut self) -> RegressionLimitsFile {
        self.per_file = self.per_file.filter(|&enabled| enabled);
        self
    }

    pub(super) fn into_regression_limits(self) -> RegressionLimits {
        let RegressionLimitsFile {
            per_file,
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
        } = self;
        let dependency_limits = dependency_metric_pairs(
            unused_dependencies,
            unlisted_dependencies,
            dev_in_production_dependencies,
        )
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
        RegressionLimits::new(
            per_file.unwrap_or(false),
            every_source_set,
            production_limits.chain(other_limits).collect(),
        )
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

macro_rules! source_set_budgets_struct {
    (
        $name:ident,
        $prod_desc:literal,
        $test_desc:literal,
        $gen_desc:literal,
        $vend_desc:literal
        $(, $(#[$extra_meta:meta])* $extra_field:ident : $extra_ty:ty )* $(,)?
    ) => {
        #[derive(Debug, Clone, Default, Deserialize, Serialize, JsonSchema)]
        #[serde(deny_unknown_fields)]
        pub(super) struct $name {
            #[serde(default)]
            #[schemars(description = $prod_desc)]
            production: ProductionMetricBudgets,
            #[serde(default)]
            #[schemars(description = $test_desc)]
            test: MetricBudgets,
            #[serde(default)]
            #[schemars(description = $gen_desc)]
            generated: MetricBudgets,
            #[serde(default)]
            #[schemars(description = $vend_desc)]
            vendored: MetricBudgets,
            $(
                $(#[$extra_meta])*
                $extra_field: $extra_ty,
            )*
        }
    };
}

source_set_budgets_struct!(
    SourceSetBudgets,
    "The Budgets of the Production Source set.",
    "The Budgets of the Test Source set.",
    "The Budgets of the Generated Source set.",
    "The Budgets of the Vendored Source set.",
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
);

source_set_budgets_struct!(
    NewFileBudgetsFile,
    "The new-file Budgets of the Production Source set.",
    "The new-file Budgets of the Test Source set.",
    "The new-file Budgets of the Generated Source set.",
    "The new-file Budgets of the Vendored Source set.",
);

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

impl NewFileBudgetsFile {
    pub(super) fn is_unset(&self) -> bool {
        self.production.is_unset()
            && self.test.is_unset()
            && self.generated.is_unset()
            && self.vendored.is_unset()
    }

    pub(super) fn into_entries(self) -> Vec<(SourceSet, String, Budget)> {
        source_set_budget_entries(self.production, self.test, self.generated, self.vendored)
    }
}

pub(super) struct BudgetEntries {
    pub(super) set: Vec<(SourceSet, String, Budget)>,
    pub(super) dependency_set: Vec<(String, Budget)>,
}

fn source_set_budget_entries(
    production: ProductionMetricBudgets,
    test: MetricBudgets,
    generated: MetricBudgets,
    vendored: MetricBudgets,
) -> Vec<(SourceSet, String, Budget)> {
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
                .map(|ratio| (Metric::TestGap.key().to_owned(), ratio)),
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
    production_entries.chain(other_entries).collect()
}

impl SourceSetBudgets {
    pub(super) fn set(&mut self, source_set: SourceSet, metric: &str, budget: Budget) {
        match source_set {
            SourceSet::Production => self.production.set(metric, budget),
            SourceSet::Test => self.test.set(metric, budget),
            SourceSet::Generated => self.generated.set(metric, budget),
            SourceSet::Vendored => self.vendored.set(metric, budget),
        }
    }

    pub(super) fn into_entries(self) -> BudgetEntries {
        let SourceSetBudgets {
            production,
            test,
            generated,
            vendored,
            unused_dependencies,
            unlisted_dependencies,
            dev_in_production_dependencies,
        } = self;
        let dependency_set = dependency_metric_pairs(
            unused_dependencies,
            unlisted_dependencies,
            dev_in_production_dependencies,
        )
        .filter_map(|(metric, ratio)| Some((metric, ratio?)))
        .collect();
        BudgetEntries {
            set: source_set_budget_entries(production, test, generated, vendored),
            dependency_set,
        }
    }
}

impl SharedMetricBudgets {
    fn is_unset(&self) -> bool {
        [
            self.erosion,
            self.cognitive_complexity,
            self.duplication,
            self.verbosity,
            self.import_cycles,
            self.boundaries,
            self.hazards,
        ]
        .into_iter()
        .all(|entry| entry.is_none())
    }

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
    fn is_unset(&self) -> bool {
        self.common.is_unset()
            && self.orphan_files.is_none()
            && self.test_gap.is_none()
            && self.external.is_empty()
    }

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
    fn is_unset(&self) -> bool {
        self.common.is_unset() && self.external.is_empty()
    }

    fn set(&mut self, metric: &str, budget: Budget) {
        if !self.common.set_known(metric, budget) {
            self.external.insert(metric.to_owned(), budget);
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct LanguageGlobs {
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

impl LanguageGlobs {
    pub(super) fn new(plugin: String, include: Vec<String>) -> LanguageGlobs {
        LanguageGlobs {
            plugin,
            include,
            exclude: Vec::new(),
        }
    }

    pub(super) fn into_parts(self) -> (String, Vec<String>, Vec<String>) {
        (self.plugin, self.include, self.exclude)
    }
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct SourceSetGlobs {
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

impl SourceSetGlobs {
    pub(super) fn slot_mut(&mut self, source_set: SourceSet) -> &mut Vec<String> {
        match source_set {
            SourceSet::Production => &mut self.production,
            SourceSet::Test => &mut self.test,
            SourceSet::Generated => &mut self.generated,
            SourceSet::Vendored => &mut self.vendored,
        }
    }

    pub(super) fn into_by_precedence(self) -> [(SourceSet, &'static str, Vec<String>); 4] {
        [
            (SourceSet::Vendored, "vendored", self.vendored),
            (SourceSet::Generated, "generated", self.generated),
            (SourceSet::Test, "test", self.test),
            (SourceSet::Production, "production", self.production),
        ]
    }
}
