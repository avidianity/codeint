#![forbid(unsafe_code)]

mod commands;
mod dialect;
mod flagged;
mod functions;
mod hazards;
mod lexer;
mod source_set;

use codeint_model::language::{
    DeclaredDep, FileModel, Guest, ImportRequest, LowerError, Manifest, ReferenceRange, Resolution,
    Token, TokenKind, WorkspaceError,
};
use codeint_model::{path_file_name, token_text_hash};

use crate::lexer::{ShellToken, ShellTokenKind};

pub use dialect::{ShellDialect, detect_dialect};
pub use flagged::{SHELL_FLAGGED_RULES, flagged_line_rules};
pub use hazards::{SHELL_HAZARD_RULES, hazard_line_rules};

const SHELL_KEYWORDS_AND_BUILTINS: &[&str] = &[
    "!", "[", "[[", "]", "]]", "{", "}", "break", "case", "cd", "continue", "coproc", "declare",
    "do", "done", "echo", "elif", "else", "esac", "eval", "exec", "exit", "export", "fi", "for",
    "function", "if", "in", "local", "read", "readonly", "return", "select", "set", "setopt",
    "shift", "source", "test", "then", "time", "trap", "typeset", "unset", "until", "while",
];

pub struct Shell;

impl Guest for Shell {
    fn extensions() -> Vec<String> {
        vec!["sh".to_owned(), "bash".to_owned(), "zsh".to_owned()]
    }

    fn lower(path: String, text: String) -> Result<FileModel, LowerError> {
        lower_file(&path, &text)
    }

    fn has_imports() -> bool {
        false
    }

    fn manifest_paths(_: Vec<String>) -> Vec<String> {
        Vec::default()
    }

    fn has_hazard_rules() -> bool {
        true
    }

    fn workspace_packages(_: Vec<Manifest>) -> Result<Vec<String>, WorkspaceError> {
        Ok(Vec::default())
    }

    fn hazard_rules() -> Vec<String> {
        SHELL_HAZARD_RULES
            .iter()
            .map(|rule| (*rule).to_owned())
            .collect()
    }

    fn resolve_imports(
        _: Vec<String>,
        _: Vec<Manifest>,
        _: Vec<ImportRequest>,
        _: Vec<String>,
    ) -> Vec<Resolution> {
        Vec::default()
    }

    fn reference_ranges() -> Vec<ReferenceRange> {
        Vec::default()
    }

    fn declared_dependencies(_: Vec<Manifest>) -> Vec<DeclaredDep> {
        Vec::default()
    }

    fn entry_points(_: Vec<String>, _: Vec<Manifest>) -> Vec<String> {
        Vec::default()
    }

    codeint_model::empty_framework_profile_guest_methods!();
}

fn lower_file(path: &str, text: &str) -> Result<FileModel, LowerError> {
    if !is_supported_path(path) {
        return Err(LowerError::UnsupportedFile(format!(
            "{path} is not a Shell source file"
        )));
    }
    let dialect = detect_dialect(path, text);
    let lexed = lexer::scan(text)?;
    let source_lines = u32::try_from(lexed.code_lines.len()).map_err(|_| LowerError::TooLarge)?;
    let extracted = functions::extract(&lexed.tokens, &lexed.code_lines);
    let flagged_lines = flagged::evaluate(&lexed.tokens, &lexed.code_lines);
    let hazard_lines = hazards::evaluate(
        &lexed.tokens,
        &lexed.code_lines,
        &extracted.spans,
        dialect,
        text,
    );
    let mut tokens = Vec::new();
    append_duplication_tokens(&lexed.tokens, &mut tokens);
    let source_set = source_set::classify(path, &lexed.header_comments);
    Ok(FileModel {
        runtime_imports: Vec::new(),
        flagged_lines,
        hazard_lines,
        tokens,
        functions: extracted.functions,
        source_lines,
        source_set,
        program_start: false,
    })
}

fn is_supported_path(path: &str) -> bool {
    let file_name = path_file_name(path);
    if source_set::is_generated_filename(file_name) {
        return true;
    }
    matches!(
        file_name.rsplit_once('.'),
        Some((stem, "sh" | "bash" | "zsh")) if !stem.is_empty()
    )
}

fn append_duplication_tokens(raw: &[ShellToken], out: &mut Vec<Token>) {
    for tok in raw {
        if tok.is_newline() {
            continue;
        }
        let kind = classify_duplication_kind(tok);
        out.push(Token {
            kind,
            text_hash: token_text_hash(&tok.text),
            line: tok.line,
        });
        for sub in &tok.sub_tokens {
            append_duplication_tokens(sub, out);
        }
    }
}

fn classify_duplication_kind(tok: &ShellToken) -> TokenKind {
    if matches!(
        tok.kind,
        ShellTokenKind::Operator | ShellTokenKind::ArithCmd
    ) {
        return TokenKind::Other;
    }
    if tok.is_fully_single_quoted || is_numeric_literal(&tok.text) {
        return TokenKind::Literal;
    }
    if tok.has_quotes && !tok.has_var_expansion {
        return TokenKind::Literal;
    }
    if !tok.has_quotes
        && (tok.text.starts_with('-') || SHELL_KEYWORDS_AND_BUILTINS.contains(&tok.text.as_str()))
    {
        return TokenKind::Other;
    }
    TokenKind::Identifier
}

fn is_numeric_literal(text: &str) -> bool {
    !text.is_empty() && text.bytes().all(|b| b.is_ascii_digit())
}
