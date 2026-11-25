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
    DeclaredDep, FileModel, FrameworkProfileResult, FrameworkProfileSelection, Function, Guest,
    ImportRequest, LowerError, Manifest, ReferenceRange, Resolution, SourceSet, Token, TokenKind,
    WorkspaceError,
};

struct FolderImportLanguagePlugin;

export!(FolderImportLanguagePlugin);

struct LowerAccumulator {
    source_lines: u32,
    functions: Vec<Function>,
    tokens: Vec<Token>,
    runtime_imports: Vec<String>,
    program_start: bool,
}

impl LowerAccumulator {
    fn new() -> Self {
        Self {
            source_lines: 0,
            functions: Vec::new(),
            tokens: Vec::new(),
            runtime_imports: Vec::new(),
            program_start: false,
        }
    }

    fn process_line(&mut self, line_no: u32, raw_line: &str) -> Result<(), LowerError> {
        let trimmed = raw_line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            return Ok(());
        }
        if trimmed == "SYNTAX_ERROR" {
            return Err(LowerError::Unparsable("syntax error".to_string()));
        }
        self.source_lines += 1;
        if trimmed == "program main" {
            self.program_start = true;
        }
        let kind = if let Some(specifier) = trimmed.strip_prefix("use ") {
            let spec = specifier.trim();
            if !spec.is_empty() {
                self.runtime_imports.push(spec.to_string());
            }
            TokenKind::ImportOrExport
        } else {
            TokenKind::Other
        };
        if let Some(rest) = trimmed.strip_prefix("fn ") {
            self.functions.push(Function {
                line: line_no,
                end_line: line_no,
                decision_points: 0,
                source_lines: 1,
                name: probe_runtime::parse_fn_name(rest),
                cognitive_complexity: 0,
                coverage: None,
            });
        }
        for word in trimmed.split_whitespace() {
            self.tokens.push(Token {
                kind,
                text_hash: probe_runtime::fnv1a_64(word),
                line: line_no,
            });
        }
        Ok(())
    }
}

fn resolve_folder_import(repository_paths: &[String], request: ImportRequest) -> Resolution {
    if request.specifier == "empty" {
        return Resolution::Resolved(Vec::new());
    }
    if request.specifier == "partial-missing" {
        let mut targets = Vec::new();
        if let Some(first) = repository_paths.first() {
            targets.push(first.clone());
        }
        targets.push("missing/does_not_exist.fld".to_string());
        return Resolution::Resolved(targets);
    }
    if request.specifier.starts_with("std:") {
        return Resolution::Builtin;
    }
    if let Some(pkg) = request.specifier.strip_prefix("ext:") {
        return Resolution::External(pkg.to_string());
    }
    let prefix = format!("{}/", request.specifier);
    let mut targets: Vec<String> = repository_paths
        .iter()
        .filter(|path| {
            path.as_str() != request.importer.as_str()
                && path.ends_with(".fld")
                && path
                    .strip_prefix(&prefix)
                    .is_some_and(|rest| !rest.contains('/'))
        })
        .cloned()
        .collect();
    targets.sort();
    if targets.is_empty() {
        Resolution::Unresolved(format!("no folder matches {:?}", request.specifier))
    } else {
        Resolution::Resolved(targets)
    }
}

impl Guest for FolderImportLanguagePlugin {
    fn extensions() -> Vec<String> {
        vec!["fld".to_string()]
    }

    fn lower(path: String, text: String) -> Result<FileModel, LowerError> {
        if !path.ends_with(".fld") {
            return Err(LowerError::UnsupportedFile("unsupported".to_string()));
        }
        let source_set = if path.ends_with("_test.fld") {
            SourceSet::Test
        } else {
            SourceSet::Production
        };
        let mut acc = LowerAccumulator::new();
        for (index, raw_line) in text.lines().enumerate() {
            acc.process_line((index + 1) as u32, raw_line)?;
        }
        Ok(probe_runtime::blank_file_model!(
            source_set,
            acc.source_lines,
            functions: acc.functions,
            tokens: acc.tokens,
            runtime_imports: acc.runtime_imports,
            program_start: acc.program_start,
        ))
    }

    fn manifest_paths(repository_paths: Vec<String>) -> Vec<String> {
        repository_paths
            .into_iter()
            .filter(|path| path == "fld.workspace" || path.ends_with("/fld.workspace"))
            .collect()
    }

    fn workspace_packages(manifests: Vec<Manifest>) -> Result<Vec<String>, WorkspaceError> {
        let pairs: Vec<(&str, &str)> = manifests
            .iter()
            .map(|m| (m.path.as_str(), m.text.as_str()))
            .collect();
        probe_runtime::parse_workspace_manifest_lines(&pairs, "entry:")
            .map_err(|(path, message)| WorkspaceError { path, message })
    }

    fn resolve_imports(
        repository_paths: Vec<String>,
        _manifests: Vec<Manifest>,
        requests: Vec<ImportRequest>,
        _active_profiles: Vec<String>,
    ) -> Vec<Resolution> {
        requests
            .into_iter()
            .map(|request| resolve_folder_import(&repository_paths, request))
            .collect()
    }

    fn declared_dependencies(_manifests: Vec<Manifest>) -> Vec<DeclaredDep> {
        Vec::new()
    }

    fn entry_points(repository_paths: Vec<String>, manifests: Vec<Manifest>) -> Vec<String> {
        let texts: Vec<&str> = manifests.iter().map(|m| m.text.as_str()).collect();
        probe_runtime::extract_manifest_entries(&repository_paths, &texts)
    }

    probe_runtime::fixture_erosion_ranges!("folder import language fixture", true);
}
