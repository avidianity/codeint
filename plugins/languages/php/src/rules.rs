use std::collections::BTreeSet;

use codeint_model::language::FlaggedLine;

use crate::lexer::{RawKind, RawToken, find_matching_delim, find_next_brace};

const RULE_IDS: [&str; 6] = [
    "else-after-return",
    "if-else-returns-boolean",
    "catch-only-rethrows",
    "ternary-yields-boolean",
    "trailing-bare-return",
    "constructor-only-forwards",
];

pub fn flagged_line_rules() -> impl Iterator<Item = &'static str> {
    RULE_IDS.into_iter()
}

pub(crate) fn flag(tokens: &[RawToken], code_lines: &BTreeSet<u32>) -> Vec<FlaggedLine> {
    let mut flagged = BTreeSet::new();
    for (i, token) in tokens.iter().enumerate() {
        if token.kind == RawKind::Keyword {
            match token.lower.as_str() {
                "if" => check_if(tokens, i, code_lines, &mut flagged),
                "catch" => check_catch(tokens, i, code_lines, &mut flagged),
                "function" => check_function_rules(tokens, i, code_lines, &mut flagged),
                _ => {}
            }
        } else if token.text == "?" {
            check_ternary_bool(tokens, i, code_lines, &mut flagged);
        }
    }
    flagged
        .into_iter()
        .map(|(line, rule)| FlaggedLine {
            line,
            rule: rule.to_owned(),
        })
        .collect()
}

fn flag_range(
    start_line: u32,
    end_line: u32,
    rule: &'static str,
    code_lines: &BTreeSet<u32>,
    out: &mut BTreeSet<(u32, &'static str)>,
) {
    for &line in code_lines.range(start_line..=end_line) {
        out.insert((line, rule));
    }
}

fn paren_then_brace(tokens: &[RawToken], kw_idx: usize) -> Option<(usize, usize, usize, usize)> {
    let paren_open = kw_idx + 1;
    if tokens.get(paren_open).map(|t| t.text.as_str()) != Some("(") {
        return None;
    }
    let paren_close = find_matching_delim(tokens, paren_open)?;
    let brace_open = paren_close + 1;
    if tokens.get(brace_open).map(|t| t.text.as_str()) != Some("{") {
        return None;
    }
    let brace_close = find_matching_delim(tokens, brace_open)?;
    Some((paren_open, paren_close, brace_open, brace_close))
}

fn check_if(
    tokens: &[RawToken],
    if_idx: usize,
    code_lines: &BTreeSet<u32>,
    out: &mut BTreeSet<(u32, &'static str)>,
) {
    let Some((_, _, then_open, then_close)) = paren_then_brace(tokens, if_idx) else {
        return;
    };
    let else_idx = then_close + 1;
    if tokens
        .get(else_idx)
        .is_none_or(|t| t.kind != RawKind::Keyword || t.lower != "else")
    {
        return;
    }
    let else_open = else_idx + 1;
    if tokens.get(else_open).map(|t| t.text.as_str()) != Some("{") {
        return;
    }
    let Some(else_close) = find_matching_delim(tokens, else_open) else {
        return;
    };
    let then_slice = &tokens[then_open + 1..then_close];
    let else_slice = &tokens[else_open + 1..else_close];
    if let (Some(a), Some(b)) = (
        single_bool_return(then_slice),
        single_bool_return(else_slice),
    ) && a != b
    {
        flag_range(
            tokens[if_idx].line,
            tokens[else_close].line,
            "if-else-returns-boolean",
            code_lines,
            out,
        );
        return;
    }
    if ends_with_unconditional_exit(then_slice) {
        flag_range(
            tokens[else_idx].line,
            tokens[else_close].line,
            "else-after-return",
            code_lines,
            out,
        );
    }
}

fn single_bool_return(slice: &[RawToken]) -> Option<bool> {
    if slice.len() == 3
        && slice[0].kind == RawKind::Keyword
        && slice[0].lower == "return"
        && slice[2].text == ";"
    {
        match slice[1].lower.as_str() {
            "true" => Some(true),
            "false" => Some(false),
            _ => None,
        }
    } else {
        None
    }
}

fn ends_with_unconditional_exit(slice: &[RawToken]) -> bool {
    if slice.last().map(|t| t.text.as_str()) != Some(";") {
        return false;
    }
    let mut depth = 0usize;
    let mut stmt_start = 0usize;
    for (idx, token) in slice.iter().enumerate() {
        match token.text.as_str() {
            "(" | "[" | "{" | "#[" => depth += 1,
            ")" | "]" | "}" => depth = depth.saturating_sub(1),
            ";" if depth == 0 && idx + 1 < slice.len() => stmt_start = idx + 1,
            _ => {}
        }
    }
    slice.get(stmt_start).is_some_and(|t| {
        t.kind == RawKind::Keyword
            && matches!(t.lower.as_str(), "return" | "throw" | "break" | "continue")
    })
}

fn check_catch(
    tokens: &[RawToken],
    catch_idx: usize,
    code_lines: &BTreeSet<u32>,
    out: &mut BTreeSet<(u32, &'static str)>,
) {
    let Some((paren_open, paren_close, open, close)) = paren_then_brace(tokens, catch_idx) else {
        return;
    };
    let Some(var_name) = tokens[paren_open + 1..paren_close]
        .iter()
        .rev()
        .find(|t| t.kind == RawKind::Variable)
        .map(|t| t.text.as_str())
    else {
        return;
    };
    let body = &tokens[open + 1..close];
    if body.len() == 3
        && body[0].kind == RawKind::Keyword
        && body[0].lower == "throw"
        && body[1].kind == RawKind::Variable
        && body[1].text == var_name
        && body[2].text == ";"
    {
        flag_range(
            tokens[catch_idx].line,
            tokens[close].line,
            "catch-only-rethrows",
            code_lines,
            out,
        );
    }
}

fn check_ternary_bool(
    tokens: &[RawToken],
    q_idx: usize,
    code_lines: &BTreeSet<u32>,
    out: &mut BTreeSet<(u32, &'static str)>,
) {
    if q_idx + 3 >= tokens.len() {
        return;
    }
    let a = tokens[q_idx + 1].lower.as_str();
    let colon = tokens[q_idx + 2].text.as_str();
    let b = tokens[q_idx + 3].lower.as_str();
    if colon == ":" && ((a == "true" && b == "false") || (a == "false" && b == "true")) {
        flag_range(
            tokens[q_idx].line,
            tokens[q_idx + 3].line,
            "ternary-yields-boolean",
            code_lines,
            out,
        );
    }
}

struct FunctionParts<'a> {
    name: Option<&'a str>,
    params: &'a [RawToken],
    body: &'a [RawToken],
    close_line: u32,
}

fn locate_function_parts(tokens: &[RawToken], fn_idx: usize) -> Option<FunctionParts<'_>> {
    let mut cursor = fn_idx + 1;
    if tokens.get(cursor).is_some_and(|t| t.text == "&") {
        cursor += 1;
    }
    let name = tokens
        .get(cursor)
        .filter(|t| matches!(t.kind, RawKind::Ident | RawKind::Keyword))
        .map(|t| {
            cursor += 1;
            t.lower.as_str()
        });
    if tokens.get(cursor).map(|t| t.text.as_str()) != Some("(") {
        return None;
    }
    let paren_close = find_matching_delim(tokens, cursor)?;
    let open = find_next_brace(tokens, paren_close + 1)?;
    let close = find_matching_delim(tokens, open)?;
    Some(FunctionParts {
        name,
        params: &tokens[cursor + 1..paren_close],
        body: &tokens[open + 1..close],
        close_line: tokens[close].line,
    })
}

fn check_function_rules(
    tokens: &[RawToken],
    fn_idx: usize,
    code_lines: &BTreeSet<u32>,
    out: &mut BTreeSet<(u32, &'static str)>,
) {
    let Some(parts) = locate_function_parts(tokens, fn_idx) else {
        return;
    };
    check_trailing_bare_return(parts.body, code_lines, out);
    if parts.name == Some("__construct") && constructor_forwards_only(parts.params, parts.body) {
        flag_range(
            tokens[fn_idx].line,
            parts.close_line,
            "constructor-only-forwards",
            code_lines,
            out,
        );
    }
}

fn check_trailing_bare_return(
    body: &[RawToken],
    code_lines: &BTreeSet<u32>,
    out: &mut BTreeSet<(u32, &'static str)>,
) {
    let len = body.len();
    if len < 2 {
        return;
    }
    let is_bare_return = body[len - 2].kind == RawKind::Keyword
        && body[len - 2].lower == "return"
        && body[len - 1].text == ";";
    let starts_statement = len == 2 || matches!(body[len - 3].text.as_str(), ";" | "}" | "{");
    if is_bare_return && starts_statement {
        flag_range(
            body[len - 2].line,
            body[len - 1].line,
            "trailing-bare-return",
            code_lines,
            out,
        );
    }
}

fn has_promoted_property(params_slice: &[RawToken]) -> bool {
    params_slice.iter().any(|t| {
        t.kind == RawKind::Keyword
            && matches!(
                t.lower.as_str(),
                "public" | "protected" | "private" | "readonly"
            )
    })
}

fn parent_construct_args(body: &[RawToken]) -> Option<&[RawToken]> {
    if body.len() >= 6
        && body[0].lower == "parent"
        && body[1].text == "::"
        && body[2].lower == "__construct"
        && body[3].text == "("
        && body[body.len() - 2].text == ")"
        && body[body.len() - 1].text == ";"
    {
        Some(&body[4..body.len() - 2])
    } else {
        None
    }
}

fn constructor_forwards_only(params_slice: &[RawToken], body: &[RawToken]) -> bool {
    if has_promoted_property(params_slice) {
        return false;
    }
    let Some(args_slice) = parent_construct_args(body) else {
        return false;
    };
    let params: Vec<&str> = params_slice
        .iter()
        .filter(|t| t.kind == RawKind::Variable)
        .map(|t| t.text.as_str())
        .collect();
    if args_slice.is_empty() {
        return params.is_empty();
    }
    let mut args = Vec::new();
    for chunk in args_slice.split(|t| t.text == ",") {
        if chunk.len() != 1 || chunk[0].kind != RawKind::Variable {
            return false;
        }
        args.push(chunk[0].text.as_str());
    }
    args == params
}
