use std::path::Path;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use codeint_config::PluginLimits;
use codeint_model::{
    DeclaredDep, DepKind, FileModel, InvalidReferenceRange, LowerError, Manifest, ReferenceRange,
    ReferenceRanges, RepoPath, SourceSet, Token, TokenKind, WorkspaceError, language,
};
use wasmtime::component::{Component, Linker};
use wasmtime::{Config, Engine, Store};

use crate::PluginHostError;
use crate::limits::{
    EpochTicker, LimitTrap, PluginStoreState, arm_call_deadline, classify_trap, new_limited_store,
};

mod bindings {
    wasmtime::component::bindgen!({
        world: "language-plugin",
        path: "../../wit",
    });
}

use bindings::exports::codeint::plugin::language as guest;

pub(crate) struct ContractSpec {
    pub(crate) interface_prefix: &'static str,
    pub(crate) expected_interface: &'static str,
}

const LANGUAGE_CONTRACT: ContractSpec = ContractSpec {
    interface_prefix: "codeint:plugin/language@",
    expected_interface: "codeint:plugin/language@0.2.0",
};

pub(crate) fn check_contract_version(
    name: &str,
    path: &Path,
    engine: &Engine,
    instance_pre: &wasmtime::component::InstancePre<PluginStoreState>,
    spec: ContractSpec,
) -> Result<(), PluginHostError> {
    let component_type = instance_pre.component().component_type();
    for (export_name, _) in component_type.exports(engine) {
        if export_name == spec.expected_interface {
            return Ok(());
        }
        if export_name.starts_with(spec.interface_prefix) {
            return Err(PluginHostError::IncompatibleContractVersion {
                name: name.to_owned(),
                path: path.to_path_buf(),
                expected: spec.expected_interface,
                found: export_name.to_owned(),
            });
        }
    }
    Ok(())
}

struct PluginInstance {
    store: Store<PluginStoreState>,
    bindings: bindings::LanguagePlugin,
}

pub struct WasmLanguagePlugin {
    name: String,
    limits: PluginLimits,
    engine: Engine,
    _ticker: EpochTicker,
    pre: bindings::LanguagePluginPre<PluginStoreState>,
    extensions: Vec<String>,
    pool: Mutex<Vec<PluginInstance>>,
    peak_memory_bytes: AtomicUsize,
}

pub(crate) fn compile_sandboxed_component(
    name: &str,
    path: &Path,
    bytes: &[u8],
) -> Result<(Engine, wasmtime::component::InstancePre<PluginStoreState>), PluginHostError> {
    let mut config = Config::new();
    config.wasm_component_model(true);
    config.epoch_interruption(true);
    let engine = Engine::new(&config).map_err(|error| PluginHostError::Compile {
        name: name.to_owned(),
        path: path.to_path_buf(),
        reason: error.to_string(),
    })?;
    let component =
        Component::from_binary(&engine, bytes).map_err(|error| PluginHostError::Compile {
            name: name.to_owned(),
            path: path.to_path_buf(),
            reason: error.to_string(),
        })?;
    let linker: Linker<PluginStoreState> = Linker::new(&engine);
    let instance_pre =
        linker
            .instantiate_pre(&component)
            .map_err(|error| PluginHostError::Instantiate {
                name: name.to_owned(),
                path: path.to_path_buf(),
                reason: error.to_string(),
            })?;
    Ok((engine, instance_pre))
}

pub(crate) struct InstantiatedComponent<Pre, Bindings> {
    pub(crate) engine: Engine,
    pub(crate) ticker: EpochTicker,
    pub(crate) pre: Pre,
    pub(crate) store: Store<PluginStoreState>,
    pub(crate) bindings: Bindings,
}

pub(crate) fn instantiate_sandboxed_component<Pre, Bindings>(
    name: &str,
    path: &Path,
    bytes: &[u8],
    limits: PluginLimits,
    spec: ContractSpec,
    bind: impl FnOnce(
        wasmtime::component::InstancePre<PluginStoreState>,
    ) -> Result<Pre, wasmtime::Error>,
    instantiate: impl FnOnce(&Pre, &mut Store<PluginStoreState>) -> Result<Bindings, wasmtime::Error>,
) -> Result<InstantiatedComponent<Pre, Bindings>, PluginHostError> {
    let (engine, instance_pre) = compile_sandboxed_component(name, path, bytes)?;
    check_contract_version(name, path, &engine, &instance_pre, spec)?;
    let ticker = EpochTicker::start(engine.clone());
    let pre = bind(instance_pre).map_err(|error| PluginHostError::Instantiate {
        name: name.to_owned(),
        path: path.to_path_buf(),
        reason: error.to_string(),
    })?;
    let mut store = new_limited_store(&engine, limits);
    let bindings = instantiate(&pre, &mut store)
        .map_err(|error| map_host_trap(name, path, None, limits, &store, error))?;
    Ok(InstantiatedComponent {
        engine,
        ticker,
        pre,
        store,
        bindings,
    })
}

impl WasmLanguagePlugin {
    pub(crate) fn from_verified_bytes(
        name: String,
        path: &Path,
        bytes: &[u8],
        limits: PluginLimits,
    ) -> Result<WasmLanguagePlugin, PluginHostError> {
        let mut component = instantiate_sandboxed_component(
            &name,
            path,
            bytes,
            limits,
            LANGUAGE_CONTRACT,
            bindings::LanguagePluginPre::new,
            |pre, store| pre.instantiate(store),
        )?;
        arm_call_deadline(&mut component.store, limits);
        let extensions = component
            .bindings
            .codeint_plugin_language()
            .call_extensions(&mut component.store)
            .map_err(|error| {
                map_host_trap(
                    &name,
                    path,
                    Some("extensions"),
                    limits,
                    &component.store,
                    error,
                )
            })?;
        let peak_memory_bytes = AtomicUsize::new(component.store.data().peak_memory_bytes());
        Ok(WasmLanguagePlugin {
            name,
            limits,
            engine: component.engine,
            _ticker: component.ticker,
            pre: component.pre,
            extensions,
            pool: Mutex::new(vec![PluginInstance {
                store: component.store,
                bindings: component.bindings,
            }]),
            peak_memory_bytes,
        })
    }

    pub fn peak_memory_bytes(&self) -> usize {
        self.peak_memory_bytes.load(Ordering::Relaxed)
    }

    fn record_peak_memory(&self, store: &Store<PluginStoreState>) {
        self.peak_memory_bytes
            .fetch_max(store.data().peak_memory_bytes(), Ordering::Relaxed);
    }

    fn checkout(&self) -> Result<PluginInstance, PluginHostError> {
        if let Ok(mut guard) = self.pool.lock()
            && let Some(instance) = guard.pop()
        {
            return Ok(instance);
        }
        let mut store = new_limited_store(&self.engine, self.limits);
        let bindings = self.pre.instantiate(&mut store).map_err(|error| {
            map_call_trap(&self.name, "instantiate", self.limits, &store, error)
        })?;
        self.record_peak_memory(&store);
        Ok(PluginInstance { store, bindings })
    }

    fn checkin(&self, instance: PluginInstance) {
        self.record_peak_memory(&instance.store);
        if let Ok(mut guard) = self.pool.lock() {
            guard.push(instance);
        }
    }

    fn with_instance<T>(
        &self,
        call: &'static str,
        operation: impl FnOnce(
            &guest::Guest,
            &mut Store<PluginStoreState>,
        ) -> Result<T, wasmtime::Error>,
    ) -> Result<T, PluginHostError> {
        let mut instance = self.checkout()?;
        arm_call_deadline(&mut instance.store, self.limits);
        let result = operation(
            instance.bindings.codeint_plugin_language(),
            &mut instance.store,
        );
        self.record_peak_memory(&instance.store);
        match result {
            Ok(value) => {
                self.checkin(instance);
                Ok(value)
            }
            Err(error) => Err(map_call_trap(
                &self.name,
                call,
                self.limits,
                &instance.store,
                error,
            )),
        }
    }

    pub(crate) fn extensions_impl(&self) -> Vec<String> {
        self.extensions.clone()
    }

    pub(crate) fn lower_impl(&self, path: &RepoPath, text: &str) -> Result<FileModel, LowerError> {
        let lowered = self
            .with_instance("lower", |api, store| {
                api.call_lower(store, path.as_str(), text)
            })
            .map_err(|error| match error {
                PluginHostError::MemoryLimitExceeded { name, memory_mb } => {
                    LowerError::MemoryLimitExceeded {
                        plugin: name,
                        memory_mb,
                    }
                }
                PluginHostError::TimeLimitExceeded { name, timeout_ms } => {
                    LowerError::TimeLimitExceeded {
                        plugin: name,
                        timeout_ms,
                    }
                }
                other => LowerError::Unparsable {
                    message: other.to_string(),
                },
            })?;
        lowered.map(convert_file_model).map_err(convert_lower_error)
    }

    pub(crate) fn manifest_paths_impl(&self, repository_paths: &[RepoPath]) -> Vec<String> {
        let paths = owned_paths(repository_paths);
        self.with_instance("manifest-paths", |api, store| {
            api.call_manifest_paths(store, &paths)
        })
        .unwrap_or_default()
    }

    pub(crate) fn workspace_packages_impl(
        &self,
        manifests: Vec<Manifest>,
    ) -> Result<Vec<RepoPath>, WorkspaceError> {
        let host_manifests: Vec<guest::Manifest> = manifests
            .into_iter()
            .map(|manifest| guest::Manifest {
                path: manifest.path,
                text: manifest.text,
            })
            .collect();
        let packages = self
            .with_instance("workspace-packages", |api, store| {
                api.call_workspace_packages(store, &host_manifests)
            })
            .map_err(|error| WorkspaceError::Manifest {
                path: self.name.clone(),
                message: error.to_string(),
            })?
            .map_err(|error| WorkspaceError::Manifest {
                path: error.path,
                message: error.message,
            })?;
        packages
            .into_iter()
            .map(|root| Ok(RepoPath::new(root)?))
            .collect()
    }

    pub(crate) fn resolve_imports_impl(
        &self,
        repository_paths: &[RepoPath],
        manifests: Vec<Manifest>,
        requests: Vec<language::ImportRequest>,
    ) -> Vec<language::Resolution> {
        let paths = owned_paths(repository_paths);
        let host_manifests: Vec<guest::Manifest> = manifests
            .into_iter()
            .map(|manifest| guest::Manifest {
                path: manifest.path,
                text: manifest.text,
            })
            .collect();
        let host_requests: Vec<guest::ImportRequest> = requests
            .into_iter()
            .map(|request| guest::ImportRequest {
                importer: request.importer,
                specifier: request.specifier,
            })
            .collect();
        self.with_instance("resolve-imports", |api, store| {
            api.call_resolve_imports(store, &paths, &host_manifests, &host_requests)
        })
        .map(|resolutions| resolutions.into_iter().map(convert_resolution).collect())
        .unwrap_or_default()
    }

    pub(crate) fn declared_dependencies_impl(&self, manifests: Vec<Manifest>) -> Vec<DeclaredDep> {
        let host_manifests: Vec<guest::Manifest> = manifests
            .into_iter()
            .map(|manifest| guest::Manifest {
                path: manifest.path,
                text: manifest.text,
            })
            .collect();
        self.with_instance("declared-dependencies", |api, store| {
            api.call_declared_dependencies(store, &host_manifests)
        })
        .map(|deps| deps.into_iter().map(convert_declared_dep).collect())
        .unwrap_or_default()
    }

    pub(crate) fn entry_points_impl(
        &self,
        repository_paths: &[RepoPath],
        manifests: Vec<Manifest>,
    ) -> Vec<String> {
        let paths = owned_paths(repository_paths);
        let host_manifests: Vec<guest::Manifest> = manifests
            .into_iter()
            .map(|manifest| guest::Manifest {
                path: manifest.path,
                text: manifest.text,
            })
            .collect();
        self.with_instance("entry-points", |api, store| {
            api.call_entry_points(store, &paths, &host_manifests)
        })
        .unwrap_or_default()
    }

    pub(crate) fn reference_ranges_impl(&self) -> Result<ReferenceRanges, InvalidReferenceRange> {
        let ranges = self
            .with_instance("reference-ranges", |api, store| {
                api.call_reference_ranges(store)
            })
            .unwrap_or_default()
            .into_iter()
            .map(|range| ReferenceRange {
                metric: range.metric,
                low: range.low,
                high: range.high,
                source: range.source,
            })
            .collect();
        ReferenceRanges::new(ranges)
    }
}

fn owned_paths(repository_paths: &[RepoPath]) -> Vec<String> {
    repository_paths
        .iter()
        .map(|path| path.as_str().to_owned())
        .collect()
}

fn convert_file_model(model: guest::FileModel) -> FileModel {
    FileModel {
        source_set: convert_source_set(model.source_set),
        source_lines: model.source_lines,
        functions: model
            .functions
            .into_iter()
            .map(|function| language::Function {
                line: function.line,
                end_line: function.end_line,
                decision_points: function.decision_points,
                source_lines: function.source_lines,
                name: function.name,
                cognitive_complexity: function.cognitive_complexity,
            })
            .collect(),
        tokens: model
            .tokens
            .into_iter()
            .map(|token| Token {
                kind: convert_token_kind(token.kind),
                text_hash: token.text_hash,
                line: token.line,
            })
            .collect(),
        flagged_lines: model
            .flagged_lines
            .into_iter()
            .map(|flagged| language::FlaggedLine {
                line: flagged.line,
                rule: flagged.rule,
            })
            .collect(),
        runtime_imports: model.runtime_imports,
    }
}

fn convert_source_set(source_set: guest::SourceSet) -> SourceSet {
    match source_set {
        guest::SourceSet::Production => SourceSet::Production,
        guest::SourceSet::Test => SourceSet::Test,
        guest::SourceSet::Generated => SourceSet::Generated,
        guest::SourceSet::Vendored => SourceSet::Vendored,
    }
}

fn convert_token_kind(kind: guest::TokenKind) -> TokenKind {
    match kind {
        guest::TokenKind::Identifier => TokenKind::Identifier,
        guest::TokenKind::Literal => TokenKind::Literal,
        guest::TokenKind::Other => TokenKind::Other,
        guest::TokenKind::ImportOrExport => TokenKind::ImportOrExport,
    }
}

fn convert_lower_error(error: guest::LowerError) -> LowerError {
    match error {
        guest::LowerError::Unparsable(message) => LowerError::Unparsable { message },
        guest::LowerError::UnsupportedFile(message) => LowerError::UnsupportedFile { message },
        guest::LowerError::TooLarge => LowerError::TooLarge,
        guest::LowerError::DeclarationOnly => LowerError::DeclarationOnly,
    }
}

fn convert_resolution(resolution: guest::Resolution) -> language::Resolution {
    match resolution {
        guest::Resolution::Resolved(path) => language::Resolution::Resolved(path),
        guest::Resolution::External(package) => language::Resolution::External(package),
        guest::Resolution::Builtin => language::Resolution::Builtin,
        guest::Resolution::Unresolved(reason) => language::Resolution::Unresolved(reason),
    }
}

fn convert_declared_dep(dep: guest::DeclaredDep) -> DeclaredDep {
    DeclaredDep {
        package_root: dep.package_root,
        name: dep.name,
        kind: match dep.kind {
            guest::DepKind::Runtime => DepKind::Runtime,
            guest::DepKind::Dev => DepKind::Dev,
            guest::DepKind::Optional => DepKind::Optional,
        },
    }
}

pub(crate) fn map_host_trap(
    name: &str,
    path: &Path,
    call: Option<&'static str>,
    limits: PluginLimits,
    store: &Store<PluginStoreState>,
    error: wasmtime::Error,
) -> PluginHostError {
    match classify_trap(store, &error) {
        LimitTrap::Memory => PluginHostError::MemoryLimitExceeded {
            name: name.to_owned(),
            memory_mb: limits.memory_mb,
        },
        LimitTrap::Time => PluginHostError::TimeLimitExceeded {
            name: name.to_owned(),
            timeout_ms: limits.timeout_ms,
        },
        LimitTrap::Other => match call {
            Some(call) => PluginHostError::Call {
                name: name.to_owned(),
                call,
                reason: error.to_string(),
            },
            None => PluginHostError::Instantiate {
                name: name.to_owned(),
                path: path.to_path_buf(),
                reason: error.to_string(),
            },
        },
    }
}

fn map_call_trap(
    name: &str,
    call: &'static str,
    limits: PluginLimits,
    store: &Store<PluginStoreState>,
    error: wasmtime::Error,
) -> PluginHostError {
    match classify_trap(store, &error) {
        LimitTrap::Memory => PluginHostError::MemoryLimitExceeded {
            name: name.to_owned(),
            memory_mb: limits.memory_mb,
        },
        LimitTrap::Time => PluginHostError::TimeLimitExceeded {
            name: name.to_owned(),
            timeout_ms: limits.timeout_ms,
        },
        LimitTrap::Other => PluginHostError::Call {
            name: name.to_owned(),
            call,
            reason: error.to_string(),
        },
    }
}
