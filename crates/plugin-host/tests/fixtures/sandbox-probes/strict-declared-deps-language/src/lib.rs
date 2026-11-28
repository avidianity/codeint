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

    probe_runtime::filter_manifest_paths!("package.json");

    fn analyze_manifests(
        manifests: Vec<Manifest>,
    ) -> exports::codeint::plugin::language::ManifestAnalysis {
        let mut analysis = probe_runtime::empty_manifest_analysis!();
        for manifest in &manifests {
            if !manifest.text.trim_start().starts_with('{') {
                analysis.invalid_manifests.push(WorkspaceError {
                    path: manifest.path.clone(),
                    message: "not valid json".to_string(),
                });
            }
        }
        analysis
    }

    probe_runtime::unresolved_imports_stub!();
    probe_runtime::empty_entry_points_and_ranges!();
    probe_runtime::empty_framework_profile_stubs!();
    probe_runtime::hazard_free_language_facts!(true);
    probe_runtime::v05_extra_language_stubs!();
}
