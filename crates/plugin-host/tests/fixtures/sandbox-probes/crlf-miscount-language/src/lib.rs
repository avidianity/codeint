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
    DeclaredDep, FileModel, FrameworkProfileResult, FrameworkProfileSelection, Function, Guest,
    ImportRequest, LowerError, Manifest, ReferenceRange, Resolution, SourceSet, WorkspaceError,
};

struct CrlfMiscountLanguagePlugin;

export!(CrlfMiscountLanguagePlugin);

impl Guest for CrlfMiscountLanguagePlugin {
    fn extensions() -> Vec<String> {
        vec!["crl".to_string()]
    }

    fn lower(path: String, text: String) -> Result<FileModel, LowerError> {
        if !path.ends_with(".crl") {
            return Err(LowerError::UnsupportedFile("unsupported".to_string()));
        }
        let source_lines = probe_runtime::count_non_empty_lines(&text);
        let breaks = text.bytes().filter(|byte| *byte == b'\n' || *byte == b'\r').count() as u32;
        let functions = if breaks == 0 {
            Vec::new()
        } else {
            vec![Function {
                line: 1,
                end_line: breaks,
                decision_points: 0,
                source_lines: 0,
                name: None,
                cognitive_complexity: 0,
                coverage: None,
            }]
        };
        Ok(probe_runtime::blank_file_model!(
            SourceSet::Production,
            source_lines,
            functions: functions,
        ))
    }

    probe_runtime::empty_language_metadata_stubs!();

    fn has_hazard_rules() -> bool {
        false
    }

    fn hazard_rules() -> Vec<String> {
        Vec::new()
    }

    fn has_imports() -> bool {
        false
    }
}
