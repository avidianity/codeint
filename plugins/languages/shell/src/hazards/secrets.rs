use std::collections::BTreeSet;

use crate::commands::parse_commands;
use crate::lexer::{ShellToken, ShellTokenKind, is_assignment_word};

use super::emit_line;

const SENSITIVE_NAME_SUBSTRINGS: &[&str] = &[
    "password",
    "passwd",
    "secret",
    "token",
    "api_key",
    "apikey",
    "private_key",
    "access_key",
    "auth_key",
    "client_secret",
    "signing_key",
    "encryption_key",
];

const NON_SECRET_SUFFIXES: &[&str] = &[
    "_cmd",
    "_command",
    "_count",
    "_dir",
    "_directory",
    "_enabled",
    "_endpoint",
    "_env",
    "_file",
    "_header",
    "_hint",
    "_host",
    "_label",
    "_len",
    "_length",
    "_message",
    "_msg",
    "_name",
    "_path",
    "_pattern",
    "_policy",
    "_port",
    "_prefix",
    "_prompt",
    "_regex",
    "_required",
    "_scope",
    "_suffix",
    "_timeout",
    "_ttl",
    "_type",
    "_uri",
    "_url",
    "_var",
];

const PLACEHOLDER_LITERALS: &[&str] = &[
    "changeme",
    "dummy",
    "example",
    "fake",
    "false",
    "fixme",
    "none",
    "null",
    "placeholder",
    "replace_me",
    "test",
    "todo",
    "true",
];

const MIN_SECRET_LITERAL_LEN: usize = 6;

pub(super) fn check_stream(
    tokens: &[ShellToken],
    code_lines: &BTreeSet<u32>,
    hits: &mut BTreeSet<(u32, &'static str)>,
) {
    let parsed = parse_commands(tokens);
    for cmd in &parsed.commands {
        for &idx in &cmd.assignments {
            check_assignment_token(&tokens[idx], code_lines, hits);
        }
        if let Some(cmd_idx) = cmd.cmd_idx
            && is_decl_builtin(&tokens[cmd_idx])
        {
            for &idx in &cmd.arg_indices {
                check_assignment_token(&tokens[idx], code_lines, hits);
            }
        }
    }
    for tok in tokens {
        if tok.kind == ShellTokenKind::Word
            && !tok.has_var_expansion
            && has_known_secret_prefix(&tok.text)
        {
            emit_line(tok.line, "hardcoded-secret", code_lines, hits);
        }
    }
}

fn is_decl_builtin(tok: &ShellToken) -> bool {
    !tok.has_quotes
        && matches!(
            tok.text.as_str(),
            "export" | "local" | "readonly" | "declare" | "typeset"
        )
}

fn check_assignment_token(
    tok: &ShellToken,
    code_lines: &BTreeSet<u32>,
    hits: &mut BTreeSet<(u32, &'static str)>,
) {
    if tok.has_var_expansion || !is_assignment_word(&tok.text) {
        return;
    }
    let Some((name, raw_value)) = tok.text.split_once('=') else {
        return;
    };
    let clean_name = name.strip_suffix('+').unwrap_or(name);
    if !is_sensitive_var_name(clean_name) {
        return;
    }
    let value = strip_outer_quotes(raw_value);
    if is_secret_literal(value) {
        emit_line(tok.line, "hardcoded-secret", code_lines, hits);
    }
}

fn is_sensitive_var_name(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    if NON_SECRET_SUFFIXES
        .iter()
        .any(|suffix| lower.ends_with(suffix))
    {
        return false;
    }
    SENSITIVE_NAME_SUBSTRINGS
        .iter()
        .any(|sub| lower.contains(sub))
}

fn strip_outer_quotes(value: &str) -> &str {
    if let Some(inner) = value.strip_prefix("$'").and_then(|s| s.strip_suffix('\'')) {
        return inner;
    }
    if let Some(inner) = value.strip_prefix('\'').and_then(|s| s.strip_suffix('\'')) {
        return inner;
    }
    if let Some(inner) = value.strip_prefix('"').and_then(|s| s.strip_suffix('"')) {
        return inner;
    }
    value
}

fn is_non_secret_shape(value: &str) -> bool {
    value.starts_with('/')
        || value.contains("://")
        || value.bytes().all(|b| b.is_ascii_digit())
        || value.contains(char::is_whitespace)
}

fn is_secret_literal(value: &str) -> bool {
    let trimmed = value.trim();
    if trimmed.len() < MIN_SECRET_LITERAL_LEN {
        return false;
    }
    if trimmed.starts_with('<') && trimmed.ends_with('>') {
        return false;
    }
    if is_non_secret_shape(trimmed) {
        return false;
    }
    let lower = trimmed.to_ascii_lowercase();
    if lower.starts_with("your_") || lower.starts_with("your-") || lower.contains("xxx") {
        return false;
    }
    !PLACEHOLDER_LITERALS.contains(&lower.as_str())
}

const PREFIXED_SECRETS: &[(&str, usize)] = &[
    ("ghp_", 16),
    ("gho_", 16),
    ("ghu_", 16),
    ("ghs_", 16),
    ("ghr_", 16),
    ("github_pat_", 16),
    ("glpat-", 16),
    ("sk_live_", 16),
    ("sk_test_", 16),
    ("rk_live_", 16),
    ("sk-", 20),
    ("xoxb-", 10),
    ("xoxp-", 10),
    ("xoxa-", 10),
    ("xoxr-", 10),
];

fn has_known_secret_prefix(text: &str) -> bool {
    if text.contains("-----BEGIN ") && text.contains("PRIVATE KEY-----") {
        return true;
    }
    has_aws_access_key(text)
        || PREFIXED_SECRETS
            .iter()
            .any(|&(prefix, min_tail)| has_prefixed_token(text, prefix, min_tail))
}

fn has_aws_access_key(text: &str) -> bool {
    let bytes = text.as_bytes();
    bytes.windows(20).any(|win| {
        &win[..4] == b"AKIA"
            && win[4..]
                .iter()
                .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit())
    })
}

const PROJECT_KEY_SHAPES: &[&str] = &["proj-", "svcacct-", "admin-", "ant-"];

fn has_prefixed_token(text: &str, prefix: &str, min_tail: usize) -> bool {
    text.match_indices(prefix).any(|(pos, _)| {
        starts_at_boundary(text, pos)
            && tail_is_token(prefix, &text[pos + prefix.len()..], min_tail)
    })
}

fn starts_at_boundary(text: &str, pos: usize) -> bool {
    pos == 0 || {
        let before = text.as_bytes()[pos - 1];
        !before.is_ascii_alphanumeric() && !matches!(before, b'_' | b'-')
    }
}

fn tail_is_token(prefix: &str, tail: &str, min_tail: usize) -> bool {
    let count = tail
        .bytes()
        .take_while(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-'))
        .count();
    if count < min_tail {
        return false;
    }
    if prefix != "sk-" {
        return true;
    }
    let leading_alphanumerics = tail.bytes().take_while(u8::is_ascii_alphanumeric).count();
    leading_alphanumerics >= min_tail
        || PROJECT_KEY_SHAPES
            .iter()
            .any(|shape| tail.starts_with(shape))
}
