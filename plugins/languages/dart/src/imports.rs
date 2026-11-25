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
    match tokens[idx].text.as_str() {
        "library" => try_library_directive(tokens, idx, marked),
        "import" | "export" => try_import_export_directive(tokens, idx, specifiers, marked),
        "part" => try_part_directive(tokens, idx, specifiers, marked),
        _ => None,
    }
}

fn try_library_directive(
    tokens: &[DartToken],
    idx: usize,
    marked: &mut BTreeSet<usize>,
) -> Option<usize> {
    can_start_directive(tokens, idx).then(|| mark_to_semicolon(tokens, idx, marked) + 1)
}

fn try_import_export_directive(
    tokens: &[DartToken],
    idx: usize,
    specifiers: &mut BTreeSet<String>,
    marked: &mut BTreeSet<usize>,
) -> Option<usize> {
    let uri = extract_string_uri(tokens.get(idx + 1))?;
    let end = mark_to_semicolon(tokens, idx, marked);
    specifiers.insert(uri);
    if idx + 2 <= end {
        collect_conditional_uris(&tokens[idx + 2..=end], specifiers);
    }
    Some(end + 1)
}

fn try_part_directive(
    tokens: &[DartToken],
    idx: usize,
    specifiers: &mut BTreeSet<String>,
    marked: &mut BTreeSet<usize>,
) -> Option<usize> {
    let is_part_of = tokens
        .get(idx + 1)
        .is_some_and(|t| t.kind == DartKind::Keyword && t.text == "of");
    if is_part_of {
        return Some(mark_to_semicolon(tokens, idx, marked) + 1);
    }
    let uri = extract_string_uri(tokens.get(idx + 1))?;
    specifiers.insert(uri);
    Some(mark_to_semicolon(tokens, idx, marked) + 1)
}

fn can_start_directive(tokens: &[DartToken], idx: usize) -> bool {
    if idx == 0 {
        return true;
    }
    let prev = &tokens[idx - 1];
    !matches!(
        prev.text.as_str(),
        "." | "?." | ".." | "=" | "," | "(" | "[" | "get" | "set" | "final" | "var" | "const"
    )
}

fn extract_string_uri(token: Option<&DartToken>) -> Option<String> {
    token
        .filter(|t| t.kind == DartKind::StringLit)
        .and_then(|t| unquote_dart_string(&t.text))
        .map(str::to_owned)
}

fn collect_conditional_uris(slice: &[DartToken], specifiers: &mut BTreeSet<String>) {
    let mut cursor = 0usize;
    while cursor < slice.len() {
        if slice[cursor].kind == DartKind::Keyword
            && slice[cursor].text == "if"
            && slice.get(cursor + 1).is_some_and(|t| t.text == "(")
        {
            let after_cond = skip_parenthesized(slice, cursor + 1);
            if let Some(uri) = extract_string_uri(slice.get(after_cond)) {
                specifiers.insert(uri);
                cursor = after_cond + 1;
                continue;
            }
            cursor = after_cond;
            continue;
        }
        cursor += 1;
    }
}

fn skip_parenthesized(slice: &[DartToken], open_paren: usize) -> usize {
    let mut depth = 0usize;
    let mut i = open_paren;
    while i < slice.len() {
        match slice[i].text.as_str() {
            "(" => depth += 1,
            ")" => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return i + 1;
                }
            }
            _ => {}
        }
        i += 1;
    }
    slice.len()
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
