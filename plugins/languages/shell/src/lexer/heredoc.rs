use codeint_model::language::LowerError;

use super::{PendingHeredoc, Scanner};

pub(super) fn unquote_delimiter(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut chars = raw.chars();
    let mut quote: Option<char> = None;
    while let Some(ch) = chars.next() {
        step_delimiter_char(ch, &mut chars, &mut quote, &mut out);
    }
    out
}

fn step_delimiter_char(
    ch: char,
    chars: &mut std::str::Chars<'_>,
    quote: &mut Option<char>,
    out: &mut String,
) {
    if let Some(active) = *quote {
        if ch == active {
            *quote = None;
        } else if active == '"' && ch == '\\' {
            out.extend(chars.next());
        } else {
            out.push(ch);
        }
        return;
    }
    if ch == '\'' || ch == '"' {
        *quote = Some(ch);
    } else if ch == '\\' {
        out.extend(chars.next());
    } else {
        out.push(ch);
    }
}

pub(super) fn consume_body(
    scanner: &mut Scanner<'_>,
    pending: &PendingHeredoc,
) -> Result<u32, LowerError> {
    while scanner.pos < scanner.bytes.len() {
        let line_start = scanner.pos;
        let line_num = scanner.line;
        while let Some(ch) = scanner.peek() {
            if ch == b'\n' {
                break;
            }
            scanner.pos += 1;
        }
        let raw_line = scanner.src[line_start..scanner.pos].trim_end_matches('\r');
        if scanner.pos < scanner.bytes.len() {
            scanner.advance_newline();
        }
        let candidate = if pending.strip_tabs {
            raw_line.trim_start_matches('\t')
        } else {
            raw_line
        };
        if !raw_line.trim().is_empty() {
            scanner.mark_code_line(line_num);
        }
        if candidate == pending.delimiter {
            return Ok(line_num);
        }
    }
    Err(LowerError::Unparsable(format!(
        "unterminated heredoc {:?} started at line {}",
        pending.delimiter, pending.start_line
    )))
}
