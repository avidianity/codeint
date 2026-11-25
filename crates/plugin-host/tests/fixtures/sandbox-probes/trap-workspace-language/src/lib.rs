#![no_std]

extern crate alloc;
extern crate probe_runtime;

use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;

wit_bindgen::generate!({
    world: "language-plugin",
    path: "../../../../../../wit",
});

use exports::codeint::plugin::language::{
    DeclaredDep, FileModel, FrameworkProfileResult, FrameworkProfileSelection, Guest,
    ImportRequest, LowerError, Manifest, ReferenceRange, Resolution, SourceSet, WorkspaceError,
};

struct TrapWorkspaceLanguagePlugin;

export!(TrapWorkspaceLanguagePlugin);

impl Guest for TrapWorkspaceLanguagePlugin {
    fn extensions() -> Vec<String> {
        vec!["trap".to_string()]
    }

    fn lower(path: String, text: String) -> Result<FileModel, LowerError> {
        if path.ends_with(".trap") {
            let lines = probe_runtime::count_non_empty_lines(&text);
            Ok(probe_runtime::blank_file_model!(SourceSet::Production, lines))
        } else {
            Err(LowerError::UnsupportedFile(path))
        }
    }

    fn manifest_paths(repository_paths: Vec<String>) -> Vec<String> {
        repository_paths
            .into_iter()
            .filter(|path| path == "trapper" || path.ends_with(".trapmanifest"))
            .collect()
    }

    fn workspace_packages(manifests: Vec<Manifest>) -> Result<Vec<String>, WorkspaceError> {
        if manifests.is_empty() {
            return Ok(Vec::new());
        }
        panic!("probe trap")
    }

    fn resolve_imports(
        _repository_paths: Vec<String>,
        _manifests: Vec<Manifest>,
        _requests: Vec<ImportRequest>,
        _active_profiles: Vec<String>,
    ) -> Vec<Resolution> {
        Vec::new()
    }

    fn declared_dependencies(_manifests: Vec<Manifest>) -> Vec<DeclaredDep> {
        Vec::new()
    }

    fn entry_points(_repository_paths: Vec<String>, _manifests: Vec<Manifest>) -> Vec<String> {
        Vec::new()
    }

    fn reference_ranges() -> Vec<ReferenceRange> {
        Vec::new()
    }

    probe_runtime::empty_framework_profile_stubs!();

    fn has_hazard_rules() -> bool {
        false
    }

    fn hazard_rules() -> Vec<String> {
        Vec::new()
    }

    fn has_imports() -> bool {
        true
    }
}
