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
    DeclaredDep, FileModel, FrameworkProfileResult, FrameworkProfileSelection, Guest, HazardLine,
    ImportRequest, LowerError, Manifest, ReferenceRange, Resolution, SourceSet, WorkspaceError,
};

struct SecondExtensionLanguagePlugin;

export!(SecondExtensionLanguagePlugin);

impl Guest for SecondExtensionLanguagePlugin {
    fn extensions() -> Vec<String> {
        vec!["aaa".to_string(), "bbb".to_string()]
    }

    fn lower(
        path: String,
        text: String,
        _dialect: Option<String>,
    ) -> Result<FileModel, LowerError> {
        let source_lines = probe_runtime::count_non_empty_lines(&text);
        if path.ends_with(".aaa") {
            return Ok(probe_runtime::blank_file_model!(
                SourceSet::Production,
                source_lines
            ));
        }
        if path.ends_with(".bbb") {
            let past_the_end = (text.lines().count() + 1) as u32;
            return Ok(probe_runtime::blank_file_model!(
                SourceSet::Production,
                source_lines,
                hazard_lines: vec![HazardLine {
                    line: past_the_end,
                    rule: "listed-rule".to_string(),
                }],
            ));
        }
        Err(LowerError::UnsupportedFile("unsupported".to_string()))
    }

    probe_runtime::empty_language_metadata_stubs!();

    fn has_hazard_rules() -> bool {
        true
    }

    fn hazard_rules() -> Vec<String> {
        vec!["listed-rule".to_string()]
    }

    fn has_imports() -> bool {
        false
    }
}
