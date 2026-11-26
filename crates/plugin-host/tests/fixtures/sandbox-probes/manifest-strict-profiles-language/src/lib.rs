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
        manifest_paths: vec!["Cargo.toml".to_string()],
        profiles: ["alpha"],
        resolve: |manifests, selection| {
            let _ = &selection;
            manifests
                .iter()
                .find(|manifest| manifest.path != "Cargo.toml")
                .map(|manifest| alloc::format!("cannot parse {} as TOML", manifest.path))
        }
    }
}
