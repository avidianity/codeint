use std::collections::BTreeSet;

use codeint_model::scan::find_matching_delim;

use crate::lexer::{GoKind, GoToken};

pub(crate) struct ScannedImports {
    pub(crate) specifiers: Vec<String>,
    pub(crate) marked_indices: BTreeSet<usize>,
    pub(crate) is_package_main: bool,
}

pub(crate) fn scan(tokens: &[GoToken]) -> ScannedImports {
    let mut state = ImportScanState::default();
    let mut cursor = 0usize;
    while cursor < tokens.len() {
        cursor = state.visit(tokens, cursor);
    }
    ScannedImports {
        specifiers: state.specifiers.into_iter().collect(),
        marked_indices: state.marked,
        is_package_main: state.is_package_main,
    }
}

#[derive(Default)]
struct ImportScanState {
    brace_depth: usize,
    is_package_main: bool,
    specifiers: BTreeSet<String>,
    marked: BTreeSet<usize>,
}

impl ImportScanState {
    fn visit(&mut self, tokens: &[GoToken], cursor: usize) -> usize {
        let tok = &tokens[cursor];
        if tok.text == "{" {
            self.brace_depth += 1;
            return cursor + 1;
        }
        if tok.text == "}" {
            self.brace_depth = self.brace_depth.saturating_sub(1);
            return cursor + 1;
        }
        if self.brace_depth == 0 && tok.is_kw("package") {
            return self.scan_package(tokens, cursor);
        }
        if self.brace_depth == 0 && tok.is_kw("import") {
            return self.scan_import(tokens, cursor);
        }
        cursor + 1
    }

    fn scan_package(&mut self, tokens: &[GoToken], pkg_idx: usize) -> usize {
        self.marked.insert(pkg_idx);
        let mut next = pkg_idx + 1;
        if let Some(ident) = tokens.get(next)
            && ident.kind == GoKind::Ident
        {
            self.is_package_main |= ident.text == "main";
            self.marked.insert(next);
            next += 1;
        }
        self.consume_optional_semi(tokens, next)
    }

    fn scan_import(&mut self, tokens: &[GoToken], import_idx: usize) -> usize {
        self.marked.insert(import_idx);
        let next = import_idx + 1;
        if tokens.get(next).is_some_and(|t| t.text == "(")
            && let Some(close) = find_matching_delim(tokens, next)
        {
            for (i, tok) in tokens.iter().enumerate().take(close + 1).skip(next) {
                self.marked.insert(i);
                if tok.kind == GoKind::StringLit
                    && let Some(path) = unquote_go_string(&tok.text)
                {
                    self.specifiers.insert(path);
                }
            }
            return self.consume_optional_semi(tokens, close + 1);
        }
        self.scan_single_import(tokens, next)
    }

    fn scan_single_import(&mut self, tokens: &[GoToken], mut cursor: usize) -> usize {
        if let Some(tok) = tokens.get(cursor)
            && (tok.kind == GoKind::Ident || tok.text == ".")
        {
            self.marked.insert(cursor);
            cursor += 1;
        }
        if let Some(tok) = tokens.get(cursor)
            && tok.kind == GoKind::StringLit
        {
            self.marked.insert(cursor);
            if let Some(path) = unquote_go_string(&tok.text) {
                self.specifiers.insert(path);
            }
            cursor += 1;
        }
        self.consume_optional_semi(tokens, cursor)
    }

    fn consume_optional_semi(&mut self, tokens: &[GoToken], cursor: usize) -> usize {
        if tokens.get(cursor).is_some_and(GoToken::is_semi) {
            self.marked.insert(cursor);
            cursor + 1
        } else {
            cursor
        }
    }
}

fn unquote_go_string(raw: &str) -> Option<String> {
    if let Some(inner) = raw.strip_prefix('`').and_then(|s| s.strip_suffix('`')) {
        return Some(inner.to_owned());
    }
    let inner = raw.strip_prefix('"').and_then(|s| s.strip_suffix('"'))?;
    if !inner.contains('\\') {
        return Some(inner.to_owned());
    }
    let mut out = String::with_capacity(inner.len());
    let mut chars = inner.chars();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            if let Some(escaped) = chars.next() {
                out.push(escaped);
            }
        } else {
            out.push(ch);
        }
    }
    Some(out)
}
