use std::path::Path;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use codeint_config::PluginLimits;
use codeint_model::{
    DeclaredDep, DepKind, FileModel, InvalidReferenceRange, LanguagePluginError, LowerError,
    Manifest, ReferenceRange, ReferenceRanges, RepoPath, SourceSet, Token, TokenKind,
    WorkspaceError, language, validate_declared_dependencies, validate_entry_points,
};
use wasmtime::component::{Component, Linker};
use wasmtime::{Config, Engine, Store};

use crate::PluginHostError;
use crate::limits::{
    EpochTicker, LimitTrap, PluginStoreState, arm_call_deadline, classify_trap,
    enforce_lifted_budget, new_limited_store,
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
    expected_interface: "codeint:plugin/language@0.3.0",
};

pub(crate) fn check_contract_version(
    name: &str,
    path: &Path,
    engine: &Engine,
    component: &Component,
    spec: ContractSpec,
) -> Result<(), PluginHostError> {
    let component_type = component.component_type();
    let mut incompatible: Option<String> = None;
    for (export_name, _) in component_type.exports(engine) {
        if export_name == spec.expected_interface {
            return Ok(());
        }
        if export_name.starts_with(spec.interface_prefix) && incompatible.is_none() {
            incompatible = Some(export_name.to_owned());
        }
    }
    if let Some(found) = incompatible {
        return Err(PluginHostError::IncompatibleContractVersion {
            name: name.to_owned(),
            path: path.to_path_buf(),
            expected: spec.expected_interface,
            found,
        });
    }
    Ok(())
}

pub(crate) struct PooledInstance<Bindings> {
    pub(crate) store: Store<PluginStoreState>,
    pub(crate) bindings: Bindings,
}

pub(crate) struct InstancePool<Pre, Bindings> {
    limits: PluginLimits,
    engine: Engine,
    _ticker: EpochTicker,
    pre: Pre,
    pool: Mutex<Vec<PooledInstance<Bindings>>>,
    peak_memory_bytes: AtomicUsize,
}

impl<Pre, Bindings> InstancePool<Pre, Bindings> {
    pub(crate) fn from_component(
        component: InstantiatedComponent<Pre, Bindings>,
        limits: PluginLimits,
    ) -> InstancePool<Pre, Bindings> {
        let peak_memory_bytes = AtomicUsize::new(component.store.data().peak_memory_bytes());
        InstancePool {
            limits,
            engine: component.engine,
            _ticker: component.ticker,
            pre: component.pre,
            pool: Mutex::new(vec![PooledInstance {
                store: component.store,
                bindings: component.bindings,
            }]),
            peak_memory_bytes,
        }
    }

    pub(crate) fn limits(&self) -> PluginLimits {
        self.limits
    }

    pub(crate) fn peak_memory_bytes(&self) -> usize {
        self.peak_memory_bytes.load(Ordering::Relaxed)
    }

    pub(crate) fn record_peak_memory(&self, store: &Store<PluginStoreState>) {
        self.peak_memory_bytes
            .fetch_max(store.data().peak_memory_bytes(), Ordering::Relaxed);
    }

    pub(crate) fn checkout<E>(
        &self,
        instantiate: impl FnOnce(
            &Pre,
            &mut Store<PluginStoreState>,
        ) -> Result<Bindings, wasmtime::Error>,
        map_error: impl FnOnce(&Store<PluginStoreState>, wasmtime::Error) -> E,
    ) -> Result<PooledInstance<Bindings>, E> {
        if let Ok(mut guard) = self.pool.lock()
            && let Some(instance) = guard.pop()
        {
            return Ok(instance);
        }
        let mut store = new_limited_store(&self.engine, self.limits);
        let bindings =
            instantiate(&self.pre, &mut store).map_err(|error| map_error(&store, error))?;
        self.record_peak_memory(&store);
        Ok(PooledInstance { store, bindings })
    }

    pub(crate) fn checkin(&self, instance: PooledInstance<Bindings>) {
        self.record_peak_memory(&instance.store);
        if let Ok(mut guard) = self.pool.lock() {
            guard.push(instance);
        }
    }
}

pub struct WasmLanguagePlugin {
    name: String,
    extensions: Vec<String>,
    instances:
        InstancePool<bindings::LanguagePluginPre<PluginStoreState>, bindings::LanguagePlugin>,
}

fn compile_component(
    name: &str,
    path: &Path,
    bytes: &[u8],
) -> Result<(Engine, Component), PluginHostError> {
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
    Ok((engine, component))
}

pub(crate) fn prepare_sandboxed_component<Pre>(
    name: &str,
    path: &Path,
    bytes: &[u8],
    spec: ContractSpec,
    bind: impl FnOnce(
        wasmtime::component::InstancePre<PluginStoreState>,
    ) -> Result<Pre, wasmtime::Error>,
) -> Result<(Engine, Pre), PluginHostError> {
    let (engine, component) = compile_component(name, path, bytes)?;
    check_contract_version(name, path, &engine, &component, spec)?;
    let linker: Linker<PluginStoreState> = Linker::new(&engine);
    let instance_pre =
        linker
            .instantiate_pre(&component)
            .map_err(|error| PluginHostError::Instantiate {
                name: name.to_owned(),
                path: path.to_path_buf(),
                reason: error.to_string(),
            })?;
    let pre = bind(instance_pre).map_err(|error| PluginHostError::Instantiate {
        name: name.to_owned(),
        path: path.to_path_buf(),
        reason: error.to_string(),
    })?;
    Ok((engine, pre))
}

pub(crate) fn verify_language_component(
    name: &str,
    path: &Path,
    bytes: &[u8],
) -> Result<(), PluginHostError> {
    prepare_sandboxed_component(
        name,
        path,
        bytes,
        LANGUAGE_CONTRACT,
        bindings::LanguagePluginPre::new,
    )?;
    Ok(())
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
    let (engine, pre) = prepare_sandboxed_component(name, path, bytes, spec, bind)?;
    let ticker = EpochTicker::start(engine.clone());
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
        let (ext_count, ext_bytes) = extensions.lifted_budget();
        enforce_lifted_budget(&name, limits, ext_count, ext_bytes)?;
        Ok(WasmLanguagePlugin {
            name,
            extensions,
            instances: InstancePool::from_component(component, limits),
        })
    }

    pub fn peak_memory_bytes(&self) -> usize {
        self.instances.peak_memory_bytes()
    }

    fn with_checked_instance<T: LiftedSize>(
        &self,
        call: &'static str,
        operation: impl FnOnce(
            &guest::Guest,
            &mut Store<PluginStoreState>,
        ) -> Result<T, wasmtime::Error>,
    ) -> Result<T, PluginHostError> {
        let limits = self.instances.limits();
        let mut instance = self.instances.checkout(
            |pre, store| pre.instantiate(store),
            |store, error| map_call_trap(&self.name, "instantiate", limits, store, error),
        )?;
        arm_call_deadline(&mut instance.store, limits);
        let result = operation(
            instance.bindings.codeint_plugin_language(),
            &mut instance.store,
        );
        self.instances.record_peak_memory(&instance.store);
        match result {
            Ok(value) => {
                let (elements, bytes) = value.lifted_budget();
                enforce_lifted_budget(&self.name, limits, elements, bytes)?;
                self.instances.checkin(instance);
                Ok(value)
            }
            Err(error) => Err(map_call_trap(
                &self.name,
                call,
                limits,
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
            .with_checked_instance("lower", |api, store| {
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
        self.with_checked_instance("manifest-paths", |api, store| {
            api.call_manifest_paths(store, &paths)
        })
        .unwrap_or_default()
    }

    pub(crate) fn workspace_packages_impl(
        &self,
        manifests: Vec<Manifest>,
    ) -> Result<Vec<RepoPath>, WorkspaceError> {
        let host_manifests = to_guest_manifests(manifests);
        let packages = self
            .with_checked_instance("workspace-packages", |api, store| {
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
        let host_manifests = to_guest_manifests(manifests);
        let host_requests: Vec<guest::ImportRequest> = requests
            .into_iter()
            .map(|request| guest::ImportRequest {
                importer: request.importer,
                specifier: request.specifier,
            })
            .collect();
        self.with_checked_instance("resolve-imports", |api, store| {
            api.call_resolve_imports(store, &paths, &host_manifests, &host_requests)
        })
        .map(|resolutions| resolutions.into_iter().map(convert_resolution).collect())
        .unwrap_or_default()
    }

    pub(crate) fn declared_dependencies_impl(
        &self,
        manifests: Vec<Manifest>,
    ) -> Result<Vec<DeclaredDep>, LanguagePluginError> {
        let host_manifests = to_guest_manifests(manifests);
        let deps = self
            .with_checked_instance("declared-dependencies", |api, store| {
                api.call_declared_dependencies(store, &host_manifests)
            })
            .map_err(map_language_plugin_error)?;
        validate_declared_dependencies(deps.into_iter().map(convert_declared_dep).collect())
    }

    pub(crate) fn entry_points_impl(
        &self,
        repository_paths: &[RepoPath],
        manifests: Vec<Manifest>,
    ) -> Result<Vec<RepoPath>, LanguagePluginError> {
        let paths = owned_paths(repository_paths);
        let host_manifests = to_guest_manifests(manifests);
        let entries = self
            .with_checked_instance("entry-points", |api, store| {
                api.call_entry_points(store, &paths, &host_manifests)
            })
            .map_err(map_language_plugin_error)?;
        validate_entry_points(repository_paths, entries)
    }

    pub(crate) fn reference_ranges_impl(&self) -> Result<ReferenceRanges, InvalidReferenceRange> {
        let ranges = self
            .with_checked_instance("reference-ranges", |api, store| {
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

trait LiftedSize {
    fn lifted_budget(&self) -> (usize, usize);
}

fn slice_budget<T>(items: &[T], extra_bytes: fn(&T) -> usize) -> (usize, usize) {
    let base = items.len().saturating_mul(size_of::<T>());
    let bytes = items
        .iter()
        .fold(base, |acc, item| acc.saturating_add(extra_bytes(item)));
    (items.len(), bytes)
}

impl LiftedSize for Vec<String> {
    fn lifted_budget(&self) -> (usize, usize) {
        slice_budget(self, String::len)
    }
}

impl LiftedSize for Vec<guest::DeclaredDep> {
    fn lifted_budget(&self) -> (usize, usize) {
        slice_budget(self, |dep| {
            dep.package_root.len().saturating_add(dep.name.len())
        })
    }
}

impl LiftedSize for Vec<guest::Resolution> {
    fn lifted_budget(&self) -> (usize, usize) {
        slice_budget(self, |resolution| match resolution {
            guest::Resolution::Resolved(text)
            | guest::Resolution::External(text)
            | guest::Resolution::Unresolved(text) => text.len(),
            guest::Resolution::Builtin => 0,
        })
    }
}

impl LiftedSize for Vec<guest::ReferenceRange> {
    fn lifted_budget(&self) -> (usize, usize) {
        slice_budget(self, |range| {
            range.metric.len().saturating_add(range.source.len())
        })
    }
}

impl LiftedSize for Result<Vec<String>, guest::WorkspaceError> {
    fn lifted_budget(&self) -> (usize, usize) {
        match self {
            Ok(packages) => packages.lifted_budget(),
            Err(error) => (1, error.path.len().saturating_add(error.message.len())),
        }
    }
}

impl LiftedSize for Result<guest::FileModel, guest::LowerError> {
    fn lifted_budget(&self) -> (usize, usize) {
        match self {
            Ok(model) => {
                let (fn_count, fn_bytes) = slice_budget(&model.functions, |function| {
                    function.name.as_ref().map_or(0, String::len)
                });
                let (token_count, token_bytes) = slice_budget(&model.tokens, |_| 0);
                let (flag_count, flag_bytes) =
                    slice_budget(&model.flagged_lines, |flagged| flagged.rule.len());
                let (import_count, import_bytes) = model.runtime_imports.lifted_budget();
                (
                    fn_count
                        .saturating_add(token_count)
                        .saturating_add(flag_count)
                        .saturating_add(import_count),
                    fn_bytes
                        .saturating_add(token_bytes)
                        .saturating_add(flag_bytes)
                        .saturating_add(import_bytes),
                )
            }
            Err(
                guest::LowerError::Unparsable(message)
                | guest::LowerError::UnsupportedFile(message),
            ) => (1, message.len()),
            Err(guest::LowerError::TooLarge | guest::LowerError::DeclarationOnly) => (0, 0),
        }
    }
}

fn to_guest_manifests(manifests: Vec<Manifest>) -> Vec<guest::Manifest> {
    manifests
        .into_iter()
        .map(|manifest| guest::Manifest {
            path: manifest.path,
            text: manifest.text,
        })
        .collect()
}

fn map_language_plugin_error(error: PluginHostError) -> LanguagePluginError {
    match error {
        PluginHostError::MemoryLimitExceeded { name, memory_mb } => {
            LanguagePluginError::MemoryLimitExceeded {
                plugin: name,
                memory_mb,
            }
        }
        PluginHostError::TimeLimitExceeded { name, timeout_ms } => {
            LanguagePluginError::TimeLimitExceeded {
                plugin: name,
                timeout_ms,
            }
        }
        PluginHostError::Call { name, call, reason } => LanguagePluginError::Call {
            plugin: name,
            call,
            reason,
        },
        other => LanguagePluginError::Call {
            plugin: String::new(),
            call: "plugin",
            reason: other.to_string(),
        },
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
                coverage: function
                    .coverage
                    .map(|coverage| language::FunctionCoverage {
                        listed_lines: coverage.listed_lines,
                        executed_lines: coverage.executed_lines,
                    }),
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
        program_start: model.program_start,
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
