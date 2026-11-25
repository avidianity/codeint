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

struct UnlistedHazardRuleLanguagePlugin;

export!(UnlistedHazardRuleLanguagePlugin);

impl Guest for UnlistedHazardRuleLanguagePlugin {
    fn extensions() -> Vec<String> {
        vec!["uhr".to_string()]
    }

    fn lower(path: String, text: String) -> Result<FileModel, LowerError> {
        if !path.ends_with(".uhr") {
            return Err(LowerError::UnsupportedFile("unsupported".to_string()));
        }
        let source_lines = probe_runtime::count_non_empty_lines(&text);
        let mut hazard_lines = Vec::new();
        for (index, line) in text.lines().enumerate() {
            let line_number = (index + 1) as u32;
            if line.contains("ALLOWED_HAZARD") {
                hazard_lines.push(HazardLine {
                    line: line_number,
                    rule: "allowed-rule".to_string(),
                });
            }
            if line.contains("UNLISTED_HAZARD") {
                hazard_lines.push(HazardLine {
                    line: line_number,
                    rule: "unlisted-secret-rule".to_string(),
                });
            }
        }
        Ok(probe_runtime::blank_file_model!(
            SourceSet::Production,
            source_lines,
            hazard_lines: hazard_lines,
        ))
    }

    probe_runtime::empty_language_metadata_stubs!();

    fn has_hazard_rules() -> bool {
        true
    }

    fn hazard_rules() -> Vec<String> {
        vec!["allowed-rule".to_string()]
    }

    fn has_imports() -> bool {
        true
    }
}
