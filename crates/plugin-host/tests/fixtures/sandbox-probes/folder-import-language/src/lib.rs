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
    DeclaredDep, FileModel, Function, Guest, ImportRequest, LowerError, Manifest, ReferenceRange,
    Resolution, SourceSet, Token, TokenKind, WorkspaceError,
};

struct FolderImportLanguagePlugin;

export!(FolderImportLanguagePlugin);

fn fnv1a_64(text: &str) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in text.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    hash
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
        let mut source_lines: u32 = 0;
        let mut functions = Vec::new();
        let mut tokens = Vec::new();
        let mut runtime_imports = Vec::new();
        let mut program_start = false;

        for (index, raw_line) in text.lines().enumerate() {
            let line_no = (index + 1) as u32;
            let trimmed = raw_line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            if trimmed == "SYNTAX_ERROR" {
                return Err(LowerError::Unparsable("syntax error".to_string()));
            }
            source_lines += 1;
            if trimmed == "program main" {
                program_start = true;
            }
            let is_import = if let Some(specifier) = trimmed.strip_prefix("use ") {
                let spec = specifier.trim();
                if !spec.is_empty() {
                    runtime_imports.push(spec.to_string());
                }
                true
            } else {
                false
            };
            if let Some(rest) = trimmed.strip_prefix("fn ") {
                let name = rest
                    .split(|ch: char| ch == '(' || ch.is_whitespace())
                    .next()
                    .filter(|name| !name.is_empty())
                    .map(ToString::to_string);
                functions.push(Function {
                    line: line_no,
                    end_line: line_no,
                    decision_points: 0,
                    source_lines: 1,
                    name,
                    cognitive_complexity: 0,
                    coverage: None,
                });
            }
            let kind = if is_import {
                TokenKind::ImportOrExport
            } else {
                TokenKind::Other
            };
            for word in trimmed.split_whitespace() {
                tokens.push(Token {
                    kind,
                    text_hash: fnv1a_64(word),
                    line: line_no,
                });
            }
        }

        Ok(FileModel {
            source_set,
            source_lines,
            functions,
            tokens,
            flagged_lines: Vec::new(),
            runtime_imports,
            program_start,
        })
    }

    fn manifest_paths(repository_paths: Vec<String>) -> Vec<String> {
        repository_paths
            .into_iter()
            .filter(|path| path == "fld.workspace" || path.ends_with("/fld.workspace"))
            .collect()
    }

    fn workspace_packages(manifests: Vec<Manifest>) -> Result<Vec<String>, WorkspaceError> {
        let mut packages = Vec::new();
        for manifest in manifests {
            for line in manifest.text.lines() {
                let trimmed = line.trim();
                if trimmed.is_empty() || trimmed.starts_with("entry:") {
                    continue;
                }
                if let Some(problem) = trimmed.strip_prefix("INVALID:") {
                    return Err(WorkspaceError {
                        path: manifest.path,
                        message: problem.trim().to_string(),
                    });
                }
                packages.push(trimmed.to_string());
            }
        }
        Ok(packages)
    }

    fn resolve_imports(
        repository_paths: Vec<String>,
        _manifests: Vec<Manifest>,
        requests: Vec<ImportRequest>,
    ) -> Vec<Resolution> {
        requests
            .into_iter()
            .map(|request| {
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
            })
            .collect()
    }

    fn declared_dependencies(_manifests: Vec<Manifest>) -> Vec<DeclaredDep> {
        Vec::new()
    }

    fn entry_points(repository_paths: Vec<String>, manifests: Vec<Manifest>) -> Vec<String> {
        let mut entries = Vec::new();
        for manifest in manifests {
            for line in manifest.text.lines() {
                if let Some(entry) = line.trim().strip_prefix("entry:") {
                    let candidate = entry.trim();
                    if repository_paths.iter().any(|path| path == candidate) {
                        entries.push(candidate.to_string());
                    }
                }
            }
        }
        entries
    }

    fn reference_ranges() -> Vec<ReferenceRange> {
        vec![ReferenceRange {
            metric: "erosion".to_string(),
            low: 0.0,
            high: 0.2,
            source: "folder import language fixture".to_string(),
        }]
    }
}
