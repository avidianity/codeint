#![forbid(unsafe_code)]

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

use codeint_model::language::{FileModel, Token, TokenKind};
use codeint_model::scan::CoreReferenceRanges;
use codeint_model::token_text_hash;

use crate::dependencies::extract_declared;
use crate::entry_points::discover as discover_entry_points;
use crate::lexer::{DartKind, DartToken};

pub use rules::flagged_line_rules;

pub struct Dart;

const DART_RANGES: CoreReferenceRanges = CoreReferenceRanges::new(
    (0.039, 0.497),
    (0.000, 0.377),
    (0.000, 0.011),
    (0.027, 0.256),
    Some((0.061, 0.600)),
);

fn map_token_kind(kind: DartKind) -> TokenKind {
    match kind {
        DartKind::Ident => TokenKind::Identifier,
        DartKind::StringLit | DartKind::NumberLit => TokenKind::Literal,
        DartKind::Keyword | DartKind::Punct => TokenKind::Other,
    }
}

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

fn lower_dart(path: &str, text: &str) -> Result<FileModel, codeint_model::LowerError> {
    let lexed = lexer::scan(text).map_err(codeint_model::LowerError::from)?;
    let (runtime_imports, marked_indices) = imports::scan_directives(&lexed.tokens);
    let functions = functions::extract(&lexed.tokens, &lexed.code_lines);
    let flagged_lines = rules::evaluate(&lexed.tokens, &lexed.code_lines);
    let tokens = lower_tokens(&lexed.tokens, &marked_indices);
    let source_set = source_set::classify(path, &lexed.header_comments);
    let source_lines =
        u32::try_from(lexed.code_lines.len()).map_err(|_| codeint_model::LowerError::TooLarge)?;
    Ok(FileModel {
        runtime_imports,
        flagged_lines,
        tokens,
        functions,
        source_lines,
        is_tooling_file: source_set::is_tooling_file(path),
        ..codeint_model::empty_file_model(source_set)
    })
}

codeint_model::impl_builtin_language_plugin!(
    Dart,
    ["dart"],
    lower_ext(lower_dart, ".dart", "Dart"),
    DART_RANGES.build("codeint stratified panel of 100 Dart repositories, ADR 0035, codeint 0.8.0"),
    flagged_lines(flagged_line_rules()),
    imports(
        resolution::find_pubspecs,
        workspaces::discover,
        by_profile(resolution::resolve_dart_imports),
        extract_declared,
        discover_entry_points,
    ),
    frameworks(frameworks),
    hazards(none),
);
