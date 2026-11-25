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
        vec!["invws".to_string()]
    }

    fn lower(path: String, text: String) -> Result<FileModel, LowerError> {
        if path.ends_with(".invws") {
            let lines = probe_runtime::count_non_empty_lines(&text);
            Ok(probe_runtime::blank_file_model!(
                SourceSet::Production,
                lines
            ))
        } else {
            Err(LowerError::UnsupportedFile(path))
        }
    }

    fn manifest_paths(_repository_paths: Vec<String>) -> Vec<String> {
        Vec::new()
    }

    fn workspace_packages(manifests: Vec<Manifest>) -> Result<Vec<String>, WorkspaceError> {
        if manifests
            .iter()
            .any(|manifest| manifest.path.ends_with("package.json"))
        {
            Ok(vec!["../outside".to_string()])
        } else {
            Ok(Vec::new())
        }
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
        _manifests: Vec<Manifest>,
        selection: FrameworkProfileSelection,
    ) -> Result<FrameworkProfileResult, String> {
        let _ = &selection;
        Ok(FrameworkProfileResult {
            active_profiles: Vec::new(),
            entry_points: Vec::new(),
            generated_paths: Vec::new(),
            reference_ranges: Vec::new(),
            reference_range_note: None,
        })
    }

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
