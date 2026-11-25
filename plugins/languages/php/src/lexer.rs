use std::collections::BTreeSet;

use codeint_model::language::LowerError;
use codeint_model::scan::{
    ScanToken, find_matching_delim as shared_matching_delim, track_delim as shared_track_delim,
};

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

impl ScanToken for RawToken {
    fn text(&self) -> &str {
        &self.text
    }

    fn is_keyword(&self, kw: &str) -> bool {
        self.kind == RawKind::Keyword && self.lower == kw
    }

    fn line(&self) -> u32 {
        self.line
    }
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
    reclassify_contextual_keywords(&mut lexer.tokens);
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
        if rest.len() >= 5
            && rest[..5].eq_ignore_ascii_case(b"<?php")
            && rest.get(5).is_none_or(|b| b.is_ascii_whitespace())
        {
            self.in_php = true;
            self.pos += 5;
            return;
        }
        if rest.starts_with(b"<?=") {
            self.in_php = true;
            self.pos += 3;
            return;
        }
        if rest.starts_with(b"<?") && !(rest.len() >= 5 && rest[..5].eq_ignore_ascii_case(b"<?xml"))
        {
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
        if matches!(rest[0], b'\'' | b'"' | b'`') {
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
            return self.lex_word();
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
        self.skip_quoted_body(quote, start_line)?;
        self.record_lines(start_line, self.line);
        self.tokens.push(RawToken {
            kind: RawKind::StringLit,
            text: self.text[start..self.pos].to_owned(),
            lower: String::new(),
            line: start_line,
        });
        Ok(())
    }

    fn skip_quoted_body(&mut self, quote: u8, start_line: u32) -> Result<(), LowerError> {
        let allows_interp = matches!(quote, b'"' | b'`');
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
            if allows_interp && self.starts_interpolation() {
                self.skip_interpolated_braces(start_line)?;
                continue;
            }
            if b == quote {
                self.pos += 1;
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

    fn starts_interpolation(&self) -> bool {
        matches!(
            (self.bytes.get(self.pos), self.bytes.get(self.pos + 1)),
            (Some(b'{'), Some(b'$')) | (Some(b'$'), Some(b'{'))
        )
    }

    fn skip_interpolated_braces(&mut self, start_line: u32) -> Result<(), LowerError> {
        self.pos += 2;
        let mut depth = 1usize;
        while self.pos < self.bytes.len() && depth > 0 {
            self.step_interpolated_token(&mut depth)?;
        }
        if depth > 0 {
            return Err(LowerError::Unparsable(format!(
                "unterminated string interpolation at line {start_line}"
            )));
        }
        Ok(())
    }

    fn step_interpolated_token(&mut self, depth: &mut usize) -> Result<(), LowerError> {
        let rest = &self.bytes[self.pos..];
        if self.try_skip_comment(rest)? {
            return Ok(());
        }
        let b = rest[0];
        if matches!(b, b'\'' | b'"' | b'`') {
            return self.skip_quoted_body(b, self.line);
        }
        match b {
            b'{' => *depth += 1,
            b'}' => *depth -= 1,
            b'\n' => self.line += 1,
            _ => {}
        }
        self.pos += 1;
        Ok(())
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

    fn lex_word(&mut self) -> Result<(), LowerError> {
        let slice = self.consume_while(is_ident_continue).to_owned();
        let lower = slice.to_ascii_lowercase();
        let kind = if is_keyword(&lower) {
            RawKind::Keyword
        } else {
            RawKind::Ident
        };
        self.push_simple_token(kind, slice, lower.clone());
        if lower == "__halt_compiler" && self.try_consume_halt_compiler()? {
            self.pos = self.bytes.len();
        }
        Ok(())
    }

    fn try_consume_halt_compiler(&mut self) -> Result<bool, LowerError> {
        if self.tokens.len() >= 2 {
            let prev = &self.tokens[self.tokens.len() - 2];
            if matches!(prev.text.as_str(), "->" | "?->" | "::")
                || (prev.kind == RawKind::Keyword
                    && matches!(prev.lower.as_str(), "function" | "const"))
            {
                return Ok(false);
            }
        }
        let saved_pos = self.pos;
        let saved_line = self.line;
        let mut matched = Vec::with_capacity(3);
        for expected in *b"();" {
            self.skip_ws_and_comments()?;
            if self.bytes.get(self.pos) != Some(&expected) {
                self.pos = saved_pos;
                self.line = saved_line;
                return Ok(false);
            }
            matched.push((expected as char, self.line));
            self.pos += 1;
        }
        for (ch, line) in matched {
            let text = ch.to_string();
            self.code_lines.insert(line);
            self.tokens.push(RawToken {
                kind: RawKind::Punct,
                text: text.clone(),
                lower: text,
                line,
            });
        }
        Ok(true)
    }

    fn skip_ws_and_comments(&mut self) -> Result<(), LowerError> {
        while self.pos < self.bytes.len() {
            let rest = &self.bytes[self.pos..];
            if rest[0].is_ascii_whitespace() {
                if rest[0] == b'\n' {
                    self.line += 1;
                }
                self.pos += 1;
                continue;
            }
            if self.try_skip_comment(rest)? {
                continue;
            }
            break;
        }
        Ok(())
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
        shared_track_delim(&mut self.delim_stack, punct, self.line)
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

fn reclassify_contextual_keywords(tokens: &mut [RawToken]) {
    for i in 0..tokens.len() {
        if tokens[i].kind == RawKind::Keyword && is_contextual_identifier(tokens, i) {
            tokens[i].kind = RawKind::Ident;
        }
    }
}

fn is_contextual_identifier(tokens: &[RawToken], i: usize) -> bool {
    let Some(prev) = i.checked_sub(1).and_then(|p| tokens.get(p)) else {
        return false;
    };
    if matches!(prev.text.as_str(), "->" | "?->" | "::") {
        return true;
    }
    let prev2 = i.checked_sub(2).and_then(|p| tokens.get(p));
    let next_text = tokens.get(i + 1).map(|t| t.text.as_str());
    is_function_name_position(prev, prev2)
        || is_named_arg_position(prev, next_text)
        || is_const_name_position(prev, prev2, next_text)
}

fn is_keyword_named(token: Option<&RawToken>, name: &str) -> bool {
    token.is_some_and(|t| t.kind == RawKind::Keyword && t.lower == name)
}

fn is_function_name_position(prev: &RawToken, prev2: Option<&RawToken>) -> bool {
    let after_fn = prev.kind == RawKind::Keyword && prev.lower == "function";
    let after_fn_ref = prev.text == "&" && is_keyword_named(prev2, "function");
    (after_fn && !is_keyword_named(prev2, "use")) || after_fn_ref
}

fn is_named_arg_position(prev: &RawToken, next_text: Option<&str>) -> bool {
    matches!(prev.text.as_str(), "(" | ",") && next_text == Some(":")
}

fn is_const_name_position(
    prev: &RawToken,
    prev2: Option<&RawToken>,
    next_text: Option<&str>,
) -> bool {
    prev.kind == RawKind::Keyword
        && prev.lower == "const"
        && !is_keyword_named(prev2, "use")
        && next_text == Some("=")
}

pub(crate) fn find_matching_delim(tokens: &[RawToken], open_idx: usize) -> Option<usize> {
    shared_matching_delim(tokens, open_idx)
}

pub(crate) fn closes_attribute(tokens: &[RawToken], close_idx: usize) -> bool {
    let mut depth = 0usize;
    for idx in (0..=close_idx).rev() {
        match tokens[idx].text.as_str() {
            "]" => depth += 1,
            "[" | "#[" => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return tokens[idx].text == "#[";
                }
            }
            _ => {}
        }
    }
    false
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
