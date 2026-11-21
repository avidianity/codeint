use std::path::Path;
use std::sync::Mutex;

use codeint_model::{
    FileModel, InvalidReferenceRange, LowerError, Manifest, ReferenceRange, ReferenceRanges,
    RepoPath, SourceSet, Token, TokenKind, WorkspaceError, language,
};
use wasmtime::component::{Component, Linker};
use wasmtime::{Config, Engine, Store};

use crate::PluginHostError;

mod bindings {
    wasmtime::component::bindgen!({
        world: "language-plugin",
        path: "../../wit",
    });
}

use bindings::exports::codeint::plugin::language as guest;

struct PluginInstance {
    store: Store<()>,
    bindings: bindings::LanguagePlugin,
}

pub struct WasmLanguagePlugin {
    name: String,
    engine: Engine,
    pre: bindings::LanguagePluginPre<()>,
    extensions: Vec<String>,
    pool: Mutex<Vec<PluginInstance>>,
}

pub(crate) fn compile_sandboxed_component(
    name: &str,
    path: &Path,
    bytes: &[u8],
) -> Result<(Engine, wasmtime::component::InstancePre<()>), PluginHostError> {
    let mut config = Config::new();
    config.wasm_component_model(true);
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
    let linker: Linker<()> = Linker::new(&engine);
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

impl WasmLanguagePlugin {
    pub(crate) fn from_verified_bytes(
        name: String,
        path: &Path,
        bytes: &[u8],
    ) -> Result<WasmLanguagePlugin, PluginHostError> {
        let (engine, instance_pre) = compile_sandboxed_component(&name, path, bytes)?;
        let pre = bindings::LanguagePluginPre::new(instance_pre).map_err(|error| {
            PluginHostError::Instantiate {
                name: name.clone(),
                path: path.to_path_buf(),
                reason: error.to_string(),
            }
        })?;
        let mut store = Store::new(&engine, ());
        let bindings =
            pre.instantiate(&mut store)
                .map_err(|error| PluginHostError::Instantiate {
                    name: name.clone(),
                    path: path.to_path_buf(),
                    reason: error.to_string(),
                })?;
        let extensions = bindings
            .codeint_plugin_language()
            .call_extensions(&mut store)
            .map_err(|error| PluginHostError::Call {
                name: name.clone(),
                call: "extensions",
                reason: error.to_string(),
            })?;
        Ok(WasmLanguagePlugin {
            name,
            engine,
            pre,
            extensions,
            pool: Mutex::new(vec![PluginInstance { store, bindings }]),
        })
    }

    fn checkout(&self) -> Result<PluginInstance, wasmtime::Error> {
        if let Ok(mut guard) = self.pool.lock()
            && let Some(instance) = guard.pop()
        {
            return Ok(instance);
        }
        let mut store = Store::new(&self.engine, ());
        let bindings = self.pre.instantiate(&mut store)?;
        Ok(PluginInstance { store, bindings })
    }

    fn checkin(&self, instance: PluginInstance) {
        if let Ok(mut guard) = self.pool.lock() {
            guard.push(instance);
        }
    }

    fn with_instance<T>(
        &self,
        call: &'static str,
        operation: impl FnOnce(&guest::Guest, &mut Store<()>) -> Result<T, wasmtime::Error>,
    ) -> Result<T, PluginHostError> {
        let mut instance = self.checkout().map_err(|error| PluginHostError::Call {
            name: self.name.clone(),
            call,
            reason: error.to_string(),
        })?;
        let result = operation(
            instance.bindings.codeint_plugin_language(),
            &mut instance.store,
        );
        match result {
            Ok(value) => {
                self.checkin(instance);
                Ok(value)
            }
            Err(error) => Err(PluginHostError::Call {
                name: self.name.clone(),
                call,
                reason: error.to_string(),
            }),
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
            .map_err(|error| LowerError::Unparsable {
                message: error.to_string(),
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
        guest::Resolution::External => language::Resolution::External,
        guest::Resolution::Unresolved(reason) => language::Resolution::Unresolved(reason),
    }
}
