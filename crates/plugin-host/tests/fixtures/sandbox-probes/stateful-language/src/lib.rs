#![no_std]

extern crate alloc;
extern crate probe_runtime;

use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicU32, Ordering};

wit_bindgen::generate!({
    world: "language-plugin",
    path: "../../../../../../wit",
});

use exports::codeint::plugin::language::{
    DeclaredDep, FileModel, FrameworkProfileResult, FrameworkProfileSelection, Guest,
    ImportRequest, LowerError, Manifest, ReferenceRange, Resolution, SourceSet, WorkspaceError,
};

static CALL_COUNT: AtomicU32 = AtomicU32::new(0);

struct Probe;

export!(Probe);

impl Guest for Probe {
    fn extensions() -> Vec<String> {
        vec!["stateful".to_string()]
    }

    fn lower(
        _path: String,
        _text: String,
        _dialect: Option<String>,
    ) -> Result<FileModel, LowerError> {
        let count = CALL_COUNT.fetch_add(1, Ordering::SeqCst) + 1;
        Ok(probe_runtime::blank_file_model!(
            SourceSet::Production,
            count,
        ))
    }

    probe_runtime::empty_language_metadata_stubs!();
    probe_runtime::hazard_free_language_facts!(false);
}
