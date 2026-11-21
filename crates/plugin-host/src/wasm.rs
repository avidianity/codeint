use std::path::Path;

use codeint_model::{
    FileModel, InvalidReferenceRange, LowerError, Manifest, ReferenceRanges, RepoPath,
    WorkspaceError, language,
};

use crate::PluginHostError;

pub struct WasmLanguagePlugin {
    _name: String,
}

impl WasmLanguagePlugin {
    pub(crate) fn from_verified_bytes(
        _name: String,
        _path: &Path,
        _bytes: &[u8],
    ) -> Result<WasmLanguagePlugin, PluginHostError> {
        todo!("compile and instantiate Wasm component with zero capabilities")
    }

    pub(crate) fn extensions_impl(&self) -> Vec<String> {
        todo!("call extensions on Wasm component")
    }

    pub(crate) fn lower_impl(
        &self,
        _path: &RepoPath,
        _text: &str,
    ) -> Result<FileModel, LowerError> {
        todo!("call lower on Wasm component")
    }

    pub(crate) fn manifest_paths_impl(&self, _repository_paths: &[RepoPath]) -> Vec<String> {
        todo!("call manifest-paths on Wasm component")
    }

    pub(crate) fn workspace_packages_impl(
        &self,
        _manifests: Vec<Manifest>,
    ) -> Result<Vec<RepoPath>, WorkspaceError> {
        todo!("call workspace-packages on Wasm component")
    }

    pub(crate) fn resolve_imports_impl(
        &self,
        _repository_paths: &[RepoPath],
        _manifests: Vec<Manifest>,
        _requests: Vec<language::ImportRequest>,
    ) -> Vec<language::Resolution> {
        todo!("call resolve-imports on Wasm component")
    }

    pub(crate) fn reference_ranges_impl(&self) -> Result<ReferenceRanges, InvalidReferenceRange> {
        todo!("call reference-ranges on Wasm component")
    }
}
