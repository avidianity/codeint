use codeint_model::language::LowerError;

use super::{Scanner, ShellToken, ShellTokenKind, is_operator_start};
use crate::dialect::ShellDialect;

struct WordAcc {
    start_pos: usize,
    start_line: u32,
    has_quotes: bool,
    single_quote_segments: usize,
    other_segments: usize,
    is_array_assignment: bool,
    has_backtick: bool,
    has_unquoted_var: bool,
    has_var_expansion: bool,
    has_param_expansion: bool,
    unquoted_var_lines: Vec<u32>,
    zsh_split_lines: Vec<u32>,
    backtick_lines: Vec<u32>,
    sub_tokens: Vec<Vec<ShellToken>>,
}

impl WordAcc {
    fn new(start_pos: usize, start_line: u32) -> Self {
        Self {
            start_pos,
            start_line,
            has_quotes: false,
            single_quote_segments: 0,
            other_segments: 0,
            is_array_assignment: false,
            has_backtick: false,
            has_unquoted_var: false,
            has_var_expansion: false,
            has_param_expansion: false,
            unquoted_var_lines: Vec::new(),
            zsh_split_lines: Vec::new(),
            backtick_lines: Vec::new(),
            sub_tokens: Vec::new(),
        }
    }

    fn record_unquoted_var(&mut self, line: u32) {
        self.has_unquoted_var = true;
        self.has_var_expansion = true;
        if self.unquoted_var_lines.last() != Some(&line) {
            self.unquoted_var_lines.push(line);
        }
    }

    fn record_zsh_split(&mut self, line: u32) {
        if self.zsh_split_lines.last() != Some(&line) {
            self.zsh_split_lines.push(line);
        }
    }

    fn record_backtick(&mut self, line: u32) {
        self.has_backtick = true;
        self.has_var_expansion = true;
        if self.backtick_lines.last() != Some(&line) {
            self.backtick_lines.push(line);
        }
    }

    fn finish(self, text: String, end_line: u32) -> ShellToken {
        let is_fully_single_quoted = self.single_quote_segments == 1 && self.other_segments == 0;
        ShellToken {
            kind: ShellTokenKind::Word,
            text,
            line: self.start_line,
            end_line,
            has_quotes: self.has_quotes,
            is_fully_single_quoted,
            is_process_sub: false,
            is_case_pattern_paren: false,
            is_array_assignment: self.is_array_assignment,
            has_backtick: self.has_backtick,
            has_unquoted_var: self.has_unquoted_var,
            has_var_expansion: self.has_var_expansion,
            has_param_expansion: self.has_param_expansion,
            is_heredoc_delimiter: false,
            unquoted_var_lines: self.unquoted_var_lines,
            zsh_split_lines: self.zsh_split_lines,
            backtick_lines: self.backtick_lines,
            sub_tokens: self.sub_tokens,
        }
    }
}

pub(super) fn scan_word(scanner: &mut Scanner<'_>) -> Result<ShellToken, LowerError> {
    let mut acc = WordAcc::new(scanner.pos, scanner.line);
    scanner.mark_code_line(scanner.line);
    while let Some(ch) = scanner.peek() {
        if ch == b'(' && is_assignment_prefix(&scanner.src[acc.start_pos..scanner.pos]) {
            scan_array_literal(scanner, &mut acc)?;
            continue;
        }
        if is_word_delimiter(ch) {
            break;
        }
        scan_word_piece(scanner, ch, &mut acc)?;
    }
    let end_line = scanner.line;
    let text = scanner.src[acc.start_pos..scanner.pos].to_owned();
    Ok(acc.finish(text, end_line))
}

pub(crate) fn is_assignment_word(text: &str) -> bool {
    let Some((lhs, _)) = text.split_once('=') else {
        return false;
    };
    let lhs = lhs.strip_suffix('+').unwrap_or(lhs);
    let lhs = lhs.split_once('[').map_or(lhs, |(name, _)| name);
    is_shell_ident(lhs)
}

fn is_shell_ident(name: &str) -> bool {
    let mut bytes = name.bytes();
    let Some(first) = bytes.next() else {
        return false;
    };
    if !(first.is_ascii_alphabetic() || first == b'_') {
        return false;
    }
    bytes.all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

fn is_assignment_prefix(text: &str) -> bool {
    text.ends_with('=') && is_assignment_word(text)
}

fn scan_array_literal(scanner: &mut Scanner<'_>, acc: &mut WordAcc) -> Result<(), LowerError> {
    let start_line = scanner.line;
    acc.is_array_assignment = true;
    scanner.pos += 1;
    let mut at_element_start = true;
    while let Some(ch) = scanner.peek() {
        match ch {
            b')' => {
                scanner.mark_code_line(scanner.line);
                scanner.pos += 1;
                return Ok(());
            }
            b' ' | b'\t' | b'\r' => {
                scanner.pos += 1;
                at_element_start = true;
            }
            b'\n' => {
                scanner.advance_newline();
                at_element_start = true;
            }
            b'#' if at_element_start => skip_array_comment(scanner),
            b'\\' if scanner.consume_line_continuation() => at_element_start = true,
            _ => {
                scan_word_piece(scanner, ch, acc)?;
                at_element_start = false;
            }
        }
    }
    Err(LowerError::Unparsable(format!(
        "unterminated array assignment at line {start_line}"
    )))
}

fn skip_array_comment(scanner: &mut Scanner<'_>) {
    while scanner.peek().is_some_and(|ch| ch != b'\n') {
        scanner.pos += 1;
    }
}

fn is_word_delimiter(ch: u8) -> bool {
    matches!(ch, b' ' | b'\t' | b'\n' | b'\r') || is_operator_start(ch)
}

fn scan_word_piece(scanner: &mut Scanner<'_>, ch: u8, acc: &mut WordAcc) -> Result<(), LowerError> {
    match ch {
        b'\'' => {
            acc.has_quotes = true;
            acc.single_quote_segments += 1;
            skip_single_quote(scanner)
        }
        b'"' => {
            acc.has_quotes = true;
            acc.other_segments += 1;
            scan_double_quote(scanner, acc)
        }
        b'\\' => {
            acc.other_segments += 1;
            scan_unquoted_escape(scanner)
        }
        b'$' => scan_dollar_unquoted(scanner, acc),
        b'`' => {
            acc.other_segments += 1;
            let bt_line = scanner.line;
            acc.record_backtick(bt_line);
            acc.record_unquoted_var(bt_line);
            acc.record_zsh_split(bt_line);
            skip_backtick(scanner, false)
        }
        _ => {
            acc.other_segments += 1;
            scanner.mark_code_line(scanner.line);
            scanner.advance_utf8_char();
            Ok(())
        }
    }
}

fn scan_unquoted_escape(scanner: &mut Scanner<'_>) -> Result<(), LowerError> {
    if scanner.consume_line_continuation() {
        return Ok(());
    }
    scanner.pos += 1;
    if scanner.pos < scanner.bytes.len() {
        scanner.mark_code_line(scanner.line);
        scanner.advance_utf8_char();
    }
    Ok(())
}

fn scan_dollar_unquoted(scanner: &mut Scanner<'_>, acc: &mut WordAcc) -> Result<(), LowerError> {
    match scanner.peek_at(1) {
        Some(b'\'') => {
            acc.has_quotes = true;
            acc.single_quote_segments += 1;
            skip_ansi_c_quote(scanner)
        }
        Some(b'"') => {
            acc.has_quotes = true;
            acc.other_segments += 1;
            scanner.pos += 1;
            scan_double_quote(scanner, acc)
        }
        _ => scan_dollar_expansion(scanner, acc),
    }
}

fn scan_dollar_expansion(scanner: &mut Scanner<'_>, acc: &mut WordAcc) -> Result<(), LowerError> {
    acc.other_segments += 1;
    let zsh = scanner.dialect == ShellDialect::Zsh;
    match scanner.peek_at(1) {
        Some(b'(') => scan_dollar_paren(scanner, acc, false),
        Some(b'{') => {
            if zsh && scanner.peek_at(2) == Some(b'=') {
                acc.record_zsh_split(scanner.line);
            }
            scan_param_expansion(scanner, acc, false)
        }
        Some(next) if is_var_start(next) => {
            scan_simple_var(scanner, acc, false);
            Ok(())
        }
        Some(b'~') if zsh => {
            acc.record_zsh_split(scanner.line);
            scanner.pos += 1;
            Ok(())
        }
        _ => {
            scanner.pos += 1;
            Ok(())
        }
    }
}

fn scan_dollar_paren(
    scanner: &mut Scanner<'_>,
    acc: &mut WordAcc,
    in_double_quotes: bool,
) -> Result<(), LowerError> {
    let start_line = scanner.line;
    let start_pos = scanner.pos;
    if scanner.peek_at(2) == Some(b'(') {
        acc.has_var_expansion = true;
        if skip_arith_expansion(scanner)? {
            acc.has_param_expansion = true;
            return Ok(());
        }
        scanner.pos = start_pos;
        scanner.line = start_line;
    }
    if !in_double_quotes {
        acc.record_unquoted_var(start_line);
        acc.record_zsh_split(start_line);
    } else {
        acc.has_var_expansion = true;
    }
    scanner.pos += 2;
    let inner = scanner.scan_stream(true)?;
    acc.sub_tokens.push(inner);
    scanner.mark_code_line(scanner.line);
    Ok(())
}

fn scan_param_expansion(
    scanner: &mut Scanner<'_>,
    acc: &mut WordAcc,
    in_double_quotes: bool,
) -> Result<(), LowerError> {
    let start_line = scanner.line;
    scanner.enter_nesting()?;
    acc.has_param_expansion = true;
    let is_length = scanner.peek_at(2) == Some(b'#')
        && scanner.peek_at(3).is_some_and(|b| b != b'}' && b != b':');
    if !in_double_quotes && !is_length {
        acc.record_unquoted_var(start_line);
    } else {
        acc.has_var_expansion = true;
    }
    scanner.pos += 2;
    let mut depth = 1usize;
    while scanner.pos < scanner.bytes.len() {
        if step_param_expansion(scanner, acc, in_double_quotes, &mut depth)? {
            scanner.leave_nesting();
            return Ok(());
        }
    }
    Err(LowerError::Unparsable(format!(
        "unterminated parameter expansion at line {start_line}"
    )))
}

fn step_param_expansion(
    scanner: &mut Scanner<'_>,
    acc: &mut WordAcc,
    in_double_quotes: bool,
    depth: &mut usize,
) -> Result<bool, LowerError> {
    let ch = scanner.bytes[scanner.pos];
    if ch == b'}' {
        *depth -= 1;
        scanner.pos += 1;
        return Ok(*depth == 0);
    }
    if ch == b'$' && scanner.peek_at(1) == Some(b'{') {
        *depth += 1;
        scanner.pos += 2;
        return Ok(false);
    }
    step_param_body_char(scanner, ch, acc, in_double_quotes)?;
    Ok(false)
}

fn step_param_body_char(
    scanner: &mut Scanner<'_>,
    ch: u8,
    acc: &mut WordAcc,
    in_double_quotes: bool,
) -> Result<(), LowerError> {
    match ch {
        b'\'' if !(in_double_quotes && scanner.dialect == ShellDialect::Zsh) => {
            skip_single_quote(scanner)
        }
        b'"' => scan_double_quote(scanner, acc),
        b'\\' => scan_unquoted_escape(scanner),
        b'$' if scanner.peek_at(1) == Some(b'(') => {
            scan_dollar_paren(scanner, acc, in_double_quotes)
        }
        b'\n' => {
            scanner.advance_newline();
            Ok(())
        }
        _ => {
            scanner.advance_utf8_char();
            Ok(())
        }
    }
}

fn scan_simple_var(scanner: &mut Scanner<'_>, acc: &mut WordAcc, in_double_quotes: bool) {
    let start_line = scanner.line;
    let next = scanner.bytes[scanner.pos + 1];
    scanner.pos += 2;
    if next.is_ascii_alphabetic() || next == b'_' {
        while scanner.peek().is_some_and(is_var_continue) {
            scanner.pos += 1;
        }
    }
    let is_always_numeric = matches!(next, b'?' | b'$' | b'!' | b'#');
    acc.has_param_expansion = true;
    if !in_double_quotes && !is_always_numeric {
        acc.record_unquoted_var(start_line);
    } else {
        acc.has_var_expansion = true;
    }
}

fn skip_arith_expansion(scanner: &mut Scanner<'_>) -> Result<bool, LowerError> {
    let start_line = scanner.line;
    scanner.pos += 3;
    let mut depth = 0usize;
    while scanner.pos < scanner.bytes.len() {
        match scanner.bytes[scanner.pos] {
            b'(' => {
                depth += 1;
                scanner.pos += 1;
            }
            b')' if depth == 0 => {
                if scanner.peek_at(1) != Some(b')') {
                    return Ok(false);
                }
                scanner.pos += 2;
                return Ok(true);
            }
            b')' => {
                depth -= 1;
                scanner.pos += 1;
            }
            b'\\' => scan_unquoted_escape(scanner)?,
            b'\n' => {
                scanner.advance_newline();
                scanner.mark_code_line(scanner.line);
            }
            _ => scanner.advance_utf8_char(),
        }
    }
    Err(LowerError::Unparsable(format!(
        "unterminated arithmetic expansion at line {start_line}"
    )))
}

#[derive(Clone, Copy)]
struct QuoteSpec {
    close: u8,
    prefix_len: usize,
    allow_escape: bool,
    nested_single_quote: bool,
    mark_lines: bool,
    label: &'static str,
}

fn skip_quoted_literal(scanner: &mut Scanner<'_>, spec: QuoteSpec) -> Result<(), LowerError> {
    let start_line = scanner.line;
    scanner.pos += spec.prefix_len;
    while scanner.pos < scanner.bytes.len() {
        if step_quoted_literal(scanner, spec)? {
            return Ok(());
        }
    }
    Err(LowerError::Unparsable(format!(
        "unterminated {} at line {start_line}",
        spec.label
    )))
}

fn step_quoted_literal(scanner: &mut Scanner<'_>, spec: QuoteSpec) -> Result<bool, LowerError> {
    let ch = scanner.bytes[scanner.pos];
    if ch == spec.close {
        scanner.pos += 1;
        if spec.mark_lines {
            scanner.mark_code_line(scanner.line);
        }
        return Ok(true);
    }
    if spec.allow_escape && ch == b'\\' {
        scan_unquoted_escape(scanner)?;
    } else if spec.nested_single_quote && ch == b'\'' {
        skip_single_quote(scanner)?;
    } else if ch == b'\n' {
        advance_quote_newline(scanner, spec.mark_lines);
    } else {
        scanner.advance_utf8_char();
    }
    Ok(false)
}

fn advance_quote_newline(scanner: &mut Scanner<'_>, mark_lines: bool) {
    scanner.advance_newline();
    if mark_lines && scanner.peek().is_some_and(|b| !matches!(b, b'\n' | b'\r')) {
        scanner.mark_code_line(scanner.line);
    }
}

pub(super) fn skip_single_quote(scanner: &mut Scanner<'_>) -> Result<(), LowerError> {
    skip_quoted_literal(
        scanner,
        QuoteSpec {
            close: b'\'',
            prefix_len: 1,
            allow_escape: false,
            nested_single_quote: false,
            mark_lines: true,
            label: "single-quoted string",
        },
    )
}

fn skip_ansi_c_quote(scanner: &mut Scanner<'_>) -> Result<(), LowerError> {
    skip_quoted_literal(
        scanner,
        QuoteSpec {
            close: b'\'',
            prefix_len: 2,
            allow_escape: true,
            nested_single_quote: false,
            mark_lines: true,
            label: "ANSI-C string",
        },
    )
}

fn scan_double_quote(scanner: &mut Scanner<'_>, acc: &mut WordAcc) -> Result<(), LowerError> {
    let start_line = scanner.line;
    scanner.pos += 1;
    while scanner.pos < scanner.bytes.len() {
        let ch = scanner.bytes[scanner.pos];
        if ch == b'"' {
            scanner.pos += 1;
            scanner.mark_code_line(scanner.line);
            return Ok(());
        }
        scan_double_quote_piece(scanner, ch, acc)?;
    }
    Err(LowerError::Unparsable(format!(
        "unterminated double-quoted string at line {start_line}"
    )))
}

fn scan_double_quote_piece(
    scanner: &mut Scanner<'_>,
    ch: u8,
    acc: &mut WordAcc,
) -> Result<(), LowerError> {
    match ch {
        b'\\' => scan_unquoted_escape(scanner),
        b'$' => scan_dollar_in_double_quote(scanner, acc),
        b'`' => {
            acc.record_backtick(scanner.line);
            skip_backtick(scanner, true)
        }
        b'\n' => {
            scanner.advance_newline();
            if scanner.peek().is_some_and(|b| !matches!(b, b'\n' | b'\r')) {
                scanner.mark_code_line(scanner.line);
            }
            Ok(())
        }
        _ => {
            scanner.advance_utf8_char();
            Ok(())
        }
    }
}

fn scan_dollar_in_double_quote(
    scanner: &mut Scanner<'_>,
    acc: &mut WordAcc,
) -> Result<(), LowerError> {
    match scanner.peek_at(1) {
        Some(b'(') => scan_dollar_paren(scanner, acc, true),
        Some(b'{') => scan_param_expansion(scanner, acc, true),
        Some(next) if is_var_start(next) => {
            scan_simple_var(scanner, acc, true);
            Ok(())
        }
        _ => {
            scanner.pos += 1;
            Ok(())
        }
    }
}

pub(super) fn skip_double_quote_simple(scanner: &mut Scanner<'_>) -> Result<(), LowerError> {
    skip_quoted_literal(
        scanner,
        QuoteSpec {
            close: b'"',
            prefix_len: 1,
            allow_escape: true,
            nested_single_quote: false,
            mark_lines: false,
            label: "double-quoted string",
        },
    )
}

fn skip_backtick(scanner: &mut Scanner<'_>, in_double_quotes: bool) -> Result<(), LowerError> {
    skip_quoted_literal(
        scanner,
        QuoteSpec {
            close: b'`',
            prefix_len: 1,
            allow_escape: true,
            nested_single_quote: !in_double_quotes,
            mark_lines: true,
            label: "backtick substitution",
        },
    )
}

fn is_var_start(ch: u8) -> bool {
    ch.is_ascii_alphabetic()
        || ch.is_ascii_digit()
        || matches!(ch, b'_' | b'@' | b'*' | b'#' | b'?' | b'$' | b'!' | b'-')
}

fn is_var_continue(ch: u8) -> bool {
    ch.is_ascii_alphanumeric() || ch == b'_'
}
