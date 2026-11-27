#![forbid(unsafe_code)]

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use codeint_config::{PluginDeclaration, PluginHostConfig, PluginKind, PluginLimits, PluginSource};
use codeint_model::{
    CodeModel, DeclaredDep, ExternalMetricMeasurement, FileModel, FrameworkProfileResult,
    FrameworkProfileSelection, LanguagePlugin, LanguagePluginError, LowerError, Manifest,
    MetricPlugin, MetricPluginError, ReferenceRanges, RepoPath, WorkspaceError, language,
};

mod builtins;
mod conformance;
mod fetch;
mod limits;
mod sha256;
mod sql;
mod wasm;
mod wasm_metric;

pub use builtins::{
    BuiltInLanguageDescriptor, NamedLanguagePlugin, SQL_PLUGIN_NAME,
    TYPESCRIPT_JAVASCRIPT_PLUGIN_NAME, built_in_language_descriptors,
    built_in_language_flagged_line_rules, built_in_language_framework_profiles,
    built_in_language_hazard_rules, built_in_language_names, built_in_language_plugin,
    built_in_language_plugins,
};
pub use conformance::{
    ConformanceCheck, ConformanceReport, ConformanceSource, ConformanceStatus, END_TO_END_CHECK,
    EndToEndFile, EndToEndProbe, LANGUAGE_CHECKS, METRIC_CHECKS, run_conformance,
};
pub use sha256::{format_hex as sha256_hex, sha256};
pub use sql::{
    LoweredSqlFile, SqlParsedFile, SqlRejectedStatement, SqlSchemaScopeSummary,
    apply_sql_schema_pass, lower_sql_source,
};
pub use wasm::WasmLanguagePlugin;
pub use wasm_metric::{WasmMetricPlugin, validate_metric_key, validate_metric_version};

pub const PLUGIN_INTERFACE_VERSION: &str = "0.4.0";

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
    #[error(
        "Plugin {name:?} at {} targets incompatible WIT contract {found}, but this version of codeint requires {expected}; rebuild the Plugin against {expected} and update its sha256 in codeint.json",
        path.display()
    )]
    IncompatibleContractVersion {
        name: String,
        path: PathBuf,
        expected: &'static str,
        found: String,
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
    #[error(
        "Metric plugin {name:?} declared an invalid Metric version {version}; Metric versions must be positive integers (>= 1)"
    )]
    InvalidMetricVersion { name: String, version: u32 },
    #[error("Plugin {name:?} URL {url:?} must use HTTPS (https://)")]
    NonHttpsUrl { name: String, url: String },
    #[error("cannot cache Plugin {name:?} from {url}: no cache directory is configured")]
    MissingCacheDir { name: String, url: String },
    #[error("cannot write cached Plugin {name:?} at {}", path.display())]
    CacheWrite {
        name: String,
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("cannot download Plugin {name:?} from {url}: {reason}")]
    Download {
        name: String,
        url: String,
        reason: String,
    },
    #[error(
        "Plugin {name:?} downloaded from {url} exceeds the maximum download size of {max_bytes} bytes"
    )]
    DownloadTooLarge {
        name: String,
        url: String,
        max_bytes: u64,
    },
    #[error(
        "SHA-256 hash mismatch for Plugin {name:?} downloaded from {url}: expected {expected}, found {actual}"
    )]
    DownloadHashMismatch {
        name: String,
        url: String,
        expected: String,
        actual: String,
    },
}

pub fn verify_plugin_component(
    name: &str,
    path: &Path,
    kind: PluginKind,
    bytes: &[u8],
) -> Result<(), PluginHostError> {
    match kind {
        PluginKind::Language => wasm::verify_language_component(name, path, bytes),
        PluginKind::Metric => wasm_metric::verify_metric_component(name, path, bytes),
    }
}

pub fn load_language_plugin(
    repository_root: &Path,
    declaration: &PluginDeclaration,
    host_config: &PluginHostConfig,
) -> Result<WasmLanguagePlugin, PluginHostError> {
    load_language_plugin_with_limits(
        repository_root,
        declaration,
        host_config,
        PluginLimits::default(),
    )
}

pub fn load_language_plugin_with_limits(
    repository_root: &Path,
    declaration: &PluginDeclaration,
    host_config: &PluginHostConfig,
    limits: PluginLimits,
) -> Result<WasmLanguagePlugin, PluginHostError> {
    let (resolved_path, bytes) = read_verified_bytes(repository_root, declaration, host_config)?;
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
    host_config: &PluginHostConfig,
) -> Result<WasmMetricPlugin, PluginHostError> {
    load_metric_plugin_with_limits(
        repository_root,
        declaration,
        host_config,
        PluginLimits::default(),
    )
}

pub fn load_metric_plugin_with_limits(
    repository_root: &Path,
    declaration: &PluginDeclaration,
    host_config: &PluginHostConfig,
    limits: PluginLimits,
) -> Result<WasmMetricPlugin, PluginHostError> {
    let (resolved_path, bytes) = read_verified_bytes(repository_root, declaration, host_config)?;
    WasmMetricPlugin::from_verified_bytes(declaration.name.clone(), &resolved_path, &bytes, limits)
}

pub fn load_language_plugins(
    repository_root: &Path,
    declarations: &[PluginDeclaration],
    reserved_names: &[&str],
    host_config: &PluginHostConfig,
) -> Result<Vec<(String, WasmLanguagePlugin)>, PluginHostError> {
    load_language_plugins_with_limits(
        repository_root,
        declarations,
        reserved_names,
        host_config,
        PluginLimits::default(),
    )
}

pub fn load_language_plugins_with_limits(
    repository_root: &Path,
    declarations: &[PluginDeclaration],
    reserved_names: &[&str],
    host_config: &PluginHostConfig,
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
        let plugin =
            load_language_plugin_with_limits(repository_root, declaration, host_config, limits)?;
        loaded.push((declaration.name.clone(), plugin));
    }
    Ok(loaded)
}

pub fn load_metric_plugins(
    repository_root: &Path,
    declarations: &[PluginDeclaration],
    reserved_names: &[&str],
    host_config: &PluginHostConfig,
) -> Result<Vec<(String, WasmMetricPlugin)>, PluginHostError> {
    load_metric_plugins_with_limits(
        repository_root,
        declarations,
        reserved_names,
        host_config,
        PluginLimits::default(),
    )
}

pub fn load_metric_plugins_with_limits(
    repository_root: &Path,
    declarations: &[PluginDeclaration],
    reserved_names: &[&str],
    host_config: &PluginHostConfig,
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
        let plugin =
            load_metric_plugin_with_limits(repository_root, declaration, host_config, limits)?;
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

    fn manifest_paths(
        &self,
        repository_paths: &[RepoPath],
    ) -> Result<Vec<String>, LanguagePluginError> {
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
        active_profiles: &[String],
    ) -> Result<Vec<language::Resolution>, LanguagePluginError> {
        self.resolve_imports_impl(repository_paths, manifests, requests, active_profiles)
    }

    fn declared_dependencies(
        &self,
        manifests: Vec<Manifest>,
    ) -> Result<Vec<DeclaredDep>, LanguagePluginError> {
        self.declared_dependencies_impl(manifests)
    }

    fn entry_points(
        &self,
        repository_paths: &[RepoPath],
        manifests: Vec<Manifest>,
    ) -> Result<Vec<RepoPath>, LanguagePluginError> {
        self.entry_points_impl(repository_paths, manifests)
    }

    fn reference_ranges(&self) -> Result<ReferenceRanges, LanguagePluginError> {
        self.reference_ranges_impl()
    }

    fn known_framework_profiles(&self) -> Vec<String> {
        self.known_framework_profiles_impl()
    }

    fn resolve_framework_profiles(
        &self,
        package_root: &str,
        files: &[RepoPath],
        manifests: Vec<Manifest>,
        selection: FrameworkProfileSelection,
    ) -> Result<FrameworkProfileResult, LanguagePluginError> {
        self.resolve_framework_profiles_impl(package_root, files, manifests, selection)
    }

    fn has_hazard_rules(&self) -> bool {
        self.has_hazard_rules_impl()
    }

    fn hazard_rules(&self) -> Vec<String> {
        self.hazard_rules_impl()
    }

    fn has_imports(&self) -> bool {
        self.has_imports_impl()
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

    fn version(&self) -> u32 {
        self.version_impl()
    }

    fn measure(&self, model: &CodeModel) -> Result<ExternalMetricMeasurement, MetricPluginError> {
        self.measure_impl(model)
    }
}

fn read_verified_bytes(
    repository_root: &Path,
    declaration: &PluginDeclaration,
    host_config: &PluginHostConfig,
) -> Result<(PathBuf, Vec<u8>), PluginHostError> {
    match &declaration.source {
        PluginSource::Path(declared_path) => {
            let resolved_path = resolve_plugin_path(repository_root, declared_path);
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
        PluginSource::HttpsUrl(url) => fetch::load_https_plugin(declaration, url, host_config),
    }
}

fn resolve_plugin_path(repository_root: &Path, declared: &Path) -> PathBuf {
    if declared.is_absolute() {
        declared.to_path_buf()
    } else {
        repository_root.join(declared)
    }
}

pub(crate) fn constant_time_equal(left: &[u8; 32], right: &[u8; 32]) -> bool {
    let mut diff = 0u8;
    for (a, b) in left.iter().zip(right.iter()) {
        diff |= a ^ b;
    }
    diff == 0
}
