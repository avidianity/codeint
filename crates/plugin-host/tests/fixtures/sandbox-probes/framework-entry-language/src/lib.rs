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
    probe_runtime::probe_claiming_language_stubs!();

    fn resolve_imports(
        _repository_paths: Vec<String>,
        _manifests: Vec<Manifest>,
        requests: Vec<ImportRequest>,
        _package_profiles: Vec<exports::codeint::plugin::language::PackageFrameworkProfiles>,
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
