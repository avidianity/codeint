use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};

use codeint_model::{Metric, SourceSet};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::config_error::{
    ConfigFileKind, ConfigurationFileError, KeyPath, contains_parent_traversal,
    is_absolute_repo_path,
};
use crate::repo_state::{
    MetricVersionMismatch, RepoStateFile, SourceSetEntries, StateFileError, metric_label,
    read_state_file, state_file_exists, write_state_file,
};

const RATCHET_DEPENDENCY_KEYS: [&str; 3] = [
    Metric::UnusedDependencies.key(),
    Metric::UnlistedDependencies.key(),
    Metric::DevInProductionDependencies.key(),
];

#[cfg(test)]
mod tests;

pub const DEFAULT_RATCHET_PATH: &str = ".codeint/ratchet.json";
pub const RATCHET_SCHEMA_VERSION: u32 = 1;
const DECIMAL_SCALE: f64 = 10_000.0;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RatchetConfig {
    pub path: String,
}

impl Default for RatchetConfig {
    fn default() -> RatchetConfig {
        RatchetConfig {
            path: DEFAULT_RATCHET_PATH.to_owned(),
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct RatchetConfigFile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(
        with = "String",
        description = "Path relative to the repository root of the Ratchet file; .codeint/ratchet.json by default."
    )]
    pub(crate) path: Option<String>,
}

pub(crate) fn is_ratchet_unset(file: &RatchetConfigFile) -> bool {
    file.path.is_none()
}

impl RatchetConfig {
    pub(crate) fn from_file(
        file: RatchetConfigFile,
    ) -> Result<RatchetConfig, ConfigurationFileError> {
        let Some(path) = file.path else {
            return Ok(RatchetConfig::default());
        };
        let key = KeyPath::nested("ratchet", "path");
        validate_relative_repo_path(&path, &key)?;
        Ok(RatchetConfig { path })
    }
}

fn validate_relative_repo_path(path: &str, key: &KeyPath) -> Result<(), ConfigurationFileError> {
    if path.trim().is_empty() {
        return Err(ConfigurationFileError::EmptyRatchetPath {
            file: ConfigFileKind::Json,
            key: key.clone(),
        });
    }
    if is_absolute_repo_path(path) {
        return Err(ConfigurationFileError::AbsoluteRatchetPath {
            file: ConfigFileKind::Json,
            key: key.clone(),
            path: path.to_owned(),
        });
    }
    if contains_parent_traversal(path) {
        return Err(ConfigurationFileError::ParentTraversalRatchetPath {
            file: ConfigFileKind::Json,
            key: key.clone(),
            path: path.to_owned(),
        });
    }
    Ok(())
}

pub fn round_ratchet_ratio(ratio: f64) -> f64 {
    let rounded = (ratio * DECIMAL_SCALE).round() / DECIMAL_SCALE;
    if rounded == 0.0 { 0.0 } else { rounded }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RatchetEntry {
    pub ceiling: f64,
    pub metric_version: u32,
}

pub type RatchetSourceSets = SourceSetEntries<RatchetEntry>;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Ratchet {
    pub schema_version: u32,
    #[serde(default)]
    pub source_sets: RatchetSourceSets,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub dependencies: BTreeMap<String, RatchetEntry>,
}

impl Default for Ratchet {
    fn default() -> Ratchet {
        Ratchet {
            schema_version: RATCHET_SCHEMA_VERSION,
            source_sets: RatchetSourceSets::default(),
            dependencies: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct MeasuredRatchetMetric {
    pub source_set: Option<SourceSet>,
    pub metric: String,
    pub ratio: f64,
    pub metric_version: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RatchetViolation {
    pub source_set: Option<SourceSet>,
    pub metric: String,
    pub ratio: f64,
    pub ceiling: f64,
}

impl RatchetViolation {
    pub fn label(&self) -> String {
        metric_label(self.source_set, &self.metric)
    }
}

impl fmt::Display for RatchetViolation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{} ({} > {})",
            self.label(),
            self.ratio,
            self.ceiling
        )
    }
}

pub type RatchetVersionMismatch = MetricVersionMismatch;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RatchetUpdateSummary {
    pub path: String,
    pub created: bool,
    pub reset: bool,
    pub tightened: usize,
    pub loosened: usize,
    pub refreshed: usize,
    pub added: usize,
    pub unchanged: usize,
    pub removed: usize,
    pub total_entries: usize,
}

pub type RatchetFileError = StateFileError;

impl Ratchet {
    pub fn exists(repository_root: &Path, relative_path: &str) -> Result<bool, RatchetFileError> {
        state_file_exists(repository_root, relative_path).map_err(|fault| {
            StateFileError::from_fault(RepoStateFile::Ratchet, relative_path, fault)
        })
    }

    pub fn load_optional(
        repository_root: &Path,
        relative_path: &str,
    ) -> Result<Option<Ratchet>, RatchetFileError> {
        read_state_file(repository_root, relative_path)
            .map_err(|fault| {
                StateFileError::from_fault(RepoStateFile::Ratchet, relative_path, fault)
            })?
            .map(|text| Ratchet::parse(&text, relative_path))
            .transpose()
    }

    pub fn parse(text: &str, path_label: &str) -> Result<Ratchet, RatchetFileError> {
        let ratchet: Ratchet =
            serde_json::from_str(text).map_err(|source| StateFileError::InvalidJson {
                file: RepoStateFile::Ratchet,
                path: path_label.to_owned(),
                source,
            })?;
        ratchet.validate(path_label)?;
        Ok(ratchet)
    }

    pub fn write_atomic(
        &self,
        repository_root: &Path,
        relative_path: &str,
    ) -> Result<PathBuf, RatchetFileError> {
        write_state_file(repository_root, relative_path, self).map_err(|fault| {
            StateFileError::from_fault(RepoStateFile::Ratchet, relative_path, fault)
        })
    }

    pub fn entry(&self, source_set: Option<SourceSet>, metric: &str) -> Option<RatchetEntry> {
        match source_set {
            Some(set) => self.source_sets.get(set).get(metric).copied(),
            None => self.dependencies.get(metric).copied(),
        }
    }

    pub fn total_entries(&self) -> usize {
        self.source_sets
            .iter()
            .into_iter()
            .map(|(_, entries)| entries.len())
            .sum::<usize>()
            + self.dependencies.len()
    }

    pub fn check(
        &self,
        measured: &[MeasuredRatchetMetric],
    ) -> Result<Vec<RatchetViolation>, Vec<RatchetVersionMismatch>> {
        let mut mismatches = Vec::new();
        let mut violations = Vec::new();
        for item in measured {
            let Some(stored) = self.entry(item.source_set, &item.metric) else {
                continue;
            };
            if stored.metric_version != item.metric_version {
                mismatches.push(MetricVersionMismatch {
                    state_file: RepoStateFile::Ratchet,
                    source_set: item.source_set,
                    metric: item.metric.clone(),
                    recorded_version: stored.metric_version,
                    current_version: item.metric_version,
                });
                continue;
            }
            let current = round_ratchet_ratio(item.ratio);
            let ceiling = round_ratchet_ratio(stored.ceiling);
            if current > ceiling {
                violations.push(RatchetViolation {
                    source_set: item.source_set,
                    metric: item.metric.clone(),
                    ratio: current,
                    ceiling,
                });
            }
        }
        if !mismatches.is_empty() {
            return Err(mismatches);
        }
        Ok(violations)
    }

    pub fn update(
        existing: Option<&Ratchet>,
        measured: &[MeasuredRatchetMetric],
        reset: bool,
        path_label: &str,
    ) -> Result<(Ratchet, RatchetUpdateSummary), Vec<RatchetViolation>> {
        let mut next = Ratchet::default();
        let mut regressions = Vec::new();
        let mut tightened = 0usize;
        let mut loosened = 0usize;
        let mut refreshed = 0usize;
        let mut added = 0usize;
        let mut unchanged = 0usize;

        for item in measured {
            let current = round_ratchet_ratio(item.ratio);
            let new_entry =
                match existing.and_then(|prev| prev.entry(item.source_set, &item.metric)) {
                    None => {
                        added += 1;
                        RatchetEntry {
                            ceiling: current,
                            metric_version: item.metric_version,
                        }
                    }
                    Some(stored) if stored.metric_version != item.metric_version => {
                        refreshed += 1;
                        RatchetEntry {
                            ceiling: current,
                            metric_version: item.metric_version,
                        }
                    }
                    Some(stored) => {
                        let stored_ceiling = round_ratchet_ratio(stored.ceiling);
                        if current < stored_ceiling {
                            tightened += 1;
                            RatchetEntry {
                                ceiling: current,
                                metric_version: item.metric_version,
                            }
                        } else if current == stored_ceiling {
                            unchanged += 1;
                            RatchetEntry {
                                ceiling: stored_ceiling,
                                metric_version: item.metric_version,
                            }
                        } else if reset {
                            loosened += 1;
                            RatchetEntry {
                                ceiling: current,
                                metric_version: item.metric_version,
                            }
                        } else {
                            regressions.push(RatchetViolation {
                                source_set: item.source_set,
                                metric: item.metric.clone(),
                                ratio: current,
                                ceiling: stored_ceiling,
                            });
                            RatchetEntry {
                                ceiling: stored_ceiling,
                                metric_version: item.metric_version,
                            }
                        }
                    }
                };
            next.insert(item.source_set, item.metric.clone(), new_entry);
        }

        if !regressions.is_empty() {
            return Err(regressions);
        }

        let previous_total = existing.map_or(0, Ratchet::total_entries);
        let retained_from_existing = tightened + loosened + refreshed + unchanged;
        let removed = previous_total.saturating_sub(retained_from_existing);
        let total_entries = next.total_entries();
        let summary = RatchetUpdateSummary {
            path: path_label.to_owned(),
            created: existing.is_none(),
            reset,
            tightened,
            loosened,
            refreshed,
            added,
            unchanged,
            removed,
            total_entries,
        };
        Ok((next, summary))
    }

    fn insert(&mut self, source_set: Option<SourceSet>, metric: String, entry: RatchetEntry) {
        match source_set {
            Some(set) => {
                self.source_sets.get_mut(set).insert(metric, entry);
            }
            None => {
                self.dependencies.insert(metric, entry);
            }
        }
    }

    fn validate(&self, path_label: &str) -> Result<(), RatchetFileError> {
        if self.schema_version != RATCHET_SCHEMA_VERSION {
            return Err(StateFileError::UnsupportedSchemaVersion {
                file: RepoStateFile::Ratchet,
                path: path_label.to_owned(),
                found: self.schema_version,
                expected: RATCHET_SCHEMA_VERSION,
            });
        }
        for (source_set, entries) in self.source_sets.iter() {
            for (metric, entry) in entries {
                validate_entry(
                    path_label,
                    &format!("{}.{metric}", source_set.key()),
                    *entry,
                )?;
            }
        }
        for (metric, entry) in &self.dependencies {
            if !RATCHET_DEPENDENCY_KEYS.contains(&metric.as_str()) {
                return Err(StateFileError::UnknownDependencyMetric {
                    file: RepoStateFile::Ratchet,
                    path: path_label.to_owned(),
                    metric: metric.clone(),
                    allowed: RATCHET_DEPENDENCY_KEYS.join(", "),
                });
            }
            validate_entry(path_label, metric, *entry)?;
        }
        Ok(())
    }
}

fn validate_entry(
    path_label: &str,
    metric_label: &str,
    entry: RatchetEntry,
) -> Result<(), RatchetFileError> {
    if !entry.ceiling.is_finite() || !(0.0..=1.0).contains(&entry.ceiling) {
        return Err(StateFileError::InvalidCeiling {
            file: RepoStateFile::Ratchet,
            path: path_label.to_owned(),
            metric: metric_label.to_owned(),
            ceiling: entry.ceiling,
        });
    }
    if entry.metric_version == 0 {
        return Err(StateFileError::InvalidMetricVersion {
            file: RepoStateFile::Ratchet,
            path: path_label.to_owned(),
            metric: metric_label.to_owned(),
        });
    }
    Ok(())
}
