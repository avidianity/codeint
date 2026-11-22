use std::collections::BTreeSet;

use codeint_model::language::LowerError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum RawKind {
    Keyword,
    Ident,
    Variable,
    StringLit,
    NumberLit,
    Punct,
}

#[derive(Debug, Clone)]
pub(crate) struct RawToken {
    pub(crate) kind: RawKind,
    pub(crate) text: String,
    pub(crate) lower: String,
    pub(crate) line: u32,
}

pub(crate) struct LexedFile {
    pub(crate) tokens: Vec<RawToken>,
    pub(crate) header_comments: String,
    pub(crate) code_lines: BTreeSet<u32>,
}

const PHP_KEYWORDS: [&str; 62] = [
    "abstract",
    "and",
    "array",
    "as",
    "break",
    "callable",
    "case",
    "catch",
    "class",
    "clone",
    "const",
    "continue",
    "declare",
    "default",
    "do",
    "echo",
    "else",
    "elseif",
    "empty",
    "enddeclare",
    "endfor",
    "endforeach",
    "endif",
    "endswitch",
    "endwhile",
    "enum",
    "eval",
    "exit",
    "extends",
    "false",
    "final",
    "finally",
    "fn",
    "for",
    "foreach",
    "function",
    "global",
    "goto",
    "if",
    "implements",
    "include",
    "include_once",
    "instanceof",
    "insteadof",
    "interface",
    "isset",
    "list",
    "match",
    "namespace",
    "new",
    "null",
    "or",
    "print",
    "private",
    "protected",
    "public",
    "readonly",
    "require",
    "require_once",
    "return",
    "static",
    "switch",
];

const MORE_PHP_KEYWORDS: [&str; 12] = [
    "throw", "trait", "true", "try", "unset", "use", "var", "while", "xor", "yield", "parent",
    "self",
];

fn is_keyword(lower: &str) -> bool {
    PHP_KEYWORDS.contains(&lower) || MORE_PHP_KEYWORDS.contains(&lower)
}

struct Lexer<'a> {
    text: &'a str,
    bytes: &'a [u8],
    pos: usize,
    line: u32,
    in_php: bool,
    tokens: Vec<RawToken>,
    header_comments: String,
    code_lines: BTreeSet<u32>,
    delim_stack: Vec<(u8, u32)>,
}

pub(crate) fn lex(text: &str) -> Result<LexedFile, LowerError> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut lexer = Lexer {
        text,
        bytes: text.as_bytes(),
        pos: 0,
        line: 1,
        in_php: false,
        tokens: Vec::new(),
        header_comments: String::new(),
        code_lines: BTreeSet::new(),
        delim_stack: Vec::new(),
    };
    lexer.run()?;
    Ok(LexedFile {
        tokens: lexer.tokens,
        header_comments: lexer.header_comments,
        code_lines: lexer.code_lines,
    })
}

impl<'a> Lexer<'a> {
    fn run(&mut self) -> Result<(), LowerError> {
        while self.pos < self.bytes.len() {
            if !self.in_php {
                self.step_html();
            } else {
                self.step_php()?;
            }
        }
        if let Some((open, open_line)) = self.delim_stack.last() {
            return Err(LowerError::Unparsable(format!(
                "unclosed {:?} opened at line {open_line}",
                *open as char
            )));
        }
        Ok(())
    }

    fn step_html(&mut self) {
        let rest = &self.bytes[self.pos..];
        if rest.starts_with(b"<?php") && rest.get(5).is_none_or(|b| b.is_ascii_whitespace()) {
            self.in_php = true;
            self.pos += 5;
            return;
        }
        if rest.starts_with(b"<?=") {
            self.in_php = true;
            self.pos += 3;
            return;
        }
        if rest.starts_with(b"<?") && !rest.starts_with(b"<?xml") {
            self.in_php = true;
            self.pos += 2;
            return;
        }
        if self.bytes[self.pos] == b'\n' {
            self.line += 1;
        }
        self.pos += 1;
    }

    fn step_php(&mut self) -> Result<(), LowerError> {
        let rest = &self.bytes[self.pos..];
        if rest[0].is_ascii_whitespace() {
            if rest[0] == b'\n' {
                self.line += 1;
            }
            self.pos += 1;
            return Ok(());
        }
        if rest.starts_with(b"?>") {
            self.in_php = false;
            self.pos += 2;
            return Ok(());
        }
        if self.try_skip_comment(rest)? {
            return Ok(());
        }
        if rest.starts_with(b"<<<") {
            return self.lex_heredoc();
        }
        if matches!(rest[0], b'\'' | b'"') {
            return self.lex_string();
        }
        self.step_php_atom()
    }

    fn try_skip_comment(&mut self, rest: &[u8]) -> Result<bool, LowerError> {
        if rest.starts_with(b"//") || (rest[0] == b'#' && !rest.starts_with(b"#[")) {
            self.skip_line_comment();
            return Ok(true);
        }
        if rest.starts_with(b"/*") {
            self.skip_block_comment()?;
            return Ok(true);
        }
        Ok(false)
    }

    fn step_php_atom(&mut self) -> Result<(), LowerError> {
        let b = self.bytes[self.pos];
        if b == b'$'
            && self
                .bytes
                .get(self.pos + 1)
                .is_some_and(|n| is_ident_start(*n))
        {
            self.lex_variable();
            return Ok(());
        }
        if is_ident_start(b) {
            self.lex_word();
            return Ok(());
        }
        if b.is_ascii_digit() {
            self.lex_number();
            return Ok(());
        }
        self.lex_punct()
    }

    fn skip_line_comment(&mut self) {
        let start = self.pos;
        while self.pos < self.bytes.len()
            && self.bytes[self.pos] != b'\n'
            && !self.bytes[self.pos..].starts_with(b"?>")
        {
            self.pos += 1;
        }
        if self.tokens.is_empty() {
            self.header_comments.push_str(&self.text[start..self.pos]);
            self.header_comments.push('\n');
        }
    }

    fn skip_block_comment(&mut self) -> Result<(), LowerError> {
        let start = self.pos;
        let start_line = self.line;
        self.pos += 2;
        while self.pos + 1 < self.bytes.len() {
            if self.bytes[self.pos..].starts_with(b"*/") {
                self.pos += 2;
                if self.tokens.is_empty() {
                    self.header_comments.push_str(&self.text[start..self.pos]);
                    self.header_comments.push('\n');
                }
                return Ok(());
            }
            if self.bytes[self.pos] == b'\n' {
                self.line += 1;
            }
            self.pos += 1;
        }
        Err(LowerError::Unparsable(format!(
            "unterminated block comment at line {start_line}"
        )))
    }

    fn lex_heredoc(&mut self) -> Result<(), LowerError> {
        let start = self.pos;
        let start_line = self.line;
        let label = self.parse_heredoc_header()?;
        self.scan_heredoc_body(start, start_line, label)
    }

    fn parse_heredoc_header(&mut self) -> Result<&'a str, LowerError> {
        self.pos += 3;
        while self.pos < self.bytes.len() && matches!(self.bytes[self.pos], b' ' | b'\t') {
            self.pos += 1;
        }
        let label = self.read_heredoc_label()?;
        while self.pos < self.bytes.len() && self.bytes[self.pos] != b'\n' {
            self.pos += 1;
        }
        if self.pos >= self.bytes.len() {
            return Err(LowerError::Unparsable(format!(
                "unterminated heredoc {label:?} at line {}",
                self.line
            )));
        }
        self.pos += 1;
        self.line += 1;
        Ok(label)
    }

    fn read_heredoc_label(&mut self) -> Result<&'a str, LowerError> {
        let quote = self
            .bytes
            .get(self.pos)
            .copied()
            .filter(|b| matches!(b, b'\'' | b'"'));
        if quote.is_some() {
            self.pos += 1;
        }
        let label_start = self.pos;
        while self.pos < self.bytes.len() && is_ident_continue(self.bytes[self.pos]) {
            self.pos += 1;
        }
        let label = &self.text[label_start..self.pos];
        if label.is_empty() {
            return Err(LowerError::Unparsable(format!(
                "missing heredoc label at line {}",
                self.line
            )));
        }
        if let Some(q) = quote {
            if self.bytes.get(self.pos) != Some(&q) {
                return Err(LowerError::Unparsable(format!(
                    "unclosed heredoc label quote at line {}",
                    self.line
                )));
            }
            self.pos += 1;
        }
        Ok(label)
    }

    fn scan_heredoc_body(
        &mut self,
        start: usize,
        start_line: u32,
        label: &str,
    ) -> Result<(), LowerError> {
        while self.pos < self.bytes.len() {
            let line_start = self.pos;
            while self.pos < self.bytes.len() && self.bytes[self.pos] != b'\n' {
                self.pos += 1;
            }
            let trimmed = self.text[line_start..self.pos].trim_start_matches([' ', '\t']);
            if let Some(after) = trimmed.strip_prefix(label)
                && after.bytes().next().is_none_or(|b| !is_ident_continue(b))
            {
                let indent = trimmed.as_ptr() as usize - self.text[line_start..].as_ptr() as usize;
                self.pos = line_start + indent + label.len();
                self.record_lines(start_line, self.line);
                self.tokens.push(RawToken {
                    kind: RawKind::StringLit,
                    text: self.text[start..self.pos].to_owned(),
                    lower: String::new(),
                    line: start_line,
                });
                return Ok(());
            }
            if self.pos < self.bytes.len() {
                self.pos += 1;
                self.line += 1;
            }
        }
        Err(LowerError::Unparsable(format!(
            "unterminated heredoc {label:?} at line {}",
            self.line
        )))
    }

    fn lex_string(&mut self) -> Result<(), LowerError> {
        let quote = self.bytes[self.pos];
        let start = self.pos;
        let start_line = self.line;
        self.pos += 1;
        while self.pos < self.bytes.len() {
            let b = self.bytes[self.pos];
            if b == b'\\' && self.pos + 1 < self.bytes.len() {
                if self.bytes[self.pos + 1] == b'\n' {
                    self.line += 1;
                }
                self.pos += 2;
                continue;
            }
            if b == quote {
                self.pos += 1;
                self.record_lines(start_line, self.line);
                self.tokens.push(RawToken {
                    kind: RawKind::StringLit,
                    text: self.text[start..self.pos].to_owned(),
                    lower: String::new(),
                    line: start_line,
                });
                return Ok(());
            }
            if b == b'\n' {
                self.line += 1;
            }
            self.pos += 1;
        }
        Err(LowerError::Unparsable(format!(
            "unterminated string literal at line {start_line}"
        )))
    }

    fn push_simple_token(&mut self, kind: RawKind, slice: String, lower: String) {
        self.code_lines.insert(self.line);
        self.tokens.push(RawToken {
            kind,
            text: slice,
            lower,
            line: self.line,
        });
    }

    fn consume_while(&mut self, pred: impl Fn(u8) -> bool) -> &str {
        let start = self.pos;
        while self.pos < self.bytes.len() && pred(self.bytes[self.pos]) {
            self.pos += 1;
        }
        &self.text[start..self.pos]
    }

    fn lex_variable(&mut self) {
        let start = self.pos;
        self.pos += 2;
        self.consume_while(is_ident_continue);
        let slice = self.text[start..self.pos].to_owned();
        self.push_simple_token(RawKind::Variable, slice.clone(), slice);
    }

    fn lex_word(&mut self) {
        let slice = self.consume_while(is_ident_continue).to_owned();
        let lower = slice.to_ascii_lowercase();
        let kind = if is_keyword(&lower) {
            RawKind::Keyword
        } else {
            RawKind::Ident
        };
        self.push_simple_token(kind, slice, lower);
    }

    fn lex_number(&mut self) {
        let slice = self
            .consume_while(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'.'))
            .to_owned();
        self.push_simple_token(RawKind::NumberLit, slice.clone(), slice);
    }

    fn lex_punct(&mut self) -> Result<(), LowerError> {
        let punct = match_multi_punct(&self.bytes[self.pos..])
            .unwrap_or(&self.text[self.pos..self.pos + 1]);
        self.track_delim(punct)?;
        self.push_simple_token(RawKind::Punct, punct.to_owned(), punct.to_owned());
        self.pos += punct.len();
        Ok(())
    }

    fn track_delim(&mut self, punct: &str) -> Result<(), LowerError> {
        match punct {
            "#[" | "[" => self.delim_stack.push((b'[', self.line)),
            "{" => self.delim_stack.push((b'{', self.line)),
            "(" => self.delim_stack.push((b'(', self.line)),
            "}" | ")" | "]" => {
                let expected = match punct {
                    "}" => b'{',
                    ")" => b'(',
                    _ => b'[',
                };
                if !matches!(self.delim_stack.pop(), Some((open, _)) if open == expected) {
                    return Err(LowerError::Unparsable(format!(
                        "unexpected {punct:?} at line {}",
                        self.line
                    )));
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn record_lines(&mut self, from: u32, to: u32) {
        for line in from..=to {
            self.code_lines.insert(line);
        }
    }
}

fn is_ident_start(b: u8) -> bool {
    b.is_ascii_alphabetic() || b == b'_' || b >= 0x80
}

fn is_ident_continue(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b >= 0x80
}

fn match_multi_punct(bytes: &[u8]) -> Option<&'static str> {
    const OPS: [&str; 23] = [
        "===", "!==", "<=>", "...", "??=", "**=", "&&", "||", "??", "=>", "->", "?->", "::", "==",
        "!=", "<=", ">=", "++", "--", "+=", "-=", "*=", "#[",
    ];
    OPS.into_iter()
        .find(|candidate| bytes.starts_with(candidate.as_bytes()))
}

fn delim_pair(open: &str) -> Option<(&'static str, &'static str)> {
    match open {
        "(" => Some(("(", ")")),
        "{" => Some(("{", "}")),
        "[" | "#[" => Some(("[", "]")),
        _ => None,
    }
}

pub(crate) fn find_matching_delim(tokens: &[RawToken], open_idx: usize) -> Option<usize> {
    let (inc, dec) = delim_pair(tokens.get(open_idx)?.text.as_str())?;
    let mut depth = 0usize;
    for (idx, token) in tokens.iter().enumerate().skip(open_idx) {
        if token.text == inc || (inc == "[" && token.text == "#[") {
            depth += 1;
        } else if token.text == dec {
            depth -= 1;
            if depth == 0 {
                return Some(idx);
            }
        }
    }
    None
}

pub(crate) fn find_next_brace(tokens: &[RawToken], mut pos: usize) -> Option<usize> {
    while pos < tokens.len() {
        match tokens[pos].text.as_str() {
            "{" => return Some(pos),
            ";" => return None,
            "(" => pos = find_matching_delim(tokens, pos)? + 1,
            _ => pos += 1,
        }
    }
    None
}
