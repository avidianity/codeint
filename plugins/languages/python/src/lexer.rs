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
    pending_line_end: u32,
}

#[derive(Clone, Copy)]
struct StringPrefix {
    is_raw: bool,
    is_interpolated: bool,
}

#[derive(Clone, Copy)]
struct InterpContext {
    open_line: u32,
    quote: u8,
    triple: bool,
    is_raw: bool,
}

enum InterpStep {
    Continue,
    Done,
    OpenField,
}

enum FieldStep {
    Continue,
    Closed,
    FormatSpec,
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
            pending_line_end: 0,
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
            return self.consume_string(
                self.pos,
                StringPrefix {
                    is_raw: false,
                    is_interpolated: false,
                },
            );
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
        self.skip_inline_comment();
        if self.tokens.is_empty() {
            self.header_comments
                .push(self.src[start..self.pos].to_owned());
        }
    }

    fn skip_inline_comment(&mut self) {
        while let Some(&b) = self.bytes.get(self.pos) {
            if b == b'\n' {
                break;
            }
            self.pos += 1;
        }
    }

    fn consume_ident_or_prefixed_string(&mut self) -> Result<(), LowerError> {
        if let Some((quote_pos, prefix)) = string_prefix_info(self.bytes, self.pos) {
            return self.consume_string(quote_pos, prefix);
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

    fn consume_string(&mut self, quote_pos: usize, prefix: StringPrefix) -> Result<(), LowerError> {
        let start = self.pos;
        let start_line = self.line;
        let quote = self.bytes[quote_pos];
        let is_triple = self.has_triple_quote_at(quote_pos, quote);
        self.pos = quote_pos + if is_triple { 3 } else { 1 };
        if prefix.is_interpolated {
            let ctx = InterpContext {
                open_line: start_line,
                quote,
                triple: is_triple,
                is_raw: prefix.is_raw,
            };
            let result = self.scan_interpolated_string(start, start_line, ctx);
            self.at_physical_line_start = false;
            return result;
        }
        if !self.scan_string_body(quote, is_triple) {
            return Err(unterminated_string(start_line));
        }
        self.push_string_segment(start, start_line);
        Ok(())
    }

    fn push_string_segment(&mut self, start: usize, start_line: u32) {
        for line in start_line..=self.line {
            self.code_lines.insert(line);
        }
        let text = self.src[start..self.pos].to_owned();
        self.push_token_span(TokenKind::Literal, false, text, start_line, self.line);
    }

    fn scan_string_body(&mut self, quote: u8, triple: bool) -> bool {
        while self.pos < self.bytes.len() {
            let b = self.bytes[self.pos];
            if b == b'\\' {
                self.advance_string_escape();
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

    fn scan_interpolated_string(
        &mut self,
        mut seg_start: usize,
        mut seg_start_line: u32,
        ctx: InterpContext,
    ) -> Result<(), LowerError> {
        while self.pos < self.bytes.len() {
            match self.step_interp_literal(ctx)? {
                InterpStep::Continue => {}
                InterpStep::Done => {
                    self.push_string_segment(seg_start, seg_start_line);
                    return Ok(());
                }
                InterpStep::OpenField => {
                    self.push_string_segment(seg_start, seg_start_line);
                    self.scan_interp_field(ctx)?;
                    seg_start = self.pos;
                    seg_start_line = self.line;
                }
            }
        }
        Err(unterminated_string(ctx.open_line))
    }

    fn step_interp_literal(&mut self, ctx: InterpContext) -> Result<InterpStep, LowerError> {
        let b = self.bytes[self.pos];
        if b == b'\\' {
            self.advance_interp_escape(ctx.is_raw);
            return Ok(InterpStep::Continue);
        }
        if b == b'\n' {
            self.consume_interp_newline(ctx)?;
            return Ok(InterpStep::Continue);
        }
        if b == ctx.quote && self.try_close_quote(ctx.quote, ctx.triple) {
            return Ok(InterpStep::Done);
        }
        if b == b'{' {
            return Ok(self.step_interp_open_brace());
        }
        if b == b'}' && self.bytes.get(self.pos + 1) == Some(&b'}') {
            self.pos += 2;
            return Ok(InterpStep::Continue);
        }
        self.pos += 1;
        Ok(InterpStep::Continue)
    }

    fn step_interp_open_brace(&mut self) -> InterpStep {
        if self.bytes.get(self.pos + 1) == Some(&b'{') {
            self.pos += 2;
            return InterpStep::Continue;
        }
        InterpStep::OpenField
    }

    fn consume_interp_newline(&mut self, ctx: InterpContext) -> Result<(), LowerError> {
        if !ctx.triple {
            return Err(unterminated_string(ctx.open_line));
        }
        self.code_lines.insert(self.line);
        self.line += 1;
        self.pos += 1;
        Ok(())
    }

    fn scan_interp_field(&mut self, ctx: InterpContext) -> Result<(), LowerError> {
        let field_line = self.line;
        self.pos += 1;
        self.delims.push((b'{', field_line));
        self.push_token(TokenKind::Other, false, "{".to_owned(), field_line);
        let base_depth = self.delims.len();
        while self.pos < self.bytes.len() {
            match self.step_interp_field(base_depth)? {
                FieldStep::Continue => {}
                FieldStep::Closed => return Ok(()),
                FieldStep::FormatSpec => return self.scan_format_spec(ctx),
            }
        }
        Err(unterminated_string(ctx.open_line))
    }

    fn step_interp_field(&mut self, base_depth: usize) -> Result<FieldStep, LowerError> {
        let b = self.bytes[self.pos];
        if self.skip_interp_trivia(b)? {
            return Ok(FieldStep::Continue);
        }
        if self.delims.len() == base_depth
            && let Some(step) = self.step_interp_field_boundary(b)
        {
            return Ok(step);
        }
        self.step_code_token(b)?;
        Ok(FieldStep::Continue)
    }

    fn step_interp_field_boundary(&mut self, b: u8) -> Option<FieldStep> {
        let next = self.bytes.get(self.pos + 1).copied();
        if b == b'}' {
            self.close_interp_field();
            return Some(FieldStep::Closed);
        }
        if b == b':' && next != Some(b'=') {
            self.pos += 1;
            return Some(FieldStep::FormatSpec);
        }
        if b == b'!' && next != Some(b'=') {
            self.skip_conversion_spec();
            return Some(FieldStep::Continue);
        }
        None
    }

    fn skip_interp_trivia(&mut self, b: u8) -> Result<bool, LowerError> {
        if matches!(b, b' ' | b'\t' | b'\r' | b'\x0c') {
            self.pos += 1;
            return Ok(true);
        }
        if b == b'\n' {
            self.pos += 1;
            self.line += 1;
            return Ok(true);
        }
        if b == b'\\' {
            self.consume_line_continuation()?;
            self.at_physical_line_start = false;
            return Ok(true);
        }
        if b == b'#' {
            self.skip_inline_comment();
            return Ok(true);
        }
        Ok(false)
    }

    fn skip_conversion_spec(&mut self) {
        self.pos += 1;
        if self
            .bytes
            .get(self.pos)
            .is_some_and(u8::is_ascii_alphabetic)
        {
            self.pos += 1;
        }
    }

    fn close_interp_field(&mut self) {
        self.pos += 1;
        self.delims.pop();
        self.push_token(TokenKind::Other, false, "}".to_owned(), self.line);
    }

    fn scan_format_spec(&mut self, ctx: InterpContext) -> Result<(), LowerError> {
        while self.pos < self.bytes.len() {
            if self.step_format_spec(ctx)? {
                return Ok(());
            }
        }
        Err(unterminated_string(ctx.open_line))
    }

    fn step_format_spec(&mut self, ctx: InterpContext) -> Result<bool, LowerError> {
        let b = self.bytes[self.pos];
        if b == b'\\' {
            self.advance_interp_escape(ctx.is_raw);
            return Ok(false);
        }
        if b == b'\n' {
            self.consume_interp_newline(ctx)?;
            return Ok(false);
        }
        if b == b'{' {
            self.scan_interp_field(ctx)?;
            return Ok(false);
        }
        if b == b'}' {
            self.close_interp_field();
            return Ok(true);
        }
        if b == ctx.quote && (!ctx.triple || self.has_triple_quote_at(self.pos, ctx.quote)) {
            return Err(unterminated_string(ctx.open_line));
        }
        self.pos += 1;
        Ok(false)
    }

    fn advance_interp_escape(&mut self, is_raw: bool) {
        if !is_raw
            && self.bytes.get(self.pos + 1) == Some(&b'N')
            && self.bytes.get(self.pos + 2) == Some(&b'{')
        {
            self.pos += 3;
            while let Some(&ch) = self.bytes.get(self.pos) {
                if ch == b'\n' {
                    break;
                }
                self.pos += 1;
                if ch == b'}' {
                    break;
                }
            }
            return;
        }
        if matches!(self.bytes.get(self.pos + 1), Some(b'{' | b'}')) {
            self.pos += 1;
            return;
        }
        self.advance_string_escape();
    }

    fn advance_string_escape(&mut self) {
        if self.bytes.get(self.pos + 1) == Some(&b'\r')
            && self.bytes.get(self.pos + 2) == Some(&b'\n')
        {
            self.line += 1;
            self.pos += 3;
            return;
        }
        if self.bytes.get(self.pos + 1) == Some(&b'\n') {
            self.line += 1;
        }
        self.pos = (self.pos + 2).min(self.bytes.len());
    }

    fn has_triple_quote_at(&self, pos: usize, quote: u8) -> bool {
        self.bytes.get(pos + 1) == Some(&quote) && self.bytes.get(pos + 2) == Some(&quote)
    }

    fn try_close_quote(&mut self, quote: u8, triple: bool) -> bool {
        if !triple {
            self.pos += 1;
            return true;
        }
        if self.has_triple_quote_at(self.pos, quote) {
            self.pos += 3;
            return true;
        }
        false
    }

    fn consume_number(&mut self) {
        let start = self.pos;
        let line = self.line;
        let has_radix_prefix = self.bytes.get(start) == Some(&b'0')
            && matches!(
                self.bytes.get(start + 1),
                Some(b'x' | b'X' | b'b' | b'B' | b'o' | b'O')
            );
        while let Some(&b) = self.bytes.get(self.pos) {
            let is_exp_sign = !has_radix_prefix
                && matches!(b, b'+' | b'-')
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
        self.push_token_span(token_kind, is_kw, text, line, line);
    }

    fn push_token_span(
        &mut self,
        token_kind: TokenKind,
        is_kw: bool,
        text: String,
        start_line: u32,
        end_line: u32,
    ) {
        if self.pending_line_start.is_none() {
            self.pending_line_start =
                Some((self.current_line_indent, start_line, self.tokens.len()));
            self.pending_line_end = end_line;
        } else {
            self.pending_line_end = self.pending_line_end.max(end_line);
        }
        self.code_lines.insert(start_line);
        self.tokens.push(PyToken {
            line: start_line,
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
            let end_line = self.pending_line_end.max(start_line);
            self.logical_lines.push(LogicalLine {
                indent,
                start_line,
                end_line,
                tokens: start_idx..self.tokens.len(),
            });
        }
        self.pending_line_end = 0;
    }
}

fn unterminated_string(start_line: u32) -> LowerError {
    LowerError::Unparsable(format!("unterminated string at line {start_line}"))
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

fn string_prefix_info(bytes: &[u8], pos: usize) -> Option<(usize, StringPrefix)> {
    let b0 = bytes.get(pos)?.to_ascii_lowercase();
    let b1 = *bytes.get(pos + 1)?;
    if b1 == b'\'' || b1 == b'"' {
        return single_char_prefix(b0).map(|prefix| (pos + 1, prefix));
    }
    if matches!(bytes.get(pos + 2), Some(b'\'' | b'"')) {
        return two_char_prefix(b0, b1.to_ascii_lowercase()).map(|prefix| (pos + 2, prefix));
    }
    None
}

fn single_char_prefix(b: u8) -> Option<StringPrefix> {
    match b {
        b'r' => Some(StringPrefix {
            is_raw: true,
            is_interpolated: false,
        }),
        b'u' | b'b' => Some(StringPrefix {
            is_raw: false,
            is_interpolated: false,
        }),
        b'f' | b't' => Some(StringPrefix {
            is_raw: false,
            is_interpolated: true,
        }),
        _ => None,
    }
}

fn two_char_prefix(first: u8, second: u8) -> Option<StringPrefix> {
    match (first, second) {
        (b'r', b'b') | (b'b', b'r') => Some(StringPrefix {
            is_raw: true,
            is_interpolated: false,
        }),
        (b'r', b'f') | (b'f', b'r') | (b'r', b't') | (b't', b'r') => Some(StringPrefix {
            is_raw: true,
            is_interpolated: true,
        }),
        _ => None,
    }
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
