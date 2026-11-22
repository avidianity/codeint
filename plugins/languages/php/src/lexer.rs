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

pub(crate) fn lex(text: &str) -> Result<LexedFile, LowerError> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let bytes = text.as_bytes();
    let mut i = 0usize;
    let mut line = 1u32;
    let mut in_php = false;
    let mut tokens = Vec::new();
    let mut header_comments = String::new();
    let mut code_lines = BTreeSet::new();
    let mut delim_stack: Vec<(u8, u32)> = Vec::new();

    while i < bytes.len() {
        if !in_php {
            if bytes[i..].starts_with(b"<?php")
                && bytes.get(i + 5).is_none_or(|b| b.is_ascii_whitespace())
            {
                in_php = true;
                i += 5;
                continue;
            }
            if bytes[i..].starts_with(b"<?=") {
                in_php = true;
                i += 3;
                continue;
            }
            if bytes[i..].starts_with(b"<?") && !bytes[i..].starts_with(b"<?xml") {
                in_php = true;
                i += 2;
                continue;
            }
            if bytes[i] == b'\n' {
                line += 1;
            }
            i += 1;
            continue;
        }

        if bytes[i].is_ascii_whitespace() {
            if bytes[i] == b'\n' {
                line += 1;
            }
            i += 1;
            continue;
        }

        if bytes[i..].starts_with(b"?>") {
            in_php = false;
            i += 2;
            continue;
        }

        if bytes[i..].starts_with(b"//") || (bytes[i] == b'#' && !bytes[i..].starts_with(b"#[")) {
            let start = i;
            while i < bytes.len() && bytes[i] != b'\n' && !bytes[i..].starts_with(b"?>") {
                i += 1;
            }
            if tokens.is_empty() {
                header_comments.push_str(&text[start..i]);
                header_comments.push('\n');
            }
            continue;
        }

        if bytes[i..].starts_with(b"/*") {
            let start = i;
            let start_line = line;
            i += 2;
            let mut closed = false;
            while i + 1 < bytes.len() {
                if bytes[i..].starts_with(b"*/") {
                    i += 2;
                    closed = true;
                    break;
                }
                if bytes[i] == b'\n' {
                    line += 1;
                }
                i += 1;
            }
            if !closed {
                return Err(LowerError::Unparsable(format!(
                    "unterminated block comment at line {start_line}"
                )));
            }
            if tokens.is_empty() {
                header_comments.push_str(&text[start..i]);
                header_comments.push('\n');
            }
            continue;
        }

        if bytes[i..].starts_with(b"<<<") {
            let start_line = line;
            let (next_i, next_line, lit) = lex_heredoc(text, i, line)?;
            for l in start_line..=next_line {
                code_lines.insert(l);
            }
            tokens.push(RawToken {
                kind: RawKind::StringLit,
                text: lit,
                lower: String::new(),
                line: start_line,
            });
            i = next_i;
            line = next_line;
            continue;
        }

        if bytes[i] == b'\'' || bytes[i] == b'"' {
            let quote = bytes[i];
            let start = i;
            let start_line = line;
            i += 1;
            let mut closed = false;
            while i < bytes.len() {
                if bytes[i] == b'\\' && i + 1 < bytes.len() {
                    if bytes[i + 1] == b'\n' {
                        line += 1;
                    }
                    i += 2;
                    continue;
                }
                if bytes[i] == quote {
                    i += 1;
                    closed = true;
                    break;
                }
                if bytes[i] == b'\n' {
                    line += 1;
                }
                i += 1;
            }
            if !closed {
                return Err(LowerError::Unparsable(format!(
                    "unterminated string literal at line {start_line}"
                )));
            }
            for l in start_line..=line {
                code_lines.insert(l);
            }
            tokens.push(RawToken {
                kind: RawKind::StringLit,
                text: text[start..i].to_owned(),
                lower: String::new(),
                line: start_line,
            });
            continue;
        }

        if bytes[i] == b'$' && bytes.get(i + 1).is_some_and(|b| is_ident_start(*b)) {
            let start = i;
            i += 2;
            while i < bytes.len() && is_ident_continue(bytes[i]) {
                i += 1;
            }
            code_lines.insert(line);
            let slice = text[start..i].to_owned();
            tokens.push(RawToken {
                kind: RawKind::Variable,
                lower: slice.clone(),
                text: slice,
                line,
            });
            continue;
        }

        if is_ident_start(bytes[i]) {
            let start = i;
            i += 1;
            while i < bytes.len() && is_ident_continue(bytes[i]) {
                i += 1;
            }
            code_lines.insert(line);
            let slice = text[start..i].to_owned();
            let lower = slice.to_ascii_lowercase();
            let kind = if is_keyword(&lower) {
                RawKind::Keyword
            } else {
                RawKind::Ident
            };
            tokens.push(RawToken {
                kind,
                text: slice,
                lower,
                line,
            });
            continue;
        }

        if bytes[i].is_ascii_digit() {
            let start = i;
            i += 1;
            while i < bytes.len()
                && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_' || bytes[i] == b'.')
            {
                i += 1;
            }
            code_lines.insert(line);
            let slice = text[start..i].to_owned();
            tokens.push(RawToken {
                kind: RawKind::NumberLit,
                lower: slice.clone(),
                text: slice,
                line,
            });
            continue;
        }

        let op = match_multi_punct(&bytes[i..]);
        let punct = op.unwrap_or(&text[i..i + 1]);
        if punct == "#[" {
            delim_stack.push((b'[', line));
        } else if punct.len() == 1 {
            match punct.as_bytes()[0] {
                b'{' | b'(' | b'[' => delim_stack.push((punct.as_bytes()[0], line)),
                b'}' | b')' | b']' => {
                    let expected = match punct.as_bytes()[0] {
                        b'}' => b'{',
                        b')' => b'(',
                        _ => b'[',
                    };
                    match delim_stack.pop() {
                        Some((open, _)) if open == expected => {}
                        _ => {
                            return Err(LowerError::Unparsable(format!(
                                "unexpected {punct:?} at line {line}"
                            )));
                        }
                    }
                }
                _ => {}
            }
        }
        code_lines.insert(line);
        tokens.push(RawToken {
            kind: RawKind::Punct,
            text: punct.to_owned(),
            lower: punct.to_owned(),
            line,
        });
        i += punct.len();
    }

    if let Some((open, open_line)) = delim_stack.last() {
        return Err(LowerError::Unparsable(format!(
            "unclosed {:?} opened at line {open_line}",
            *open as char
        )));
    }

    Ok(LexedFile {
        tokens,
        header_comments,
        code_lines,
    })
}

fn lex_heredoc(
    text: &str,
    start: usize,
    mut line: u32,
) -> Result<(usize, u32, String), LowerError> {
    let bytes = text.as_bytes();
    let mut i = start + 3;
    while i < bytes.len() && (bytes[i] == b' ' || bytes[i] == b'\t') {
        i += 1;
    }
    let quote = if i < bytes.len() && (bytes[i] == b'\'' || bytes[i] == b'"') {
        let q = bytes[i];
        i += 1;
        Some(q)
    } else {
        None
    };
    let label_start = i;
    while i < bytes.len() && is_ident_continue(bytes[i]) {
        i += 1;
    }
    let label = &text[label_start..i];
    if label.is_empty() {
        return Err(LowerError::Unparsable(format!(
            "missing heredoc label at line {line}"
        )));
    }
    if let Some(q) = quote {
        if i >= bytes.len() || bytes[i] != q {
            return Err(LowerError::Unparsable(format!(
                "unclosed heredoc label quote at line {line}"
            )));
        }
        i += 1;
    }
    while i < bytes.len() && bytes[i] != b'\n' {
        i += 1;
    }
    if i >= bytes.len() {
        return Err(LowerError::Unparsable(format!(
            "unterminated heredoc {label:?} at line {line}"
        )));
    }
    i += 1;
    line += 1;
    while i < bytes.len() {
        let line_start = i;
        while i < bytes.len() && bytes[i] != b'\n' {
            i += 1;
        }
        let line_slice = text[line_start..i].trim_start_matches([' ', '\t']);
        if let Some(after_label) = line_slice.strip_prefix(label)
            && after_label
                .bytes()
                .next()
                .is_none_or(|b| !is_ident_continue(b))
        {
            let offset_in_line =
                line_slice.as_ptr() as usize - text[line_start..].as_ptr() as usize;
            let end_pos = line_start + offset_in_line + label.len();
            return Ok((end_pos, line, text[start..end_pos].to_owned()));
        }
        if i < bytes.len() {
            i += 1;
            line += 1;
        }
    }
    Err(LowerError::Unparsable(format!(
        "unterminated heredoc {label:?} at line {line}"
    )))
}

fn is_ident_start(b: u8) -> bool {
    b.is_ascii_alphabetic() || b == b'_' || b >= 0x80
}

fn is_ident_continue(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b >= 0x80
}

fn match_multi_punct(bytes: &[u8]) -> Option<&'static str> {
    const THREE: [&str; 6] = ["===", "!==", "<=>", "...", "??=", "**="];
    for candidate in THREE {
        if bytes.starts_with(candidate.as_bytes()) {
            return Some(candidate);
        }
    }
    const TWO: [&str; 17] = [
        "&&", "||", "??", "=>", "->", "?->", "::", "==", "!=", "<=", ">=", "++", "--", "+=", "-=",
        "*=", "#[",
    ];
    TWO.into_iter()
        .find(|candidate| bytes.starts_with(candidate.as_bytes()))
}

pub(crate) fn find_matching_delim(tokens: &[RawToken], open_idx: usize) -> Option<usize> {
    let open = tokens.get(open_idx)?.text.as_str();
    let (inc, dec) = match open {
        "(" => ("(", ")"),
        "{" => ("{", "}"),
        "[" | "#[" => ("[", "]"),
        _ => return None,
    };
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
