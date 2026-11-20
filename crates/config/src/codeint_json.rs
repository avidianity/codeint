use std::fmt;
use std::path::Path;

use codeint_model::{Metric, RepoPath, SourceSet};

use crate::RegressionLimits;
use crate::regression_limits::RegressionLimitSetting;
use globset::{GlobBuilder, GlobSet, GlobSetBuilder};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

mod shape;
mod starter;

pub use starter::{
    DetectedLanguage, StarterConfigurationError, published_schema_address,
    write_starter_configuration,
};

pub const CONFIGURATION_FILE_NAME: &str = "codeint.json";

#[derive(Debug, Default)]
pub struct Configuration {
    pub source_set_overrides: SourceSetOverrides,
    pub budgets: Budgets,
    pub languages: Languages,
    pub regression_limits: RegressionLimits,
}

#[derive(Debug, Default)]
pub struct Budgets {
    set: Vec<(SourceSet, Metric, Budget)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Deserialize, Serialize)]
#[serde(transparent)]
pub struct Budget(f64);

impl Budgets {
    pub fn budget(&self, source_set: SourceSet, metric: Metric) -> Option<Budget> {
        self.set
            .iter()
            .find(|(set, measured, _)| *set == source_set && *measured == metric)
            .map(|(_, _, budget)| *budget)
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JsonType {
    Object,
    Array,
    String,
    Number,
    Integer,
    Boolean,
    Null,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeySegment {
    Key(String),
    Index(usize),
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct KeyPath(Vec<KeySegment>);

#[derive(Debug, thiserror::Error)]
pub enum ConfigurationFileError {
    #[error("cannot read {CONFIGURATION_FILE_NAME}")]
    Unreadable(#[source] std::io::Error),
    #[error("{CONFIGURATION_FILE_NAME} is not valid JSON")]
    NotJson(#[source] serde_json::Error),
    #[error(
        "{CONFIGURATION_FILE_NAME} has an unknown key {key}; the keys allowed there are {}",
        allowed.join(", ")
    )]
    UnknownKey { key: KeyPath, allowed: Vec<String> },
    #[error(
        "{CONFIGURATION_FILE_NAME}: {key} must be {}, not {}",
        any_of(expected),
        found.with_article()
    )]
    WrongType {
        key: KeyPath,
        expected: Vec<JsonType>,
        found: JsonType,
    },
    #[error(
        "{CONFIGURATION_FILE_NAME}: {key} must be between {minimum} and {maximum}, not {found}"
    )]
    OutOfRange {
        key: KeyPath,
        minimum: f64,
        maximum: f64,
        found: f64,
    },
    #[error(
        "{CONFIGURATION_FILE_NAME}: {key} must be one of {}, not {found}",
        allowed.join(", ")
    )]
    NotOneOf {
        key: KeyPath,
        allowed: Vec<String>,
        found: Value,
    },
    #[error("{CONFIGURATION_FILE_NAME}: {key} needs the key {missing}")]
    MissingKey { key: KeyPath, missing: String },
    #[error(
        "{CONFIGURATION_FILE_NAME} names an unknown Language plugin {name} at {key}; the Language plugins are {}",
        known.join(", ")
    )]
    UnknownLanguagePlugin {
        key: KeyPath,
        name: String,
        known: Vec<String>,
    },
    #[error("{CONFIGURATION_FILE_NAME}: {key} is not a valid glob")]
    InvalidGlob {
        key: KeyPath,
        #[source]
        source: globset::Error,
    },
}

#[derive(Deserialize, Serialize, JsonSchema)]
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
}

#[derive(Default, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct RegressionLimitsFile {
    #[serde(default, skip_serializing_if = "RegressionLimitSetting::is_unset")]
    #[schemars(description = "The Regression limit of Erosion in every Source set.")]
    erosion: RegressionLimitSetting,
    #[serde(default, skip_serializing_if = "RegressionLimitSetting::is_unset")]
    #[schemars(description = "The Regression limit of Duplication in every Source set.")]
    duplication: RegressionLimitSetting,
    #[serde(default, skip_serializing_if = "RegressionLimitSetting::is_unset")]
    #[schemars(description = "The Regression limit of Verbosity in every Source set.")]
    verbosity: RegressionLimitSetting,
    #[serde(default, skip_serializing_if = "RegressionLimitSetting::is_unset")]
    #[schemars(description = "The Regression limit of Import cycles in every Source set.")]
    import_cycles: RegressionLimitSetting,
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
}

#[derive(Default, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct MetricRegressionLimits {
    #[serde(default, skip_serializing_if = "RegressionLimitSetting::is_unset")]
    #[schemars(description = "The Regression limit of Erosion in this Source set.")]
    erosion: RegressionLimitSetting,
    #[serde(default, skip_serializing_if = "RegressionLimitSetting::is_unset")]
    #[schemars(description = "The Regression limit of Duplication in this Source set.")]
    duplication: RegressionLimitSetting,
    #[serde(default, skip_serializing_if = "RegressionLimitSetting::is_unset")]
    #[schemars(description = "The Regression limit of Verbosity in this Source set.")]
    verbosity: RegressionLimitSetting,
    #[serde(default, skip_serializing_if = "RegressionLimitSetting::is_unset")]
    #[schemars(description = "The Regression limit of Import cycles in this Source set.")]
    import_cycles: RegressionLimitSetting,
}

impl MetricRegressionLimits {
    fn is_unset(&self) -> bool {
        [
            &self.erosion,
            &self.duplication,
            &self.verbosity,
            &self.import_cycles,
        ]
        .into_iter()
        .all(RegressionLimitSetting::is_unset)
    }

    fn by_metric(self) -> impl Iterator<Item = (Metric, RegressionLimitSetting)> {
        let MetricRegressionLimits {
            erosion,
            duplication,
            verbosity,
            import_cycles,
        } = self;
        [
            (Metric::Erosion, erosion),
            (Metric::Duplication, duplication),
            (Metric::Verbosity, verbosity),
            (Metric::ImportCycles, import_cycles),
        ]
        .into_iter()
        .filter(|(_, setting)| !setting.is_unset())
    }
}

#[derive(Default, Deserialize, Serialize, JsonSchema)]
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
}

#[derive(Default, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
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
}

#[derive(Deserialize, Serialize, JsonSchema)]
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

#[derive(Default, Deserialize, Serialize, JsonSchema)]
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

fn parse(text: &str) -> Result<Configuration, ConfigurationFileError> {
    let value: Value = serde_json::from_str(text).map_err(ConfigurationFileError::NotJson)?;
    let schema = schemars::schema_for!(ConfigurationFile);
    shape::check_shape(&value, schema.as_value())?;
    let file: ConfigurationFile =
        serde_json::from_value(value).map_err(ConfigurationFileError::NotJson)?;
    Ok(Configuration {
        source_set_overrides: source_set_overrides(file.source_sets)?,
        budgets: budgets(file.budgets),
        languages: languages(file.languages)?,
        regression_limits: regression_limits(file.regression_limits),
    })
}

fn regression_limits(file: RegressionLimitsFile) -> RegressionLimits {
    let RegressionLimitsFile {
        erosion,
        duplication,
        verbosity,
        import_cycles,
        production,
        test,
        generated,
        vendored,
    } = file;
    let every_source_set = MetricRegressionLimits {
        erosion,
        duplication,
        verbosity,
        import_cycles,
    }
    .by_metric()
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

fn budgets(file: SourceSetBudgets) -> Budgets {
    let SourceSetBudgets {
        production,
        test,
        generated,
        vendored,
    } = file;
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
            duplication,
            verbosity,
            import_cycles,
        } = metric_budgets;
        [
            (Metric::Erosion, erosion),
            (Metric::Duplication, duplication),
            (Metric::Verbosity, verbosity),
            (Metric::ImportCycles, import_cycles),
        ]
        .into_iter()
        .filter_map(move |(metric, ratio)| Some((source_set, metric, ratio?)))
    })
    .collect();
    Budgets { set }
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

fn glob_set(key: &KeyPath, globs: &[String]) -> Result<GlobSet, ConfigurationFileError> {
    let mut builder = GlobSetBuilder::new();
    for (index, glob) in globs.iter().enumerate() {
        let compiled = GlobBuilder::new(glob)
            .literal_separator(true)
            .backslash_escape(true)
            .build()
            .map_err(|source| ConfigurationFileError::InvalidGlob {
                key: key.child(KeySegment::Index(index)),
                source,
            })?;
        builder.add(compiled);
    }
    builder
        .build()
        .map_err(|source| ConfigurationFileError::InvalidGlob {
            key: key.clone(),
            source,
        })
}

impl KeyPath {
    fn child(&self, segment: KeySegment) -> KeyPath {
        let mut segments = self.0.clone();
        segments.push(segment);
        KeyPath(segments)
    }
}

impl fmt::Display for KeyPath {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (position, segment) in self.0.iter().enumerate() {
            match segment {
                KeySegment::Key(key) if position == 0 => write!(formatter, "{key}")?,
                KeySegment::Key(key) => write!(formatter, ".{key}")?,
                KeySegment::Index(index) => write!(formatter, "[{index}]")?,
            }
        }
        Ok(())
    }
}

impl JsonType {
    fn of(value: &Value) -> JsonType {
        match value {
            Value::Object(_) => JsonType::Object,
            Value::Array(_) => JsonType::Array,
            Value::String(_) => JsonType::String,
            Value::Number(number) if number.is_i64() || number.is_u64() => JsonType::Integer,
            Value::Number(_) => JsonType::Number,
            Value::Bool(_) => JsonType::Boolean,
            Value::Null => JsonType::Null,
        }
    }

    fn from_schema_name(name: &str) -> Option<JsonType> {
        match name {
            "object" => Some(JsonType::Object),
            "array" => Some(JsonType::Array),
            "string" => Some(JsonType::String),
            "number" => Some(JsonType::Number),
            "integer" => Some(JsonType::Integer),
            "boolean" => Some(JsonType::Boolean),
            "null" => Some(JsonType::Null),
            _ => None,
        }
    }

    fn accepts(self, value: &Value) -> bool {
        let found = JsonType::of(value);
        found == self || (self == JsonType::Number && found == JsonType::Integer)
    }

    fn with_article(self) -> &'static str {
        match self {
            JsonType::Object => "an object",
            JsonType::Array => "an array",
            JsonType::String => "a string",
            JsonType::Number => "a number",
            JsonType::Integer => "an integer",
            JsonType::Boolean => "a boolean",
            JsonType::Null => "null",
        }
    }
}

fn any_of(types: &[JsonType]) -> String {
    types
        .iter()
        .map(|json_type| json_type.with_article())
        .collect::<Vec<_>>()
        .join(" or ")
}
