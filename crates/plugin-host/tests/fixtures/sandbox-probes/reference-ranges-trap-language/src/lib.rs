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

    fn lower(
        _path: String,
        text: String,
        _dialect: Option<String>,
    ) -> Result<FileModel, LowerError> {
        Ok(probe_runtime::blank_file_model!(
            SourceSet::Production,
            u32::try_from(text.lines().count()).unwrap_or(u32::MAX),
        ))
    }

    fn manifest_paths(_repository_paths: Vec<String>) -> Vec<String> {
        Vec::new()
    }

    fn analyze_manifests(
        _manifests: Vec<Manifest>,
    ) -> exports::codeint::plugin::language::ManifestAnalysis {
        probe_runtime::empty_manifest_analysis!()
    }

    fn resolve_imports(
        _repository_paths: Vec<String>,
        _manifests: Vec<Manifest>,
        requests: Vec<ImportRequest>,
        _package_profiles: Vec<exports::codeint::plugin::language::PackageFrameworkProfiles>,
    ) -> Vec<Resolution> {
        requests
            .into_iter()
            .map(|request| Resolution::External(request.specifier))
            .collect()
    }

    fn entry_points(_repository_paths: Vec<String>, _manifests: Vec<Manifest>) -> Vec<String> {
        Vec::new()
    }

    fn reference_ranges() -> Vec<ReferenceRange> {
        panic!("reference-ranges trapped")
    }

    probe_runtime::empty_framework_profile_stubs!();

    probe_runtime::hazard_free_language_facts!(false);

    probe_runtime::v05_extra_language_stubs!();
}
