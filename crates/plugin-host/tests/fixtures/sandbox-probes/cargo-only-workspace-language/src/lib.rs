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
        manifest_paths: Vec::new(),
        workspace_packages: |manifests| {
            match manifests.iter().find(|manifest| manifest.path != "Cargo.toml") {
                Some(manifest) => Err(WorkspaceError {
                    path: manifest.path.clone(),
                    message: "not a Cargo manifest".to_string(),
                }),
                None => Ok(Vec::new()),
            }
        },
        framework_profiles: |selection| {
            let _ = &selection;
        }
    }
}
