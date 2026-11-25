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
        workspace_packages: |manifests| {
            if let Some(binary) = manifests.iter().find(|manifest| manifest.text.contains('\0')) {
                return Err(WorkspaceError {
                    path: binary.path.clone(),
                    message: "binary manifest".to_string(),
                });
            }
            Ok(Vec::new())
        },
        framework_profiles: |selection| {
            let _ = &selection;
        }
    }
}
