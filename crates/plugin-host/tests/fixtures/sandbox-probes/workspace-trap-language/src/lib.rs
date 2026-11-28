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
        manifest_paths: vec!["Cargo.toml".to_string()],
        analyze_manifests: |manifests| {
            if manifests.iter().any(|manifest| manifest.path.ends_with("Cargo.toml")) {
                panic!("analyze-manifests trapped on Cargo.toml");
            }
            probe_runtime::empty_manifest_analysis!()
        },
        framework_profiles: |selection| {
            let _ = &selection;
        }
    }
}
