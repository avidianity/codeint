use std::collections::BTreeSet;

use codeint_model::language::LowerError;
use codeint_model::scan::{ScanToken, track_delim};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GoKind {
    Keyword,
    Ident,
    StringLit,
    RuneLit,
    NumberLit,
    Punct,
    AutoSemi,
}

#[derive(Debug, Clone)]
pub(crate) struct GoToken {
    pub(crate) line: u32,
    pub(crate) text: String,
    pub(crate) kind: GoKind,
}

impl GoToken {
    pub(crate) fn is_kw(&self, kw: &str) -> bool {
        self.kind == GoKind::Keyword && self.text == kw
    }

    pub(crate) fn is_semi(&self) -> bool {
        matches!(self.kind, GoKind::AutoSemi) || (self.kind == GoKind::Punct && self.text == ";")
    }
}

impl ScanToken for GoToken {
    fn text(&self) -> &str {
        self.text.as_str()
    }

    fn is_keyword(&self, kw: &str) -> bool {
        self.is_kw(kw)
    }

    fn line(&self) -> u32 {
        self.line
    }
}

pub(crate) struct ScannedGo {
    pub(crate) tokens: Vec<GoToken>,
    pub(crate) code_lines: BTreeSet<u32>,
    pub(crate) header_line_comments: Vec<String>,
}

const GO_KEYWORDS: [&str; 25] = [
    "break",
    "case",
    "chan",
    "const",
    "continue",
    "default",
    "defer",
    "else",
    "fallthrough",
    "for",
    "func",
    "go",
    "goto",
    "if",
    "import",
    "interface",
    "map",
    "package",
    "range",
    "return",
    "select",
    "struct",
    "switch",
    "type",
    "var",
];

fn triggers_auto_semi(tok: &GoToken) -> bool {
    match tok.kind {
        GoKind::Ident | GoKind::NumberLit | GoKind::RuneLit | GoKind::StringLit => true,
        GoKind::Keyword => matches!(
            tok.text.as_str(),
            "break" | "continue" | "fallthrough" | "return"
        ),
        GoKind::Punct => matches!(tok.text.as_str(), "++" | "--" | ")" | "]" | "}"),
        GoKind::AutoSemi => false,
    }
}

pub(crate) fn scan(text: &str) -> Result<ScannedGo, LowerError> {
    let source = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut lexer = GoLexer {
        source,
        bytes: source.as_bytes(),
        pos: 0,
        line: 1,
        at_line_start: true,
        delims: Vec::new(),
        tokens: Vec::new(),
        code_lines: BTreeSet::new(),
        header_line_comments: Vec::new(),
    };
    lexer.lex_all()?;
    Ok(ScannedGo {
        tokens: lexer.tokens,
        code_lines: lexer.code_lines,
        header_line_comments: lexer.header_line_comments,
    })
}

struct GoLexer<'a> {
    source: &'a str,
    bytes: &'a [u8],
    pos: usize,
    line: u32,
    at_line_start: bool,
    delims: Vec<(u8, u32)>,
    tokens: Vec<GoToken>,
    code_lines: BTreeSet<u32>,
    header_line_comments: Vec<String>,
}

impl GoLexer<'_> {
    fn lex_all(&mut self) -> Result<(), LowerError> {
        while self.pos < self.bytes.len() {
            self.step()?;
        }
        self.insert_auto_semi_if_needed();
        if let Some((delim, opened_line)) = self.delims.pop() {
            return Err(LowerError::Unparsable(format!(
                "unclosed {:?} opened at line {opened_line}",
                delim as char
            )));
        }
        Ok(())
    }

    fn step(&mut self) -> Result<(), LowerError> {
        let rest = &self.bytes[self.pos..];
        let b = rest[0];
        if b == b'\n' {
            self.insert_auto_semi_if_needed();
            self.line += 1;
            self.pos += 1;
            self.at_line_start = true;
            return Ok(());
        }
        if matches!(b, b' ' | b'\t' | b'\r') {
            if b != b'\r' {
                self.at_line_start = false;
            }
            self.pos += 1;
            return Ok(());
        }
        if rest.starts_with(b"//") {
            self.read_line_comment();
            return Ok(());
        }
        if rest.starts_with(b"/*") {
            return self.read_block_comment();
        }
        self.at_line_start = false;
        match b {
            b'"' => self.read_quoted_literal(b'"', GoKind::StringLit, "string", true),
            b'\'' => self.read_quoted_literal(b'\'', GoKind::RuneLit, "rune", false),
            b'`' => self.read_raw_string(),
            _ => self.read_word_number_or_punct(),
        }
    }

    fn read_word_number_or_punct(&mut self) -> Result<(), LowerError> {
        let ch = self.source[self.pos..].chars().next().unwrap_or('\0');
        if ch == '_' || ch.is_alphabetic() {
            self.read_word();
            return Ok(());
        }
        if ch.is_ascii_digit()
            || (ch == '.' && self.bytes.get(self.pos + 1).is_some_and(u8::is_ascii_digit))
        {
            self.read_number();
            return Ok(());
        }
        self.read_operator(ch)
    }

    fn insert_auto_semi_if_needed(&mut self) {
        if self.tokens.last().is_some_and(triggers_auto_semi) {
            self.tokens.push(GoToken {
                line: self.line,
                text: ";".to_owned(),
                kind: GoKind::AutoSemi,
            });
        }
    }

    fn read_line_comment(&mut self) {
        let start = self.pos;
        let from_col_zero = self.at_line_start;
        self.at_line_start = false;
        while self.pos < self.bytes.len() && self.bytes[self.pos] != b'\n' {
            self.pos += 1;
        }
        if from_col_zero && self.tokens.is_empty() {
            let trimmed = self.source[start..self.pos].trim_end_matches('\r');
            self.header_line_comments.push(trimmed.to_owned());
        }
    }

    fn read_block_comment(&mut self) -> Result<(), LowerError> {
        let opened = self.line;
        self.pos += 2;
        self.at_line_start = false;
        let mut emitted_semi = false;
        while self.pos < self.bytes.len() {
            if self.bytes[self.pos..].starts_with(b"*/") {
                self.pos += 2;
                return Ok(());
            }
            if self.bytes[self.pos] == b'\n' {
                if !emitted_semi {
                    self.insert_auto_semi_if_needed();
                    emitted_semi = true;
                }
                self.line += 1;
            }
            self.pos += 1;
        }
        Err(LowerError::Unparsable(format!(
            "unterminated block comment at line {opened}"
        )))
    }

    fn read_quoted_literal(
        &mut self,
        quote: u8,
        kind: GoKind,
        label: &str,
        allow_empty: bool,
    ) -> Result<(), LowerError> {
        let start = self.pos;
        let opened = self.line;
        self.pos += 1;
        let mut body_bytes = 0usize;
        while self.pos < self.bytes.len() {
            let b = self.bytes[self.pos];
            if b == b'\\' {
                if self.pos + 1 >= self.bytes.len() || self.bytes[self.pos + 1] == b'\n' {
                    break;
                }
                self.pos += 2;
                body_bytes += 2;
                continue;
            }
            if b == quote {
                if !allow_empty && body_bytes == 0 {
                    return Err(LowerError::Unparsable(format!(
                        "empty {label} literal at line {opened}"
                    )));
                }
                self.pos += 1;
                self.record(kind, &self.source[start..self.pos]);
                return Ok(());
            }
            if b == b'\n' {
                break;
            }
            self.pos += 1;
            body_bytes += 1;
        }
        Err(LowerError::Unparsable(format!(
            "unterminated {label} literal at line {opened}"
        )))
    }

    fn read_raw_string(&mut self) -> Result<(), LowerError> {
        let start = self.pos;
        let opened = self.line;
        self.pos += 1;
        while self.pos < self.bytes.len() {
            let b = self.bytes[self.pos];
            if b == b'`' {
                self.pos += 1;
                for l in opened..=self.line {
                    self.code_lines.insert(l);
                }
                self.tokens.push(GoToken {
                    line: opened,
                    text: self.source[start..self.pos].to_owned(),
                    kind: GoKind::StringLit,
                });
                return Ok(());
            }
            if b == b'\n' {
                self.line += 1;
            }
            self.pos += 1;
        }
        Err(LowerError::Unparsable(format!(
            "unterminated raw string literal at line {opened}"
        )))
    }

    fn read_word(&mut self) {
        let start = self.pos;
        for ch in self.source[self.pos..].chars() {
            if ch == '_' || ch.is_alphanumeric() {
                self.pos += ch.len_utf8();
            } else {
                break;
            }
        }
        let word = &self.source[start..self.pos];
        let kind = if GO_KEYWORDS.binary_search(&word).is_ok() {
            GoKind::Keyword
        } else {
            GoKind::Ident
        };
        self.record(kind, word);
    }

    fn read_number(&mut self) {
        let start = self.pos;
        if self.bytes[self.pos] == b'0' && self.pos + 1 < self.bytes.len() {
            match self.bytes[self.pos + 1] {
                b'x' | b'X' => {
                    self.pos += 2;
                    self.consume_radix_float(u8::is_ascii_hexdigit, b'p', b'P');
                }
                b'b' | b'B' => {
                    self.pos += 2;
                    self.consume_while(|b| matches!(b, b'0' | b'1' | b'_'));
                }
                b'o' | b'O' => {
                    self.pos += 2;
                    self.consume_while(|b| matches!(b, b'0'..=b'7' | b'_'));
                }
                _ => self.consume_radix_float(u8::is_ascii_digit, b'e', b'E'),
            }
        } else {
            self.consume_radix_float(u8::is_ascii_digit, b'e', b'E');
        }
        if self.bytes.get(self.pos) == Some(&b'i') {
            self.pos += 1;
        }
        self.record(GoKind::NumberLit, &self.source[start..self.pos]);
    }

    fn consume_radix_float(&mut self, is_digit: fn(&u8) -> bool, exp_lo: u8, exp_hi: u8) {
        self.consume_while(|b| is_digit(&b) || b == b'_');
        if self.bytes.get(self.pos) == Some(&b'.') {
            self.pos += 1;
            self.consume_while(|b| is_digit(&b) || b == b'_');
        }
        if self
            .bytes
            .get(self.pos)
            .is_some_and(|&b| b == exp_lo || b == exp_hi)
        {
            self.pos += 1;
            if self
                .bytes
                .get(self.pos)
                .is_some_and(|&b| b == b'+' || b == b'-')
            {
                self.pos += 1;
            }
            self.consume_while(|b| b.is_ascii_digit() || b == b'_');
        }
    }

    fn consume_while(&mut self, pred: impl Fn(u8) -> bool) {
        while self.pos < self.bytes.len() && pred(self.bytes[self.pos]) {
            self.pos += 1;
        }
    }

    fn read_operator(&mut self, ch: char) -> Result<(), LowerError> {
        let Some(op) = scan_go_operator(&self.bytes[self.pos..]) else {
            return Err(LowerError::Unparsable(format!(
                "unexpected character {ch:?} at line {}",
                self.line
            )));
        };
        track_delim(&mut self.delims, op, self.line)?;
        self.pos += op.len();
        self.record(GoKind::Punct, op);
        Ok(())
    }

    fn record(&mut self, kind: GoKind, text: &str) {
        self.code_lines.insert(self.line);
        self.tokens.push(GoToken {
            line: self.line,
            text: text.to_owned(),
            kind,
        });
    }
}

const GO_OPERATORS: [&str; 48] = [
    "...", "<<=", ">>=", "&^=", ":=", "<-", "&&", "||", "==", "!=", "<=", ">=", "<<", ">>", "&^",
    "++", "--", "+=", "-=", "*=", "/=", "%=", "&=", "|=", "^=", "+", "-", "*", "/", "%", "&", "|",
    "^", "<", ">", "=", "!", "~", "(", ")", "[", "]", "{", "}", ",", ".", ";", ":",
];

fn scan_go_operator(rest: &[u8]) -> Option<&'static str> {
    GO_OPERATORS
        .into_iter()
        .find(|op| rest.starts_with(op.as_bytes()))
}
