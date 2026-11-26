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

    fn manifest_paths(repository_paths: Vec<String>) -> Vec<String> {
        repository_paths
            .into_iter()
            .filter(|path| path == "package.json" || path.ends_with("/package.json"))
            .collect()
    }

    fn workspace_packages(manifests: Vec<Manifest>) -> Result<Vec<String>, WorkspaceError> {
        for manifest in &manifests {
            if !manifest.text.trim_start().starts_with('{') {
                return Err(WorkspaceError {
                    path: manifest.path.clone(),
                    message: "not valid json".to_string(),
                });
            }
        }
        Ok(Vec::new())
    }

    probe_runtime::unresolved_imports_stub!();

    fn declared_dependencies(manifests: Vec<Manifest>) -> Vec<DeclaredDep> {
        for manifest in &manifests {
            if !manifest.text.trim_start().starts_with('{') {
                panic!("received invalid manifest that workspace-packages already rejected");
            }
        }
        Vec::new()
    }

    fn entry_points(_repository_paths: Vec<String>, _manifests: Vec<Manifest>) -> Vec<String> {
        Vec::new()
    }

    fn reference_ranges() -> Vec<ReferenceRange> {
        Vec::new()
    }

    probe_runtime::empty_framework_profile_stubs!();

    probe_runtime::hazard_free_language_facts!(true);
}
