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
        manifest_paths: vec!["src/main.probe".to_string()],
        analyze_manifests: |manifests| {
            let mut analysis = probe_runtime::empty_manifest_analysis!();
            for binary in manifests.into_iter().filter(|manifest| manifest.text.contains('\0')) {
                analysis.invalid_manifests.push(WorkspaceError {
                    path: binary.path,
                    message: "binary manifest".to_string(),
                });
            }
            analysis
        },
        framework_profiles: |selection| {
            let _ = &selection;
        }
    }
}
