use std::collections::BTreeSet;

use crate::lexer::{RawKind, RawToken, find_matching_delim};

pub(crate) const USE_PREFIX: &str = "use:";
pub(crate) const REQ_PREFIX: &str = "req:";

pub(crate) struct ExtractedImports {
    pub(crate) specifiers: Vec<String>,
    pub(crate) import_token_indices: BTreeSet<usize>,
}

pub(crate) fn extract(tokens: &[RawToken]) -> ExtractedImports {
    let mut specifiers = BTreeSet::new();
    let mut import_token_indices = BTreeSet::new();
    let mut type_brace_depth = 0usize;
    let mut i = 0usize;

    while i < tokens.len() {
        let token = &tokens[i];
        if token.kind == RawKind::Keyword
            && matches!(
                token.lower.as_str(),
                "class" | "interface" | "trait" | "enum"
            )
            && !tokens
                .get(i.saturating_sub(1))
                .is_some_and(|prev| prev.text == "::")
            && let Some(brace) = find_next_brace(tokens, i + 1)
            && let Some(close) = find_matching_delim(tokens, brace)
        {
            let mut inner = brace + 1;
            while inner < close {
                extract_require(tokens, inner, &mut specifiers);
                inner += 1;
            }
            i = close + 1;
            continue;
        }
        if token.text == "{" {
            type_brace_depth += 1;
        } else if token.text == "}" {
            type_brace_depth = type_brace_depth.saturating_sub(1);
        }

        if token.kind == RawKind::Keyword && token.lower == "namespace" {
            let end = mark_statement(tokens, i, &mut import_token_indices);
            i = end + 1;
            continue;
        }

        if token.kind == RawKind::Keyword
            && token.lower == "use"
            && type_brace_depth == 0
            && !is_closure_use(tokens, i)
        {
            let end = parse_use_statement(tokens, i, &mut specifiers, &mut import_token_indices);
            i = end + 1;
            continue;
        }

        extract_require(tokens, i, &mut specifiers);
        i += 1;
    }

    ExtractedImports {
        specifiers: specifiers.into_iter().collect(),
        import_token_indices,
    }
}

fn find_next_brace(tokens: &[RawToken], start: usize) -> Option<usize> {
    let mut i = start;
    while i < tokens.len() {
        match tokens[i].text.as_str() {
            "{" => return Some(i),
            ";" => return None,
            "(" => i = find_matching_delim(tokens, i)? + 1,
            _ => i += 1,
        }
    }
    None
}

fn is_closure_use(tokens: &[RawToken], use_idx: usize) -> bool {
    tokens.get(use_idx + 1).is_some_and(|t| t.text == "(")
}

fn mark_statement(tokens: &[RawToken], start: usize, marked: &mut BTreeSet<usize>) -> usize {
    let mut i = start;
    while i < tokens.len() {
        marked.insert(i);
        if tokens[i].text == ";" || tokens[i].text == "{" {
            return i;
        }
        i += 1;
    }
    tokens.len().saturating_sub(1)
}

fn parse_use_statement(
    tokens: &[RawToken],
    start: usize,
    specifiers: &mut BTreeSet<String>,
    marked: &mut BTreeSet<usize>,
) -> usize {
    let mut i = start;
    marked.insert(i);
    i += 1;
    let skip_symbol = tokens.get(i).is_some_and(|t| {
        t.kind == RawKind::Keyword && matches!(t.lower.as_str(), "function" | "const")
    });
    if skip_symbol {
        return mark_statement(tokens, i, marked);
    }

    let mut current = String::new();
    let mut group_prefix: Option<String> = None;
    let mut in_alias = false;
    while i < tokens.len() {
        marked.insert(i);
        let t = &tokens[i];
        match t.text.as_str() {
            ";" => {
                push_use_specifier(&group_prefix, &current, specifiers);
                return i;
            }
            "{" => {
                group_prefix = Some(current.clone());
                current.clear();
                in_alias = false;
            }
            "}" => {
                push_use_specifier(&group_prefix, &current, specifiers);
                current.clear();
                group_prefix = None;
                in_alias = false;
            }
            "," => {
                push_use_specifier(&group_prefix, &current, specifiers);
                current.clear();
                in_alias = false;
            }
            _ if t.kind == RawKind::Keyword && t.lower == "as" => {
                in_alias = true;
            }
            "\\" => {
                if !in_alias && !current.is_empty() && !current.ends_with('\\') {
                    current.push('\\');
                }
            }
            _ => {
                if !in_alias {
                    current.push_str(&t.text);
                }
            }
        }
        i += 1;
    }
    tokens.len().saturating_sub(1)
}

fn push_use_specifier(
    group_prefix: &Option<String>,
    current: &str,
    specifiers: &mut BTreeSet<String>,
) {
    let trimmed = current.trim_matches('\\');
    if trimmed.is_empty() {
        return;
    }
    let full = match group_prefix {
        Some(prefix) => {
            let p = prefix.trim_matches('\\');
            if p.is_empty() {
                trimmed.to_owned()
            } else {
                format!("{p}\\{trimmed}")
            }
        }
        None => trimmed.to_owned(),
    };
    specifiers.insert(format!("{USE_PREFIX}{full}"));
}

fn extract_require(tokens: &[RawToken], idx: usize, specifiers: &mut BTreeSet<String>) {
    let token = &tokens[idx];
    if token.kind != RawKind::Keyword
        || !matches!(
            token.lower.as_str(),
            "require" | "require_once" | "include" | "include_once"
        )
    {
        return;
    }
    let mut cursor = idx + 1;
    if tokens.get(cursor).is_some_and(|t| t.text == "(") {
        cursor += 1;
    }
    if tokens.get(cursor).is_some_and(|t| t.text == "__DIR__")
        && tokens.get(cursor + 1).is_some_and(|t| t.text == ".")
        && let Some(lit) = tokens
            .get(cursor + 2)
            .filter(|t| t.kind == RawKind::StringLit)
    {
        if let Some(unquoted) = unquote_simple(&lit.text) {
            let rel = unquoted.strip_prefix('/').unwrap_or(unquoted);
            specifiers.insert(format!("{REQ_PREFIX}./{rel}"));
        }
        return;
    }
    if let Some(lit) = tokens.get(cursor).filter(|t| t.kind == RawKind::StringLit)
        && let Some(unquoted) = unquote_simple(&lit.text)
    {
        specifiers.insert(format!("{REQ_PREFIX}{unquoted}"));
    }
}

fn unquote_simple(lit: &str) -> Option<&str> {
    if lit.len() >= 2
        && ((lit.starts_with('\'') && lit.ends_with('\''))
            || (lit.starts_with('"') && lit.ends_with('"')))
    {
        let inner = &lit[1..lit.len() - 1];
        (!inner.contains('$')).then_some(inner)
    } else {
        None
    }
}
