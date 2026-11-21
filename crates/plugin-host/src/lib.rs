#![forbid(unsafe_code)]

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use codeint_config::{PluginDeclaration, PluginKind, PluginLimits};
use codeint_model::{
    CodeModel, ExternalMetricMeasurement, FileModel, InvalidReferenceRange, LanguagePlugin,
    LowerError, Manifest, MetricPlugin, MetricPluginError, ReferenceRanges, RepoPath,
    WorkspaceError, language,
};

mod limits;
mod sha256;
mod wasm;
mod wasm_metric;

pub use sha256::{format_hex as sha256_hex, sha256};
pub use wasm::WasmLanguagePlugin;
pub use wasm_metric::WasmMetricPlugin;

#[derive(Debug, thiserror::Error)]
pub enum PluginHostError {
    #[error("cannot read Plugin {name:?} at {}", path.display())]
    Read {
        name: String,
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error(
        "SHA-256 hash mismatch for Plugin {name:?} at {}: expected {expected}, found {actual}",
        path.display()
    )]
    HashMismatch {
        name: String,
        path: PathBuf,
        expected: String,
        actual: String,
    },
    #[error("cannot compile Plugin {name:?} at {}: {reason}", path.display())]
    Compile {
        name: String,
        path: PathBuf,
        reason: String,
    },
    #[error("cannot instantiate Plugin {name:?} at {}: {reason}", path.display())]
    Instantiate {
        name: String,
        path: PathBuf,
        reason: String,
    },
    #[error("Plugin {name:?} failed during {call}: {reason}")]
    Call {
        name: String,
        call: &'static str,
        reason: String,
    },
    #[error("Plugin {name:?} exceeded its memory cap of {memory_mb} MiB")]
    MemoryLimitExceeded { name: String, memory_mb: u64 },
    #[error("Plugin {name:?} exceeded its time limit of {timeout_ms} ms")]
    TimeLimitExceeded { name: String, timeout_ms: u64 },
    #[error("Plugin {name:?} duplicates an existing Plugin name")]
    DuplicatePluginName { name: String },
    #[error("Metric plugin {name:?} declared an invalid Metric key {key:?}: {reason}")]
    InvalidMetricKey {
        name: String,
        key: String,
        reason: &'static str,
    },
    #[error("Metric plugin {name:?} declared a duplicate Metric key {key:?}")]
    DuplicateMetricKey { name: String, key: String },
}

pub fn load_language_plugin(
    repository_root: &Path,
    declaration: &PluginDeclaration,
) -> Result<WasmLanguagePlugin, PluginHostError> {
    load_language_plugin_with_limits(repository_root, declaration, PluginLimits::default())
}

pub fn load_language_plugin_with_limits(
    repository_root: &Path,
    declaration: &PluginDeclaration,
    limits: PluginLimits,
) -> Result<WasmLanguagePlugin, PluginHostError> {
    let (resolved_path, bytes) = read_verified_bytes(repository_root, declaration)?;
    WasmLanguagePlugin::from_verified_bytes(
        declaration.name.clone(),
        &resolved_path,
        &bytes,
        limits,
    )
}

pub fn load_metric_plugin(
    repository_root: &Path,
    declaration: &PluginDeclaration,
) -> Result<WasmMetricPlugin, PluginHostError> {
    load_metric_plugin_with_limits(repository_root, declaration, PluginLimits::default())
}

pub fn load_metric_plugin_with_limits(
    repository_root: &Path,
    declaration: &PluginDeclaration,
    limits: PluginLimits,
) -> Result<WasmMetricPlugin, PluginHostError> {
    let (resolved_path, bytes) = read_verified_bytes(repository_root, declaration)?;
    WasmMetricPlugin::from_verified_bytes(declaration.name.clone(), &resolved_path, &bytes, limits)
}

pub fn load_language_plugins(
    repository_root: &Path,
    declarations: &[PluginDeclaration],
    reserved_names: &[&str],
) -> Result<Vec<(String, WasmLanguagePlugin)>, PluginHostError> {
    load_language_plugins_with_limits(
        repository_root,
        declarations,
        reserved_names,
        PluginLimits::default(),
    )
}

pub fn load_language_plugins_with_limits(
    repository_root: &Path,
    declarations: &[PluginDeclaration],
    reserved_names: &[&str],
    limits: PluginLimits,
) -> Result<Vec<(String, WasmLanguagePlugin)>, PluginHostError> {
    let mut loaded = Vec::new();
    for declaration in declarations
        .iter()
        .filter(|declaration| declaration.kind == PluginKind::Language)
    {
        if reserved_names.contains(&declaration.name.as_str())
            || loaded.iter().any(|(name, _)| name == &declaration.name)
        {
            return Err(PluginHostError::DuplicatePluginName {
                name: declaration.name.clone(),
            });
        }
        let plugin = load_language_plugin_with_limits(repository_root, declaration, limits)?;
        loaded.push((declaration.name.clone(), plugin));
    }
    Ok(loaded)
}

pub fn load_metric_plugins(
    repository_root: &Path,
    declarations: &[PluginDeclaration],
    reserved_names: &[&str],
) -> Result<Vec<(String, WasmMetricPlugin)>, PluginHostError> {
    load_metric_plugins_with_limits(
        repository_root,
        declarations,
        reserved_names,
        PluginLimits::default(),
    )
}

pub fn load_metric_plugins_with_limits(
    repository_root: &Path,
    declarations: &[PluginDeclaration],
    reserved_names: &[&str],
    limits: PluginLimits,
) -> Result<Vec<(String, WasmMetricPlugin)>, PluginHostError> {
    let mut loaded: Vec<(String, WasmMetricPlugin)> = Vec::new();
    let mut seen_keys = BTreeSet::new();
    for declaration in declarations
        .iter()
        .filter(|declaration| declaration.kind == PluginKind::Metric)
    {
        if reserved_names.contains(&declaration.name.as_str())
            || loaded.iter().any(|(name, _)| name == &declaration.name)
        {
            return Err(PluginHostError::DuplicatePluginName {
                name: declaration.name.clone(),
            });
        }
        let plugin = load_metric_plugin_with_limits(repository_root, declaration, limits)?;
        if !seen_keys.insert(plugin.key().to_owned()) {
            return Err(PluginHostError::DuplicateMetricKey {
                name: declaration.name.clone(),
                key: plugin.key().to_owned(),
            });
        }
        loaded.push((declaration.name.clone(), plugin));
    }
    Ok(loaded)
}

impl LanguagePlugin for WasmLanguagePlugin {
    fn extensions(&self) -> Vec<String> {
        self.extensions_impl()
    }

    fn lower(&self, path: &RepoPath, text: &str) -> Result<FileModel, LowerError> {
        self.lower_impl(path, text)
    }

    fn manifest_paths(&self, repository_paths: &[RepoPath]) -> Vec<String> {
        self.manifest_paths_impl(repository_paths)
    }

    fn workspace_packages(
        &self,
        manifests: Vec<Manifest>,
    ) -> Result<Vec<RepoPath>, WorkspaceError> {
        self.workspace_packages_impl(manifests)
    }

    fn resolve_imports(
        &self,
        repository_paths: &[RepoPath],
        manifests: Vec<Manifest>,
        requests: Vec<language::ImportRequest>,
    ) -> Vec<language::Resolution> {
        self.resolve_imports_impl(repository_paths, manifests, requests)
    }

    fn reference_ranges(&self) -> Result<ReferenceRanges, InvalidReferenceRange> {
        self.reference_ranges_impl()
    }
}

impl MetricPlugin for WasmMetricPlugin {
    fn key(&self) -> &str {
        self.key_impl()
    }

    fn name(&self) -> &str {
        self.name_impl()
    }

    fn description(&self) -> &str {
        self.description_impl()
    }

    fn measure(&self, model: &CodeModel) -> Result<ExternalMetricMeasurement, MetricPluginError> {
        self.measure_impl(model)
    }
}

fn read_verified_bytes(
    repository_root: &Path,
    declaration: &PluginDeclaration,
) -> Result<(PathBuf, Vec<u8>), PluginHostError> {
    let resolved_path = resolve_plugin_path(repository_root, &declaration.path);
    let bytes = std::fs::read(&resolved_path).map_err(|source| PluginHostError::Read {
        name: declaration.name.clone(),
        path: resolved_path.clone(),
        source,
    })?;
    let actual = sha256(&bytes);
    if !constant_time_equal(&actual, declaration.sha256.as_bytes()) {
        return Err(PluginHostError::HashMismatch {
            name: declaration.name.clone(),
            path: resolved_path,
            expected: declaration.sha256.to_hex(),
            actual: sha256_hex(&actual),
        });
    }
    Ok((resolved_path, bytes))
}

fn resolve_plugin_path(repository_root: &Path, declared: &Path) -> PathBuf {
    if declared.is_absolute() {
        declared.to_path_buf()
    } else {
        repository_root.join(declared)
    }
}

fn constant_time_equal(left: &[u8; 32], right: &[u8; 32]) -> bool {
    let mut diff = 0u8;
    for (a, b) in left.iter().zip(right.iter()) {
        diff |= a ^ b;
    }
    diff == 0
}
