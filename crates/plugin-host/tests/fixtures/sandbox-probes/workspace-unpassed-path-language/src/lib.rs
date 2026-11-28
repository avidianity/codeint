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

probe_runtime::package_json_manifest_probe!(|manifests| {
    let mut analysis = probe_runtime::empty_manifest_analysis!();
    if manifests
        .iter()
        .any(|manifest| !manifest.text.trim_start().starts_with('{'))
    {
        analysis.invalid_manifests.push(WorkspaceError {
            path: "wrong/unpassed.json".to_string(),
            message: "not valid json".to_string(),
        });
    }
    analysis
});
