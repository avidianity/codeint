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
mod stdlib;
mod workspaces;

use codeint_model::language::{FileModel, LowerError, Token, TokenKind};
use codeint_model::scan::CoreReferenceRanges;
use codeint_model::token_text_hash;

use crate::dependencies::extract_declared;
use crate::entry_points::discover as discover_entry_points;
use crate::resolution::{find_python_manifests, resolve_with_profiles};

pub use rules::flagged_line_rules;

pub struct Python;

const PYTHON_RANGES: CoreReferenceRanges = CoreReferenceRanges::new(
    (0.239, 0.717),
    (0.249, 0.637),
    (0.000, 0.011),
    (0.011, 0.220),
    Some((0.000, 0.446)),
);

fn lower_python(
    _plugin: &Python,
    path: &codeint_model::RepoPath,
    text: &str,
) -> Result<FileModel, codeint_model::LowerError> {
    let path = path.as_str();
    if !path.ends_with(".py") {
        return Err(
            LowerError::UnsupportedFile(format!("{path} is not a Python source file")).into(),
        );
    }
    let lexed = lexer::scan(text).map_err(codeint_model::LowerError::from)?;
    let (runtime_imports, import_indices) =
        imports::scan_imports(&lexed.tokens, &lexed.logical_lines);
    let tokens = lexed
        .tokens
        .iter()
        .enumerate()
        .map(|(idx, tok)| Token {
            kind: if import_indices.contains(&idx) {
                TokenKind::ImportOrExport
            } else {
                tok.token_kind
            },
            text_hash: token_text_hash(&tok.text),
            line: tok.line,
        })
        .collect();
    let source_lines =
        u32::try_from(lexed.code_lines.len()).map_err(|_| codeint_model::LowerError::TooLarge)?;
    Ok(FileModel {
        functions: functions::extract(&lexed.tokens, &lexed.logical_lines, &lexed.code_lines),
        flagged_lines: rules::evaluate(&lexed.tokens, &lexed.logical_lines, &lexed.code_lines),
        hazard_lines: Vec::new(),
        source_set: source_set::classify(path, &lexed.header_comments),
        runtime_imports,
        tokens,
        source_lines,
        program_start: false,
    })
}

codeint_model::impl_builtin_language_plugin!(
    Python,
    ["py"],
    lower_python,
    PYTHON_RANGES
        .build("codeint stratified panel of 100 Python repositories, ADR 0035, codeint 0.8.0"),
    imports(
        find_python_manifests,
        workspaces::discover,
        resolve_with_profiles,
        extract_declared,
        discover_entry_points,
    ),
    frameworks(frameworks),
    hazards(none),
);
