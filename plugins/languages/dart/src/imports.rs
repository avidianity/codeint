use std::collections::BTreeSet;

use crate::lexer::{DartKind, DartToken};

pub(crate) fn scan_directives(tokens: &[DartToken]) -> (Vec<String>, BTreeSet<usize>) {
    let mut marked = BTreeSet::new();
    let mut specifiers = BTreeSet::new();
    let mut depth = 0usize;
    let mut idx = 0usize;

    while idx < tokens.len() {
        let tok = &tokens[idx];
        depth = match tok.text.as_str() {
            "{" => depth + 1,
            "}" => depth.saturating_sub(1),
            _ => depth,
        };
        if depth == 0
            && tok.kind == DartKind::Keyword
            && let Some(next) = try_directive(tokens, idx, &mut specifiers, &mut marked)
        {
            idx = next;
            continue;
        }
        idx += 1;
    }

    (specifiers.into_iter().collect(), marked)
}

fn try_directive(
    tokens: &[DartToken],
    idx: usize,
    specifiers: &mut BTreeSet<String>,
    marked: &mut BTreeSet<usize>,
) -> Option<usize> {
    let kw = tokens[idx].text.as_str();
    match kw {
        "library" => Some(mark_to_semicolon(tokens, idx, marked) + 1),
        "import" | "export" => {
            if let Some(uri) = tokens
                .get(idx + 1)
                .filter(|t| t.kind == DartKind::StringLit)
                .and_then(|t| unquote_dart_string(&t.text))
            {
                specifiers.insert(uri.to_owned());
            }
            Some(mark_to_semicolon(tokens, idx, marked) + 1)
        }
        "part" => {
            let is_part_of = tokens
                .get(idx + 1)
                .is_some_and(|t| t.kind == DartKind::Keyword && t.text == "of");
            if !is_part_of
                && let Some(uri) = tokens
                    .get(idx + 1)
                    .filter(|t| t.kind == DartKind::StringLit)
                    .and_then(|t| unquote_dart_string(&t.text))
            {
                specifiers.insert(uri.to_owned());
            }
            Some(mark_to_semicolon(tokens, idx, marked) + 1)
        }
        _ => None,
    }
}

fn mark_to_semicolon(tokens: &[DartToken], start: usize, marked: &mut BTreeSet<usize>) -> usize {
    let mut i = start;
    while i < tokens.len() {
        marked.insert(i);
        if tokens[i].text == ";" {
            return i;
        }
        i += 1;
    }
    tokens.len().saturating_sub(1)
}

fn unquote_dart_string(lit: &str) -> Option<&str> {
    let body = lit.strip_prefix('r').unwrap_or(lit);
    for delim in ["'''", "\"\"\"", "'", "\""] {
        if body.len() >= delim.len() * 2 && body.starts_with(delim) && body.ends_with(delim) {
            let inner = &body[delim.len()..body.len() - delim.len()];
            return (!inner.contains('$')).then_some(inner);
        }
    }
    None
}
