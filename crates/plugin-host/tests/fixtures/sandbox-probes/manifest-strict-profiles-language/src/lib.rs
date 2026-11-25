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

struct Probe;

export!(Probe);

impl Guest for Probe {
    fn extensions() -> Vec<String> {
        vec!["probe".to_string()]
    }

    probe_runtime::probe_extension_lower!();

    fn manifest_paths(_repository_paths: Vec<String>) -> Vec<String> {
        vec!["Cargo.toml".to_string()]
    }

    fn workspace_packages(_manifests: Vec<Manifest>) -> Result<Vec<String>, WorkspaceError> {
        Ok(Vec::new())
    }

    fn resolve_imports(
        _repository_paths: Vec<String>,
        _manifests: Vec<Manifest>,
        requests: Vec<ImportRequest>,
        _active_profiles: Vec<String>,
    ) -> Vec<Resolution> {
        requests
            .iter()
            .map(|_| Resolution::Unresolved("probe".to_string()))
            .collect()
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

    fn known_framework_profiles() -> Vec<String> {
        vec!["alpha".to_string()]
    }

    fn resolve_framework_profiles(
        _package_root: String,
        _files: Vec<String>,
        manifests: Vec<Manifest>,
        _selection: FrameworkProfileSelection,
    ) -> Result<FrameworkProfileResult, String> {
        if let Some(manifest) = manifests
            .iter()
            .find(|manifest| manifest.path != "Cargo.toml")
        {
            return Err(alloc::format!("cannot parse {} as TOML", manifest.path));
        }
        Ok(FrameworkProfileResult {
            active_profiles: Vec::new(),
            entry_points: Vec::new(),
            generated_paths: Vec::new(),
            reference_ranges: Vec::new(),
            reference_range_note: None,
        })
    }

    probe_runtime::hazard_free_language_facts!(true);
}
