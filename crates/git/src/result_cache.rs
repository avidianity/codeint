use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::CommitId;

#[derive(Debug, thiserror::Error)]
pub enum ResultCacheError {
    #[error("cannot write the cache entry {}", path.display())]
    Write {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("cannot describe the settings of the cached result")]
    Settings(#[source] serde_json::Error),
    #[error("{failure}, and cannot remove the unfinished file {}", path.display())]
    Unfinished {
        path: PathBuf,
        failure: Box<ResultCacheError>,
        #[source]
        source: std::io::Error,
    },
    #[error("cannot name the cache entry")]
    EntryName(#[source] gix::Error),
    #[error("cannot describe the result to cache")]
    Result(#[source] serde_json::Error),
}

pub struct ResultCache {
    folder: PathBuf,
}

#[derive(Serialize)]
struct EntryToStore<'entry, T> {
    commit: String,
    settings: &'entry str,
    result: &'entry T,
}

#[derive(Deserialize)]
struct StoredEntry<T> {
    commit: String,
    settings: String,
    result: T,
}

struct EntryAddress {
    path: PathBuf,
    commit: String,
    settings: String,
}

static TEMPORARY_FILES_OF_THIS_PROCESS: AtomicU64 = AtomicU64::new(0);

const MAX_ENTRY_AGE: Duration = Duration::from_secs(30 * 24 * 60 * 60);
const STALE_TEMP_AGE: Duration = Duration::from_secs(60 * 60);
const MAX_FUTURE_CLOCK_SKEW: Duration = Duration::from_secs(300);
const MAX_ENTRIES: usize = 5_000;

impl ResultCache {
    pub fn in_folder(folder: PathBuf) -> ResultCache {
        ResultCache { folder }
    }

    pub fn folder(&self) -> &Path {
        &self.folder
    }

    pub fn read<T: DeserializeOwned>(
        &self,
        commit: CommitId,
        settings: &impl Serialize,
    ) -> Option<T> {
        let address = self.address(commit, settings).ok()?;
        let bytes = std::fs::read(&address.path).ok()?;
        let stored: StoredEntry<T> = serde_json::from_slice(&bytes).ok()?;
        if stored.commit == address.commit && stored.settings == address.settings {
            touch_entry(&address.path);
            Some(stored.result)
        } else {
            None
        }
    }

    pub fn write<T: Serialize>(
        &self,
        commit: CommitId,
        settings: &impl Serialize,
        result: &T,
    ) -> Result<(), ResultCacheError> {
        let address = self.address(commit, settings)?;
        let bytes = serde_json::to_vec(&EntryToStore {
            commit: address.commit,
            settings: &address.settings,
            result,
        })
        .map_err(ResultCacheError::Result)?;
        let write_error = |path: &Path| {
            let path = path.to_path_buf();
            move |source| ResultCacheError::Write { path, source }
        };
        std::fs::create_dir_all(&self.folder).map_err(write_error(&self.folder))?;
        let temporary = self.temporary_path();
        let written = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .and_then(|mut file| file.write_all(&bytes).and_then(|()| file.sync_all()))
            .map_err(write_error(&temporary))
            .and_then(|()| {
                std::fs::rename(&temporary, &address.path).map_err(write_error(&address.path))
            });
        let Err(failure) = written else {
            self.prune(&address.path);
            return Ok(());
        };
        match std::fs::remove_file(&temporary) {
            Err(cleanup) if cleanup.kind() != std::io::ErrorKind::NotFound => {
                Err(ResultCacheError::Unfinished {
                    path: temporary,
                    failure: Box::new(failure),
                    source: cleanup,
                })
            }
            _ if address.path.is_file() => {
                self.prune(&address.path);
                Ok(())
            }
            _ => Err(failure),
        }
    }

    fn prune(&self, keep: &Path) {
        let Ok(entries) = std::fs::read_dir(&self.folder) else {
            return;
        };
        let now = SystemTime::now();
        let mut live = Vec::new();
        for entry in entries.flatten() {
            if let Some(retained) = inspect_cache_entry(&entry, keep, now) {
                live.push(retained);
            }
        }
        if live.len() <= MAX_ENTRIES {
            return;
        }
        let excess = live.len() - MAX_ENTRIES;
        let mut evictable: Vec<(PathBuf, SystemTime)> =
            live.into_iter().filter(|(path, _)| path != keep).collect();
        evictable.sort_by(|left, right| left.1.cmp(&right.1).then_with(|| left.0.cmp(&right.0)));
        for (path, _) in evictable.into_iter().take(excess) {
            let _ = std::fs::remove_file(path);
        }
    }

    fn address(
        &self,
        commit: CommitId,
        settings: &impl Serialize,
    ) -> Result<EntryAddress, ResultCacheError> {
        let settings = serde_json::to_string(settings).map_err(ResultCacheError::Settings)?;
        let mut hasher = gix::hash::hasher(gix::hash::Kind::Sha1);
        hasher.update(settings.as_bytes());
        let settings_id = hasher.try_finalize().map_err(ResultCacheError::EntryName)?;
        let commit = commit.to_string();
        Ok(EntryAddress {
            path: self.folder.join(format!("{commit}-{settings_id}.json")),
            commit,
            settings,
        })
    }

    fn temporary_path(&self) -> PathBuf {
        let since_epoch = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |duration| duration.as_nanos());
        let sequence = TEMPORARY_FILES_OF_THIS_PROCESS.fetch_add(1, Ordering::Relaxed);
        self.folder.join(format!(
            ".writing-{}-{since_epoch}-{sequence}.tmp",
            std::process::id()
        ))
    }
}

enum CacheFileKind {
    Entry,
    Temporary,
}

fn classify_cache_file(path: &Path) -> Option<CacheFileKind> {
    if is_temporary_file(path) {
        Some(CacheFileKind::Temporary)
    } else if path.extension().and_then(|ext| ext.to_str()) == Some("json") {
        Some(CacheFileKind::Entry)
    } else {
        None
    }
}

fn is_temporary_file(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.starts_with(".writing-") && name.ends_with(".tmp"))
}

fn effective_modified(modified: SystemTime, now: SystemTime) -> SystemTime {
    if modified
        .duration_since(now)
        .is_ok_and(|ahead| ahead > MAX_FUTURE_CLOCK_SKEW)
    {
        UNIX_EPOCH
    } else {
        modified
    }
}

fn is_older_than(modified: SystemTime, now: SystemTime, max_age: Duration) -> bool {
    now.duration_since(modified).is_ok_and(|age| age > max_age)
}

fn inspect_cache_entry(
    entry: &std::fs::DirEntry,
    keep: &Path,
    now: SystemTime,
) -> Option<(PathBuf, SystemTime)> {
    if !entry.file_type().is_ok_and(|file_type| file_type.is_file()) {
        return None;
    }
    let path = entry.path();
    let kind = classify_cache_file(&path)?;
    let modified = entry
        .metadata()
        .and_then(|metadata| metadata.modified())
        .ok()?;
    let effective = effective_modified(modified, now);
    match kind {
        CacheFileKind::Temporary => {
            if is_older_than(effective, now, STALE_TEMP_AGE) {
                let _ = std::fs::remove_file(&path);
            }
            None
        }
        CacheFileKind::Entry => {
            if path != keep && is_older_than(effective, now, MAX_ENTRY_AGE) {
                let _ = std::fs::remove_file(&path);
                return None;
            }
            Some((path, effective))
        }
    }
}

fn touch_entry(path: &Path) {
    if let Ok(file) = OpenOptions::new().write(true).open(path) {
        let _ = file.set_modified(SystemTime::now());
    }
}
