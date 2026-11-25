use std::collections::BTreeMap;
use std::fmt;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use codeint_model::{Metric, SourceSet};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::config_error::{
    ConfigFileKind, ConfigurationFileError, KeyPath, contains_parent_traversal,
    is_absolute_repo_path,
};

const RATCHET_DEPENDENCY_KEYS: [&str; 3] = [
    Metric::UnusedDependencies.key(),
    Metric::UnlistedDependencies.key(),
    Metric::DevInProductionDependencies.key(),
];

fn is_broken_symlink(path: &Path) -> bool {
    std::fs::symlink_metadata(path)
        .is_ok_and(|meta| meta.file_type().is_symlink() && std::fs::metadata(path).is_err())
}

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

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RatchetSourceSets {
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub production: BTreeMap<String, RatchetEntry>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub test: BTreeMap<String, RatchetEntry>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub generated: BTreeMap<String, RatchetEntry>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub vendored: BTreeMap<String, RatchetEntry>,
}

impl RatchetSourceSets {
    pub fn get(&self, source_set: SourceSet) -> &BTreeMap<String, RatchetEntry> {
        match source_set {
            SourceSet::Production => &self.production,
            SourceSet::Test => &self.test,
            SourceSet::Generated => &self.generated,
            SourceSet::Vendored => &self.vendored,
        }
    }

    pub fn get_mut(&mut self, source_set: SourceSet) -> &mut BTreeMap<String, RatchetEntry> {
        match source_set {
            SourceSet::Production => &mut self.production,
            SourceSet::Test => &mut self.test,
            SourceSet::Generated => &mut self.generated,
            SourceSet::Vendored => &mut self.vendored,
        }
    }

    pub fn iter(&self) -> [(SourceSet, &BTreeMap<String, RatchetEntry>); 4] {
        [
            (SourceSet::Production, &self.production),
            (SourceSet::Test, &self.test),
            (SourceSet::Generated, &self.generated),
            (SourceSet::Vendored, &self.vendored),
        ]
    }
}

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
        match self.source_set {
            Some(source_set) => format!("{}.{}", source_set.key(), self.metric),
            None => self.metric.clone(),
        }
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RatchetVersionMismatch {
    pub source_set: Option<SourceSet>,
    pub metric: String,
    pub ratchet_version: u32,
    pub current_version: u32,
}

impl RatchetVersionMismatch {
    pub fn label(&self) -> String {
        match self.source_set {
            Some(source_set) => format!("{}.{}", source_set.key(), self.metric),
            None => self.metric.clone(),
        }
    }
}

impl fmt::Display for RatchetVersionMismatch {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{} (ratchet Metric version {}, current Metric version {})",
            self.label(),
            self.ratchet_version,
            self.current_version
        )
    }
}

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

#[derive(Debug, thiserror::Error)]
pub enum RatchetFileError {
    #[error("Ratchet file {path:?} is outside the Git repository")]
    OutsideRepository { path: String },
    #[error("Ratchet file {path:?} is a broken symlink")]
    BrokenSymlink { path: String },
    #[error("cannot read Ratchet file {path:?}")]
    Unreadable {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("Ratchet file {path:?} is not valid JSON")]
    InvalidJson {
        path: String,
        #[source]
        source: serde_json::Error,
    },
    #[error(
        "Ratchet file {path:?} has unsupported schema_version {found}; expected {RATCHET_SCHEMA_VERSION}"
    )]
    UnsupportedSchemaVersion { path: String, found: u32 },
    #[error(
        "Ratchet file {path:?} has invalid ceiling {ceiling} for {metric}; ceiling must be between 0 and 1"
    )]
    InvalidCeiling {
        path: String,
        metric: String,
        ceiling: f64,
    },
    #[error(
        "Ratchet file {path:?} has invalid metric_version 0 for {metric}; Metric version must be at least 1"
    )]
    InvalidMetricVersion { path: String, metric: String },
    #[error(
        "Ratchet file {path:?} lists unknown dependency Metric {metric:?}; allowed keys are {}",
        RATCHET_DEPENDENCY_KEYS.join(", ")
    )]
    UnknownDependencyMetric { path: String, metric: String },
    #[error("cannot write Ratchet file {}", path.display())]
    Write {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

impl Ratchet {
    pub fn exists(repository_root: &Path, relative_path: &str) -> Result<bool, RatchetFileError> {
        let full = resolve_ratchet_path(repository_root, relative_path)?;
        match std::fs::symlink_metadata(&full) {
            Ok(_) => {
                if is_broken_symlink(&full) {
                    return Err(RatchetFileError::BrokenSymlink {
                        path: relative_path.to_owned(),
                    });
                }
                ensure_canonical_within(repository_root, &full, relative_path)?;
                Ok(true)
            }
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::NotFound | std::io::ErrorKind::NotADirectory
                ) =>
            {
                Ok(false)
            }
            Err(source) => Err(RatchetFileError::Unreadable {
                path: relative_path.to_owned(),
                source,
            }),
        }
    }

    pub fn load_optional(
        repository_root: &Path,
        relative_path: &str,
    ) -> Result<Option<Ratchet>, RatchetFileError> {
        if !Ratchet::exists(repository_root, relative_path)? {
            return Ok(None);
        }
        let full = resolve_ratchet_path(repository_root, relative_path)?;
        let canonical = ensure_canonical_within(repository_root, &full, relative_path)?;
        let text =
            std::fs::read_to_string(&canonical).map_err(|source| RatchetFileError::Unreadable {
                path: relative_path.to_owned(),
                source,
            })?;
        let ratchet = Ratchet::parse(&text, relative_path)?;
        Ok(Some(ratchet))
    }

    pub fn parse(text: &str, path_label: &str) -> Result<Ratchet, RatchetFileError> {
        let ratchet: Ratchet =
            serde_json::from_str(text).map_err(|source| RatchetFileError::InvalidJson {
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
        let full = resolve_ratchet_path(repository_root, relative_path)?;
        let parent = prepare_write_parent(repository_root, &full, relative_path)?;
        let rendered = format!(
            "{}\n",
            serde_json::to_string_pretty(self).expect("Ratchet serializes to JSON")
        );
        write_temp_and_rename(parent, &full, rendered.as_bytes())?;
        Ok(full)
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
                mismatches.push(RatchetVersionMismatch {
                    source_set: item.source_set,
                    metric: item.metric.clone(),
                    ratchet_version: stored.metric_version,
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
            return Err(RatchetFileError::UnsupportedSchemaVersion {
                path: path_label.to_owned(),
                found: self.schema_version,
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
                return Err(RatchetFileError::UnknownDependencyMetric {
                    path: path_label.to_owned(),
                    metric: metric.clone(),
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
        return Err(RatchetFileError::InvalidCeiling {
            path: path_label.to_owned(),
            metric: metric_label.to_owned(),
            ceiling: entry.ceiling,
        });
    }
    if entry.metric_version == 0 {
        return Err(RatchetFileError::InvalidMetricVersion {
            path: path_label.to_owned(),
            metric: metric_label.to_owned(),
        });
    }
    Ok(())
}

fn resolve_ratchet_path(
    repository_root: &Path,
    relative_path: &str,
) -> Result<PathBuf, RatchetFileError> {
    if relative_path.trim().is_empty()
        || is_absolute_repo_path(relative_path)
        || contains_parent_traversal(relative_path)
    {
        return Err(RatchetFileError::OutsideRepository {
            path: relative_path.to_owned(),
        });
    }
    let full = repository_root.join(relative_path);
    ensure_existing_ancestors_within(repository_root, &full, relative_path)?;
    Ok(full)
}

fn ensure_existing_ancestors_within(
    repository_root: &Path,
    target: &Path,
    relative_path: &str,
) -> Result<(), RatchetFileError> {
    let canonical_root =
        std::fs::canonicalize(repository_root).map_err(|source| RatchetFileError::Unreadable {
            path: relative_path.to_owned(),
            source,
        })?;
    let mut cursor = target.parent();
    while let Some(ancestor) = cursor {
        if ancestor == repository_root {
            return Ok(());
        }
        if std::fs::symlink_metadata(ancestor).is_ok() {
            let canonical_ancestor =
                std::fs::canonicalize(ancestor).map_err(|source| RatchetFileError::Unreadable {
                    path: relative_path.to_owned(),
                    source,
                })?;
            if !canonical_ancestor.starts_with(&canonical_root) {
                return Err(RatchetFileError::OutsideRepository {
                    path: relative_path.to_owned(),
                });
            }
            return Ok(());
        }
        cursor = ancestor.parent();
    }
    Ok(())
}

fn ensure_canonical_within(
    repository_root: &Path,
    path: &Path,
    relative_path: &str,
) -> Result<PathBuf, RatchetFileError> {
    let canonical_root =
        std::fs::canonicalize(repository_root).map_err(|source| RatchetFileError::Unreadable {
            path: relative_path.to_owned(),
            source,
        })?;
    let canonical_path =
        std::fs::canonicalize(path).map_err(|source| RatchetFileError::Unreadable {
            path: relative_path.to_owned(),
            source,
        })?;
    if !canonical_path.starts_with(&canonical_root) {
        return Err(RatchetFileError::OutsideRepository {
            path: relative_path.to_owned(),
        });
    }
    Ok(canonical_path)
}

fn prepare_write_parent<'a>(
    repository_root: &Path,
    full: &'a Path,
    relative_path: &str,
) -> Result<&'a Path, RatchetFileError> {
    if std::fs::symlink_metadata(full).is_ok() {
        if is_broken_symlink(full) {
            return Err(RatchetFileError::BrokenSymlink {
                path: relative_path.to_owned(),
            });
        }
        ensure_canonical_within(repository_root, full, relative_path)?;
    }
    let parent = full
        .parent()
        .ok_or_else(|| RatchetFileError::OutsideRepository {
            path: relative_path.to_owned(),
        })?;
    std::fs::create_dir_all(parent).map_err(|source| RatchetFileError::Write {
        path: full.to_path_buf(),
        source,
    })?;
    ensure_canonical_within(repository_root, parent, relative_path)?;
    Ok(parent)
}

fn write_temp_and_rename(
    parent: &Path,
    destination: &Path,
    bytes: &[u8],
) -> Result<(), RatchetFileError> {
    let temp_path = unique_temp_path(parent);
    let result = (|| -> Result<(), std::io::Error> {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp_path)?;
        file.write_all(bytes)?;
        file.flush()?;
        file.sync_all()?;
        std::fs::rename(&temp_path, destination)
    })();
    if let Err(source) = result {
        let _ = std::fs::remove_file(&temp_path);
        return Err(RatchetFileError::Write {
            path: destination.to_path_buf(),
            source,
        });
    }
    Ok(())
}

fn unique_temp_path(parent: &Path) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos());
    parent.join(format!(".ratchet.{}.{nanos}.tmp", std::process::id()))
}
