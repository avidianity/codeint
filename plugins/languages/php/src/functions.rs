use std::collections::BTreeSet;

use codeint_model::language::Function;

use crate::lexer::{RawKind, RawToken, find_matching_delim};

type FunctionSpan = (usize, usize, u32, u32);

pub(crate) fn functions(tokens: &[RawToken], code_lines: &BTreeSet<u32>) -> Vec<Function> {
    let spans = collect_function_spans(tokens);
    let mut result: Vec<Function> = spans
        .iter()
        .enumerate()
        .map(|(idx, &span)| build_function_metric(tokens, code_lines, &spans, idx, span))
        .collect();
    result.sort_by_key(|f| (f.line, f.end_line));
    result
}

fn collect_function_spans(tokens: &[RawToken]) -> Vec<FunctionSpan> {
    let mut spans = Vec::new();
    for (i, token) in tokens.iter().enumerate() {
        if token.kind != RawKind::Keyword {
            continue;
        }
        let body = match token.lower.as_str() {
            "function" => find_function_body(tokens, i),
            "fn" => find_arrow_fn_body(tokens, i),
            _ => None,
        };
        if let Some((start, end)) = body {
            spans.push((start, end, token.line, tokens[end].line));
        }
    }
    spans
}

fn build_function_metric(
    tokens: &[RawToken],
    code_lines: &BTreeSet<u32>,
    spans: &[FunctionSpan],
    owner: usize,
    (body_start, body_end, start_line, end_line): FunctionSpan,
) -> Function {
    let mut decisions = 0u32;
    let mut cursor = body_start;
    while cursor <= body_end {
        if let Some(nested_end) = nested_function_end(spans, owner, cursor) {
            cursor = nested_end + 1;
            continue;
        }
        decisions += decision_increment(tokens, cursor);
        cursor += 1;
    }
    Function {
        line: start_line,
        end_line,
        decision_points: decisions,
        source_lines: code_lines.range(start_line..=end_line).count() as u32,
    }
}

fn nested_function_end(spans: &[FunctionSpan], owner: usize, cursor: usize) -> Option<usize> {
    spans
        .iter()
        .enumerate()
        .find(|&(idx, &(start, _, _, _))| idx != owner && start == cursor)
        .map(|(_, &(_, end, _, _))| end)
}

fn skip_fn_params(tokens: &[RawToken], fn_idx: usize, allow_name: bool) -> Option<usize> {
    let mut cursor = fn_idx + 1;
    if tokens.get(cursor).is_some_and(|t| t.text == "&") {
        cursor += 1;
    }
    if allow_name
        && tokens
            .get(cursor)
            .is_some_and(|t| matches!(t.kind, RawKind::Ident | RawKind::Keyword))
    {
        cursor += 1;
    }
    if tokens.get(cursor).map(|t| t.text.as_str()) != Some("(") {
        return None;
    }
    find_matching_delim(tokens, cursor)
}

fn find_function_body(tokens: &[RawToken], fn_idx: usize) -> Option<(usize, usize)> {
    let mut after = skip_fn_params(tokens, fn_idx, true)? + 1;
    while after < tokens.len() {
        match tokens[after].text.as_str() {
            "{" => return Some((after, find_matching_delim(tokens, after)?)),
            ";" => return None,
            "(" => after = find_matching_delim(tokens, after)? + 1,
            _ => after += 1,
        }
    }
    None
}

fn find_arrow_fn_body(tokens: &[RawToken], fn_idx: usize) -> Option<(usize, usize)> {
    let mut after = skip_fn_params(tokens, fn_idx, false)? + 1;
    while after < tokens.len() && tokens[after].text != "=>" {
        if matches!(tokens[after].text.as_str(), ";" | "{") {
            return None;
        }
        after += 1;
    }
    if after >= tokens.len() {
        return None;
    }
    let body_start = after + 1;
    Some((body_start, find_arrow_end(tokens, body_start)))
}

fn find_arrow_end(tokens: &[RawToken], body_start: usize) -> usize {
    let mut depth = 0usize;
    for (idx, token) in tokens.iter().enumerate().skip(body_start) {
        match token.text.as_str() {
            "(" | "[" | "{" | "#[" => depth += 1,
            ")" | "]" | "}" if depth == 0 => return idx.saturating_sub(1),
            ")" | "]" | "}" => depth -= 1,
            ";" | "," if depth == 0 => return idx.saturating_sub(1),
            _ => {}
        }
    }
    tokens.len().saturating_sub(1)
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
    let Some(close_paren) = tokens
        .get(match_idx + 1)
        .filter(|t| t.text == "(")
        .and_then(|_| find_matching_delim(tokens, match_idx + 1))
    else {
        return 0;
    };
    let brace_open = close_paren + 1;
    let Some(close_brace) = tokens
        .get(brace_open)
        .filter(|t| t.text == "{")
        .and_then(|_| find_matching_delim(tokens, brace_open))
    else {
        return 0;
    };
    let mut arms = 0u32;
    let mut depth = 0usize;
    for token in &tokens[brace_open + 1..close_brace] {
        match token.text.as_str() {
            "(" | "[" | "{" | "#[" => depth += 1,
            ")" | "]" | "}" => depth = depth.saturating_sub(1),
            "=>" if depth == 0 => arms += 1,
            _ => {}
        }
    }
    arms.saturating_sub(1)
}
