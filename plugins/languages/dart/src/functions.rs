use std::collections::BTreeSet;

use codeint_model::language::Function;
use codeint_model::scan::{find_arrow_expr_end, find_matching_delim, score_function_spans};

use crate::lexer::{DartKind, DartToken};

type RawSpan = (usize, usize, u32, u32);

const NON_FN_PAREN_KEYWORDS: [&str; 17] = [
    "if", "for", "while", "switch", "catch", "assert", "return", "throw", "new", "const", "await",
    "yield", "case", "super", "this", "on", "when",
];

pub(crate) fn extract(tokens: &[DartToken], code_lines: &BTreeSet<u32>) -> Vec<Function> {
    let mut spans = Vec::new();
    for (idx, tok) in tokens.iter().enumerate() {
        let found = match (tok.kind, tok.text.as_str()) {
            (DartKind::Punct, "(") => try_paren_function(tokens, idx),
            (DartKind::Keyword, "get") => try_getter_function(tokens, idx),
            _ => None,
        };
        if let Some(span) = found {
            spans.push(span);
        }
    }
    score_function_spans(tokens, code_lines, &spans, decision_weight)
}

fn try_paren_function(tokens: &[DartToken], open_paren: usize) -> Option<RawSpan> {
    if open_paren > 0 {
        let prev = &tokens[open_paren - 1];
        if prev.kind == DartKind::Keyword && NON_FN_PAREN_KEYWORDS.contains(&prev.text.as_str()) {
            return None;
        }
        if prev.text == "=>" {
            return None;
        }
    }
    let close_paren = find_matching_delim(tokens, open_paren)?;
    let (body_start, body_end) = locate_body_after_params(tokens, close_paren + 1)?;
    Some((
        body_start,
        body_end,
        tokens[open_paren].line,
        tokens[body_end].line,
    ))
}

fn try_getter_function(tokens: &[DartToken], get_idx: usize) -> Option<RawSpan> {
    if tokens.get(get_idx + 1)?.kind != DartKind::Ident {
        return None;
    }
    let (body_start, body_end) = locate_body_after_params(tokens, get_idx + 2)?;
    Some((
        body_start,
        body_end,
        tokens[get_idx].line,
        tokens[body_end].line,
    ))
}

fn locate_body_after_params(tokens: &[DartToken], start: usize) -> Option<(usize, usize)> {
    let pos = skip_modifiers_and_initializers(tokens, start)?;
    let tok = tokens.get(pos)?;
    if tok.text == "{" {
        return Some((pos, find_matching_delim(tokens, pos)?));
    }
    if tok.text == "=>" {
        let body_start = pos + 1;
        return Some((body_start, find_arrow_expr_end(tokens, body_start)));
    }
    None
}

fn skip_modifiers_and_initializers(tokens: &[DartToken], mut pos: usize) -> Option<usize> {
    if tokens.get(pos).is_some_and(|t| t.text == ":") {
        pos = skip_initializer_list(tokens, pos + 1)?;
    }
    while tokens
        .get(pos)
        .is_some_and(|t| matches!(t.text.as_str(), "async" | "sync" | "*"))
    {
        pos += 1;
    }
    Some(pos)
}

fn skip_initializer_list(tokens: &[DartToken], mut pos: usize) -> Option<usize> {
    let mut depth = 0usize;
    while pos < tokens.len() {
        match tokens[pos].text.as_str() {
            "(" | "[" => depth += 1,
            ")" | "]" => depth = depth.saturating_sub(1),
            "{" | "=>" if depth == 0 => return Some(pos),
            ";" if depth == 0 => return None,
            _ => {}
        }
        pos += 1;
    }
    None
}

fn decision_weight(tokens: &[DartToken], idx: usize) -> u32 {
    let tok = &tokens[idx];
    match (tok.kind, tok.text.as_str()) {
        (DartKind::Keyword, "if" | "for" | "while" | "do" | "case" | "catch") => 1,
        (DartKind::Keyword, "switch") => count_switch_expr_arms(tokens, idx),
        (DartKind::Punct, "&&" | "||" | "??") => 1,
        (DartKind::Punct, "?") if is_ternary_operator(tokens, idx) => 1,
        _ => 0,
    }
}

fn is_ternary_operator(tokens: &[DartToken], q_idx: usize) -> bool {
    let mut depth = 0usize;
    for tok in tokens.iter().skip(q_idx + 1) {
        match tok.text.as_str() {
            "(" | "[" | "{" => depth += 1,
            ")" | "]" | "}" if depth == 0 => return false,
            ")" | "]" | "}" => depth -= 1,
            ":" if depth == 0 => return true,
            ";" | "," if depth == 0 => return false,
            _ => {}
        }
    }
    false
}

fn count_switch_expr_arms(tokens: &[DartToken], switch_idx: usize) -> u32 {
    let Some((_, _, open_brace, close_brace)) =
        codeint_model::scan::paren_then_brace(tokens, switch_idx)
    else {
        return 0;
    };
    let mut arms = 0u32;
    let mut depth = 0usize;
    for tok in &tokens[open_brace + 1..close_brace] {
        match tok.text.as_str() {
            "(" | "[" | "{" => depth += 1,
            ")" | "]" | "}" => depth = depth.saturating_sub(1),
            "case" if depth == 0 => return 0,
            "=>" if depth == 0 => arms += 1,
            _ => {}
        }
    }
    arms.saturating_sub(1)
}
