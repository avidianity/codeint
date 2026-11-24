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
    DeclaredDep, FileModel, Function, Guest, ImportRequest, LowerError, Manifest, ReferenceRange,
    Resolution, SourceSet, Token, TokenKind, WorkspaceError,
};

struct NoImportsLanguagePlugin;

export!(NoImportsLanguagePlugin);

fn fnv1a_64(text: &str) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in text.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    hash
}

impl Guest for NoImportsLanguagePlugin {
    fn extensions() -> Vec<String> {
        vec!["noimp".to_string()]
    }

    fn lower(path: String, text: String) -> Result<FileModel, LowerError> {
        if !path.ends_with(".noimp") {
            return Err(LowerError::UnsupportedFile("unsupported".to_string()));
        }
        if text.contains("SYNTAX_ERROR") {
            return Err(LowerError::Unparsable("syntax error".to_string()));
        }
        let source_set = if path.ends_with("_test.noimp") || path.ends_with(".test.noimp") {
            SourceSet::Test
        } else {
            SourceSet::Production
        };
        let mut source_lines: u32 = 0;
        let mut functions = Vec::new();
        let mut tokens = Vec::new();
        for (index, raw_line) in text.lines().enumerate() {
            let trimmed = raw_line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            let line_number = (index + 1) as u32;
            source_lines += 1;
            if let Some(rest) = trimmed.strip_prefix("fn ") {
                let name = rest
                    .split(|ch: char| ch == '(' || ch.is_whitespace())
                    .next()
                    .unwrap_or("")
                    .to_string();
                functions.push(Function {
                    line: line_number,
                    end_line: line_number,
                    decision_points: 0,
                    source_lines: 1,
                    name: if name.is_empty() { None } else { Some(name) },
                    cognitive_complexity: 0,
                    coverage: None,
                });
            }
            for word in trimmed.split_whitespace() {
                let kind = if word == "fn" {
                    TokenKind::Other
                } else {
                    TokenKind::Identifier
                };
                tokens.push(Token {
                    kind,
                    text_hash: fnv1a_64(word),
                    line: line_number,
                });
            }
        }
        Ok(FileModel {
            source_set,
            source_lines,
            functions,
            tokens,
            flagged_lines: Vec::new(),
            hazard_lines: Vec::new(),
            runtime_imports: Vec::new(),
            program_start: false,
        })
    }

    fn manifest_paths(_repository_paths: Vec<String>) -> Vec<String> {
        panic!("manifest-paths must not be called when has-imports is false");
    }

    fn workspace_packages(_manifests: Vec<Manifest>) -> Result<Vec<String>, WorkspaceError> {
        panic!("workspace-packages must not be called when has-imports is false");
    }

    fn resolve_imports(
        _repository_paths: Vec<String>,
        _manifests: Vec<Manifest>,
        _requests: Vec<ImportRequest>,
    ) -> Vec<Resolution> {
        panic!("resolve-imports must not be called when has-imports is false");
    }

    fn declared_dependencies(_manifests: Vec<Manifest>) -> Vec<DeclaredDep> {
        panic!("declared-dependencies must not be called when has-imports is false");
    }

    fn entry_points(_repository_paths: Vec<String>, _manifests: Vec<Manifest>) -> Vec<String> {
        panic!("entry-points must not be called when has-imports is false");
    }

    fn reference_ranges() -> Vec<ReferenceRange> {
        vec![ReferenceRange {
            metric: "erosion".to_string(),
            low: 0.0,
            high: 0.2,
            source: "no-imports language fixture".to_string(),
        }]
    }

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
