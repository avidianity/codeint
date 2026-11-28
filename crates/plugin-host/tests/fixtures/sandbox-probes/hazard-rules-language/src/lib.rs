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

struct HazardRulesLanguagePlugin;

export!(HazardRulesLanguagePlugin);

impl Guest for HazardRulesLanguagePlugin {
    probe_runtime::lower_with_extension!("haz", |_path, text| {
        let line_count = text.lines().count() as u32;
        let source_lines = probe_runtime::count_non_empty_lines(&text);
        let mut hazard_lines = Vec::new();
        for (index, line) in text.lines().enumerate() {
            let line_number = (index + 1) as u32;
            if line.contains("DELETE FROM") && !line.contains("WHERE") {
                hazard_lines.push(HazardLine {
                    line: line_number,
                    rule: "delete-without-where".to_string(),
                });
            }
            if line.contains("LIKE '%") {
                hazard_lines.push(HazardLine {
                    line: line_number,
                    rule: "leading-wildcard-like".to_string(),
                });
            }
            if line.contains("HAZARD_BEYOND_EOF") {
                hazard_lines.push(HazardLine {
                    line: line_count + 1,
                    rule: "delete-without-where".to_string(),
                });
            }
            if line.contains("HAZARD_LINE_ZERO") {
                hazard_lines.push(HazardLine {
                    line: 0,
                    rule: "delete-without-where".to_string(),
                });
            }
        }
        Ok(probe_runtime::blank_file_model!(
            SourceSet::Production,
            source_lines,
            hazard_lines: hazard_lines,
        ))
    });

    probe_runtime::hazard_language_tail!("delete-without-where", "leading-wildcard-like");
}
