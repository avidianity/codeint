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
    probe_runtime::profile_resolving_language! {
        manifest_paths: Vec::new(),
        profiles: ["alpha", "beta", "gamma"],
        resolve: |manifests, selection| {
            let _ = &manifests;
            match &selection {
                FrameworkProfileSelection::Only(names) if names.len() == 2 => {
                    Some("two-profile only selection is not handled".to_string())
                }
                _ => None,
            }
        }
    }
}
