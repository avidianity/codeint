#![forbid(unsafe_code)]

mod entry_points;
mod functions;
mod imports;
mod lexer;
mod resolution;
mod rules;
mod source_set;
mod workspaces;

use std::collections::BTreeSet;

use codeint_model::language::{
    DeclaredDep, FileModel, Guest, ImportRequest, LowerError, Manifest, ReferenceRange, Resolution,
    Token, TokenKind, WorkspaceError,
};
use codeint_model::token_text_hash;

use crate::lexer::{RawKind, RawToken};

pub use rules::flagged_line_rules;

pub struct Php;

fn build_tokens(raw_tokens: &[RawToken], import_indices: &BTreeSet<usize>) -> Vec<Token> {
    raw_tokens
        .iter()
        .enumerate()
        .map(|(idx, raw)| Token {
            kind: classify_token(raw, import_indices.contains(&idx)),
            text_hash: token_text_hash(&raw.text),
            line: raw.line,
        })
        .collect()
}

fn classify_token(raw: &RawToken, is_import: bool) -> TokenKind {
    if is_import {
        return TokenKind::ImportOrExport;
    }
    match raw.kind {
        RawKind::Ident | RawKind::Variable => TokenKind::Identifier,
        RawKind::StringLit | RawKind::NumberLit => TokenKind::Literal,
        RawKind::Keyword | RawKind::Punct => TokenKind::Other,
    }
}

impl Guest for Php {
    fn reference_ranges() -> Vec<ReferenceRange> {
        codeint_model::scan::build_reference_ranges(
            "codeint panel of 20 PHP repositories, ADR 0011, codeint 0.2.1",
            [
                ("erosion", 0.055, 0.677),
                ("verbosity", 0.042, 0.292),
                ("duplication", 0.041, 0.290),
                ("import_cycles", 0.000, 0.107),
            ],
        )
    }

    fn extensions() -> Vec<String> {
        vec!["php".to_owned()]
    }

    fn manifest_paths(repository_paths: Vec<String>) -> Vec<String> {
        resolution::manifest_paths(&repository_paths)
    }

    fn workspace_packages(manifests: Vec<Manifest>) -> Result<Vec<String>, WorkspaceError> {
        workspaces::workspace_packages(&manifests)
    }

    fn lower(path: String, text: String) -> Result<FileModel, LowerError> {
        if !path.ends_with(".php") {
            return Err(LowerError::UnsupportedFile(format!(
                "{path} is not a PHP source file"
            )));
        }
        let lexed = lexer::lex(&text)?;
        let extracted = imports::extract(&lexed.tokens);
        let source_lines =
            u32::try_from(lexed.code_lines.len()).map_err(|_| LowerError::TooLarge)?;
        Ok(FileModel {
            source_set: source_set::classify(&path, &lexed.header_comments),
            source_lines,
            functions: functions::functions(&lexed.tokens, &lexed.code_lines),
            tokens: build_tokens(&lexed.tokens, &extracted.import_token_indices),
            flagged_lines: rules::flag(&lexed.tokens, &lexed.code_lines),
            runtime_imports: extracted.specifiers,
        })
    }

    fn resolve_imports(
        repository_paths: Vec<String>,
        manifests: Vec<Manifest>,
        requests: Vec<ImportRequest>,
    ) -> Vec<Resolution> {
        resolution::resolve_batch(&repository_paths, &manifests, &requests)
    }

    fn declared_dependencies(_manifests: Vec<Manifest>) -> Vec<DeclaredDep> {
        Vec::new()
    }

    fn entry_points(repository_paths: Vec<String>, manifests: Vec<Manifest>) -> Vec<String> {
        entry_points::extract(&repository_paths, &manifests)
    }
}
