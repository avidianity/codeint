#![forbid(unsafe_code)]

mod dependencies;
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

use crate::dependencies::extract_declared;
use crate::entry_points::discover as discover_entry_points;
use crate::lexer::{DartKind, DartToken};

pub use rules::flagged_line_rules;

pub struct Dart;

fn lower_tokens(raw: &[DartToken], import_spans: &BTreeSet<usize>) -> Vec<Token> {
    let mut out = Vec::with_capacity(raw.len());
    for (idx, tok) in raw.iter().enumerate() {
        let kind = if import_spans.contains(&idx) {
            TokenKind::ImportOrExport
        } else {
            map_token_kind(tok.kind)
        };
        out.push(Token {
            kind,
            text_hash: token_text_hash(&tok.text),
            line: tok.line,
        });
    }
    out
}

fn map_token_kind(kind: DartKind) -> TokenKind {
    match kind {
        DartKind::Ident => TokenKind::Identifier,
        DartKind::StringLit | DartKind::NumberLit => TokenKind::Literal,
        DartKind::Keyword | DartKind::Punct => TokenKind::Other,
    }
}

fn lower_dart(path: &str, text: &str) -> Result<FileModel, LowerError> {
    if !path.ends_with(".dart") {
        return Err(LowerError::UnsupportedFile(format!(
            "{path} is not a Dart source file"
        )));
    }
    let lexed = lexer::scan(text)?;
    let (runtime_imports, marked_indices) = imports::scan_directives(&lexed.tokens);
    let functions = functions::extract(&lexed.tokens, &lexed.code_lines);
    let flagged_lines = rules::evaluate(&lexed.tokens, &lexed.code_lines);
    let tokens = lower_tokens(&lexed.tokens, &marked_indices);
    let source_set = source_set::classify(path, &lexed.header_comments);
    let source_lines = u32::try_from(lexed.code_lines.len()).map_err(|_| LowerError::TooLarge)?;
    Ok(FileModel {
        runtime_imports,
        flagged_lines,
        hazard_lines: Vec::new(),
        tokens,
        functions,
        source_lines,
        source_set,
        program_start: false,
    })
}

impl Guest for Dart {
    fn declared_dependencies(manifests: Vec<Manifest>) -> Vec<DeclaredDep> {
        extract_declared(&manifests)
    }

    fn workspace_packages(manifests: Vec<Manifest>) -> Result<Vec<String>, WorkspaceError> {
        workspaces::discover(&manifests)
    }

    fn manifest_paths(repository_paths: Vec<String>) -> Vec<String> {
        resolution::find_pubspecs(&repository_paths)
    }

    fn resolve_imports(
        repository_paths: Vec<String>,
        manifests: Vec<Manifest>,
        requests: Vec<ImportRequest>,
    ) -> Vec<Resolution> {
        resolution::resolve_all(&repository_paths, &manifests, &requests)
    }

    fn lower(path: String, text: String) -> Result<FileModel, LowerError> {
        lower_dart(&path, &text)
    }

    fn entry_points(repository_paths: Vec<String>, manifests: Vec<Manifest>) -> Vec<String> {
        discover_entry_points(&repository_paths, &manifests)
    }

    fn extensions() -> Vec<String> {
        vec!["dart".to_owned()]
    }

    fn reference_ranges() -> Vec<ReferenceRange> {
        codeint_model::scan::build_reference_ranges(
            "codeint panel of 20 Dart repositories, ADR 0012, codeint 0.2.1",
            [
                ("erosion", 0.047, 0.419),
                ("verbosity", 0.080, 0.259),
                ("duplication", 0.073, 0.256),
                ("import_cycles", 0.073, 0.524),
            ],
        )
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
