#![no_std]

extern crate alloc;
extern crate probe_runtime;

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;

wit_bindgen::generate!({
    world: "language-plugin",
    path: "../../../../../../wit",
});

use exports::codeint::plugin::language::{
    DeclaredDep, FileModel, Guest, ImportRequest, LowerError, Manifest, ReferenceRange, Resolution,
    SourceSet, WorkspaceError,
};

struct ProgramStartLanguagePlugin;

export!(ProgramStartLanguagePlugin);

impl Guest for ProgramStartLanguagePlugin {
    fn extensions() -> Vec<String> {
        vec!["prog".to_string()]
    }

    fn lower(path: String, text: String) -> Result<FileModel, LowerError> {
        if !path.ends_with(".prog") {
            return Err(LowerError::UnsupportedFile("unsupported".to_string()));
        }
        let source_set = if path.ends_with("_test.prog") || path.ends_with(".test.prog") {
            SourceSet::Test
        } else {
            SourceSet::Production
        };
        let source_lines = text.lines().filter(|line| !line.trim().is_empty()).count() as u32;
        let program_start = text.lines().any(|line| line.trim() == "program main");
        let runtime_imports = text
            .lines()
            .filter_map(|line| line.trim().strip_prefix("use "))
            .map(|spec| spec.trim().to_string())
            .filter(|spec| !spec.is_empty())
            .collect();
        Ok(FileModel {
            source_set,
            source_lines,
            functions: Vec::new(),
            tokens: Vec::new(),
            flagged_lines: Vec::new(),
            runtime_imports,
            program_start,
        })
    }

    fn manifest_paths(_repository_paths: Vec<String>) -> Vec<String> {
        Vec::new()
    }

    fn workspace_packages(_manifests: Vec<Manifest>) -> Result<Vec<String>, WorkspaceError> {
        Ok(Vec::new())
    }

    fn resolve_imports(
        repository_paths: Vec<String>,
        _manifests: Vec<Manifest>,
        requests: Vec<ImportRequest>,
    ) -> Vec<Resolution> {
        requests
            .into_iter()
            .map(|request| {
                let candidate = if let Some(relative) = request.specifier.strip_prefix("./") {
                    match request.importer.rsplit_once('/') {
                        Some((dir, _)) => format!("{dir}/{relative}"),
                        None => relative.to_string(),
                    }
                } else {
                    request.specifier
                };
                let with_ext = if candidate.ends_with(".prog") {
                    candidate.clone()
                } else {
                    format!("{candidate}.prog")
                };
                if repository_paths.iter().any(|path| path == &candidate) {
                    Resolution::Resolved(vec![candidate])
                } else if repository_paths.iter().any(|path| path == &with_ext) {
                    Resolution::Resolved(vec![with_ext])
                } else {
                    Resolution::Unresolved(format!("cannot find {with_ext}"))
                }
            })
            .collect()
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
        false
    }

    fn hazard_rules() -> Vec<String> {
        Vec::new()
    }
}
