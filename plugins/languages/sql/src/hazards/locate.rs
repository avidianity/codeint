use sqlparser::ast::{Select, Spanned};
use sqlparser::keywords::Keyword;
use sqlparser::tokenizer::{Location, Span, Token, TokenWithSpan};

pub(super) fn span_line(span: Span) -> u32 {
    span.start.line as u32
}

pub(super) fn is_keyword(tok: &TokenWithSpan, kw: Keyword) -> bool {
    matches!(&tok.token, Token::Word(w) if w.quote_style.is_none() && w.keyword == kw)
}

pub(super) fn is_code_token(tok: &TokenWithSpan) -> bool {
    !matches!(tok.token, Token::Whitespace(_) | Token::EOF)
}

pub(super) fn first_keyword_line(tokens: &[TokenWithSpan], kw: Keyword) -> u32 {
    tokens
        .iter()
        .find(|t| is_keyword(t, kw))
        .map_or(0, |t| t.span.start.line as u32)
}

pub(super) fn keyword_line_at_or_after(
    tokens: &[TokenWithSpan],
    kw: Keyword,
    min_loc: Location,
) -> u32 {
    if min_loc.line == 0 {
        return first_keyword_line(tokens, kw);
    }
    tokens
        .iter()
        .find(|t| t.span.start >= min_loc && is_keyword(t, kw))
        .map_or(0, |t| t.span.start.line as u32)
}

pub(super) fn keyword_line_in_range(
    tokens: &[TokenWithSpan],
    kw: Keyword,
    start: Location,
    end: Location,
) -> u32 {
    if start.line == 0 || end.line == 0 {
        return first_keyword_line(tokens, kw);
    }
    tokens
        .iter()
        .find(|t| t.span.start >= start && t.span.end <= end && is_keyword(t, kw))
        .map_or(0, |t| t.span.start.line as u32)
}

pub(super) fn keyword_line_before(
    tokens: &[TokenWithSpan],
    kw: Keyword,
    before_loc: Location,
) -> u32 {
    if before_loc.line == 0 {
        return first_keyword_line(tokens, kw);
    }
    let Some(idx) = tokens
        .iter()
        .rposition(|t| t.span.start < before_loc && is_code_token(t))
    else {
        return 0;
    };
    for tok in tokens[..=idx].iter().rev() {
        if !is_code_token(tok) {
            continue;
        }
        if is_keyword(tok, kw) {
            return tok.span.start.line as u32;
        }
        if !matches!(tok.token, Token::LParen | Token::Word(_)) {
            break;
        }
    }
    0
}

pub(super) fn keyword_pair_line_before(
    tokens: &[TokenWithSpan],
    first: Keyword,
    second: Keyword,
    before_loc: Location,
) -> u32 {
    if before_loc.line == 0 {
        return first_keyword_pair_line(tokens, first, second);
    }
    let code: Vec<&TokenWithSpan> = tokens
        .iter()
        .filter(|t| t.span.start < before_loc && is_code_token(t))
        .collect();
    for pair in code.windows(2).rev() {
        if is_keyword(pair[0], first) && is_keyword(pair[1], second) {
            return pair[0].span.start.line as u32;
        }
    }
    0
}

pub(super) fn first_keyword_pair_line(
    tokens: &[TokenWithSpan],
    first: Keyword,
    second: Keyword,
) -> u32 {
    let mut prev_matches = false;
    let mut prev_line = 0u32;
    for tok in tokens {
        if !is_code_token(tok) {
            continue;
        }
        if prev_matches && is_keyword(tok, second) {
            return prev_line;
        }
        prev_matches = is_keyword(tok, first);
        prev_line = tok.span.start.line as u32;
    }
    0
}

pub(super) fn select_line(select: &Select, tokens: &[TokenWithSpan]) -> u32 {
    let tok_line = select.select_token.0.span.start.line as u32;
    if tok_line > 0 {
        return tok_line;
    }
    let span_l = span_line(select.span());
    if span_l > 0 {
        return span_l;
    }
    first_keyword_line(tokens, Keyword::SELECT)
}

pub(super) fn select_start_loc(select: &Select) -> Location {
    if select.select_token.0.span.start.line > 0 {
        select.select_token.0.span.start
    } else {
        select.span().start
    }
}
