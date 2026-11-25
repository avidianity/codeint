mod heredoc;
mod word;

pub(crate) use word::is_assignment_word;

use std::collections::BTreeSet;

use codeint_model::language::LowerError;

use crate::dialect::ShellDialect;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ShellTokenKind {
    Word,
    Operator,
    Newline,
    ArithCmd,
}

#[derive(Clone, Debug)]
pub(crate) struct ShellToken {
    pub(crate) kind: ShellTokenKind,
    pub(crate) text: String,
    pub(crate) line: u32,
    pub(crate) end_line: u32,
    pub(crate) has_quotes: bool,
    pub(crate) is_fully_single_quoted: bool,
    pub(crate) is_process_sub: bool,
    pub(crate) is_case_pattern_paren: bool,
    pub(crate) is_array_assignment: bool,
    pub(crate) has_backtick: bool,
    pub(crate) has_unquoted_var: bool,
    pub(crate) has_var_expansion: bool,
    pub(crate) unquoted_var_lines: Vec<u32>,
    pub(crate) backtick_lines: Vec<u32>,
    pub(crate) sub_tokens: Vec<Vec<ShellToken>>,
}

impl ShellToken {
    pub(crate) fn is_word(&self, expected: &str) -> bool {
        self.kind == ShellTokenKind::Word && !self.has_quotes && self.text == expected
    }

    pub(crate) fn is_op(&self, expected: &str) -> bool {
        self.kind == ShellTokenKind::Operator && self.text == expected
    }

    pub(crate) fn is_newline(&self) -> bool {
        self.kind == ShellTokenKind::Newline
    }

    pub(crate) fn next_is_cmd_pos(&self) -> bool {
        if self.is_newline() {
            return true;
        }
        if self.kind == ShellTokenKind::Operator {
            return (self.is_case_pattern_paren && self.text == ")")
                || matches!(
                    self.text.as_str(),
                    ";" | "&" | "&&" | "||" | "|" | "|&" | "(" | ";;" | ";&" | ";;&"
                );
        }
        self.kind == ShellTokenKind::Word && !self.has_quotes && is_cmd_prefix_keyword(&self.text)
    }

    pub(crate) fn simple(kind: ShellTokenKind, text: String, line: u32, end_line: u32) -> Self {
        Self {
            kind,
            text,
            line,
            end_line,
            has_quotes: false,
            is_fully_single_quoted: false,
            is_process_sub: false,
            is_case_pattern_paren: false,
            is_array_assignment: false,
            has_backtick: false,
            has_unquoted_var: false,
            has_var_expansion: false,
            unquoted_var_lines: Vec::new(),
            backtick_lines: Vec::new(),
            sub_tokens: Vec::new(),
        }
    }
}

pub(crate) struct LexedShell {
    pub(crate) tokens: Vec<ShellToken>,
    pub(crate) code_lines: BTreeSet<u32>,
    pub(crate) header_comments: Vec<String>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum CaseMode {
    WaitingIn,
    InPattern,
    InBody,
}

pub(crate) struct PendingHeredoc {
    pub(crate) delimiter: String,
    pub(crate) strip_tabs: bool,
    pub(crate) start_line: u32,
}

pub(crate) fn scan(source: &str, dialect: ShellDialect) -> Result<LexedShell, LowerError> {
    let mut scanner = Scanner::new(source, dialect);
    let tokens = scanner.scan_stream(false)?;
    scanner.finish_pending_heredocs()?;
    Ok(LexedShell {
        tokens,
        code_lines: scanner.code_lines,
        header_comments: scanner.header_comments,
    })
}

pub(crate) struct Scanner<'a> {
    pub(crate) src: &'a str,
    pub(crate) bytes: &'a [u8],
    pub(crate) pos: usize,
    pub(crate) line: u32,
    pub(crate) code_lines: BTreeSet<u32>,
    pub(crate) header_comments: Vec<String>,
    pub(crate) saw_code: bool,
    pub(crate) pending_heredocs: Vec<PendingHeredoc>,
    pub(crate) dialect: ShellDialect,
}

struct StreamState {
    tokens: Vec<ShellToken>,
    expect_heredoc: Option<bool>,
    paren_depth: usize,
    case_stack: Vec<CaseMode>,
    at_cmd_pos: bool,
    stop_on_close_paren: bool,
}

impl StreamState {
    fn new(stop_on_close_paren: bool) -> Self {
        Self {
            tokens: Vec::new(),
            expect_heredoc: None,
            paren_depth: 0,
            case_stack: Vec::new(),
            at_cmd_pos: true,
            stop_on_close_paren,
        }
    }

    fn in_case_pattern(&self) -> bool {
        self.case_stack.last() == Some(&CaseMode::InPattern)
    }

    fn record_word_state(&mut self, token: &ShellToken) {
        if self.update_case_keyword(token) {
            self.at_cmd_pos = false;
            return;
        }
        self.at_cmd_pos = token.kind == ShellTokenKind::Word
            && !token.has_quotes
            && is_cmd_prefix_keyword(&token.text);
    }

    fn update_case_keyword(&mut self, token: &ShellToken) -> bool {
        if let Some(mode) = self.case_stack.last_mut()
            && *mode == CaseMode::WaitingIn
            && token.is_word("in")
        {
            *mode = CaseMode::InPattern;
            return true;
        }
        if self.at_cmd_pos && token.is_word("case") {
            self.case_stack.push(CaseMode::WaitingIn);
            return true;
        }
        if self.at_cmd_pos && token.is_word("esac") && !self.case_stack.is_empty() {
            self.case_stack.pop();
            return true;
        }
        false
    }

    fn record_op_state(&mut self, op: &str, ends_case_pattern: bool) {
        if ends_case_pattern {
            self.at_cmd_pos = true;
            return;
        }
        match op {
            ";" | "&" | "&&" | "||" | "|" | "|&" | "(" => {
                self.at_cmd_pos = true;
            }
            ";;" | ";&" | ";;&" => {
                if let Some(mode) = self.case_stack.last_mut() {
                    *mode = CaseMode::InPattern;
                }
                self.at_cmd_pos = true;
            }
            _ => {
                self.at_cmd_pos = false;
            }
        }
    }
}

fn is_cmd_prefix_keyword(word: &str) -> bool {
    matches!(
        word,
        "if" | "then" | "elif" | "else" | "do" | "while" | "until" | "!" | "{" | "time" | "coproc"
    )
}

impl<'a> Scanner<'a> {
    fn new(src: &'a str, dialect: ShellDialect) -> Self {
        let pos = if src.starts_with('\u{feff}') { 3 } else { 0 };
        Self {
            src,
            bytes: src.as_bytes(),
            pos,
            line: 1,
            code_lines: BTreeSet::new(),
            header_comments: Vec::new(),
            saw_code: false,
            pending_heredocs: Vec::new(),
            dialect,
        }
    }

    pub(crate) fn peek(&self) -> Option<u8> {
        self.bytes.get(self.pos).copied()
    }

    pub(crate) fn peek_at(&self, offset: usize) -> Option<u8> {
        self.bytes.get(self.pos + offset).copied()
    }

    pub(crate) fn mark_code_line(&mut self, line: u32) {
        self.saw_code = true;
        self.code_lines.insert(line);
    }

    pub(crate) fn advance_newline(&mut self) {
        if self.peek() == Some(b'\r') && self.peek_at(1) == Some(b'\n') {
            self.pos += 2;
        } else {
            self.pos += 1;
        }
        self.line += 1;
    }

    pub(crate) fn consume_line_continuation(&mut self) -> bool {
        if self.peek() != Some(b'\\') {
            return false;
        }
        if self.peek_at(1) == Some(b'\n') {
            self.pos += 2;
            self.line += 1;
            return true;
        }
        if self.peek_at(1) == Some(b'\r') && self.peek_at(2) == Some(b'\n') {
            self.pos += 3;
            self.line += 1;
            return true;
        }
        false
    }

    pub(crate) fn advance_utf8_char(&mut self) {
        let step = self.src[self.pos..]
            .chars()
            .next()
            .map_or(1, char::len_utf8);
        self.pos += step;
    }

    pub(crate) fn scan_stream(
        &mut self,
        stop_on_close_paren: bool,
    ) -> Result<Vec<ShellToken>, LowerError> {
        let mut state = StreamState::new(stop_on_close_paren);
        while self.pos < self.bytes.len() {
            if self.skip_inline_whitespace() {
                continue;
            }
            if self.pos >= self.bytes.len() {
                break;
            }
            if !self.scan_next_item(&mut state)? {
                return Ok(state.tokens);
            }
        }
        if stop_on_close_paren {
            return Err(LowerError::Unparsable(format!(
                "unterminated command substitution before line {}",
                self.line
            )));
        }
        Ok(state.tokens)
    }

    fn skip_inline_whitespace(&mut self) -> bool {
        let start = self.pos;
        while let Some(ch) = self.peek() {
            if matches!(ch, b' ' | b'\t') || (ch == b'\r' && self.peek_at(1) != Some(b'\n')) {
                self.pos += 1;
            } else if !self.consume_line_continuation() {
                break;
            }
        }
        self.pos > start
    }

    fn scan_next_item(&mut self, state: &mut StreamState) -> Result<bool, LowerError> {
        if self.try_scan_trivia(state)? {
            return Ok(true);
        }
        let ch = self.bytes[self.pos];
        if ch == b')' && self.try_close_paren(state) {
            return Ok(false);
        }
        if !self.try_scan_special_paren(state)? {
            self.scan_operator_or_word(state, ch)?;
        }
        Ok(true)
    }

    fn try_scan_trivia(&mut self, state: &mut StreamState) -> Result<bool, LowerError> {
        let ch = self.bytes[self.pos];
        if ch == b'#' {
            self.scan_comment();
            return Ok(true);
        }
        if ch == b'\n' || (ch == b'\r' && self.peek_at(1) == Some(b'\n')) {
            self.handle_newline(state)?;
            return Ok(true);
        }
        Ok(false)
    }

    fn scan_operator_or_word(&mut self, state: &mut StreamState, ch: u8) -> Result<(), LowerError> {
        if is_operator_start(ch) {
            self.scan_operator_token(state);
            Ok(())
        } else {
            self.scan_word_token(state)
        }
    }

    fn scan_comment(&mut self) {
        let start = self.pos;
        while let Some(ch) = self.peek() {
            if ch == b'\n' || ch == b'\r' {
                break;
            }
            self.pos += 1;
        }
        if !self.saw_code {
            self.header_comments
                .push(self.src[start..self.pos].to_owned());
        }
    }

    fn handle_newline(&mut self, state: &mut StreamState) -> Result<(), LowerError> {
        let nl_line = self.line;
        self.advance_newline();
        state.tokens.push(ShellToken::simple(
            ShellTokenKind::Newline,
            "\n".to_owned(),
            nl_line,
            nl_line,
        ));
        state.expect_heredoc = None;
        state.at_cmd_pos = true;
        self.consume_pending_heredocs()
    }

    fn try_close_paren(&mut self, state: &mut StreamState) -> bool {
        if !state.stop_on_close_paren || state.paren_depth > 0 || state.in_case_pattern() {
            return false;
        }
        self.pos += 1;
        true
    }

    fn try_scan_special_paren(&mut self, state: &mut StreamState) -> Result<bool, LowerError> {
        let ch = self.bytes[self.pos];
        if matches!(ch, b'<' | b'>') && self.peek_at(1) == Some(b'(') {
            let token = self.scan_process_sub(ch)?;
            state.at_cmd_pos = false;
            state.tokens.push(token);
            return Ok(true);
        }
        if ch == b'('
            && self.peek_at(1) == Some(b'(')
            && (state.at_cmd_pos || state.tokens.last().is_some_and(|tok| tok.is_word("for")))
            && let Some(token) = self.try_scan_arith_cmd()?
        {
            state.at_cmd_pos = false;
            state.tokens.push(token);
            return Ok(true);
        }
        Ok(false)
    }

    fn scan_process_sub(&mut self, dir: u8) -> Result<ShellToken, LowerError> {
        let start = self.pos;
        let start_line = self.line;
        self.mark_code_line(start_line);
        self.pos += 2;
        let inner = self.scan_stream(true)?;
        let end_line = self.line;
        self.mark_code_line(end_line);
        let mut token = ShellToken::simple(
            ShellTokenKind::Word,
            self.src[start..self.pos].to_owned(),
            start_line,
            end_line,
        );
        token.is_process_sub = true;
        token.has_var_expansion = true;
        token.sub_tokens.push(inner);
        let _ = dir;
        Ok(token)
    }

    fn try_scan_arith_cmd(&mut self) -> Result<Option<ShellToken>, LowerError> {
        let saved_pos = self.pos;
        let saved_line = self.line;
        self.pos += 2;
        let mut depth = 0usize;
        while self.pos < self.bytes.len() {
            match self.step_arith_cmd(&mut depth)? {
                ArithStep::Continue => {}
                ArithStep::Closed => {
                    return Ok(Some(self.finish_arith_cmd(saved_pos, saved_line)));
                }
                ArithStep::Abort => break,
            }
        }
        self.pos = saved_pos;
        self.line = saved_line;
        Ok(None)
    }

    fn finish_arith_cmd(&mut self, start_pos: usize, start_line: u32) -> ShellToken {
        let end_line = self.line;
        for line in start_line..=end_line {
            self.mark_code_line(line);
        }
        ShellToken::simple(
            ShellTokenKind::ArithCmd,
            self.src[start_pos..self.pos].to_owned(),
            start_line,
            end_line,
        )
    }

    fn step_arith_cmd(&mut self, depth: &mut usize) -> Result<ArithStep, LowerError> {
        if self.try_skip_arith_nested()? {
            return Ok(ArithStep::Continue);
        }
        match self.bytes[self.pos] {
            b'(' => {
                *depth += 1;
                self.pos += 1;
            }
            b')' if *depth == 0 => return Ok(self.close_arith_outer_paren()),
            b')' => {
                *depth -= 1;
                self.pos += 1;
            }
            b'\n' => self.advance_newline(),
            _ => self.advance_utf8_char(),
        }
        Ok(ArithStep::Continue)
    }

    fn close_arith_outer_paren(&mut self) -> ArithStep {
        if self.peek_at(1) == Some(b')') {
            self.pos += 2;
            ArithStep::Closed
        } else {
            ArithStep::Abort
        }
    }

    fn try_skip_arith_nested(&mut self) -> Result<bool, LowerError> {
        match self.bytes[self.pos] {
            b'\\' => {
                if !self.consume_line_continuation() {
                    self.pos += 1;
                    if self.pos < self.bytes.len() {
                        self.advance_utf8_char();
                    }
                }
                Ok(true)
            }
            b'\'' => {
                word::skip_single_quote(self)?;
                Ok(true)
            }
            b'"' => {
                word::skip_double_quote_simple(self)?;
                Ok(true)
            }
            _ => Ok(false),
        }
    }

    fn scan_operator_token(&mut self, state: &mut StreamState) {
        let start_line = self.line;
        let op = self.consume_operator();
        self.mark_code_line(start_line);
        let is_case_pattern_paren = matches!(op, "(" | ")") && state.in_case_pattern();
        self.update_paren_and_case_on_op(state, op);
        state.expect_heredoc = match op {
            "<<" => Some(false),
            "<<-" => Some(true),
            _ => None,
        };
        state.record_op_state(op, is_case_pattern_paren && op == ")");
        let mut token = ShellToken::simple(
            ShellTokenKind::Operator,
            op.to_owned(),
            start_line,
            start_line,
        );
        token.is_case_pattern_paren = is_case_pattern_paren;
        state.tokens.push(token);
    }

    fn update_paren_and_case_on_op(&self, state: &mut StreamState, op: &str) {
        if op == "(" && !state.in_case_pattern() {
            state.paren_depth += 1;
        } else if op == ")" {
            if state.in_case_pattern() {
                if let Some(mode) = state.case_stack.last_mut() {
                    *mode = CaseMode::InBody;
                }
            } else {
                state.paren_depth = state.paren_depth.saturating_sub(1);
            }
        }
    }

    fn consume_operator(&mut self) -> &'static str {
        let rest = &self.bytes[self.pos..];
        let matched = match_multi_byte_op(rest).unwrap_or_else(|| match_one_byte_op(rest[0]));
        self.pos += matched.len();
        matched
    }

    fn scan_word_token(&mut self, state: &mut StreamState) -> Result<(), LowerError> {
        let token = word::scan_word(self)?;
        if let Some(strip_tabs) = state.expect_heredoc.take() {
            let delimiter = heredoc::unquote_delimiter(&token.text);
            self.pending_heredocs.push(PendingHeredoc {
                delimiter,
                strip_tabs,
                start_line: token.line,
            });
            state.at_cmd_pos = false;
        } else {
            state.record_word_state(&token);
        }
        state.tokens.push(token);
        Ok(())
    }

    fn finish_pending_heredocs(&mut self) -> Result<(), LowerError> {
        if let Some(pending) = self.pending_heredocs.first() {
            return Err(LowerError::Unparsable(format!(
                "unterminated heredoc {:?} started at line {}",
                pending.delimiter, pending.start_line
            )));
        }
        Ok(())
    }

    fn consume_pending_heredocs(&mut self) -> Result<(), LowerError> {
        let pending = std::mem::take(&mut self.pending_heredocs);
        for item in pending {
            heredoc::consume_body(self, &item)?;
        }
        Ok(())
    }
}

enum ArithStep {
    Continue,
    Closed,
    Abort,
}

const MULTI_BYTE_OPS: &[(&[u8], &str)] = &[
    (b"<<<", "<<<"),
    (b"<<-", "<<-"),
    (b";;&", ";;&"),
    (b"&>>", "&>>"),
    (b"&&", "&&"),
    (b"||", "||"),
    (b"|&", "|&"),
    (b";;", ";;"),
    (b";&", ";&"),
    (b"<<", "<<"),
    (b">>", ">>"),
    (b"<&", "<&"),
    (b">&", ">&"),
    (b"<>", "<>"),
    (b">|", ">|"),
    (b"&>", "&>"),
];

pub(crate) fn is_operator_start(ch: u8) -> bool {
    matches!(ch, b'|' | b'&' | b';' | b'(' | b')' | b'<' | b'>')
}

fn match_multi_byte_op(rest: &[u8]) -> Option<&'static str> {
    MULTI_BYTE_OPS
        .iter()
        .find_map(|&(prefix, label)| rest.starts_with(prefix).then_some(label))
}

fn match_one_byte_op(ch: u8) -> &'static str {
    match ch {
        b'|' => "|",
        b'&' => "&",
        b';' => ";",
        b'(' => "(",
        b')' => ")",
        b'<' => "<",
        _ => ">",
    }
}
