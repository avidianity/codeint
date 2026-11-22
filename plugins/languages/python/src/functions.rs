use std::collections::BTreeSet;

use codeint_model::language::Function;
use codeint_model::scan::score_function_spans;

use crate::lexer::{LogicalLine, PyToken};

pub(crate) fn extract(
    tokens: &[PyToken],
    logical_lines: &[LogicalLine],
    code_lines: &BTreeSet<u32>,
) -> Vec<Function> {
    let mut spans = Vec::new();
    let case_indices = collect_decision_case_indices(tokens, logical_lines);
    for (line_idx, line) in logical_lines.iter().enumerate() {
        if let Some(span) = extract_def_span(tokens, logical_lines, line_idx) {
            spans.push(span);
        }
        collect_lambda_spans(tokens, line, &mut spans);
    }
    score_function_spans(tokens, code_lines, &spans, |toks, idx| {
        decision_weight(toks, idx, &case_indices)
    })
}

pub(crate) fn is_def_header(slice: &[PyToken]) -> bool {
    slice.first().is_some_and(|t| t.is_kw("def"))
        || (slice.first().is_some_and(|t| t.is_kw("async"))
            && slice.get(1).is_some_and(|t| t.is_kw("def")))
}

fn extract_def_span(
    tokens: &[PyToken],
    logical_lines: &[LogicalLine],
    line_idx: usize,
) -> Option<(usize, usize, u32, u32)> {
    let line = &logical_lines[line_idx];
    if !is_def_header(&tokens[line.tokens.clone()]) {
        return None;
    }
    let colon_idx = find_header_colon(tokens, line.tokens.start, line.tokens.end)?;
    let block_end = block_end_line(logical_lines, line_idx);
    let end_logical = &logical_lines[block_end];
    let body_start = colon_idx + 1;
    let body_end = end_logical.tokens.end.saturating_sub(1);
    (body_start <= body_end).then_some((
        body_start,
        body_end,
        line.start_line,
        end_logical.end_line,
    ))
}

fn find_header_colon(tokens: &[PyToken], start: usize, end: usize) -> Option<usize> {
    let mut depth = 0usize;
    let mut saw_params = false;
    for (idx, tok) in tokens.iter().enumerate().take(end).skip(start) {
        match tok.text.as_str() {
            "(" | "[" | "{" => {
                saw_params |= tok.text == "(" && depth == 0;
                depth += 1;
            }
            ")" | "]" | "}" => depth = depth.saturating_sub(1),
            ":" if depth == 0 && saw_params => return Some(idx),
            _ => {}
        }
    }
    None
}

pub(crate) fn block_end_line(logical_lines: &[LogicalLine], header_idx: usize) -> usize {
    let base_indent = logical_lines[header_idx].indent;
    let mut end = header_idx;
    while let Some(next) = logical_lines.get(end + 1) {
        if next.indent > base_indent {
            end += 1;
        } else {
            break;
        }
    }
    end
}

fn collect_lambda_spans(
    tokens: &[PyToken],
    line: &LogicalLine,
    spans: &mut Vec<(usize, usize, u32, u32)>,
) {
    for idx in line.tokens.clone() {
        if !tokens[idx].is_kw("lambda") {
            continue;
        }
        let Some(colon_idx) = find_lambda_colon(tokens, idx + 1, line.tokens.end) else {
            continue;
        };
        let body_start = colon_idx + 1;
        if body_start >= line.tokens.end {
            continue;
        }
        let body_end = find_lambda_body_end(tokens, body_start, line.tokens.end);
        if body_start <= body_end {
            spans.push((
                body_start,
                body_end,
                tokens[idx].line,
                tokens[body_end].line,
            ));
        }
    }
}

fn find_lambda_colon(tokens: &[PyToken], start: usize, end: usize) -> Option<usize> {
    let mut depth = 0usize;
    for (idx, tok) in tokens.iter().enumerate().take(end).skip(start) {
        match tok.text.as_str() {
            "(" | "[" | "{" => depth += 1,
            ")" | "]" | "}" if depth == 0 => return None,
            ")" | "]" | "}" => depth -= 1,
            ":" if depth == 0 => return Some(idx),
            _ => {}
        }
    }
    None
}

fn find_lambda_body_end(tokens: &[PyToken], start: usize, end: usize) -> usize {
    let mut depth = 0usize;
    for (idx, tok) in tokens.iter().enumerate().take(end).skip(start) {
        match tok.text.as_str() {
            "(" | "[" | "{" => depth += 1,
            ")" | "]" | "}" if depth == 0 => return idx.saturating_sub(1),
            ")" | "]" | "}" => depth -= 1,
            "," | ";" if depth == 0 => return idx.saturating_sub(1),
            _ => {}
        }
    }
    end.saturating_sub(1)
}

fn collect_decision_case_indices(
    tokens: &[PyToken],
    logical_lines: &[LogicalLine],
) -> BTreeSet<usize> {
    let mut out = BTreeSet::new();
    for line in logical_lines {
        let slice = &tokens[line.tokens.clone()];
        if slice
            .first()
            .is_some_and(|t| t.is_ident() && t.text == "case")
            && slice.last().is_some_and(|t| t.text == ":")
            && !is_wildcard_case(slice)
        {
            out.insert(line.tokens.start);
        }
    }
    out
}

fn is_wildcard_case(slice: &[PyToken]) -> bool {
    slice.len() == 3 && slice[1].is_ident() && slice[1].text == "_"
}

fn decision_weight(tokens: &[PyToken], idx: usize, case_indices: &BTreeSet<usize>) -> u32 {
    if case_indices.contains(&idx) {
        return 1;
    }
    let tok = &tokens[idx];
    if !tok.is_kw {
        return 0;
    }
    match tok.text.as_str() {
        "if" | "elif" | "for" | "while" | "except" | "and" | "or" => 1,
        _ => 0,
    }
}
