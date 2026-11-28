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

use codeint_model::language::{FileModel, LowerError, Token, TokenKind};
use codeint_model::scan::CoreReferenceRanges;
use codeint_model::token_text_hash;

use crate::lexer::{GoKind, GoToken};

pub use rules::flagged_line_rules;

pub struct Go;

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
        tokens,
        functions: extracted.functions,
        source_lines,
        program_start: scanned_imports.is_package_main,
        ..codeint_model::empty_file_model(source_set)
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

fn lower_go(
    _plugin: &Go,
    path: &codeint_model::RepoPath,
    text: &str,
    _dialect: Option<&str>,
) -> Result<FileModel, codeint_model::LowerError> {
    lower_file(path.as_str(), text).map_err(codeint_model::LowerError::from)
}

const GO_RANGES: CoreReferenceRanges = CoreReferenceRanges::new(
    (0.304, 0.746),
    (0.306, 0.671),
    (0.000, 0.004),
    (0.034, 0.243),
    Some((0.000, 0.000)),
);

codeint_model::impl_builtin_language_plugin!(
    Go,
    ["go"],
    lower_go,
    GO_RANGES.build("codeint stratified panel of 100 Go repositories, ADR 0035, codeint 0.8.0"),
    flagged_lines(flagged_line_rules()),
    imports(
        resolution::find_go_mods,
        workspaces::analyze_manifests,
        |paths, manifests, requests, _| resolution::resolve_all(paths, manifests, requests, &[]),
        entry_points::discover,
    ),
    frameworks(none),
    hazards(none),
);
