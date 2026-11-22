use std::collections::BTreeSet;
use std::ops::Range;

use codeint_model::language::{LowerError, TokenKind};
use codeint_model::scan::{ScanToken, track_delim};

#[derive(Debug, Clone)]
pub(crate) struct PyToken {
    pub(crate) line: u32,
    pub(crate) text: String,
    pub(crate) token_kind: TokenKind,
    pub(crate) is_kw: bool,
}

impl PyToken {
    pub(crate) fn is_ident(&self) -> bool {
        self.token_kind == TokenKind::Identifier
    }

    pub(crate) fn is_kw(&self, kw: &str) -> bool {
        self.is_kw && self.text == kw
    }
}

impl ScanToken for PyToken {
    fn text(&self) -> &str {
        &self.text
    }

    fn is_keyword(&self, kw: &str) -> bool {
        self.is_kw(kw)
    }

    fn line(&self) -> u32 {
        self.line
    }
}

#[derive(Debug, Clone)]
pub(crate) struct LogicalLine {
    pub(crate) indent: usize,
    pub(crate) start_line: u32,
    pub(crate) end_line: u32,
    pub(crate) tokens: Range<usize>,
}

pub(crate) struct LexResult {
    pub(crate) tokens: Vec<PyToken>,
    pub(crate) logical_lines: Vec<LogicalLine>,
    pub(crate) code_lines: BTreeSet<u32>,
    pub(crate) header_comments: Vec<String>,
}

const KEYWORDS: &[&str] = &[
    "False", "None", "True", "and", "as", "assert", "async", "await", "break", "class", "continue",
    "def", "del", "elif", "else", "except", "finally", "for", "from", "global", "if", "import",
    "in", "is", "lambda", "nonlocal", "not", "or", "pass", "raise", "return", "try", "while",
    "with", "yield",
];

const THREE_CHAR_OPS: &[&str] = &["...", "//=", "**=", "<<=", ">>="];

const TWO_CHAR_OPS: &[&str] = &[
    "==", "!=", "<=", ">=", "//", "**", "<<", ">>", ":=", "->", "+=", "-=", "*=", "/=", "%=", "&=",
    "|=", "^=", "@=",
];

pub(crate) fn scan(text: &str) -> Result<LexResult, LowerError> {
    let mut state = Scanner::new(text);
    state.run()?;
    state.finish()
}

struct Scanner<'a> {
    bytes: &'a [u8],
    src: &'a str,
    pos: usize,
    line: u32,
    tokens: Vec<PyToken>,
    logical_lines: Vec<LogicalLine>,
    code_lines: BTreeSet<u32>,
    header_comments: Vec<String>,
    delims: Vec<(u8, u32)>,
    at_physical_line_start: bool,
    current_line_indent: usize,
    pending_line_start: Option<(usize, u32, usize)>,
}

impl<'a> Scanner<'a> {
    fn new(src: &'a str) -> Self {
        let pos = if src.starts_with('\u{feff}') { 3 } else { 0 };
        Self {
            bytes: src.as_bytes(),
            src,
            pos,
            line: 1,
            tokens: Vec::new(),
            logical_lines: Vec::new(),
            code_lines: BTreeSet::new(),
            header_comments: Vec::new(),
            delims: Vec::new(),
            at_physical_line_start: true,
            current_line_indent: 0,
            pending_line_start: None,
        }
    }

    fn run(&mut self) -> Result<(), LowerError> {
        while self.pos < self.bytes.len() {
            if self.at_physical_line_start {
                self.consume_indent();
                if self.pos >= self.bytes.len() {
                    break;
                }
            }
            self.step()?;
        }
        self.flush_logical_line();
        if let Some((open, open_line)) = self.delims.pop() {
            return Err(LowerError::Unparsable(format!(
                "unclosed {:?} opened at line {open_line}",
                open as char
            )));
        }
        Ok(())
    }

    fn finish(self) -> Result<LexResult, LowerError> {
        reject_python2_print(&self.tokens, &self.logical_lines)?;
        Ok(LexResult {
            tokens: self.tokens,
            logical_lines: self.logical_lines,
            code_lines: self.code_lines,
            header_comments: self.header_comments,
        })
    }

    fn consume_indent(&mut self) {
        let mut indent = 0usize;
        while let Some(&b) = self.bytes.get(self.pos) {
            match b {
                b' ' => {
                    indent += 1;
                    self.pos += 1;
                }
                b'\t' => {
                    indent += 8 - (indent % 8);
                    self.pos += 1;
                }
                b'\x0c' => self.pos += 1,
                _ => break,
            }
        }
        self.current_line_indent = indent;
        self.at_physical_line_start = false;
    }

    fn step(&mut self) -> Result<(), LowerError> {
        let b = self.bytes[self.pos];
        if matches!(b, b' ' | b'\t' | b'\r' | b'\x0c') {
            self.pos += 1;
            return Ok(());
        }
        if b == b'\n' {
            self.pos += 1;
            if self.delims.is_empty() {
                self.flush_logical_line();
            }
            self.line += 1;
            self.at_physical_line_start = true;
            return Ok(());
        }
        if b == b'\\' {
            return self.consume_line_continuation();
        }
        if b == b'#' {
            self.consume_comment();
            return Ok(());
        }
        self.step_code_token(b)
    }

    fn step_code_token(&mut self, b: u8) -> Result<(), LowerError> {
        if matches!(b, b'\'' | b'"') {
            return self.consume_string(self.pos);
        }
        if b.is_ascii_digit()
            || (b == b'.' && self.bytes.get(self.pos + 1).is_some_and(u8::is_ascii_digit))
        {
            self.consume_number();
            return Ok(());
        }
        if is_ident_start(b) {
            return self.consume_ident_or_prefixed_string();
        }
        self.consume_punct()
    }

    fn consume_line_continuation(&mut self) -> Result<(), LowerError> {
        self.pos += 1;
        while matches!(self.bytes.get(self.pos), Some(b' ' | b'\t' | b'\r')) {
            self.pos += 1;
        }
        if self.bytes.get(self.pos) == Some(&b'\n') {
            self.pos += 1;
            self.line += 1;
            self.at_physical_line_start = true;
            return Ok(());
        }
        Err(LowerError::Unparsable(format!(
            "unexpected backslash at line {}",
            self.line
        )))
    }

    fn consume_comment(&mut self) {
        let start = self.pos;
        while let Some(&b) = self.bytes.get(self.pos) {
            if b == b'\n' {
                break;
            }
            self.pos += 1;
        }
        if self.tokens.is_empty() {
            self.header_comments
                .push(self.src[start..self.pos].to_owned());
        }
    }

    fn consume_ident_or_prefixed_string(&mut self) -> Result<(), LowerError> {
        if let Some(quote_pos) = string_prefix_end(self.bytes, self.pos) {
            return self.consume_string(quote_pos);
        }
        let start = self.pos;
        self.pos += 1;
        while let Some(&b) = self.bytes.get(self.pos) {
            if is_ident_continue(b) {
                self.pos += 1;
            } else {
                break;
            }
        }
        let word = &self.src[start..self.pos];
        let is_kw = KEYWORDS.contains(&word);
        let token_kind = if is_kw {
            TokenKind::Other
        } else {
            TokenKind::Identifier
        };
        self.push_token(token_kind, is_kw, word.to_owned(), self.line);
        Ok(())
    }

    fn consume_string(&mut self, quote_pos: usize) -> Result<(), LowerError> {
        let start = self.pos;
        let start_line = self.line;
        let quote = self.bytes[quote_pos];
        let is_triple = self.bytes.get(quote_pos + 1) == Some(&quote)
            && self.bytes.get(quote_pos + 2) == Some(&quote);
        self.pos = quote_pos + if is_triple { 3 } else { 1 };
        if !self.scan_string_body(quote, is_triple) {
            return Err(LowerError::Unparsable(format!(
                "unterminated string at line {start_line}"
            )));
        }
        for line in start_line..=self.line {
            self.code_lines.insert(line);
        }
        let text = self.src[start..self.pos].to_owned();
        self.push_token(TokenKind::Literal, false, text, start_line);
        Ok(())
    }

    fn scan_string_body(&mut self, quote: u8, triple: bool) -> bool {
        while self.pos < self.bytes.len() {
            let b = self.bytes[self.pos];
            if b == b'\\' {
                if self.bytes.get(self.pos + 1) == Some(&b'\n') {
                    self.line += 1;
                }
                self.pos = (self.pos + 2).min(self.bytes.len());
                continue;
            }
            if b == b'\n' {
                if !triple {
                    return false;
                }
                self.line += 1;
                self.pos += 1;
                continue;
            }
            if b == quote && self.try_close_quote(quote, triple) {
                return true;
            }
            self.pos += 1;
        }
        false
    }

    fn try_close_quote(&mut self, quote: u8, triple: bool) -> bool {
        if !triple {
            self.pos += 1;
            return true;
        }
        if self.bytes.get(self.pos + 1) == Some(&quote)
            && self.bytes.get(self.pos + 2) == Some(&quote)
        {
            self.pos += 3;
            return true;
        }
        false
    }

    fn consume_number(&mut self) {
        let start = self.pos;
        let line = self.line;
        while let Some(&b) = self.bytes.get(self.pos) {
            let is_exp_sign = matches!(b, b'+' | b'-')
                && self.pos > start
                && matches!(self.bytes[self.pos - 1], b'e' | b'E');
            if b.is_ascii_alphanumeric() || b == b'_' || b == b'.' || is_exp_sign {
                self.pos += 1;
            } else {
                break;
            }
        }
        self.push_token(
            TokenKind::Literal,
            false,
            self.src[start..self.pos].to_owned(),
            line,
        );
    }

    fn consume_punct(&mut self) -> Result<(), LowerError> {
        let rest = &self.src[self.pos..];
        if rest.starts_with("<>") {
            return Err(LowerError::Unparsable(format!(
                "Python 2 '<>' operator at line {}",
                self.line
            )));
        }
        let op = match_operator(rest);
        self.pos += op.len();
        track_delim(&mut self.delims, op, self.line)?;
        self.push_token(TokenKind::Other, false, op.to_owned(), self.line);
        if op == ";" && self.delims.is_empty() {
            self.flush_logical_line();
        }
        Ok(())
    }

    fn push_token(&mut self, token_kind: TokenKind, is_kw: bool, text: String, line: u32) {
        if self.pending_line_start.is_none() {
            self.pending_line_start = Some((self.current_line_indent, line, self.tokens.len()));
        }
        self.code_lines.insert(line);
        self.tokens.push(PyToken {
            line,
            text,
            token_kind,
            is_kw,
        });
    }

    fn flush_logical_line(&mut self) {
        let Some((indent, start_line, start_idx)) = self.pending_line_start.take() else {
            return;
        };
        if start_idx < self.tokens.len() {
            let end_line = self.tokens[self.tokens.len() - 1].line;
            self.logical_lines.push(LogicalLine {
                indent,
                start_line,
                end_line,
                tokens: start_idx..self.tokens.len(),
            });
        }
    }
}

fn match_operator(rest: &str) -> &str {
    for &op in THREE_CHAR_OPS {
        if rest.starts_with(op) {
            return op;
        }
    }
    for &op in TWO_CHAR_OPS {
        if rest.starts_with(op) {
            return op;
        }
    }
    let ch = rest.chars().next().map_or(1, char::len_utf8);
    &rest[..ch]
}

fn string_prefix_end(bytes: &[u8], pos: usize) -> Option<usize> {
    let b0 = *bytes.get(pos)?;
    if !is_string_prefix_char(b0) {
        return None;
    }
    let b1 = *bytes.get(pos + 1)?;
    if b1 == b'\'' || b1 == b'"' {
        return Some(pos + 1);
    }
    if is_string_prefix_char(b1) && matches!(bytes.get(pos + 2), Some(b'\'' | b'"')) {
        return Some(pos + 2);
    }
    None
}

fn is_string_prefix_char(b: u8) -> bool {
    matches!(b, b'r' | b'R' | b'u' | b'U' | b'b' | b'B' | b'f' | b'F')
}

fn is_ident_start(b: u8) -> bool {
    b.is_ascii_alphabetic() || b == b'_' || b >= 0x80
}

fn is_ident_continue(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b >= 0x80
}

fn reject_python2_print(
    tokens: &[PyToken],
    logical_lines: &[LogicalLine],
) -> Result<(), LowerError> {
    for line in logical_lines {
        let slice = &tokens[line.tokens.clone()];
        let Some(first) = slice.first() else {
            continue;
        };
        if !first.is_ident() || first.text != "print" {
            continue;
        }
        let Some(second) = slice.get(1) else {
            continue;
        };
        let is_py2_arg = matches!(
            second.token_kind,
            TokenKind::Literal | TokenKind::Identifier
        ) || second.text == ">>";
        if is_py2_arg {
            return Err(LowerError::Unparsable(format!(
                "Python 2 print statement at line {}",
                first.line
            )));
        }
    }
    Ok(())
}
