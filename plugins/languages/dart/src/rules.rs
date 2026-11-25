use std::collections::BTreeSet;

use codeint_model::language::FlaggedLine;
use codeint_model::scan::{
    check_c_style_if_else, check_ternary_bool, finalize_flagged, flag_range, paren_then_brace,
};

use crate::lexer::{DartKind, DartToken, matching_open_paren};

pub fn flagged_line_rules() -> impl Iterator<Item = &'static str> {
    [
        "else-after-return",
        "if-else-returns-boolean",
        "catch-only-rethrows",
        "ternary-yields-boolean",
        "trailing-bare-return",
    ]
    .into_iter()
}

pub(crate) fn evaluate(tokens: &[DartToken], code_lines: &BTreeSet<u32>) -> Vec<FlaggedLine> {
    const EXITS: [&str; 5] = ["return", "throw", "rethrow", "break", "continue"];
    let mut hits = BTreeSet::new();
    for (idx, tok) in tokens.iter().enumerate() {
        if tok.kind == DartKind::Keyword && tok.text == "if" {
            check_c_style_if_else(tokens, idx, &EXITS, code_lines, &mut hits);
        } else if tok.kind == DartKind::Keyword && tok.text == "catch" {
            inspect_catch(tokens, idx, code_lines, &mut hits);
        } else if tok.kind == DartKind::Punct && tok.text == "?" {
            check_ternary_bool(tokens, idx, code_lines, &mut hits);
        } else if tok.kind == DartKind::Punct && tok.text == "}" {
            inspect_trailing_return(tokens, idx, code_lines, &mut hits);
        }
    }
    finalize_flagged(hits)
}

fn inspect_catch(
    tokens: &[DartToken],
    catch_idx: usize,
    code_lines: &BTreeSet<u32>,
    out: &mut BTreeSet<(u32, &'static str)>,
) {
    if let Some((p_open, p_close, b_open, b_close)) = paren_then_brace(tokens, catch_idx) {
        let caught = tokens[p_open + 1..p_close]
            .iter()
            .find_map(|t| (t.kind == DartKind::Ident).then_some(t.text.as_str()));
        if is_rethrow_only_body(&tokens[b_open + 1..b_close], caught) {
            flag_range(
                tokens[catch_idx].line,
                tokens[b_close].line,
                "catch-only-rethrows",
                code_lines,
                out,
            );
        }
    }
}

fn is_rethrow_only_body(body: &[DartToken], caught: Option<&str>) -> bool {
    if body.len() == 2 && body[0].kind == DartKind::Keyword && body[0].text == "rethrow" {
        return body[1].text == ";";
    }
    body.len() == 3
        && body[0].kind == DartKind::Keyword
        && body[0].text == "throw"
        && caught.is_some_and(|name| body[1].kind == DartKind::Ident && body[1].text == name)
        && body[2].text == ";"
}

fn inspect_trailing_return(
    tokens: &[DartToken],
    close_brace: usize,
    code_lines: &BTreeSet<u32>,
    out: &mut BTreeSet<(u32, &'static str)>,
) {
    if close_brace < 3 {
        return;
    }
    let semi = &tokens[close_brace - 1];
    let ret = &tokens[close_brace - 2];
    let prev = &tokens[close_brace - 3];
    if semi.text == ";"
        && ret.kind == DartKind::Keyword
        && ret.text == "return"
        && matches!(prev.text.as_str(), ";" | "{" | "}")
        && is_function_block_close(tokens, close_brace)
    {
        flag_range(ret.line, semi.line, "trailing-bare-return", code_lines, out);
    }
}

fn is_function_block_close(tokens: &[DartToken], close_brace: usize) -> bool {
    let mut depth = 0usize;
    let mut idx = close_brace;
    while idx > 0 {
        match tokens[idx].text.as_str() {
            "}" => depth += 1,
            "{" => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return looks_like_function_open(tokens, idx);
                }
            }
            _ => {}
        }
        idx -= 1;
    }
    false
}

fn looks_like_function_open(tokens: &[DartToken], open_brace: usize) -> bool {
    let mut cursor = open_brace;
    while cursor > 0 {
        cursor -= 1;
        let text = tokens[cursor].text.as_str();
        if matches!(text, "async" | "sync" | "*") {
            continue;
        }
        if text != ")" {
            return false;
        }
        let Some(open_paren) = matching_open_paren(tokens, cursor) else {
            return false;
        };
        return open_paren == 0
            || !matches!(
                tokens[open_paren - 1].text.as_str(),
                "if" | "for" | "while" | "switch" | "catch"
            );
    }
    false
}
