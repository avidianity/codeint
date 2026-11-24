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
    DeclaredDep, FileModel, Guest, HazardLine, ImportRequest, LowerError, Manifest, ReferenceRange,
    Resolution, SourceSet, WorkspaceError,
};

struct HazardRulesLanguagePlugin;

export!(HazardRulesLanguagePlugin);

impl Guest for HazardRulesLanguagePlugin {
    fn extensions() -> Vec<String> {
        vec!["haz".to_string()]
    }

    fn lower(path: String, text: String) -> Result<FileModel, LowerError> {
        if !path.ends_with(".haz") {
            return Err(LowerError::UnsupportedFile("unsupported".to_string()));
        }
        let source_lines = text.lines().filter(|line| !line.trim().is_empty()).count() as u32;
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
        }
        Ok(FileModel {
            source_set: SourceSet::Production,
            source_lines,
            functions: Vec::new(),
            tokens: Vec::new(),
            flagged_lines: Vec::new(),
            hazard_lines,
            runtime_imports: Vec::new(),
            program_start: false,
        })
    }

    fn manifest_paths(_repository_paths: Vec<String>) -> Vec<String> {
        Vec::new()
    }

    fn workspace_packages(_manifests: Vec<Manifest>) -> Result<Vec<String>, WorkspaceError> {
        Ok(Vec::new())
    }

    fn resolve_imports(
        _repository_paths: Vec<String>,
        _manifests: Vec<Manifest>,
        _requests: Vec<ImportRequest>,
    ) -> Vec<Resolution> {
        Vec::new()
    }

    fn declared_dependencies(_manifests: Vec<Manifest>) -> Vec<DeclaredDep> {
        Vec::new()
    }

    fn entry_points(_repository_paths: Vec<String>, _manifests: Vec<Manifest>) -> Vec<String> {
        Vec::new()
    }

    fn reference_ranges() -> Vec<ReferenceRange> {
        Vec::new()
    }

    fn has_hazard_rules() -> bool {
        true
    }

    fn hazard_rules() -> Vec<String> {
        vec![
            "delete-without-where".to_string(),
            "leading-wildcard-like".to_string(),
        ]
    }
}
