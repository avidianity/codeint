#![forbid(unsafe_code)]

mod dependencies;
mod entry_points;
mod functions;
mod go_mod;
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
use codeint_model::scan::build_reference_ranges;
use codeint_model::token_text_hash;

use crate::lexer::{GoKind, GoToken};

pub use rules::flagged_line_rules;

pub struct Go;

impl Guest for Go {
    fn extensions() -> Vec<String> {
        vec!["go".to_owned()]
    }

    fn has_imports() -> bool {
        true
    }

    fn has_hazard_rules() -> bool {
        false
    }

    fn hazard_rules() -> Vec<String> {
        Vec::new()
    }

    fn reference_ranges() -> Vec<ReferenceRange> {
        build_reference_ranges(
            "codeint panel of 20 Go repositories, ADR 0021, codeint 0.6.0",
            [
                ("erosion", 0.379, 0.692),
                ("verbosity", 0.045, 0.183),
                ("duplication", 0.044, 0.181),
                ("import_cycles", 0.000, 0.000),
            ],
        )
    }

    fn lower(path: String, text: String) -> Result<FileModel, LowerError> {
        lower_file(&path, &text)
    }

    fn manifest_paths(repository_paths: Vec<String>) -> Vec<String> {
        resolution::find_go_mods(&repository_paths)
    }

    fn workspace_packages(manifests: Vec<Manifest>) -> Result<Vec<String>, WorkspaceError> {
        workspaces::discover(&manifests)
    }

    fn resolve_imports(
        repository_paths: Vec<String>,
        manifests: Vec<Manifest>,
        requests: Vec<ImportRequest>,
        _active_profiles: Vec<String>,
    ) -> Vec<Resolution> {
        resolution::resolve_all(&repository_paths, &manifests, &requests)
    }

    fn declared_dependencies(manifests: Vec<Manifest>) -> Vec<DeclaredDep> {
        dependencies::extract_declared(&manifests)
    }

    fn entry_points(repository_paths: Vec<String>, manifests: Vec<Manifest>) -> Vec<String> {
        entry_points::discover(&repository_paths, &manifests)
    }

    codeint_model::empty_framework_profile_guest_methods!();
}

fn lower_file(path: &str, text: &str) -> Result<FileModel, LowerError> {
    if !path.ends_with(".go") {
        return Err(LowerError::UnsupportedFile(format!(
            "{path} is not a Go source file"
        )));
    }
    let scanned = lexer::scan(text)?;
    let source_lines = u32::try_from(scanned.code_lines.len()).map_err(|_| LowerError::TooLarge)?;
    let scanned_imports = imports::scan(&scanned.tokens);
    let extracted = functions::extract(&scanned.tokens, &scanned.code_lines);
    let flagged_lines = rules::evaluate(&scanned.tokens, &extracted.spans, &scanned.code_lines);
    let tokens = build_duplication_tokens(&scanned.tokens, &scanned_imports.marked_indices);
    let source_set = source_set::classify(path, &scanned.header_line_comments);
    Ok(FileModel {
        runtime_imports: scanned_imports.specifiers,
        flagged_lines,
        hazard_lines: Vec::new(),
        tokens,
        functions: extracted.functions,
        source_lines,
        source_set,
        program_start: scanned_imports.is_package_main,
    })
}

fn build_duplication_tokens(raw: &[GoToken], import_indices: &BTreeSet<usize>) -> Vec<Token> {
    raw.iter()
        .enumerate()
        .filter(|(_, tok)| tok.kind != GoKind::AutoSemi)
        .map(|(idx, tok)| {
            let kind = if import_indices.contains(&idx) {
                TokenKind::ImportOrExport
            } else if tok.kind == GoKind::Ident {
                TokenKind::Identifier
            } else if matches!(
                tok.kind,
                GoKind::StringLit | GoKind::RuneLit | GoKind::NumberLit
            ) {
                TokenKind::Literal
            } else {
                TokenKind::Other
            };
            Token {
                kind,
                text_hash: token_text_hash(&tok.text),
                line: tok.line,
            }
        })
        .collect()
}
