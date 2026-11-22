use std::collections::BTreeSet;

use crate::lexer::{RawKind, RawToken, find_matching_delim, find_next_brace};

pub(crate) const USE_PREFIX: &str = "use:";
pub(crate) const REQ_PREFIX: &str = "req:";

pub(crate) struct ExtractedImports {
    pub(crate) specifiers: Vec<String>,
    pub(crate) import_token_indices: BTreeSet<usize>,
}

pub(crate) fn extract(tokens: &[RawToken]) -> ExtractedImports {
    let mut specifiers = BTreeSet::new();
    let mut import_token_indices = BTreeSet::new();
    let mut brace_depth = 0usize;
    let mut i = 0usize;

    while i < tokens.len() {
        if let Some(next) = scan_type_body(tokens, i, &mut specifiers) {
            i = next;
            continue;
        }
        let token = &tokens[i];
        brace_depth = update_brace_depth(brace_depth, token.text.as_str());
        if let Some(next) = scan_top_level_import(
            tokens,
            i,
            brace_depth,
            &mut specifiers,
            &mut import_token_indices,
        ) {
            i = next;
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

fn update_brace_depth(depth: usize, text: &str) -> usize {
    match text {
        "{" => depth + 1,
        "}" => depth.saturating_sub(1),
        _ => depth,
    }
}

fn scan_type_body(
    tokens: &[RawToken],
    idx: usize,
    specifiers: &mut BTreeSet<String>,
) -> Option<usize> {
    let token = &tokens[idx];
    let is_type_kw = token.kind == RawKind::Keyword
        && matches!(
            token.lower.as_str(),
            "class" | "interface" | "trait" | "enum"
        );
    if !is_type_kw
        || tokens
            .get(idx.saturating_sub(1))
            .is_some_and(|p| p.text == "::")
    {
        return None;
    }
    let brace = find_next_brace(tokens, idx + 1)?;
    let close = find_matching_delim(tokens, brace)?;
    for inner in brace + 1..close {
        extract_require(tokens, inner, specifiers);
    }
    Some(close + 1)
}

fn scan_top_level_import(
    tokens: &[RawToken],
    idx: usize,
    brace_depth: usize,
    specifiers: &mut BTreeSet<String>,
    marked: &mut BTreeSet<usize>,
) -> Option<usize> {
    let token = &tokens[idx];
    if token.kind != RawKind::Keyword {
        return None;
    }
    if token.lower == "namespace" {
        return Some(mark_statement(tokens, idx, marked) + 1);
    }
    if token.lower == "use" && brace_depth == 0 && !is_closure_use(tokens, idx) {
        return Some(parse_use_statement(tokens, idx, specifiers, marked) + 1);
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

struct UseAccumulator<'a> {
    current: String,
    group_prefix: Option<String>,
    in_alias: bool,
    specifiers: &'a mut BTreeSet<String>,
}

impl UseAccumulator<'_> {
    fn feed(&mut self, token: &RawToken) -> bool {
        match token.text.as_str() {
            ";" => {
                self.flush();
                return true;
            }
            "{" => {
                self.group_prefix = Some(std::mem::take(&mut self.current));
                self.in_alias = false;
            }
            "}" => {
                self.flush();
                self.group_prefix = None;
                self.in_alias = false;
            }
            "," => {
                self.flush();
                self.in_alias = false;
            }
            _ => self.append_part(token),
        }
        false
    }

    fn append_part(&mut self, token: &RawToken) {
        if token.kind == RawKind::Keyword && token.lower == "as" {
            self.in_alias = true;
            return;
        }
        if self.in_alias {
            return;
        }
        if token.text == "\\" {
            if !self.current.is_empty() && !self.current.ends_with('\\') {
                self.current.push('\\');
            }
        } else {
            self.current.push_str(&token.text);
        }
    }

    fn flush(&mut self) {
        push_use_specifier(&self.group_prefix, &self.current, self.specifiers);
        self.current.clear();
    }
}

fn parse_use_statement(
    tokens: &[RawToken],
    start: usize,
    specifiers: &mut BTreeSet<String>,
    marked: &mut BTreeSet<usize>,
) -> usize {
    marked.insert(start);
    let mut i = start + 1;
    if tokens.get(i).is_some_and(|t| {
        t.kind == RawKind::Keyword && matches!(t.lower.as_str(), "function" | "const")
    }) {
        return mark_statement(tokens, i, marked);
    }
    let mut acc = UseAccumulator {
        current: String::new(),
        group_prefix: None,
        in_alias: false,
        specifiers,
    };
    while i < tokens.len() {
        marked.insert(i);
        if acc.feed(&tokens[i]) {
            return i;
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
