use std::collections::{BTreeMap, VecDeque};
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock};

use codeint_config::PluginLimits;
use codeint_model::{
    DeclaredDep, DepKind, FileModel, FrameworkProfileResult, FrameworkProfileSelection,
    LanguagePluginError, LowerError, Manifest, ReferenceRange, ReferenceRanges, RepoPath,
    SourceSet, Token, TokenKind, WorkspaceError, language, validate_declared_dependencies,
    validate_entry_points, validate_framework_profile_result,
};
use wasmtime::component::{Component, Linker};
use wasmtime::{Config, Engine, Store};

use crate::PluginHostError;
use crate::limits::{
    EpochTicker, LiftedBudget, LiftedSize, LimitTrap, PluginStoreState, arm_call_deadline,
    classify_trap, enforce_lifted_budget, new_limited_store, slice_budget,
};

#[cfg(test)]
mod tests;

const MAX_CACHED_COMPONENTS: usize = 64;

static SHARED_ENGINE: OnceLock<Result<Engine, String>> = OnceLock::new();
static COMPONENT_CACHE: OnceLock<Mutex<ComponentCache>> = OnceLock::new();

struct ComponentCache {
    order: VecDeque<[u8; 32]>,
    entries: BTreeMap<[u8; 32], Component>,
}

impl ComponentCache {
    fn new() -> ComponentCache {
        ComponentCache {
            order: VecDeque::new(),
            entries: BTreeMap::new(),
        }
    }

    fn get(&self, digest: &[u8; 32]) -> Option<Component> {
        self.entries.get(digest).cloned()
    }

    fn insert(&mut self, digest: [u8; 32], component: Component, capacity: usize) {
        if capacity == 0 || self.entries.contains_key(&digest) {
            return;
        }
        while self.order.len() >= capacity {
            if let Some(oldest) = self.order.pop_front() {
                self.entries.remove(&oldest);
            }
        }
        self.order.push_back(digest);
        self.entries.insert(digest, component);
    }
}

fn shared_engine(name: &str, path: &Path) -> Result<Engine, PluginHostError> {
    SHARED_ENGINE
        .get_or_init(|| {
            let mut config = Config::new();
            config.wasm_component_model(true);
            config.epoch_interruption(true);
            Engine::new(&config).map_err(|error| error.to_string())
        })
        .as_ref()
        .cloned()
        .map_err(|reason| PluginHostError::Compile {
            name: name.to_owned(),
            path: path.to_path_buf(),
            reason: reason.clone(),
        })
}

fn component_cache() -> &'static Mutex<ComponentCache> {
    COMPONENT_CACHE.get_or_init(|| Mutex::new(ComponentCache::new()))
}

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
    expected_interface: "codeint:plugin/language@0.4.0",
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

pub(crate) struct SandboxedComponent<Pre> {
    limits: PluginLimits,
    engine: Engine,
    _ticker: EpochTicker,
    pre: Pre,
    peak_memory_bytes: AtomicUsize,
}

impl<Pre> SandboxedComponent<Pre> {
    pub(crate) fn from_init<Bindings>(
        component: InstantiatedComponent<Pre, Bindings>,
        limits: PluginLimits,
    ) -> SandboxedComponent<Pre> {
        let peak_memory_bytes = AtomicUsize::new(component.store.data().peak_memory_bytes());
        SandboxedComponent {
            limits,
            engine: component.engine,
            _ticker: component.ticker,
            pre: component.pre,
            peak_memory_bytes,
        }
    }

    pub(crate) fn limits(&self) -> PluginLimits {
        self.limits
    }

    pub(crate) fn peak_memory_bytes(&self) -> usize {
        self.peak_memory_bytes.load(Ordering::Relaxed)
    }

    fn record_peak_memory(&self, store: &Store<PluginStoreState>) {
        self.peak_memory_bytes
            .fetch_max(store.data().peak_memory_bytes(), Ordering::Relaxed);
    }

    pub(crate) fn call_fresh<Bindings, T, E>(
        &self,
        instantiate: impl FnOnce(
            &Pre,
            &mut Store<PluginStoreState>,
        ) -> Result<Bindings, wasmtime::Error>,
        invoke: impl FnOnce(&Bindings, &mut Store<PluginStoreState>) -> Result<T, wasmtime::Error>,
        map_instantiate_error: impl FnOnce(&Store<PluginStoreState>, wasmtime::Error) -> E,
        map_call_error: impl FnOnce(&Store<PluginStoreState>, wasmtime::Error) -> E,
    ) -> Result<T, E> {
        let mut store = new_limited_store(&self.engine, self.limits);
        let bindings = match instantiate(&self.pre, &mut store) {
            Ok(bindings) => bindings,
            Err(error) => {
                self.record_peak_memory(&store);
                return Err(map_instantiate_error(&store, error));
            }
        };
        arm_call_deadline(&mut store, self.limits);
        let result = invoke(&bindings, &mut store);
        self.record_peak_memory(&store);
        result.map_err(|error| map_call_error(&store, error))
    }
}

pub struct WasmLanguagePlugin {
    name: String,
    extensions: Vec<String>,
    known_framework_profiles: Vec<String>,
    has_hazard_rules: bool,
    hazard_rules: Vec<String>,
    has_imports: bool,
    component: SandboxedComponent<bindings::LanguagePluginPre<PluginStoreState>>,
}

fn compile_component(
    name: &str,
    path: &Path,
    bytes: &[u8],
) -> Result<(Engine, Component), PluginHostError> {
    let engine = shared_engine(name, path)?;
    let digest = crate::sha256::sha256(bytes);
    if let Some(component) = component_cache()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .get(&digest)
    {
        return Ok((engine, component));
    }
    let component =
        Component::from_binary(&engine, bytes).map_err(|error| PluginHostError::Compile {
            name: name.to_owned(),
            path: path.to_path_buf(),
            reason: error.to_string(),
        })?;
    component_cache()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .insert(digest, component.clone(), MAX_CACHED_COMPONENTS);
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

pub(crate) struct VerifiedComponent<'a> {
    pub(crate) name: &'a str,
    pub(crate) path: &'a Path,
    pub(crate) bytes: &'a [u8],
    pub(crate) limits: PluginLimits,
}

pub(crate) fn instantiate_sandboxed_component<Pre, Bindings>(
    verified: VerifiedComponent<'_>,
    spec: ContractSpec,
    bind: impl FnOnce(
        wasmtime::component::InstancePre<PluginStoreState>,
    ) -> Result<Pre, wasmtime::Error>,
    instantiate: impl FnOnce(&Pre, &mut Store<PluginStoreState>) -> Result<Bindings, wasmtime::Error>,
) -> Result<InstantiatedComponent<Pre, Bindings>, PluginHostError> {
    let (engine, pre) =
        prepare_sandboxed_component(verified.name, verified.path, verified.bytes, spec, bind)?;
    let ticker = EpochTicker::acquire(&engine);
    let mut store = new_limited_store(&engine, verified.limits);
    let bindings = instantiate(&pre, &mut store).map_err(|error| {
        map_host_trap(
            verified.name,
            verified.path,
            None,
            verified.limits,
            &store,
            error,
        )
    })?;
    Ok(InstantiatedComponent {
        engine,
        ticker,
        pre,
        store,
        bindings,
    })
}

pub(crate) struct InitSession<'a, Pre, Bindings> {
    component: &'a mut InstantiatedComponent<Pre, Bindings>,
    name: &'a str,
    path: &'a Path,
    limits: PluginLimits,
}

impl<Pre, Bindings> InstantiatedComponent<Pre, Bindings> {
    pub(crate) fn init_session<'a>(
        &'a mut self,
        name: &'a str,
        path: &'a Path,
        limits: PluginLimits,
    ) -> InitSession<'a, Pre, Bindings> {
        InitSession {
            component: self,
            name,
            path,
            limits,
        }
    }
}

impl<Pre, Bindings> InitSession<'_, Pre, Bindings> {
    pub(crate) fn call<T>(
        &mut self,
        call: &'static str,
        invoke: impl FnOnce(&Bindings, &mut Store<PluginStoreState>) -> Result<T, wasmtime::Error>,
    ) -> Result<T, PluginHostError> {
        arm_call_deadline(&mut self.component.store, self.limits);
        invoke(&self.component.bindings, &mut self.component.store).map_err(|error| {
            map_host_trap(
                self.name,
                self.path,
                Some(call),
                self.limits,
                &self.component.store,
                error,
            )
        })
    }

    pub(crate) fn call_lifted<T: LiftedSize>(
        &mut self,
        call: &'static str,
        invoke: impl FnOnce(&Bindings, &mut Store<PluginStoreState>) -> Result<T, wasmtime::Error>,
    ) -> Result<T, PluginHostError> {
        let value = self.call(call, invoke)?;
        enforce_lifted_budget(self.name, self.limits, value.lifted_budget())?;
        Ok(value)
    }
}

impl WasmLanguagePlugin {
    pub(crate) fn from_verified_bytes(
        name: String,
        path: &Path,
        bytes: &[u8],
        limits: PluginLimits,
    ) -> Result<WasmLanguagePlugin, PluginHostError> {
        let mut component = instantiate_sandboxed_component(
            VerifiedComponent {
                name: &name,
                path,
                bytes,
                limits,
            },
            LANGUAGE_CONTRACT,
            bindings::LanguagePluginPre::new,
            |pre, store| pre.instantiate(store),
        )?;
        let mut init = component.init_session(&name, path, limits);
        let extensions = init.call_lifted("extensions", |b, s| {
            b.codeint_plugin_language().call_extensions(s)
        })?;
        let known_framework_profiles = init.call_lifted("known-framework-profiles", |b, s| {
            b.codeint_plugin_language().call_known_framework_profiles(s)
        })?;
        let has_hazard_rules = init.call("has-hazard-rules", |b, s| {
            b.codeint_plugin_language().call_has_hazard_rules(s)
        })?;
        let hazard_rules = init.call_lifted("hazard-rules", |b, s| {
            b.codeint_plugin_language().call_hazard_rules(s)
        })?;
        let has_imports = init.call("has-imports", |b, s| {
            b.codeint_plugin_language().call_has_imports(s)
        })?;
        Ok(WasmLanguagePlugin {
            name,
            extensions,
            known_framework_profiles,
            has_hazard_rules,
            hazard_rules,
            has_imports,
            component: SandboxedComponent::from_init(component, limits),
        })
    }

    pub fn peak_memory_bytes(&self) -> usize {
        self.component.peak_memory_bytes()
    }

    fn call_checked<T: LiftedSize>(
        &self,
        call: &'static str,
        operation: impl FnOnce(
            &guest::Guest,
            &mut Store<PluginStoreState>,
        ) -> Result<T, wasmtime::Error>,
    ) -> Result<T, PluginHostError> {
        let limits = self.component.limits();
        let value = self.component.call_fresh(
            |pre, store| pre.instantiate(store),
            |bindings, store| operation(bindings.codeint_plugin_language(), store),
            |store, error| map_call_trap(&self.name, "instantiate", limits, store, error),
            |store, error| map_call_trap(&self.name, call, limits, store, error),
        )?;
        enforce_lifted_budget(&self.name, limits, value.lifted_budget())?;
        Ok(value)
    }

    pub(crate) fn extensions_impl(&self) -> Vec<String> {
        self.extensions.clone()
    }

    pub(crate) fn known_framework_profiles_impl(&self) -> Vec<String> {
        self.known_framework_profiles.clone()
    }

    pub(crate) fn has_hazard_rules_impl(&self) -> bool {
        self.has_hazard_rules
    }

    pub(crate) fn hazard_rules_impl(&self) -> Vec<String> {
        self.hazard_rules.clone()
    }

    pub(crate) fn has_imports_impl(&self) -> bool {
        self.has_imports
    }

    pub(crate) fn lower_impl(&self, path: &RepoPath, text: &str) -> Result<FileModel, LowerError> {
        let lowered = self
            .call_checked("lower", |api, store| {
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

    pub(crate) fn manifest_paths_impl(
        &self,
        repository_paths: &[RepoPath],
    ) -> Result<Vec<String>, LanguagePluginError> {
        self.manifest_paths_checked(repository_paths)
            .map_err(map_language_plugin_error)
    }

    pub fn manifest_paths_checked(
        &self,
        repository_paths: &[RepoPath],
    ) -> Result<Vec<String>, PluginHostError> {
        let paths = owned_paths(repository_paths);
        self.call_checked("manifest-paths", |api, store| {
            api.call_manifest_paths(store, &paths)
        })
    }

    pub(crate) fn workspace_packages_impl(
        &self,
        manifests: Vec<Manifest>,
    ) -> Result<Vec<RepoPath>, WorkspaceError> {
        self.workspace_packages_checked(manifests)
            .map_err(|error| WorkspaceError::PluginFailed {
                plugin: self.name.clone(),
                message: error.to_string(),
            })?
    }

    pub fn workspace_packages_checked(
        &self,
        manifests: Vec<Manifest>,
    ) -> Result<Result<Vec<RepoPath>, WorkspaceError>, PluginHostError> {
        let host_manifests = to_guest_manifests(manifests);
        let packages = self.call_checked("workspace-packages", |api, store| {
            api.call_workspace_packages(store, &host_manifests)
        })?;
        Ok(match packages {
            Err(error) => Err(WorkspaceError::Manifest {
                path: error.path,
                message: error.message,
            }),
            Ok(roots) => roots
                .into_iter()
                .map(|root| Ok(RepoPath::new(root)?))
                .collect(),
        })
    }

    pub(crate) fn resolve_imports_impl(
        &self,
        repository_paths: &[RepoPath],
        manifests: Vec<Manifest>,
        requests: Vec<language::ImportRequest>,
        active_profiles: &[String],
    ) -> Result<Vec<language::Resolution>, LanguagePluginError> {
        let paths = owned_paths(repository_paths);
        let host_manifests = to_guest_manifests(manifests);
        let host_requests: Vec<guest::ImportRequest> = requests
            .into_iter()
            .map(|request| guest::ImportRequest {
                importer: request.importer,
                specifier: request.specifier,
            })
            .collect();
        let resolutions = self
            .call_checked("resolve-imports", |api, store| {
                api.call_resolve_imports(
                    store,
                    &paths,
                    &host_manifests,
                    &host_requests,
                    active_profiles,
                )
            })
            .map_err(map_language_plugin_error)?;
        Ok(resolutions.into_iter().map(convert_resolution).collect())
    }

    pub(crate) fn declared_dependencies_impl(
        &self,
        manifests: Vec<Manifest>,
    ) -> Result<Vec<DeclaredDep>, LanguagePluginError> {
        let host_manifests = to_guest_manifests(manifests);
        let deps = self
            .call_checked("declared-dependencies", |api, store| {
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
            .call_checked("entry-points", |api, store| {
                api.call_entry_points(store, &paths, &host_manifests)
            })
            .map_err(map_language_plugin_error)?;
        validate_entry_points(repository_paths, entries)
    }

    pub(crate) fn reference_ranges_impl(&self) -> Result<ReferenceRanges, LanguagePluginError> {
        let ranges = self
            .call_checked("reference-ranges", |api, store| {
                api.call_reference_ranges(store)
            })
            .map_err(map_language_plugin_error)?
            .into_iter()
            .map(convert_reference_range)
            .collect();
        ReferenceRanges::new(ranges).map_err(LanguagePluginError::InvalidReferenceRange)
    }

    pub(crate) fn resolve_framework_profiles_impl(
        &self,
        package_root: &str,
        files: &[RepoPath],
        manifests: Vec<Manifest>,
        selection: FrameworkProfileSelection,
    ) -> Result<FrameworkProfileResult, LanguagePluginError> {
        let file_paths = owned_paths(files);
        let host_manifests = to_guest_manifests(manifests);
        let guest_selection = to_guest_framework_profile_selection(selection);
        let resolved = self
            .call_checked("resolve-framework-profiles", |api, store| {
                api.call_resolve_framework_profiles(
                    store,
                    package_root,
                    &file_paths,
                    &host_manifests,
                    &guest_selection,
                )
            })
            .map_err(map_language_plugin_error)?
            .map_err(|message| LanguagePluginError::FrameworkProfile { message })?;
        validate_framework_profile_result(
            &self.known_framework_profiles,
            files,
            convert_framework_profile_result(resolved),
        )
    }
}

impl LiftedSize for Vec<guest::DeclaredDep> {
    fn lifted_budget(&self) -> LiftedBudget {
        slice_budget(self, |dep| {
            dep.package_root.len().saturating_add(dep.name.len())
        })
    }
}

impl LiftedSize for Vec<guest::Resolution> {
    fn lifted_budget(&self) -> LiftedBudget {
        let base_bytes = self.len().saturating_mul(size_of::<guest::Resolution>());
        self.iter().fold(
            LiftedBudget::new(self.len(), base_bytes),
            |acc, resolution| {
                let extra = match resolution {
                    guest::Resolution::Resolved(targets) => targets.lifted_budget(),
                    guest::Resolution::External(text) | guest::Resolution::Unresolved(text) => {
                        LiftedBudget::new(0, text.len())
                    }
                    guest::Resolution::Builtin => LiftedBudget::default(),
                };
                acc.saturating_add(extra)
            },
        )
    }
}

impl LiftedSize for Vec<guest::ReferenceRange> {
    fn lifted_budget(&self) -> LiftedBudget {
        slice_budget(self, |range| {
            range.metric.len().saturating_add(range.source.len())
        })
    }
}

impl LiftedSize for Result<guest::FrameworkProfileResult, String> {
    fn lifted_budget(&self) -> LiftedBudget {
        match self {
            Ok(result) => {
                let note = result
                    .reference_range_note
                    .as_ref()
                    .map_or(LiftedBudget::default(), |note| {
                        LiftedBudget::new(1, note.len())
                    });
                result
                    .active_profiles
                    .lifted_budget()
                    .saturating_add(result.entry_points.lifted_budget())
                    .saturating_add(result.generated_paths.lifted_budget())
                    .saturating_add(result.reference_ranges.lifted_budget())
                    .saturating_add(note)
            }
            Err(message) => LiftedBudget::new(1, message.len()),
        }
    }
}

impl LiftedSize for Result<Vec<String>, guest::WorkspaceError> {
    fn lifted_budget(&self) -> LiftedBudget {
        match self {
            Ok(packages) => packages.lifted_budget(),
            Err(error) => {
                LiftedBudget::new(1, error.path.len().saturating_add(error.message.len()))
            }
        }
    }
}

impl LiftedSize for Result<guest::FileModel, guest::LowerError> {
    fn lifted_budget(&self) -> LiftedBudget {
        match self {
            Ok(model) => {
                let functions = slice_budget(&model.functions, |function| {
                    function.name.as_ref().map_or(0, String::len)
                });
                let tokens = slice_budget(&model.tokens, |_| 0);
                let flagged = slice_budget(&model.flagged_lines, |flagged| flagged.rule.len());
                let hazards = slice_budget(&model.hazard_lines, |hazard| hazard.rule.len());
                functions
                    .saturating_add(tokens)
                    .saturating_add(flagged)
                    .saturating_add(hazards)
                    .saturating_add(model.runtime_imports.lifted_budget())
            }
            Err(
                guest::LowerError::Unparsable(message)
                | guest::LowerError::UnsupportedFile(message),
            ) => LiftedBudget::new(1, message.len()),
            Err(guest::LowerError::TooLarge | guest::LowerError::DeclarationOnly) => {
                LiftedBudget::default()
            }
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
        hazard_lines: model
            .hazard_lines
            .into_iter()
            .map(|hazard| language::HazardLine {
                line: hazard.line,
                rule: hazard.rule,
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
        guest::Resolution::Resolved(paths) => language::Resolution::Resolved(paths),
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

fn convert_reference_range(range: guest::ReferenceRange) -> ReferenceRange {
    ReferenceRange {
        metric: range.metric,
        metric_version: range.metric_version,
        low: range.low,
        high: range.high,
        source: range.source,
    }
}

fn to_guest_framework_profile_selection(
    selection: FrameworkProfileSelection,
) -> guest::FrameworkProfileSelection {
    match selection {
        FrameworkProfileSelection::Auto => guest::FrameworkProfileSelection::Auto,
        FrameworkProfileSelection::Disabled => guest::FrameworkProfileSelection::Disabled,
        FrameworkProfileSelection::Exclude(names) => {
            guest::FrameworkProfileSelection::Exclude(names)
        }
        FrameworkProfileSelection::Only(names) => guest::FrameworkProfileSelection::Only(names),
    }
}

fn convert_framework_profile_result(
    result: guest::FrameworkProfileResult,
) -> FrameworkProfileResult {
    FrameworkProfileResult {
        active_profiles: result.active_profiles,
        entry_points: result.entry_points,
        generated_paths: result.generated_paths,
        reference_ranges: result
            .reference_ranges
            .into_iter()
            .map(convert_reference_range)
            .collect(),
        reference_range_note: result.reference_range_note,
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
