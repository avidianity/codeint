use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use codeint_config::{
    ConfigFileKind, Configuration, ConfigurationFileError, PluginHostConfig, PluginSource,
    read_configuration,
};
use codeint_plugin_host::sha256;

use crate::check::{CheckError, LoadedPlugins};

const MAX_CACHED_WORKSPACES: usize = 16;

#[derive(Debug, Clone, PartialEq, Eq)]
enum FileFingerprint {
    Missing,
    Present {
        len: u64,
        modified: Option<SystemTime>,
        digest: [u8; 32],
    },
    Unreadable {
        is_symlink: bool,
        target: Option<PathBuf>,
    },
}

impl FileFingerprint {
    fn capture(path: &Path) -> FileFingerprint {
        let sym_meta = match std::fs::symlink_metadata(path) {
            Ok(meta) => meta,
            Err(err)
                if matches!(
                    err.kind(),
                    std::io::ErrorKind::NotFound | std::io::ErrorKind::NotADirectory
                ) =>
            {
                return FileFingerprint::Missing;
            }
            Err(_) => {
                return FileFingerprint::Unreadable {
                    is_symlink: false,
                    target: None,
                };
            }
        };
        match (std::fs::metadata(path), std::fs::read(path)) {
            (Ok(meta), Ok(bytes)) => FileFingerprint::Present {
                len: meta.len(),
                modified: meta.modified().ok(),
                digest: sha256(&bytes),
            },
            _ => FileFingerprint::Unreadable {
                is_symlink: sym_meta.file_type().is_symlink(),
                target: std::fs::read_link(path).ok(),
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ConfigFingerprint([FileFingerprint; ConfigFileKind::ALL.len()]);

impl ConfigFingerprint {
    fn capture(repo_root: &Path) -> ConfigFingerprint {
        ConfigFingerprint(
            ConfigFileKind::ALL
                .map(|kind| FileFingerprint::capture(&repo_root.join(kind.file_name()))),
        )
    }
}

struct CachedPlugins {
    plugin_host: PluginHostConfig,
    local_plugins: Vec<(PathBuf, FileFingerprint)>,
    loaded: LoadedPlugins,
}

impl CachedPlugins {
    fn matches(&self, plugin_host: &PluginHostConfig) -> bool {
        self.plugin_host == *plugin_host
            && self
                .local_plugins
                .iter()
                .all(|(path, expected)| FileFingerprint::capture(path) == *expected)
    }
}

struct CachedWorkspace {
    config_fingerprint: ConfigFingerprint,
    configuration: Configuration,
    plugins: Option<CachedPlugins>,
}

#[derive(Default)]
pub(super) struct WorkspaceCache {
    entries: HashMap<PathBuf, CachedWorkspace>,
    config_loads: usize,
    plugin_loads: usize,
}

impl WorkspaceCache {
    pub(super) fn configuration(
        &mut self,
        repo_root: &Path,
    ) -> Result<&Configuration, ConfigurationFileError> {
        let entry = ensure_entry(&mut self.entries, &mut self.config_loads, repo_root)?;
        Ok(&entry.configuration)
    }

    pub(super) fn workspace(
        &mut self,
        repo_root: &Path,
        plugin_host: &PluginHostConfig,
    ) -> Result<(&Configuration, &LoadedPlugins), CheckError> {
        let entry = ensure_entry(&mut self.entries, &mut self.config_loads, repo_root)?;
        if entry
            .plugins
            .as_ref()
            .is_some_and(|cached| !cached.matches(plugin_host))
        {
            entry.plugins = None;
        }
        let cached = match entry.plugins {
            Some(ref cached) => cached,
            None => {
                let local_plugins = capture_local_plugins(repo_root, &entry.configuration);
                let loaded = LoadedPlugins::load(repo_root, &entry.configuration, plugin_host)?;
                self.plugin_loads += 1;
                entry.plugins.insert(CachedPlugins {
                    plugin_host: plugin_host.clone(),
                    local_plugins,
                    loaded,
                })
            }
        };
        Ok((&entry.configuration, &cached.loaded))
    }

    #[cfg(test)]
    pub(super) fn config_load_count(&self) -> usize {
        self.config_loads
    }

    #[cfg(test)]
    pub(super) fn plugin_load_count(&self) -> usize {
        self.plugin_loads
    }
}

fn ensure_entry<'a>(
    entries: &'a mut HashMap<PathBuf, CachedWorkspace>,
    config_loads: &mut usize,
    repo_root: &Path,
) -> Result<&'a mut CachedWorkspace, ConfigurationFileError> {
    let key = canonical_root(repo_root);
    let fingerprint = ConfigFingerprint::capture(repo_root);
    if !entries.contains_key(&key) && entries.len() >= MAX_CACHED_WORKSPACES {
        entries.clear();
    }
    match entries.entry(key) {
        Entry::Occupied(mut occupied) => {
            if occupied.get().config_fingerprint != fingerprint {
                match read_configuration(repo_root) {
                    Ok(configuration) => {
                        *config_loads += 1;
                        occupied.insert(CachedWorkspace {
                            config_fingerprint: fingerprint,
                            configuration,
                            plugins: None,
                        });
                    }
                    Err(err) => {
                        occupied.remove();
                        return Err(err);
                    }
                }
            }
            Ok(occupied.into_mut())
        }
        Entry::Vacant(vacant) => {
            let configuration = read_configuration(repo_root)?;
            *config_loads += 1;
            Ok(vacant.insert(CachedWorkspace {
                config_fingerprint: fingerprint,
                configuration,
                plugins: None,
            }))
        }
    }
}

fn canonical_root(repo_root: &Path) -> PathBuf {
    std::fs::canonicalize(repo_root).unwrap_or_else(|_| repo_root.to_path_buf())
}

fn capture_local_plugins(
    repo_root: &Path,
    configuration: &Configuration,
) -> Vec<(PathBuf, FileFingerprint)> {
    configuration
        .plugins
        .iter()
        .filter_map(|declaration| match &declaration.source {
            PluginSource::Path(path) => {
                let resolved = if path.is_absolute() {
                    path.clone()
                } else {
                    repo_root.join(path)
                };
                let fingerprint = FileFingerprint::capture(&resolved);
                Some((resolved, fingerprint))
            }
            PluginSource::HttpsUrl(_) => None,
        })
        .collect()
}
