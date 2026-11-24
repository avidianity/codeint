use std::collections::BTreeSet;

use codeint_model::{LowerError, Token as ModelToken, TokenKind, token_text_hash};
use sqlparser::dialect::{
    Dialect, DuckDbDialect, GenericDialect, MySqlDialect, PostgreSqlDialect, SQLiteDialect,
};
use sqlparser::keywords::Keyword;
use sqlparser::tokenizer::{Location, Token, TokenWithSpan, Tokenizer};

use crate::SqlDialect;
use crate::source_set::{is_pg_escaped_prefix, parse_dollar_tag};

pub(crate) struct ExpandedRoutineBody {
    pub(crate) outer_start: Location,
    pub(crate) tokens: Vec<TokenWithSpan>,
}

static GENERIC: GenericDialect = GenericDialect;
static POSTGRESQL: PostgreSqlDialect = PostgreSqlDialect {};
static MYSQL: MySqlDialect = MySqlDialect {};
static SQLITE: SQLiteDialect = SQLiteDialect {};
static DUCKDB: DuckDbDialect = DuckDbDialect;

pub(crate) fn sqlparser_dialect(dialect: SqlDialect) -> &'static dyn Dialect {
    match dialect {
        SqlDialect::Generic => &GENERIC,
        SqlDialect::Postgresql => &POSTGRESQL,
        SqlDialect::Mysql => &MYSQL,
        SqlDialect::Sqlite => &SQLITE,
        SqlDialect::Duckdb => &DUCKDB,
    }
}

pub(crate) struct TokenChunk {
    pub(crate) tokens: Vec<TokenWithSpan>,
    pub(crate) is_custom_delimited: bool,
}

pub(crate) struct TokenizedFile {
    pub(crate) chunks: Vec<TokenChunk>,
    pub(crate) command_lines: BTreeSet<u32>,
}

pub(crate) fn tokenize_source(
    source: &str,
    dialect: SqlDialect,
) -> Result<TokenizedFile, LowerError> {
    let normalized = normalize_source(source);
    match dialect {
        SqlDialect::Mysql => tokenize_mysql(&normalized),
        SqlDialect::Duckdb | SqlDialect::Postgresql => {
            tokenize_with_cli_commands(&normalized, dialect)
        }
        SqlDialect::Generic | SqlDialect::Sqlite => tokenize_plain(&normalized, dialect),
    }
}

fn normalize_source(source: &str) -> String {
    let stripped = source.strip_prefix('\u{feff}').unwrap_or(source);
    if stripped.contains('\r') {
        stripped.replace("\r\n", "\n").replace('\r', " ")
    } else {
        stripped.to_owned()
    }
}

fn tokenize_plain(source: &str, dialect: SqlDialect) -> Result<TokenizedFile, LowerError> {
    let tokens = run_tokenizer(source, dialect, 1)?;
    Ok(TokenizedFile {
        chunks: vec![TokenChunk {
            tokens,
            is_custom_delimited: false,
        }],
        command_lines: BTreeSet::new(),
    })
}

fn run_tokenizer(
    source: &str,
    dialect: SqlDialect,
    base_line: u32,
) -> Result<Vec<TokenWithSpan>, LowerError> {
    let mut tokens = Tokenizer::new(sqlparser_dialect(dialect), source)
        .tokenize_with_location()
        .map_err(|err| LowerError::Unparsable {
            message: err.to_string(),
        })?;
    let line_offset = u64::from(base_line.saturating_sub(1));
    if line_offset > 0 {
        shift_token_lines(&mut tokens, line_offset);
    }
    Ok(tokens)
}

pub(crate) fn tokenize_pg_routine_body(body: &str, base_line: u32) -> Option<Vec<TokenWithSpan>> {
    let mut tokens = Tokenizer::new(&POSTGRESQL, body)
        .tokenize_with_location()
        .ok()?;
    let line_offset = u64::from(base_line.saturating_sub(1));
    if line_offset > 0 {
        shift_token_lines(&mut tokens, line_offset);
    }
    Some(tokens)
}

fn shift_token_lines(tokens: &mut [TokenWithSpan], offset: u64) {
    for tok in tokens {
        if tok.span.start.line > 0 {
            tok.span.start.line = tok.span.start.line.saturating_add(offset);
        }
        if tok.span.end.line > 0 {
            tok.span.end.line = tok.span.end.line.saturating_add(offset);
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
enum ScanState {
    Normal,
    SingleQuote { backslash_escape: bool },
    DoubleQuote,
    Backtick,
    BlockComment(usize),
    DollarQuote(Vec<u8>),
    CopyPayload,
}

fn tokenize_with_cli_commands(
    source: &str,
    dialect: SqlDialect,
) -> Result<TokenizedFile, LowerError> {
    let mut state = ScanState::Normal;
    let mut cleaned = String::with_capacity(source.len());
    let mut command_lines = BTreeSet::new();

    for (idx, line) in source.split('\n').enumerate() {
        let line_no = u32::try_from(idx + 1).map_err(|_| LowerError::TooLarge)?;
        if idx > 0 {
            cleaned.push('\n');
        }
        process_cli_line(
            line,
            line_no,
            dialect,
            &mut state,
            &mut cleaned,
            &mut command_lines,
        );
    }

    let tokens = run_tokenizer(&cleaned, dialect, 1)?;
    Ok(TokenizedFile {
        chunks: vec![TokenChunk {
            tokens,
            is_custom_delimited: false,
        }],
        command_lines,
    })
}

fn process_cli_line(
    line: &str,
    line_no: u32,
    dialect: SqlDialect,
    state: &mut ScanState,
    cleaned: &mut String,
    command_lines: &mut BTreeSet<u32>,
) {
    if *state == ScanState::CopyPayload {
        cleaned.push_str(line);
        if line == "\\." {
            *state = ScanState::Normal;
        }
        return;
    }
    if *state == ScanState::Normal && is_cli_command_line(line, dialect) {
        command_lines.insert(line_no);
        return;
    }
    cleaned.push_str(line);
    advance_scan_state(line, state, true, false);
    if dialect == SqlDialect::Postgresql
        && *state == ScanState::Normal
        && is_pg_copy_from_stdin_line(line)
    {
        *state = ScanState::CopyPayload;
    }
}

fn is_cli_command_line(line: &str, dialect: SqlDialect) -> bool {
    let trimmed = line.trim_start();
    match dialect {
        SqlDialect::Duckdb => trimmed
            .strip_prefix('.')
            .and_then(|rest| rest.chars().next())
            .is_some_and(|ch| ch.is_ascii_alphabetic()),
        SqlDialect::Postgresql => trimmed.starts_with('\\'),
        _ => false,
    }
}

fn is_pg_copy_from_stdin_line(line: &str) -> bool {
    let upper = line.to_ascii_uppercase();
    let trimmed = upper.trim();
    trimmed.starts_with("COPY ") && trimmed.contains("FROM STDIN") && trimmed.ends_with(';')
}

struct MysqlRawChunk {
    text: String,
    base_line: u32,
    is_custom_delimited: bool,
}

fn tokenize_mysql(source: &str) -> Result<TokenizedFile, LowerError> {
    let raw_chunks = split_mysql_by_delimiters(source)?;
    let mut chunks = Vec::with_capacity(raw_chunks.len());
    for raw in raw_chunks {
        if raw.text.trim().is_empty() {
            continue;
        }
        let tokens = run_tokenizer(&raw.text, SqlDialect::Mysql, raw.base_line)?;
        chunks.push(TokenChunk {
            tokens,
            is_custom_delimited: raw.is_custom_delimited,
        });
    }
    Ok(TokenizedFile {
        chunks,
        command_lines: BTreeSet::new(),
    })
}

fn split_mysql_by_delimiters(source: &str) -> Result<Vec<MysqlRawChunk>, LowerError> {
    let mut active_delim = ";".to_owned();
    let mut state = ScanState::Normal;
    let mut out = Vec::new();
    let mut buf = String::new();
    let mut buf_base_line = 1u32;

    for (idx, line) in source.split('\n').enumerate() {
        let line_no = u32::try_from(idx + 1).map_err(|_| LowerError::TooLarge)?;
        if state == ScanState::Normal
            && let Some(new_delim) = parse_mysql_delimiter_directive(line)
        {
            flush_mysql_chunk(&mut buf, buf_base_line, active_delim != ";", &mut out);
            active_delim = new_delim;
            buf_base_line = line_no + 1;
            continue;
        }
        if active_delim == ";" {
            append_line_to_buf(&mut buf, &mut buf_base_line, line, line_no);
            advance_scan_state(line, &mut state, false, true);
        } else {
            process_custom_delim_line(
                line,
                line_no,
                &active_delim,
                &mut state,
                &mut buf,
                &mut buf_base_line,
                &mut out,
            );
        }
    }

    flush_mysql_chunk(&mut buf, buf_base_line, active_delim != ";", &mut out);
    Ok(out)
}

fn parse_mysql_delimiter_directive(line: &str) -> Option<String> {
    let trimmed = line.trim();
    let rest = trimmed
        .get(..9)
        .filter(|prefix| prefix.eq_ignore_ascii_case("DELIMITER"))
        .and_then(|_| trimmed.get(9..))?;
    if !rest.starts_with(char::is_whitespace) {
        return None;
    }
    let delim = rest.split_whitespace().next()?;
    (!delim.is_empty()).then(|| delim.to_owned())
}

fn append_line_to_buf(buf: &mut String, base_line: &mut u32, line: &str, line_no: u32) {
    if buf.is_empty() {
        *base_line = line_no;
    } else {
        buf.push('\n');
    }
    buf.push_str(line);
}

fn flush_mysql_chunk(
    buf: &mut String,
    base_line: u32,
    is_custom_delimited: bool,
    out: &mut Vec<MysqlRawChunk>,
) {
    if !buf.trim().is_empty() {
        out.push(MysqlRawChunk {
            text: std::mem::take(buf),
            base_line,
            is_custom_delimited,
        });
    } else {
        buf.clear();
    }
}

fn process_custom_delim_line(
    line: &str,
    line_no: u32,
    delim: &str,
    state: &mut ScanState,
    buf: &mut String,
    base_line: &mut u32,
    out: &mut Vec<MysqlRawChunk>,
) {
    let bytes = line.as_bytes();
    let delim_bytes = delim.as_bytes();
    let mut pos = 0usize;
    let mut seg_start = 0usize;

    while pos < bytes.len() {
        if *state == ScanState::Normal && bytes[pos..].starts_with(delim_bytes) {
            append_segment(
                buf,
                base_line,
                &line[seg_start..pos],
                line_no,
                seg_start == 0,
            );
            flush_mysql_chunk(buf, *base_line, true, out);
            pos += delim_bytes.len();
            seg_start = pos;
            *base_line = line_no;
            continue;
        }
        pos = step_scan_state(bytes, pos, state, false, true);
    }

    if seg_start < line.len() {
        append_segment(buf, base_line, &line[seg_start..], line_no, seg_start == 0);
    } else if !buf.is_empty() && seg_start == 0 {
        buf.push('\n');
    }
}

fn append_segment(
    buf: &mut String,
    base_line: &mut u32,
    segment: &str,
    line_no: u32,
    at_line_start: bool,
) {
    if buf.is_empty() {
        *base_line = line_no;
    } else if at_line_start {
        buf.push('\n');
    }
    buf.push_str(segment);
}

fn advance_scan_state(line: &str, state: &mut ScanState, nested_comments: bool, mysql_mode: bool) {
    let bytes = line.as_bytes();
    let mut pos = 0usize;
    while pos < bytes.len() {
        pos = step_scan_state(bytes, pos, state, nested_comments, mysql_mode);
    }
}

fn step_scan_state(
    bytes: &[u8],
    pos: usize,
    state: &mut ScanState,
    nested_comments: bool,
    mysql_mode: bool,
) -> usize {
    match state {
        ScanState::Normal => step_normal_state(bytes, pos, state, mysql_mode),
        ScanState::SingleQuote { backslash_escape } => {
            let escape = *backslash_escape;
            step_quote_state(bytes, pos, b'\'', escape, state)
        }
        ScanState::DoubleQuote => step_quote_state(bytes, pos, b'"', mysql_mode, state),
        ScanState::Backtick => step_quote_state(bytes, pos, b'`', false, state),
        ScanState::BlockComment(depth) => {
            step_block_comment_state(bytes, pos, *depth, nested_comments, state)
        }
        ScanState::DollarQuote(tag) => step_dollar_quote_state(bytes, pos, tag.clone(), state),
        ScanState::CopyPayload => bytes.len(),
    }
}

fn step_normal_state(bytes: &[u8], pos: usize, state: &mut ScanState, mysql_mode: bool) -> usize {
    if let Some(next) = try_enter_comment(bytes, pos, state, mysql_mode) {
        return next;
    }
    try_enter_quote(bytes, pos, state, mysql_mode).unwrap_or(pos + 1)
}

fn try_enter_comment(
    bytes: &[u8],
    pos: usize,
    state: &mut ScanState,
    mysql_mode: bool,
) -> Option<usize> {
    let ch = bytes[pos];
    if ch == b'-' && bytes.get(pos + 1) == Some(&b'-') {
        return Some(bytes.len());
    }
    if mysql_mode && ch == b'#' {
        return Some(bytes.len());
    }
    if ch == b'/' && bytes.get(pos + 1) == Some(&b'*') {
        *state = ScanState::BlockComment(1);
        return Some(pos + 2);
    }
    None
}

fn try_enter_quote(
    bytes: &[u8],
    pos: usize,
    state: &mut ScanState,
    mysql_mode: bool,
) -> Option<usize> {
    match bytes[pos] {
        b'\'' => {
            let backslash_escape = mysql_mode || is_pg_escaped_prefix(bytes, pos);
            *state = ScanState::SingleQuote { backslash_escape };
            Some(pos + 1)
        }
        b'"' => {
            *state = ScanState::DoubleQuote;
            Some(pos + 1)
        }
        b'`' => {
            *state = ScanState::Backtick;
            Some(pos + 1)
        }
        b'$' if !mysql_mode => {
            let tag = parse_dollar_tag(bytes, pos)?;
            let len = tag.len();
            *state = ScanState::DollarQuote(tag.to_vec());
            Some(pos + len)
        }
        _ => None,
    }
}

fn step_quote_state(
    bytes: &[u8],
    pos: usize,
    quote: u8,
    backslash_escape: bool,
    state: &mut ScanState,
) -> usize {
    if backslash_escape && bytes[pos] == b'\\' && pos + 1 < bytes.len() {
        return pos + 2;
    }
    if bytes[pos] == quote {
        if bytes.get(pos + 1) == Some(&quote) {
            return pos + 2;
        }
        *state = ScanState::Normal;
    }
    pos + 1
}

fn step_block_comment_state(
    bytes: &[u8],
    pos: usize,
    depth: usize,
    nested: bool,
    state: &mut ScanState,
) -> usize {
    if nested && bytes[pos] == b'/' && bytes.get(pos + 1) == Some(&b'*') {
        *state = ScanState::BlockComment(depth + 1);
        return pos + 2;
    }
    if bytes[pos] == b'*' && bytes.get(pos + 1) == Some(&b'/') {
        *state = if depth <= 1 {
            ScanState::Normal
        } else {
            ScanState::BlockComment(depth - 1)
        };
        return pos + 2;
    }
    pos + 1
}

fn step_dollar_quote_state(bytes: &[u8], pos: usize, tag: Vec<u8>, state: &mut ScanState) -> usize {
    if bytes[pos..].starts_with(&tag) {
        *state = ScanState::Normal;
        return pos + tag.len();
    }
    pos + 1
}

pub(crate) fn collect_statement_lines_and_tokens(
    tokens: &[TokenWithSpan],
    routine_bodies: &[ExpandedRoutineBody],
    code_lines: &mut BTreeSet<u32>,
    out: &mut Vec<ModelToken>,
) {
    let mut body_idx = 0usize;
    for tok in tokens {
        if matches!(tok.token, Token::Whitespace(_) | Token::EOF) {
            continue;
        }
        if matches!(tok.token, Token::DollarQuotedString(_))
            && let Some(body) = routine_bodies.get(body_idx)
            && body.outer_start == tok.span.start
        {
            body_idx += 1;
            collect_dollar_body(tok, &body.tokens, code_lines, out);
            continue;
        }
        insert_span_lines(tok, code_lines);
        if let Some(model_tok) = to_model_token(tok) {
            out.push(model_tok);
        }
    }
}

fn collect_dollar_body(
    outer: &TokenWithSpan,
    body_tokens: &[TokenWithSpan],
    code_lines: &mut BTreeSet<u32>,
    out: &mut Vec<ModelToken>,
) {
    if outer.span.start.line > 0 {
        code_lines.insert(outer.span.start.line as u32);
    }
    if outer.span.end.line > 0 {
        code_lines.insert(outer.span.end.line as u32);
    }
    for tok in body_tokens {
        if matches!(tok.token, Token::Whitespace(_) | Token::EOF) {
            continue;
        }
        insert_span_lines(tok, code_lines);
        if let Some(model_tok) = to_model_token(tok) {
            out.push(model_tok);
        }
    }
}

fn insert_span_lines(tok: &TokenWithSpan, code_lines: &mut BTreeSet<u32>) {
    let start = tok.span.start.line as u32;
    let end = tok.span.end.line as u32;
    if start == 0 {
        return;
    }
    let last = end.max(start);
    for line in start..=last {
        code_lines.insert(line);
    }
}

fn to_model_token(tok: &TokenWithSpan) -> Option<ModelToken> {
    if matches!(tok.token, Token::Whitespace(_) | Token::EOF) {
        return None;
    }
    let line = tok.span.start.line as u32;
    let (kind, normalized_text) = classify_token(&tok.token);
    Some(ModelToken {
        kind,
        text_hash: token_text_hash(&normalized_text),
        line,
    })
}

fn classify_token(token: &Token) -> (TokenKind, String) {
    match token {
        Token::Word(word) => {
            if word.quote_style.is_none() && word.keyword != Keyword::NoKeyword {
                (TokenKind::Other, word.value.to_ascii_uppercase())
            } else {
                (TokenKind::Identifier, word.value.clone())
            }
        }
        Token::Number(_, _)
        | Token::SingleQuotedString(_)
        | Token::DoubleQuotedString(_)
        | Token::TripleSingleQuotedString(_)
        | Token::TripleDoubleQuotedString(_)
        | Token::DollarQuotedString(_)
        | Token::SingleQuotedByteStringLiteral(_)
        | Token::DoubleQuotedByteStringLiteral(_)
        | Token::TripleSingleQuotedByteStringLiteral(_)
        | Token::TripleDoubleQuotedByteStringLiteral(_)
        | Token::SingleQuotedRawStringLiteral(_)
        | Token::DoubleQuotedRawStringLiteral(_)
        | Token::TripleSingleQuotedRawStringLiteral(_)
        | Token::TripleDoubleQuotedRawStringLiteral(_)
        | Token::NationalStringLiteral(_)
        | Token::QuoteDelimitedStringLiteral(_)
        | Token::NationalQuoteDelimitedStringLiteral(_)
        | Token::EscapedStringLiteral(_)
        | Token::UnicodeStringLiteral(_)
        | Token::HexStringLiteral(_) => (TokenKind::Literal, token.to_string()),
        _ => (TokenKind::Other, token.to_string()),
    }
}
