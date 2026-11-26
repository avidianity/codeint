#![forbid(unsafe_code)]

mod builtins;
mod dependencies;
mod entry_points;
mod frameworks;
mod functions;
mod imports;
mod lexer;
mod resolution;
mod rules;
mod source_set;
mod workspaces;

use std::collections::BTreeSet;

use codeint_model::language::{
    DeclaredDep, FileModel, FrameworkProfileResult, FrameworkProfileSelection, Guest,
    ImportRequest, LowerError, Manifest, ReferenceRange, Resolution, Token, TokenKind,
    WorkspaceError,
};
use codeint_model::token_text_hash;

use crate::dependencies::extract_declared;
use crate::entry_points::extract as extract_entry_points;
use crate::lexer::{RawKind, RawToken};
use crate::resolution::{manifest_paths as find_manifest_paths, resolve_batch_with_profiles};

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
    fn known_framework_profiles() -> Vec<String> {
        frameworks::known_profiles()
    }

    fn resolve_framework_profiles(
        package_root: String,
        files: Vec<String>,
        manifests: Vec<Manifest>,
        selection: FrameworkProfileSelection,
    ) -> Result<FrameworkProfileResult, String> {
        frameworks::resolve_framework_profiles(&package_root, &files, &manifests, selection)
    }

    fn reference_ranges() -> Vec<ReferenceRange> {
        codeint_model::scan::build_reference_ranges(
            "codeint stratified panel of 50 PHP repositories, ADR 0028, codeint 0.6.0",
            [
                ("erosion", 1, 0.084, 0.630),
                ("verbosity", 2, 0.000, 0.005),
                ("duplication", 1, 0.017, 0.189),
                ("import_cycles", 1, 0.000, 0.145),
            ],
        )
    }

    fn extensions() -> Vec<String> {
        vec!["php".to_owned()]
    }

    fn manifest_paths(repository_paths: Vec<String>) -> Vec<String> {
        find_manifest_paths(&repository_paths)
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
            hazard_lines: Vec::new(),
            runtime_imports: extracted.specifiers,
            program_start: false,
        })
    }

    fn resolve_imports(
        repository_paths: Vec<String>,
        manifests: Vec<Manifest>,
        requests: Vec<ImportRequest>,
        active_profiles: Vec<String>,
    ) -> Vec<Resolution> {
        resolve_batch_with_profiles(&repository_paths, &manifests, &requests, &active_profiles)
    }

    fn declared_dependencies(manifests: Vec<Manifest>) -> Vec<DeclaredDep> {
        extract_declared(&manifests)
    }

    fn entry_points(repository_paths: Vec<String>, manifests: Vec<Manifest>) -> Vec<String> {
        extract_entry_points(&repository_paths, &manifests)
    }

    fn has_hazard_rules() -> bool {
        false
    }

    fn hazard_rules() -> Vec<String> {
        Vec::new()
    }

    fn has_imports() -> bool {
        true
    }
}
