use std::collections::BTreeSet;

use crate::commands::is_assignment_word;
use crate::lexer::{ShellToken, ShellTokenKind};

use super::emit_line;

const BASHISM_CMD_KEYWORDS: &[&str] = &[
    "[[",
    "declare",
    "function",
    "mapfile",
    "readarray",
    "select",
    "shopt",
    "source",
    "typeset",
];

const BASHISM_OPERATORS: &[&str] = &["<<<", "|&", "&>", "&>>"];

pub(super) fn check_stream(
    tokens: &[ShellToken],
    code_lines: &BTreeSet<u32>,
    hits: &mut BTreeSet<(u32, &'static str)>,
) {
    let mut at_cmd_pos = true;
    for (idx, tok) in tokens.iter().enumerate() {
        if is_bashism_token(tokens, idx, tok, at_cmd_pos) {
            emit_line(tok.line, "posix-sh-bashism", code_lines, hits);
        }
        at_cmd_pos = tok.next_is_cmd_pos();
    }
}

fn is_bashism_token(tokens: &[ShellToken], idx: usize, tok: &ShellToken, at_cmd_pos: bool) -> bool {
    if tok.kind == ShellTokenKind::ArithCmd || tok.is_process_sub {
        return true;
    }
    if tok.kind == ShellTokenKind::Operator && BASHISM_OPERATORS.contains(&tok.text.as_str()) {
        return true;
    }
    if at_cmd_pos && !tok.has_quotes && BASHISM_CMD_KEYWORDS.contains(&tok.text.as_str()) {
        return true;
    }
    is_array_assignment(tokens, idx, tok)
}

fn is_array_assignment(tokens: &[ShellToken], idx: usize, tok: &ShellToken) -> bool {
    tok.kind == ShellTokenKind::Word
        && !tok.has_quotes
        && tok.text.ends_with('=')
        && is_assignment_word(&tok.text)
        && tokens
            .get(idx + 1)
            .is_some_and(|next| next.is_op("(") && next.line == tok.end_line)
}
