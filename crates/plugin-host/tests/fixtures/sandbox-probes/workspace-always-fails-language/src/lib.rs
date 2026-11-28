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
    probe_runtime::faulty_import_language! {
        extensions: vec!["probe".to_string()],
        manifest_paths: vec!["package.json".to_string()],
        analyze_manifests: |manifests| {
            let _ = &manifests;
            let mut analysis = probe_runtime::empty_manifest_analysis!();
            analysis.invalid_manifests.push(WorkspaceError {
                path: "package.json".to_string(),
                message: "always fails".to_string(),
            });
            analysis
        },
        framework_profiles: |selection| {
            let _ = &selection;
        }
    }
}
