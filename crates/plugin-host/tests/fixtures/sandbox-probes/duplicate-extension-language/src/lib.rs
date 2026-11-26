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

struct DuplicateExtensionLanguagePlugin;

export!(DuplicateExtensionLanguagePlugin);

impl Guest for DuplicateExtensionLanguagePlugin {
    fn extensions() -> Vec<String> {
        vec!["dup".to_string(), "dup".to_string()]
    }

    fn lower(path: String, text: String) -> Result<FileModel, LowerError> {
        if !path.ends_with(".dup") {
            return Err(LowerError::UnsupportedFile("unsupported".to_string()));
        }
        Ok(probe_runtime::blank_file_model!(
            SourceSet::Production,
            probe_runtime::count_non_empty_lines(&text)
        ))
    }

    probe_runtime::empty_language_metadata_stubs!();

    fn has_hazard_rules() -> bool {
        true
    }

    fn hazard_rules() -> Vec<String> {
        vec!["same-rule".to_string(), "same-rule".to_string()]
    }

    fn has_imports() -> bool {
        false
    }
}
