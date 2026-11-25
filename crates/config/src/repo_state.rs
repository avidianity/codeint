use std::collections::BTreeMap;
use std::fmt;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use codeint_model::{Metric, SourceSet};
use serde::{Deserialize, Serialize};

use crate::config_error::{contains_parent_traversal, is_absolute_repo_path};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceSetEntries<E> {
    #[serde(default = "BTreeMap::new", skip_serializing_if = "BTreeMap::is_empty")]
    pub production: BTreeMap<String, E>,
    #[serde(default = "BTreeMap::new", skip_serializing_if = "BTreeMap::is_empty")]
    pub test: BTreeMap<String, E>,
    #[serde(default = "BTreeMap::new", skip_serializing_if = "BTreeMap::is_empty")]
    pub generated: BTreeMap<String, E>,
    #[serde(default = "BTreeMap::new", skip_serializing_if = "BTreeMap::is_empty")]
    pub vendored: BTreeMap<String, E>,
}

impl<E> Default for SourceSetEntries<E> {
    fn default() -> SourceSetEntries<E> {
        SourceSetEntries {
            production: BTreeMap::new(),
            test: BTreeMap::new(),
            generated: BTreeMap::new(),
            vendored: BTreeMap::new(),
        }
    }
}

impl<E> SourceSetEntries<E> {
    pub fn get(&self, source_set: SourceSet) -> &BTreeMap<String, E> {
        match source_set {
            SourceSet::Production => &self.production,
            SourceSet::Test => &self.test,
            SourceSet::Generated => &self.generated,
            SourceSet::Vendored => &self.vendored,
        }
    }

    pub fn get_mut(&mut self, source_set: SourceSet) -> &mut BTreeMap<String, E> {
        match source_set {
            SourceSet::Production => &mut self.production,
            SourceSet::Test => &mut self.test,
            SourceSet::Generated => &mut self.generated,
            SourceSet::Vendored => &mut self.vendored,
        }
    }

    pub fn iter(&self) -> [(SourceSet, &BTreeMap<String, E>); 4] {
        [
            (SourceSet::Production, &self.production),
            (SourceSet::Test, &self.test),
            (SourceSet::Generated, &self.generated),
            (SourceSet::Vendored, &self.vendored),
        ]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepoStateFile {
    Ratchet,
    Baseline,
}

impl RepoStateFile {
    pub fn name(self) -> &'static str {
        match self {
            RepoStateFile::Ratchet => "Ratchet",
            RepoStateFile::Baseline => "Baseline",
        }
    }
}

impl fmt::Display for RepoStateFile {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.name())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum StateFileError {
    #[error("{file} file {path:?} is outside the Git repository")]
    OutsideRepository { file: RepoStateFile, path: String },
    #[error("{file} file {path:?} is a broken symlink")]
    BrokenSymlink { file: RepoStateFile, path: String },
    #[error("cannot read {file} file {path:?}")]
    Unreadable {
        file: RepoStateFile,
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("{file} file {path:?} is not valid JSON")]
    InvalidJson {
        file: RepoStateFile,
        path: String,
        #[source]
        source: serde_json::Error,
    },
    #[error("{file} file {path:?} has unsupported schema_version {found}; expected {expected}")]
    UnsupportedSchemaVersion {
        file: RepoStateFile,
        path: String,
        found: u32,
        expected: u32,
    },
    #[error(
        "{file} file {path:?} has invalid ceiling {ceiling} for {metric}; ceiling must be between 0 and 1"
    )]
    InvalidCeiling {
        file: RepoStateFile,
        path: String,
        metric: String,
        ceiling: f64,
    },
    #[error(
        "{file} file {path:?} has invalid metric_version 0 for {metric}; Metric version must be at least 1"
    )]
    InvalidMetricVersion {
        file: RepoStateFile,
        path: String,
        metric: String,
    },
    #[error(
        "{file} file {path:?} lists unknown dependency Metric {metric:?}; allowed keys are {allowed}"
    )]
    UnknownDependencyMetric {
        file: RepoStateFile,
        path: String,
        metric: String,
        allowed: String,
    },
    #[error("cannot write {file} file {}", path.display())]
    Write {
        file: RepoStateFile,
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

impl StateFileError {
    pub fn state_file(&self) -> RepoStateFile {
        match self {
            StateFileError::OutsideRepository { file, .. }
            | StateFileError::BrokenSymlink { file, .. }
            | StateFileError::Unreadable { file, .. }
            | StateFileError::InvalidJson { file, .. }
            | StateFileError::UnsupportedSchemaVersion { file, .. }
            | StateFileError::InvalidCeiling { file, .. }
            | StateFileError::InvalidMetricVersion { file, .. }
            | StateFileError::UnknownDependencyMetric { file, .. }
            | StateFileError::Write { file, .. } => *file,
        }
    }

    pub fn is_invalid_content(&self) -> bool {
        matches!(
            self,
            StateFileError::InvalidJson { .. }
                | StateFileError::UnsupportedSchemaVersion { .. }
                | StateFileError::InvalidCeiling { .. }
                | StateFileError::InvalidMetricVersion { .. }
                | StateFileError::UnknownDependencyMetric { .. }
        )
    }

    pub(crate) fn from_fault(
        file: RepoStateFile,
        relative_path: &str,
        fault: StateFileFault,
    ) -> StateFileError {
        let path = relative_path.to_owned();
        match fault {
            StateFileFault::OutsideRepository => StateFileError::OutsideRepository { file, path },
            StateFileFault::BrokenSymlink => StateFileError::BrokenSymlink { file, path },
            StateFileFault::Unreadable(source) => StateFileError::Unreadable { file, path, source },
            StateFileFault::Write { path, source } => StateFileError::Write { file, path, source },
        }
    }
}

pub(crate) const DEPENDENCY_METRIC_KEYS: [&str; 3] = [
    Metric::UnusedDependencies.key(),
    Metric::UnlistedDependencies.key(),
    Metric::DevInProductionDependencies.key(),
];

pub struct LoadedForUpdate<T> {
    pub existing: Option<T>,
    pub replaced_invalid: bool,
}

pub(crate) fn recover_invalid_for_update<T>(
    loaded: Result<Option<T>, StateFileError>,
    reset: bool,
) -> Result<LoadedForUpdate<T>, StateFileError> {
    match loaded {
        Ok(existing) => Ok(LoadedForUpdate {
            existing,
            replaced_invalid: false,
        }),
        Err(error) if reset && error.is_invalid_content() => Ok(LoadedForUpdate {
            existing: None,
            replaced_invalid: true,
        }),
        Err(error) => Err(error),
    }
}

pub(crate) fn metric_label(source_set: Option<SourceSet>, metric: &str) -> String {
    match source_set {
        Some(source_set) => format!("{}.{metric}", source_set.key()),
        None => metric.to_owned(),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MetricVersionMismatch {
    pub state_file: RepoStateFile,
    pub source_set: Option<SourceSet>,
    pub metric: String,
    pub recorded_version: u32,
    pub current_version: u32,
}

impl fmt::Display for MetricVersionMismatch {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let recorded_in = self.state_file.name().to_lowercase();
        let label = metric_label(self.source_set, &self.metric);
        formatter.write_fmt(format_args!(
            "{label} ({recorded_in} Metric version {}, current Metric version {})",
            self.recorded_version, self.current_version
        ))
    }
}

#[derive(Debug)]
pub(crate) enum StateFileFault {
    OutsideRepository,
    BrokenSymlink,
    Unreadable(std::io::Error),
    Write {
        path: PathBuf,
        source: std::io::Error,
    },
}

pub(crate) fn state_file_exists(
    repository_root: &Path,
    relative_path: &str,
) -> Result<bool, StateFileFault> {
    let full = resolve_state_path(repository_root, relative_path)?;
    match std::fs::symlink_metadata(&full) {
        Ok(_) => {
            if is_broken_symlink(&full) {
                return Err(StateFileFault::BrokenSymlink);
            }
            ensure_canonical_within(repository_root, &full)?;
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
        Err(source) => Err(StateFileFault::Unreadable(source)),
    }
}

pub(crate) fn read_state_file(
    repository_root: &Path,
    relative_path: &str,
) -> Result<Option<String>, StateFileFault> {
    if !state_file_exists(repository_root, relative_path)? {
        return Ok(None);
    }
    let full = resolve_state_path(repository_root, relative_path)?;
    let canonical = ensure_canonical_within(repository_root, &full)?;
    std::fs::read_to_string(&canonical)
        .map(Some)
        .map_err(StateFileFault::Unreadable)
}

pub(crate) fn write_state_file<T: Serialize>(
    repository_root: &Path,
    relative_path: &str,
    value: &T,
) -> Result<PathBuf, StateFileFault> {
    let full = resolve_state_path(repository_root, relative_path)?;
    let parent = prepare_write_parent(repository_root, &full)?;
    let rendered = format!(
        "{}\n",
        serde_json::to_string_pretty(value).expect("repository state serializes to JSON")
    );
    write_temp_and_rename(parent, &full, rendered.as_bytes())?;
    Ok(full)
}

fn is_broken_symlink(path: &Path) -> bool {
    std::fs::symlink_metadata(path)
        .is_ok_and(|meta| meta.file_type().is_symlink() && std::fs::metadata(path).is_err())
}

fn resolve_state_path(
    repository_root: &Path,
    relative_path: &str,
) -> Result<PathBuf, StateFileFault> {
    if relative_path.trim().is_empty()
        || is_absolute_repo_path(relative_path)
        || contains_parent_traversal(relative_path)
    {
        return Err(StateFileFault::OutsideRepository);
    }
    let full = repository_root.join(relative_path);
    ensure_existing_ancestors_within(repository_root, &full)?;
    Ok(full)
}

fn ensure_existing_ancestors_within(
    repository_root: &Path,
    target: &Path,
) -> Result<(), StateFileFault> {
    let canonical_root =
        std::fs::canonicalize(repository_root).map_err(StateFileFault::Unreadable)?;
    let mut cursor = target.parent();
    while let Some(ancestor) = cursor {
        if ancestor == repository_root {
            return Ok(());
        }
        if std::fs::symlink_metadata(ancestor).is_ok() {
            let canonical_ancestor =
                std::fs::canonicalize(ancestor).map_err(StateFileFault::Unreadable)?;
            if !canonical_ancestor.starts_with(&canonical_root) {
                return Err(StateFileFault::OutsideRepository);
            }
            return Ok(());
        }
        cursor = ancestor.parent();
    }
    Ok(())
}

fn ensure_canonical_within(repository_root: &Path, path: &Path) -> Result<PathBuf, StateFileFault> {
    let canonical_root =
        std::fs::canonicalize(repository_root).map_err(StateFileFault::Unreadable)?;
    let canonical_path = std::fs::canonicalize(path).map_err(StateFileFault::Unreadable)?;
    if !canonical_path.starts_with(&canonical_root) {
        return Err(StateFileFault::OutsideRepository);
    }
    Ok(canonical_path)
}

fn prepare_write_parent<'a>(
    repository_root: &Path,
    full: &'a Path,
) -> Result<&'a Path, StateFileFault> {
    if std::fs::symlink_metadata(full).is_ok() {
        if is_broken_symlink(full) {
            return Err(StateFileFault::BrokenSymlink);
        }
        ensure_canonical_within(repository_root, full)?;
    }
    let parent = full.parent().ok_or(StateFileFault::OutsideRepository)?;
    std::fs::create_dir_all(parent).map_err(|source| StateFileFault::Write {
        path: full.to_path_buf(),
        source,
    })?;
    ensure_canonical_within(repository_root, parent)?;
    Ok(parent)
}

fn write_temp_and_rename(
    parent: &Path,
    destination: &Path,
    bytes: &[u8],
) -> Result<(), StateFileFault> {
    let temp_path = unique_temp_path(parent, destination);
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
        return Err(StateFileFault::Write {
            path: destination.to_path_buf(),
            source,
        });
    }
    Ok(())
}

fn unique_temp_path(parent: &Path, destination: &Path) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos());
    let stem = destination
        .file_stem()
        .map_or_else(|| "state".into(), |stem| stem.to_string_lossy());
    parent.join(format!(".{stem}.{}.{nanos}.tmp", std::process::id()))
}
