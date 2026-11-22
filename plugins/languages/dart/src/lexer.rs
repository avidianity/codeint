use std::collections::BTreeSet;

use codeint_model::language::LowerError;
use codeint_model::scan::{ScanToken, track_delim};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DartKind {
    Keyword,
    Ident,
    StringLit,
    NumberLit,
    Punct,
}

#[derive(Debug, Clone)]
pub(crate) struct DartToken {
    pub(crate) line: u32,
    pub(crate) text: String,
    pub(crate) kind: DartKind,
}

impl ScanToken for DartToken {
    fn line(&self) -> u32 {
        self.line
    }

    fn is_keyword(&self, kw: &str) -> bool {
        matches!(self.kind, DartKind::Keyword) && self.text == kw
    }

    fn text(&self) -> &str {
        self.text.as_str()
    }
}

pub(crate) struct ScannedDart {
    pub(crate) code_lines: BTreeSet<u32>,
    pub(crate) header_comments: String,
    pub(crate) tokens: Vec<DartToken>,
}

fn is_dart_keyword(word: &str) -> bool {
    matches!(
        word,
        "abstract"
            | "as"
            | "assert"
            | "async"
            | "await"
            | "base"
            | "break"
            | "case"
            | "catch"
            | "class"
            | "const"
            | "continue"
            | "covariant"
            | "default"
            | "deferred"
            | "do"
            | "dynamic"
            | "else"
            | "enum"
            | "export"
            | "extends"
            | "extension"
            | "external"
            | "factory"
            | "false"
            | "final"
            | "finally"
            | "for"
            | "Function"
            | "get"
            | "hide"
            | "if"
            | "implements"
            | "import"
            | "in"
            | "interface"
            | "is"
            | "late"
            | "library"
            | "mixin"
            | "new"
            | "null"
            | "of"
            | "on"
            | "operator"
            | "part"
            | "required"
            | "rethrow"
            | "return"
            | "sealed"
            | "set"
            | "show"
            | "static"
            | "super"
            | "switch"
            | "sync"
            | "this"
            | "throw"
            | "true"
            | "try"
            | "typedef"
            | "var"
            | "void"
            | "when"
            | "while"
            | "with"
            | "yield"
    )
}

struct Scanner<'a> {
    delims: Vec<(u8, u32)>,
    code_lines: BTreeSet<u32>,
    header_comments: String,
    tokens: Vec<DartToken>,
    line: u32,
    idx: usize,
    bytes: &'a [u8],
    src: &'a str,
}

pub(crate) fn scan(text: &str) -> Result<ScannedDart, LowerError> {
    let src = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut scanner = Scanner::new(src);
    scanner.run()?;
    Ok(ScannedDart {
        code_lines: scanner.code_lines,
        header_comments: scanner.header_comments,
        tokens: scanner.tokens,
    })
}

impl<'a> Scanner<'a> {
    fn new(src: &'a str) -> Scanner<'a> {
        Scanner {
            delims: Vec::new(),
            code_lines: BTreeSet::new(),
            header_comments: String::new(),
            tokens: Vec::new(),
            line: 1,
            idx: 0,
            bytes: src.as_bytes(),
            src,
        }
    }
    fn run(&mut self) -> Result<(), LowerError> {
        if self.bytes.starts_with(b"#!") {
            self.skip_to_eol();
        }
        while self.idx < self.bytes.len() {
            self.next_unit()?;
        }
        if let Some((open, open_line)) = self.delims.last() {
            return Err(LowerError::Unparsable(format!(
                "unclosed {:?} opened at line {open_line}",
                *open as char
            )));
        }
        Ok(())
    }

    fn next_unit(&mut self) -> Result<(), LowerError> {
        let rest = &self.bytes[self.idx..];
        let b = rest[0];
        if b.is_ascii_whitespace() {
            if b == b'\n' {
                self.line += 1;
            }
            self.idx += 1;
            return Ok(());
        }
        if rest.starts_with(b"//") {
            self.read_line_comment();
            return Ok(());
        }
        if rest.starts_with(b"/*") {
            return self.read_block_comment();
        }
        if self.is_raw_string_start() {
            return self.read_raw_string();
        }
        if matches!(b, b'\'' | b'"') {
            return self.read_string();
        }
        self.next_atom()
    }

    fn next_atom(&mut self) -> Result<(), LowerError> {
        let b = self.bytes[self.idx];
        if is_id_start(b) {
            self.read_word();
            return Ok(());
        }
        if b.is_ascii_digit() {
            self.read_number();
            return Ok(());
        }
        self.read_punct()
    }

    fn skip_to_eol(&mut self) {
        while self.idx < self.bytes.len() && self.bytes[self.idx] != b'\n' {
            self.idx += 1;
        }
    }

    fn read_line_comment(&mut self) {
        let start = self.idx;
        self.skip_to_eol();
        if self.tokens.is_empty() {
            self.header_comments
                .extend([&self.src[start..self.idx], "\n"]);
        }
    }

    fn read_block_comment(&mut self) -> Result<(), LowerError> {
        let start = self.idx;
        let start_line = self.line;
        self.idx += 2;
        let mut depth = 1usize;
        while self.idx < self.bytes.len() {
            let rest = &self.bytes[self.idx..];
            if rest.starts_with(b"/*") {
                depth += 1;
                self.idx += 2;
            } else if rest.starts_with(b"*/") {
                depth -= 1;
                self.idx += 2;
                if depth == 0 {
                    if self.tokens.is_empty() {
                        self.header_comments.push_str(&self.src[start..self.idx]);
                        self.header_comments.push('\n');
                    }
                    return Ok(());
                }
            } else {
                if rest[0] == b'\n' {
                    self.line += 1;
                }
                self.idx += 1;
            }
        }
        Err(LowerError::Unparsable(format!(
            "unterminated block comment at line {start_line}"
        )))
    }

    fn is_raw_string_start(&self) -> bool {
        self.bytes[self.idx] == b'r'
            && self
                .bytes
                .get(self.idx + 1)
                .is_some_and(|b| matches!(b, b'\'' | b'"'))
    }

    fn read_raw_string(&mut self) -> Result<(), LowerError> {
        let start = self.idx;
        let start_line = self.line;
        self.idx += 1;
        let quote = self.bytes[self.idx];
        let triple = self.is_triple_quote(quote);
        self.idx += if triple { 3 } else { 1 };
        self.consume_string_body(start_line, quote, triple, true)?;
        self.emit_multiline_string(start, start_line);
        Ok(())
    }

    fn read_string(&mut self) -> Result<(), LowerError> {
        let start = self.idx;
        let start_line = self.line;
        let quote = self.bytes[self.idx];
        let triple = self.is_triple_quote(quote);
        self.idx += if triple { 3 } else { 1 };
        self.consume_string_body(start_line, quote, triple, false)?;
        self.emit_multiline_string(start, start_line);
        Ok(())
    }

    fn is_triple_quote(&self, quote: u8) -> bool {
        self.bytes.get(self.idx + 1) == Some(&quote) && self.bytes.get(self.idx + 2) == Some(&quote)
    }

    fn consume_string_body(
        &mut self,
        start_line: u32,
        quote: u8,
        triple: bool,
        raw: bool,
    ) -> Result<(), LowerError> {
        while self.idx < self.bytes.len() {
            if !raw && self.try_escape_or_interp(start_line)? {
                continue;
            }
            if self.matches_string_end(quote, triple) {
                self.idx += if triple { 3 } else { 1 };
                return Ok(());
            }
            if self.bytes[self.idx] == b'\n' {
                if !triple {
                    return Err(LowerError::Unparsable(format!(
                        "unterminated string literal at line {start_line}"
                    )));
                }
                self.line += 1;
            }
            self.idx += 1;
        }
        Err(LowerError::Unparsable(format!(
            "unterminated string literal at line {start_line}"
        )))
    }

    fn try_escape_or_interp(&mut self, start_line: u32) -> Result<bool, LowerError> {
        let b = self.bytes[self.idx];
        if b == b'\\' && self.idx + 1 < self.bytes.len() {
            if self.bytes[self.idx + 1] == b'\n' {
                self.line += 1;
            }
            self.idx += 2;
            return Ok(true);
        }
        if b == b'$' && self.bytes.get(self.idx + 1) == Some(&b'{') {
            self.skip_interpolation(start_line)?;
            return Ok(true);
        }
        Ok(false)
    }

    fn matches_string_end(&self, quote: u8, triple: bool) -> bool {
        if self.bytes[self.idx] != quote {
            return false;
        }
        !triple || self.is_triple_quote(quote)
    }

    fn emit_multiline_string(&mut self, start: usize, start_line: u32) {
        for l in start_line..=self.line {
            self.code_lines.insert(l);
        }
        self.tokens.push(DartToken {
            kind: DartKind::StringLit,
            text: self.src[start..self.idx].to_owned(),
            line: start_line,
        });
    }

    fn skip_interpolation(&mut self, start_line: u32) -> Result<(), LowerError> {
        self.idx += 2;
        let mut depth = 1usize;
        while self.idx < self.bytes.len() && depth > 0 {
            depth = self.step_interpolation_byte(depth, start_line)?;
        }
        if depth != 0 {
            return Err(LowerError::Unparsable(format!(
                "unterminated string interpolation at line {start_line}"
            )));
        }
        Ok(())
    }

    fn step_interpolation_byte(
        &mut self,
        depth: usize,
        start_line: u32,
    ) -> Result<usize, LowerError> {
        let b = self.bytes[self.idx];
        match b {
            b'{' => {
                self.idx += 1;
                Ok(depth + 1)
            }
            b'}' => {
                self.idx += 1;
                Ok(depth - 1)
            }
            b'\'' | b'"' => {
                self.skip_embedded_string(start_line, false)?;
                Ok(depth)
            }
            _ if self.is_raw_string_start() => {
                self.skip_embedded_string(start_line, true)?;
                Ok(depth)
            }
            _ => {
                if b == b'\n' {
                    self.line += 1;
                }
                self.idx += 1;
                Ok(depth)
            }
        }
    }

    fn skip_embedded_string(&mut self, start_line: u32, raw: bool) -> Result<(), LowerError> {
        if raw {
            self.idx += 1;
        }
        let quote = self.bytes[self.idx];
        let triple = self.is_triple_quote(quote);
        self.idx += if triple { 3 } else { 1 };
        self.consume_string_body(start_line, quote, triple, raw)
    }

    fn read_word(&mut self) {
        let start = self.idx;
        self.idx += 1;
        while self.idx < self.bytes.len() && is_id_continue(self.bytes[self.idx]) {
            self.idx += 1;
        }
        let slice = &self.src[start..self.idx];
        let kind = if is_dart_keyword(slice) {
            DartKind::Keyword
        } else {
            DartKind::Ident
        };
        self.push_tok(kind, slice);
    }

    fn read_number(&mut self) {
        let start = self.idx;
        self.idx += 1;
        while self.idx < self.bytes.len() && is_number_char(self.bytes[self.idx]) {
            self.idx += 1;
        }
        let slice = &self.src[start..self.idx];
        self.push_tok(DartKind::NumberLit, slice);
    }

    fn read_punct(&mut self) -> Result<(), LowerError> {
        let op =
            match_dart_op(&self.bytes[self.idx..]).unwrap_or(&self.src[self.idx..self.idx + 1]);
        track_delim(&mut self.delims, op, self.line)?;
        self.push_tok(DartKind::Punct, op);
        self.idx += op.len();
        Ok(())
    }

    fn push_tok(&mut self, kind: DartKind, text: &str) {
        self.code_lines.insert(self.line);
        self.tokens.push(DartToken {
            kind,
            text: text.to_owned(),
            line: self.line,
        });
    }
}

fn is_id_start(b: u8) -> bool {
    matches!(b, b'a'..=b'z' | b'A'..=b'Z' | b'_' | b'$' | 0x80..=0xff)
}

fn is_id_continue(b: u8) -> bool {
    is_id_start(b) || b.is_ascii_digit()
}

fn is_number_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || matches!(b, b'_' | b'.')
}

fn match_dart_op(bytes: &[u8]) -> Option<&'static str> {
    static MULTI_OPS: &[&str] = &[
        "...?", ">>>=", "...", "?..", "??=", "~/=", ">>=", "<<=", ">>>", "~/", "??", "?.", "..",
        "=>", "&&", "||", "==", "!=", "<=", ">=", "++", "--", "+=", "-=", "*=", "/=", "%=", "&=",
        "|=", "^=", "<<", ">>",
    ];
    MULTI_OPS
        .iter()
        .copied()
        .find(|op| bytes.starts_with(op.as_bytes()))
}
