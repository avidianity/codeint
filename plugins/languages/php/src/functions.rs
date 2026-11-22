use std::collections::BTreeSet;

use codeint_model::language::Function;

use crate::lexer::{RawKind, RawToken, find_matching_delim};

pub(crate) fn functions(tokens: &[RawToken], code_lines: &BTreeSet<u32>) -> Vec<Function> {
    let mut spans: Vec<(usize, usize, u32, u32)> = Vec::new();
    let mut i = 0usize;
    while i < tokens.len() {
        if tokens[i].kind == RawKind::Keyword
            && tokens[i].lower == "function"
            && let Some((body_open, body_close)) = find_function_body(tokens, i)
        {
            spans.push((
                body_open,
                body_close,
                tokens[i].line,
                tokens[body_close].line,
            ));
        } else if tokens[i].kind == RawKind::Keyword
            && tokens[i].lower == "fn"
            && let Some((body_start, body_end)) = find_arrow_fn_body(tokens, i)
        {
            spans.push((body_start, body_end, tokens[i].line, tokens[body_end].line));
        }
        i += 1;
    }

    let mut result = Vec::with_capacity(spans.len());
    for (idx, &(body_start, body_end, start_line, end_line)) in spans.iter().enumerate() {
        let mut decisions = 0u32;
        let mut cursor = body_start;
        while cursor <= body_end {
            if let Some(nested_end) = nested_function_end(&spans, idx, cursor) {
                cursor = nested_end + 1;
                continue;
            }
            decisions += decision_increment(tokens, cursor);
            cursor += 1;
        }
        let source_lines = code_lines.range(start_line..=end_line).count() as u32;
        result.push(Function {
            line: start_line,
            end_line,
            decision_points: decisions,
            source_lines,
        });
    }
    result.sort_by_key(|f| (f.line, f.end_line));
    result
}

fn nested_function_end(
    spans: &[(usize, usize, u32, u32)],
    owner: usize,
    cursor: usize,
) -> Option<usize> {
    spans
        .iter()
        .enumerate()
        .find(|&(idx, &(start, _, _, _))| idx != owner && start == cursor)
        .map(|(_, &(_, end, _, _))| end)
}

fn find_function_body(tokens: &[RawToken], fn_idx: usize) -> Option<(usize, usize)> {
    let mut cursor = fn_idx + 1;
    if tokens.get(cursor).is_some_and(|t| t.text == "&") {
        cursor += 1;
    }
    if tokens
        .get(cursor)
        .is_some_and(|t| matches!(t.kind, RawKind::Ident | RawKind::Keyword))
    {
        cursor += 1;
    }
    if tokens.get(cursor).map(|t| t.text.as_str()) != Some("(") {
        return None;
    }
    let close_paren = find_matching_delim(tokens, cursor)?;
    let mut after = close_paren + 1;
    while after < tokens.len() {
        match tokens[after].text.as_str() {
            "{" => {
                let close_brace = find_matching_delim(tokens, after)?;
                return Some((after, close_brace));
            }
            ";" => return None,
            "(" => {
                after = find_matching_delim(tokens, after)? + 1;
            }
            _ => after += 1,
        }
    }
    None
}

fn find_arrow_fn_body(tokens: &[RawToken], fn_idx: usize) -> Option<(usize, usize)> {
    let mut cursor = fn_idx + 1;
    if tokens.get(cursor).is_some_and(|t| t.text == "&") {
        cursor += 1;
    }
    if tokens.get(cursor).map(|t| t.text.as_str()) != Some("(") {
        return None;
    }
    let close_paren = find_matching_delim(tokens, cursor)?;
    let mut after = close_paren + 1;
    while after < tokens.len() && tokens[after].text != "=>" {
        if tokens[after].text == ";" || tokens[after].text == "{" {
            return None;
        }
        after += 1;
    }
    if after >= tokens.len() {
        return None;
    }
    let body_start = after + 1;
    let mut end = body_start;
    let mut depth = 0usize;
    while end < tokens.len() {
        match tokens[end].text.as_str() {
            "(" | "[" | "{" | "#[" => depth += 1,
            ")" | "]" | "}" => {
                if depth == 0 {
                    return Some((body_start, end.saturating_sub(1)));
                }
                depth -= 1;
            }
            ";" | "," if depth == 0 => {
                return Some((body_start, end.saturating_sub(1)));
            }
            _ => {}
        }
        end += 1;
    }
    Some((body_start, tokens.len().saturating_sub(1)))
}

fn decision_increment(tokens: &[RawToken], idx: usize) -> u32 {
    let token = &tokens[idx];
    if token.kind == RawKind::Keyword {
        return match token.lower.as_str() {
            "if" | "elseif" | "for" | "foreach" | "while" | "case" | "catch" | "and" | "or" => 1,
            "match" => count_match_arms(tokens, idx),
            _ => 0,
        };
    }
    match token.text.as_str() {
        "&&" | "||" | "??" => 1,
        "?" if is_ternary_question(tokens, idx) => 1,
        _ => 0,
    }
}

fn is_ternary_question(tokens: &[RawToken], idx: usize) -> bool {
    let prev = idx.checked_sub(1).and_then(|i| tokens.get(i));
    match prev {
        None => false,
        Some(t) => {
            matches!(
                t.kind,
                RawKind::Variable | RawKind::Ident | RawKind::NumberLit | RawKind::StringLit
            ) || matches!(t.text.as_str(), ")" | "]" | "true" | "false" | "null")
        }
    }
}

fn count_match_arms(tokens: &[RawToken], match_idx: usize) -> u32 {
    let mut cursor = match_idx + 1;
    if tokens.get(cursor).map(|t| t.text.as_str()) != Some("(") {
        return 0;
    }
    let Some(close_paren) = find_matching_delim(tokens, cursor) else {
        return 0;
    };
    cursor = close_paren + 1;
    if tokens.get(cursor).map(|t| t.text.as_str()) != Some("{") {
        return 0;
    }
    let Some(close_brace) = find_matching_delim(tokens, cursor) else {
        return 0;
    };
    let mut arms = 0u32;
    let mut depth = 0usize;
    for token in &tokens[cursor + 1..close_brace] {
        match token.text.as_str() {
            "(" | "[" | "{" | "#[" => depth += 1,
            ")" | "]" | "}" => depth = depth.saturating_sub(1),
            "=>" if depth == 0 => arms += 1,
            _ => {}
        }
    }
    arms.saturating_sub(1)
}
