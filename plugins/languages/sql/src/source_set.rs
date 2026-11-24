use codeint_model::{RepoPath, SourceSet};

use crate::SqlDialect;

const DUMP_HEADER_LINES: usize = 20;

const DUMP_HEADER_PREFIXES: &[&str] = &[
    "-- PostgreSQL database dump",
    "-- Dumped by pg_dump",
    "-- MySQL dump ",
    "-- MariaDB database dump",
    "-- MariaDB dump ",
];

const MIGRATION_MARKER_PREFIXES: &[&str] = &[
    "--> statement-breakpoint",
    "-- atlas:txmode",
    "-- atlas:nolint",
];

const TEST_FOLDER_NAMES: &[&str] = &["test", "tests", "fixtures"];

pub(crate) fn classify(path: &RepoPath, source: &str) -> SourceSet {
    if has_dump_header(source)
        || has_migration_marker(source)
        || is_generated_migration_folder(path)
    {
        return SourceSet::Generated;
    }
    if is_test_folder(path) {
        return SourceSet::Test;
    }
    SourceSet::Production
}

fn has_dump_header(source: &str) -> bool {
    let text = source.strip_prefix('\u{feff}').unwrap_or(source);
    text.lines().take(DUMP_HEADER_LINES).any(|line| {
        let trimmed = line.trim_start();
        DUMP_HEADER_PREFIXES
            .iter()
            .any(|prefix| trimmed.starts_with(prefix))
    })
}

fn has_migration_marker(source: &str) -> bool {
    let text = source.strip_prefix('\u{feff}').unwrap_or(source);
    text.lines().any(|line| {
        let trimmed = line.trim_start();
        MIGRATION_MARKER_PREFIXES
            .iter()
            .any(|prefix| trimmed.starts_with(prefix))
    })
}

fn is_generated_migration_folder(path: &RepoPath) -> bool {
    let Some((parent, _)) = path.as_str().rsplit_once('/') else {
        return false;
    };
    let segments: Vec<&str> = parent.split('/').collect();
    let has_drizzle = segments.contains(&"drizzle");
    let has_prisma_migrations = segments
        .windows(2)
        .any(|pair| pair == ["prisma", "migrations"]);
    has_drizzle || has_prisma_migrations
}

fn is_test_folder(path: &RepoPath) -> bool {
    let Some((parent, _)) = path.as_str().rsplit_once('/') else {
        return false;
    };
    parent
        .split('/')
        .any(|segment| TEST_FOLDER_NAMES.contains(&segment))
}

pub(crate) fn has_template_markers(source: &str, dialect: SqlDialect) -> bool {
    let bytes = source.as_bytes();
    let nested_comments = matches!(
        dialect,
        SqlDialect::Generic | SqlDialect::Postgresql | SqlDialect::Duckdb
    );
    let mysql_mode = dialect == SqlDialect::Mysql;
    let mut pos = 0usize;

    while pos < bytes.len() {
        if let Some(next) = skip_comment_or_string(bytes, pos, nested_comments, mysql_mode) {
            pos = next;
            continue;
        }
        if is_template_marker_at(bytes, pos) {
            return true;
        }
        pos += 1;
    }
    false
}

fn is_template_marker_at(bytes: &[u8], pos: usize) -> bool {
    if pos + 1 >= bytes.len() {
        return false;
    }
    matches!(&bytes[pos..pos + 2], b"{{" | b"{%" | b"%}" | b"<%" | b"%>")
}

fn skip_comment_or_string(
    bytes: &[u8],
    pos: usize,
    nested_comments: bool,
    mysql_mode: bool,
) -> Option<usize> {
    skip_comment(bytes, pos, nested_comments, mysql_mode)
        .or_else(|| skip_quoted_or_dollar(bytes, pos, mysql_mode))
}

fn skip_comment(
    bytes: &[u8],
    pos: usize,
    nested_comments: bool,
    mysql_mode: bool,
) -> Option<usize> {
    let ch = bytes[pos];
    if ch == b'-' && bytes.get(pos + 1) == Some(&b'-') {
        return Some(skip_to_line_end(bytes, pos + 2));
    }
    if mysql_mode && ch == b'#' {
        return Some(skip_to_line_end(bytes, pos + 1));
    }
    if ch == b'/' && bytes.get(pos + 1) == Some(&b'*') {
        return Some(skip_block_comment(bytes, pos + 2, nested_comments));
    }
    None
}

fn skip_quoted_or_dollar(bytes: &[u8], pos: usize, mysql_mode: bool) -> Option<usize> {
    match bytes[pos] {
        b'\'' => {
            let backslash = mysql_mode || is_pg_escaped_prefix(bytes, pos);
            Some(skip_quoted(bytes, pos + 1, b'\'', backslash))
        }
        b'"' => Some(skip_quoted(bytes, pos + 1, b'"', mysql_mode)),
        b'`' => Some(skip_quoted(bytes, pos + 1, b'`', false)),
        b'$' => skip_dollar_quoted(bytes, pos),
        _ => None,
    }
}

fn is_pg_escaped_prefix(bytes: &[u8], quote_pos: usize) -> bool {
    if quote_pos == 0 || !matches!(bytes[quote_pos - 1], b'e' | b'E') {
        return false;
    }
    quote_pos == 1 || !is_ident_byte(bytes[quote_pos - 2])
}

fn is_ident_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

fn skip_to_line_end(bytes: &[u8], mut pos: usize) -> usize {
    while pos < bytes.len() && bytes[pos] != b'\n' {
        pos += 1;
    }
    pos
}

fn skip_block_comment(bytes: &[u8], mut pos: usize, nested: bool) -> usize {
    let mut depth = 1usize;
    while pos + 1 < bytes.len() {
        if nested && bytes[pos] == b'/' && bytes[pos + 1] == b'*' {
            depth += 1;
            pos += 2;
        } else if bytes[pos] == b'*' && bytes[pos + 1] == b'/' {
            depth -= 1;
            pos += 2;
            if depth == 0 {
                return pos;
            }
        } else {
            pos += 1;
        }
    }
    bytes.len()
}

fn skip_quoted(bytes: &[u8], mut pos: usize, quote: u8, backslash_escape: bool) -> usize {
    while pos < bytes.len() {
        let ch = bytes[pos];
        if backslash_escape && ch == b'\\' && pos + 1 < bytes.len() {
            pos += 2;
        } else if ch == quote {
            if bytes.get(pos + 1) == Some(&quote) {
                pos += 2;
            } else {
                return pos + 1;
            }
        } else {
            pos += 1;
        }
    }
    bytes.len()
}

pub(crate) fn parse_dollar_tag(bytes: &[u8], start: usize) -> Option<&[u8]> {
    if bytes.get(start) != Some(&b'$') {
        return None;
    }
    if bytes.get(start + 1) == Some(&b'$') {
        return Some(&bytes[start..start + 2]);
    }
    let first = *bytes.get(start + 1)?;
    if !(first.is_ascii_alphabetic() || first == b'_') {
        return None;
    }
    let mut pos = start + 2;
    while pos < bytes.len() && is_ident_byte(bytes[pos]) {
        pos += 1;
    }
    (bytes.get(pos) == Some(&b'$')).then_some(&bytes[start..=pos])
}

fn skip_dollar_quoted(bytes: &[u8], start: usize) -> Option<usize> {
    let tag = parse_dollar_tag(bytes, start)?;
    let mut pos = start + tag.len();
    while pos + tag.len() <= bytes.len() {
        if &bytes[pos..pos + tag.len()] == tag {
            return Some(pos + tag.len());
        }
        pos += 1;
    }
    Some(bytes.len())
}
