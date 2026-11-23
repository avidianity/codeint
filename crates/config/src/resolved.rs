use std::fmt;
use std::path::PathBuf;
use std::str::FromStr;

use codeint_model::SourceSet;
use serde::Serialize;
use serde_json::Value;

use crate::{Budget, Configuration};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ConfigProvenance {
    Default,
    File,
    Cli,
}

impl ConfigProvenance {
    pub fn label(self) -> &'static str {
        match self {
            ConfigProvenance::Default => "default",
            ConfigProvenance::File => "file",
            ConfigProvenance::Cli => "cli",
        }
    }
}

impl fmt::Display for ConfigProvenance {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.label())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ResolvedSetting {
    pub key: String,
    pub value: Value,
    pub source: ConfigProvenance,
}

#[derive(Debug)]
pub struct ResolvedConfiguration {
    pub config_file: PathBuf,
    pub config_file_present: bool,
    pub configuration: Configuration,
    pub settings: Vec<ResolvedSetting>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BudgetOverride {
    pub source_set: SourceSet,
    pub metric: String,
    pub budget: Budget,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceSetOverride {
    pub source_set: SourceSet,
    pub glob: String,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ConfigOverrides {
    pub budgets: Vec<BudgetOverride>,
    pub source_sets: Vec<SourceSetOverride>,
    pub plugin_memory_mb: Option<u64>,
    pub plugin_timeout_ms: Option<u64>,
}

impl FromStr for BudgetOverride {
    type Err = String;

    fn from_str(raw: &str) -> Result<BudgetOverride, String> {
        let (target, ratio_text) = raw.split_once('=').ok_or_else(|| {
            format!("{raw:?} must use SOURCE_SET.METRIC=RATIO, such as production.erosion=0.25")
        })?;
        let (source_set_text, metric) = target.split_once('.').ok_or_else(|| {
            format!("{target:?} must name SOURCE_SET.METRIC, such as production.erosion")
        })?;
        let source_set = parse_source_set(source_set_text)?;
        if metric.is_empty()
            || metric.trim() != metric
            || metric.contains('.')
            || metric.chars().any(char::is_whitespace)
        {
            return Err(format!("{metric:?} is not a valid Metric key in {raw:?}"));
        }
        let ratio: f64 = ratio_text
            .parse()
            .map_err(|_| format!("{ratio_text:?} is not a number between 0 and 1"))?;
        let budget = Budget::new(ratio)
            .ok_or_else(|| format!("Budget ratio must be between 0 and 1, not {ratio}"))?;
        Ok(BudgetOverride {
            source_set,
            metric: metric.to_owned(),
            budget,
        })
    }
}

impl FromStr for SourceSetOverride {
    type Err = String;

    fn from_str(raw: &str) -> Result<SourceSetOverride, String> {
        let (source_set_text, glob) = raw
            .split_once('=')
            .ok_or_else(|| format!("{raw:?} must use SOURCE_SET=GLOB, such as test=spec/**"))?;
        let source_set = parse_source_set(source_set_text)?;
        if glob.trim().is_empty() {
            return Err(format!("glob in {raw:?} must not be empty"));
        }
        crate::config_error::compile_glob(glob)
            .map_err(|error| format!("glob {glob:?} in {raw:?} is not a valid glob: {error}"))?;
        Ok(SourceSetOverride {
            source_set,
            glob: glob.to_owned(),
        })
    }
}

pub(crate) fn parse_source_set(name: &str) -> Result<SourceSet, String> {
    match name {
        "production" => Ok(SourceSet::Production),
        "test" => Ok(SourceSet::Test),
        "generated" => Ok(SourceSet::Generated),
        "vendored" => Ok(SourceSet::Vendored),
        _ => Err(format!(
            "{name:?} is not a Source set; use one of production, test, generated, vendored"
        )),
    }
}

pub(crate) fn resolve_with_provenance<T>(
    default: T,
    from_file: Option<T>,
    from_cli: Option<T>,
) -> (T, ConfigProvenance) {
    if let Some(value) = from_cli {
        return (value, ConfigProvenance::Cli);
    }
    if let Some(value) = from_file {
        return (value, ConfigProvenance::File);
    }
    (default, ConfigProvenance::Default)
}
