use std::collections::BTreeSet;

use codeint_model::language::FlaggedLine;
use codeint_model::scan::{finalize_flagged, flag_range};

use crate::functions::{block_end_line, is_def_header};
use crate::lexer::{LogicalLine, PyToken};

const RULES: &[&str] = &[
    "conditional-yields-boolean",
    "else-after-return",
    "except-only-reraises",
    "if-else-returns-boolean",
    "trailing-bare-return",
];

pub fn flagged_line_rules() -> impl Iterator<Item = &'static str> {
    RULES.iter().copied()
}

pub(crate) fn evaluate(
    tokens: &[PyToken],
    logical_lines: &[LogicalLine],
    code_lines: &BTreeSet<u32>,
) -> Vec<FlaggedLine> {
    let mut out = BTreeSet::new();
    for (idx, line) in logical_lines.iter().enumerate() {
        let slice = &tokens[line.tokens.clone()];
        if slice
            .first()
            .is_some_and(|t| t.is_kw("if") || t.is_kw("elif"))
        {
            check_if_else(tokens, logical_lines, idx, code_lines, &mut out);
        } else if slice.first().is_some_and(|t| t.is_kw("except")) {
            check_except_only_reraises(tokens, logical_lines, idx, code_lines, &mut out);
        } else if is_def_header(slice) {
            check_trailing_bare_return(tokens, logical_lines, idx, code_lines, &mut out);
        }
        check_conditional_bool(slice, code_lines, &mut out);
    }
    finalize_flagged(out)
}

fn check_if_else(
    tokens: &[PyToken],
    logical_lines: &[LogicalLine],
    if_idx: usize,
    code_lines: &BTreeSet<u32>,
    out: &mut BTreeSet<(u32, &'static str)>,
) {
    let then_end = block_end_line(logical_lines, if_idx);
    if then_end == if_idx {
        return;
    }
    let else_idx = then_end + 1;
    let Some(else_line) = logical_lines.get(else_idx) else {
        return;
    };
    if else_line.indent != logical_lines[if_idx].indent
        || !tokens[else_line.tokens.clone()]
            .first()
            .is_some_and(|t| t.is_kw("else"))
    {
        return;
    }
    let else_end = block_end_line(logical_lines, else_idx);
    if else_end == else_idx {
        return;
    }
    if tokens[logical_lines[if_idx].tokens.start].is_kw("if")
        && let (Some(a), Some(b)) = (
            single_bool_return(tokens, logical_lines, if_idx + 1, then_end),
            single_bool_return(tokens, logical_lines, else_idx + 1, else_end),
        )
        && a != b
    {
        flag_range(
            logical_lines[if_idx].start_line,
            logical_lines[else_end].end_line,
            "if-else-returns-boolean",
            code_lines,
            out,
        );
        return;
    }
    if ends_with_unconditional_exit(tokens, logical_lines, if_idx + 1, then_end) {
        flag_range(
            else_line.start_line,
            logical_lines[else_end].end_line,
            "else-after-return",
            code_lines,
            out,
        );
    }
}

fn single_bool_return(
    tokens: &[PyToken],
    logical_lines: &[LogicalLine],
    start_idx: usize,
    end_idx: usize,
) -> Option<bool> {
    if start_idx != end_idx {
        return None;
    }
    let slice = trim_trailing_semi(&tokens[logical_lines[start_idx].tokens.clone()]);
    if slice.len() == 2 && slice[0].is_kw("return") {
        bool_literal(&slice[1])
    } else {
        None
    }
}

fn ends_with_unconditional_exit(
    tokens: &[PyToken],
    logical_lines: &[LogicalLine],
    start_idx: usize,
    end_idx: usize,
) -> bool {
    let body_indent = logical_lines[start_idx].indent;
    let Some(last) = logical_lines[start_idx..=end_idx]
        .iter()
        .rev()
        .find(|line| line.indent == body_indent)
    else {
        return false;
    };
    tokens[last.tokens.clone()]
        .first()
        .is_some_and(|t| t.is_kw("return") || t.is_kw("raise"))
}

fn check_except_only_reraises(
    tokens: &[PyToken],
    logical_lines: &[LogicalLine],
    except_idx: usize,
    code_lines: &BTreeSet<u32>,
    out: &mut BTreeSet<(u32, &'static str)>,
) {
    let body_end = block_end_line(logical_lines, except_idx);
    if body_end != except_idx + 1 {
        return;
    }
    let caught_var = extract_caught_var(&tokens[logical_lines[except_idx].tokens.clone()]);
    let body = trim_trailing_semi(&tokens[logical_lines[body_end].tokens.clone()]);
    if is_reraise_stmt(body, caught_var) {
        flag_range(
            logical_lines[except_idx].start_line,
            logical_lines[body_end].end_line,
            "except-only-reraises",
            code_lines,
            out,
        );
    }
}

fn extract_caught_var(header: &[PyToken]) -> Option<&str> {
    header
        .iter()
        .position(|t| t.is_kw("as"))
        .and_then(|pos| header.get(pos + 1))
        .filter(|t| t.is_ident())
        .map(|t| t.text.as_str())
}

fn is_reraise_stmt(body: &[PyToken], caught_var: Option<&str>) -> bool {
    if !body.first().is_some_and(|t| t.is_kw("raise")) {
        return false;
    }
    match body.len() {
        1 => true,
        2 => caught_var.is_some_and(|name| body[1].is_ident() && body[1].text == name),
        _ => false,
    }
}

fn check_trailing_bare_return(
    tokens: &[PyToken],
    logical_lines: &[LogicalLine],
    def_idx: usize,
    code_lines: &BTreeSet<u32>,
    out: &mut BTreeSet<(u32, &'static str)>,
) {
    let body_end = block_end_line(logical_lines, def_idx);
    if body_end <= def_idx + 1 {
        return;
    }
    let body_indent = logical_lines[def_idx + 1].indent;
    let last = &logical_lines[body_end];
    if last.indent != body_indent {
        return;
    }
    let slice = trim_trailing_semi(&tokens[last.tokens.clone()]);
    if is_bare_or_none_return(slice) {
        flag_range(
            last.start_line,
            last.end_line,
            "trailing-bare-return",
            code_lines,
            out,
        );
    }
}

fn is_bare_or_none_return(slice: &[PyToken]) -> bool {
    if !slice.first().is_some_and(|t| t.is_kw("return")) {
        return false;
    }
    slice.len() == 1 || (slice.len() == 2 && slice[1].is_kw("None"))
}

fn check_conditional_bool(
    slice: &[PyToken],
    code_lines: &BTreeSet<u32>,
    out: &mut BTreeSet<(u32, &'static str)>,
) {
    for (idx, tok) in slice.iter().enumerate() {
        let Some(first_bool) = bool_literal(tok) else {
            continue;
        };
        if !slice.get(idx + 1).is_some_and(|t| t.is_kw("if")) {
            continue;
        }
        let Some(else_pos) = find_matching_inline_else(slice, idx + 2) else {
            continue;
        };
        let Some(second) = slice.get(else_pos + 1) else {
            continue;
        };
        if bool_literal(second).is_some_and(|second_bool| second_bool != first_bool) {
            flag_range(
                tok.line,
                second.line,
                "conditional-yields-boolean",
                code_lines,
                out,
            );
        }
    }
}

fn find_matching_inline_else(slice: &[PyToken], start: usize) -> Option<usize> {
    let mut depth = 0usize;
    for (idx, tok) in slice.iter().enumerate().skip(start) {
        match tok.text.as_str() {
            "(" | "[" | "{" => depth += 1,
            ")" | "]" | "}" if depth == 0 => return None,
            ")" | "]" | "}" => depth -= 1,
            _ if depth == 0 && tok.is_kw("else") => return Some(idx),
            _ => {}
        }
    }
    None
}

fn bool_literal(tok: &PyToken) -> Option<bool> {
    if !tok.is_kw {
        return None;
    }
    match tok.text.as_str() {
        "True" => Some(true),
        "False" => Some(false),
        _ => None,
    }
}

fn trim_trailing_semi(slice: &[PyToken]) -> &[PyToken] {
    if slice.last().is_some_and(|t| t.text == ";") {
        &slice[..slice.len() - 1]
    } else {
        slice
    }
}
