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
pub use rules::flagged_line_rules;
mod source_set;
mod workspaces;

use std::collections::BTreeSet;

use codeint_model::language::LowerError;
use codeint_model::scan::CoreReferenceRanges;
use codeint_model::{FileModel, Token, TokenKind, token_text_hash};

use crate::dependencies::extract_declared;
use crate::entry_points::extract as extract_entry_points;
use crate::lexer::{RawKind, RawToken};
use crate::resolution::{manifest_paths as find_manifest_paths, resolve_batch_with_profiles};

pub struct Php;

const PHP_RANGES: CoreReferenceRanges = CoreReferenceRanges::new(
    (0.064, 0.591),
    (0.044, 0.542),
    (0.000, 0.004),
    (0.025, 0.209),
    Some((0.000, 0.133)),
);

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

fn lower_php(
    _plugin: &Php,
    path: &codeint_model::RepoPath,
    text: &str,
) -> Result<FileModel, codeint_model::LowerError> {
    let path = path.as_str();
    if !path.ends_with(".php") {
        return Err(LowerError::UnsupportedFile(format!("{path} is not a PHP source file")).into());
    }
    let lexed = lexer::lex(text).map_err(codeint_model::LowerError::from)?;
    let extracted = imports::extract(&lexed.tokens);
    let source_lines =
        u32::try_from(lexed.code_lines.len()).map_err(|_| codeint_model::LowerError::TooLarge)?;
    Ok(FileModel {
        source_set: source_set::classify(path, &lexed.header_comments),
        source_lines,
        functions: functions::functions(&lexed.tokens, &lexed.code_lines),
        tokens: build_tokens(&lexed.tokens, &extracted.import_token_indices),
        flagged_lines: rules::flag(&lexed.tokens, &lexed.code_lines),
        hazard_lines: Vec::new(),
        runtime_imports: extracted.specifiers,
        program_start: false,
    })
}

codeint_model::impl_builtin_language_plugin!(
    Php,
    ["php"],
    lower_php,
    PHP_RANGES.build("codeint stratified panel of 100 PHP repositories, ADR 0035, codeint 0.8.0"),
    imports(
        find_manifest_paths,
        workspaces::workspace_packages,
        resolve_batch_with_profiles,
        extract_declared,
        extract_entry_points,
    ),
    frameworks(frameworks),
    hazards(none),
);
