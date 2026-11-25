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

    fn lower(path: String, text: String) -> Result<FileModel, LowerError> {
        if path.ends_with(".probe") {
            let lines = probe_runtime::count_non_empty_lines(&text);
            Ok(probe_runtime::blank_file_model!(SourceSet::Production, lines))
        } else {
            Err(LowerError::UnsupportedFile(path))
        }
    }

    fn manifest_paths(_repository_paths: Vec<String>) -> Vec<String> {
        Vec::new()
    }

    fn workspace_packages(_manifests: Vec<Manifest>) -> Result<Vec<String>, WorkspaceError> {
        Ok(Vec::new())
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

    fn has_hazard_rules() -> bool {
        false
    }

    fn hazard_rules() -> Vec<String> {
        Vec::new()
    }

    fn has_imports() -> bool {
        true
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

    fn resolve_framework_profiles(
        _package_root: String,
        files: Vec<String>,
        _manifests: Vec<Manifest>,
        _selection: FrameworkProfileSelection,
    ) -> Result<FrameworkProfileResult, String> {
        let entry_points = if files.iter().any(|file| file.ends_with(".probe")) {
            vec!["src/not-in-repo.probe".to_string()]
        } else {
            Vec::new()
        };
        Ok(FrameworkProfileResult {
            active_profiles: Vec::new(),
            entry_points,
            generated_paths: Vec::new(),
            reference_ranges: Vec::new(),
            reference_range_note: None,
        })
    }
}
