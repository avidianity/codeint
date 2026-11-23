use std::collections::BTreeSet;

use codeint_model::language::Function;
use codeint_model::scan::{
    find_arrow_expr_end, find_matching_delim, paren_then_brace, score_function_spans,
};

use crate::lexer::{DartKind, DartToken};

struct DartFunctionSpan {
    header_start: usize,
    body_start: usize,
    body_end: usize,
    start_line: u32,
    end_line: u32,
    name: Option<String>,
    is_block: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum DartBoolOp {
    And,
    Or,
}

const NON_FN_PAREN_KEYWORDS: [&str; 17] = [
    "if", "for", "while", "switch", "catch", "assert", "return", "throw", "new", "const", "await",
    "yield", "case", "super", "this", "on", "when",
];

pub(crate) fn extract(tokens: &[DartToken], code_lines: &BTreeSet<u32>) -> Vec<Function> {
    let mut spans = Vec::new();
    for (idx, tok) in tokens.iter().enumerate() {
        let found = match (tok.kind, tok.text.as_str()) {
            (DartKind::Punct, "(") => try_paren_function(tokens, idx),
            (DartKind::Keyword, "get") => try_getter_function(tokens, idx),
            _ => None,
        };
        if let Some(span) = found {
            spans.push(span);
        }
    }
    spans.sort_by_key(|s| (s.start_line, s.end_line));
    let raw_spans: Vec<(usize, usize, u32, u32)> = spans
        .iter()
        .map(|s| (s.body_start, s.body_end, s.start_line, s.end_line))
        .collect();
    let mut functions = score_function_spans(tokens, code_lines, &raw_spans, decision_weight);
    for (owner, func) in functions.iter_mut().enumerate() {
        func.name = spans[owner].name.clone();
        func.cognitive_complexity = compute_dart_cognitive(tokens, &spans, owner);
    }
    functions
}

fn try_paren_function(tokens: &[DartToken], open_paren: usize) -> Option<DartFunctionSpan> {
    if open_paren > 0 {
        let prev = &tokens[open_paren - 1];
        if prev.kind == DartKind::Keyword && NON_FN_PAREN_KEYWORDS.contains(&prev.text.as_str()) {
            return None;
        }
        if prev.text == "=>" {
            return None;
        }
    }
    let close_paren = find_matching_delim(tokens, open_paren)?;
    let (body_start, body_end, is_block) = locate_body_after_params(tokens, close_paren + 1)?;
    let (name, header_start) = extract_paren_name(tokens, open_paren);
    Some(DartFunctionSpan {
        header_start,
        body_start,
        body_end,
        start_line: tokens[open_paren].line,
        end_line: tokens[body_end].line,
        name,
        is_block,
    })
}

fn extract_paren_name(tokens: &[DartToken], open_paren: usize) -> (Option<String>, usize) {
    let cursor = skip_trailing_type_params(tokens, open_paren);
    if let Some(op) = extract_operator_name(tokens, cursor) {
        return op;
    }
    if let Some(ident) = extract_ident_name(tokens, cursor) {
        return ident;
    }
    (None, open_paren)
}

fn extract_operator_name(tokens: &[DartToken], cursor: usize) -> Option<(Option<String>, usize)> {
    if let Some((label, start)) = match_index_operator(tokens, cursor) {
        return Some((Some(label.to_owned()), start));
    }
    if cursor >= 2 && is_kw(&tokens[cursor - 2], "operator") {
        return Some((
            Some(format!("operator {}", tokens[cursor - 1].text)),
            cursor - 2,
        ));
    }
    None
}

fn match_index_operator(tokens: &[DartToken], cursor: usize) -> Option<(&'static str, usize)> {
    if ends_with_operator_tokens(tokens, cursor, &["[", "]", "="]) {
        return Some(("operator []=", cursor - 4));
    }
    if ends_with_operator_tokens(tokens, cursor, &["[", "]"]) {
        return Some(("operator []", cursor - 3));
    }
    None
}

fn ends_with_operator_tokens(tokens: &[DartToken], cursor: usize, expected: &[&str]) -> bool {
    let Some(start) = cursor.checked_sub(expected.len() + 1) else {
        return false;
    };
    is_kw(&tokens[start], "operator")
        && tokens[start + 1..cursor]
            .iter()
            .zip(expected)
            .all(|(tok, want)| tok.text == *want)
}

fn extract_ident_name(tokens: &[DartToken], cursor: usize) -> Option<(Option<String>, usize)> {
    if cursor == 0 || tokens[cursor - 1].kind != DartKind::Ident {
        return None;
    }
    if cursor >= 3 && tokens[cursor - 2].text == "." && tokens[cursor - 3].kind == DartKind::Ident {
        let full = format!("{}.{}", tokens[cursor - 3].text, tokens[cursor - 1].text);
        return Some((Some(full), cursor - 3));
    }
    Some((Some(tokens[cursor - 1].text.clone()), cursor - 1))
}

fn skip_trailing_type_params(tokens: &[DartToken], open_paren: usize) -> usize {
    if open_paren == 0 {
        return open_paren;
    }
    let mut depth = angle_close_count(tokens[open_paren - 1].text.as_str());
    if depth == 0 {
        return open_paren;
    }
    let mut pos = open_paren - 1;
    while pos > 0 {
        pos -= 1;
        let text = tokens[pos].text.as_str();
        depth += angle_close_count(text);
        if text == "<" {
            depth = depth.saturating_sub(1);
            if depth == 0 {
                return pos;
            }
        }
    }
    open_paren
}

fn angle_close_count(text: &str) -> usize {
    match text {
        ">" => 1,
        ">>" => 2,
        ">>>" => 3,
        _ => 0,
    }
}

fn try_getter_function(tokens: &[DartToken], get_idx: usize) -> Option<DartFunctionSpan> {
    let name_tok = tokens.get(get_idx + 1)?;
    if name_tok.kind != DartKind::Ident {
        return None;
    }
    let (body_start, body_end, is_block) = locate_body_after_params(tokens, get_idx + 2)?;
    Some(DartFunctionSpan {
        header_start: get_idx,
        body_start,
        body_end,
        start_line: tokens[get_idx].line,
        end_line: tokens[body_end].line,
        name: Some(name_tok.text.clone()),
        is_block,
    })
}

fn locate_body_after_params(tokens: &[DartToken], start: usize) -> Option<(usize, usize, bool)> {
    let pos = skip_modifiers_and_initializers(tokens, start)?;
    let tok = tokens.get(pos)?;
    if tok.text == "{" {
        return Some((pos, find_matching_delim(tokens, pos)?, true));
    }
    if tok.text == "=>" {
        let body_start = pos + 1;
        return Some((body_start, find_arrow_expr_end(tokens, body_start), false));
    }
    None
}

fn skip_modifiers_and_initializers(tokens: &[DartToken], mut pos: usize) -> Option<usize> {
    if tokens.get(pos).is_some_and(|t| t.text == ":") {
        pos = skip_initializer_list(tokens, pos + 1)?;
    }
    while tokens
        .get(pos)
        .is_some_and(|t| matches!(t.text.as_str(), "async" | "sync" | "*"))
    {
        pos += 1;
    }
    Some(pos)
}

fn skip_initializer_list(tokens: &[DartToken], mut pos: usize) -> Option<usize> {
    let mut depth = 0usize;
    while pos < tokens.len() {
        match tokens[pos].text.as_str() {
            "(" | "[" => depth += 1,
            ")" | "]" => depth = depth.saturating_sub(1),
            "{" | "=>" if depth == 0 => return Some(pos),
            ";" if depth == 0 => return None,
            _ => {}
        }
        pos += 1;
    }
    None
}

fn decision_weight(tokens: &[DartToken], idx: usize) -> u32 {
    let tok = &tokens[idx];
    match (tok.kind, tok.text.as_str()) {
        (DartKind::Keyword, "if" | "for" | "while" | "do" | "case" | "catch") => 1,
        (DartKind::Keyword, "switch") => count_switch_expr_arms(tokens, idx),
        (DartKind::Punct, "&&" | "||" | "??") => 1,
        (DartKind::Punct, "?") if is_ternary_operator(tokens, idx) => 1,
        _ => 0,
    }
}

fn is_ternary_operator(tokens: &[DartToken], q_idx: usize) -> bool {
    let mut depth = 0usize;
    for tok in tokens.iter().skip(q_idx + 1) {
        match tok.text.as_str() {
            "(" | "[" | "{" => depth += 1,
            ")" | "]" | "}" if depth == 0 => return false,
            ")" | "]" | "}" => depth -= 1,
            ":" if depth == 0 => return true,
            ";" | "," if depth == 0 => return false,
            _ => {}
        }
    }
    false
}

fn count_switch_expr_arms(tokens: &[DartToken], switch_idx: usize) -> u32 {
    let Some((_, _, open_brace, close_brace)) = paren_then_brace(tokens, switch_idx) else {
        return 0;
    };
    let mut arms = 0u32;
    let mut level = 0usize;
    let mut idx = open_brace + 1;
    while idx < close_brace {
        let text = tokens[idx].text.as_str();
        if matches!(text, "(" | "[" | "{") {
            level += 1;
        } else if matches!(text, ")" | "]" | "}") {
            level = level.saturating_sub(1);
        } else if level == 0 {
            if text == "case" {
                return 0;
            }
            if text == "=>" {
                arms += 1;
            }
        }
        idx += 1;
    }
    arms.saturating_sub(1)
}

fn is_kw(tok: &DartToken, kw: &str) -> bool {
    tok.kind == DartKind::Keyword && tok.text == kw
}

fn compute_dart_cognitive(tokens: &[DartToken], spans: &[DartFunctionSpan], owner: usize) -> u32 {
    let span = &spans[owner];
    let simple_name = span
        .name
        .as_deref()
        .filter(|n| !n.contains(' ') && !n.contains('.'));
    let mut analyzer = DartCognitiveAnalyzer {
        tokens,
        spans,
        owner,
        simple_name,
        complexity: 0,
        saw_recursion: false,
    };
    if span.is_block {
        if span.body_start + 1 < span.body_end {
            analyzer.analyze_span(span.body_start + 1, span.body_end, 0);
        }
    } else {
        analyzer.analyze_span(span.body_start, span.body_end + 1, 0);
    }
    analyzer.complexity
}

struct DartBoolTracker {
    active: Option<DartBoolOp>,
    saved: Vec<Option<Option<DartBoolOp>>>,
}

impl DartBoolTracker {
    fn new() -> Self {
        Self {
            active: None,
            saved: Vec::new(),
        }
    }

    fn clear(&mut self) {
        self.active = None;
    }

    fn track(&mut self, op: DartBoolOp) -> u32 {
        let changed = self.active != Some(op);
        self.active = Some(op);
        u32::from(changed)
    }

    fn enter_paren(&mut self, preceded_by_callee: bool) {
        if preceded_by_callee {
            self.enter_nested();
        } else {
            self.saved.push(None);
        }
    }

    fn enter_nested(&mut self) {
        self.saved.push(Some(self.active.take()));
    }

    fn leave(&mut self) {
        if let Some(Some(prev)) = self.saved.pop() {
            self.active = prev;
        }
    }
}

struct DartCognitiveAnalyzer<'a> {
    tokens: &'a [DartToken],
    spans: &'a [DartFunctionSpan],
    owner: usize,
    simple_name: Option<&'a str>,
    complexity: u32,
    saw_recursion: bool,
}

impl DartCognitiveAnalyzer<'_> {
    fn analyze_span(&mut self, from: usize, to: usize, depth: u32) {
        let mut tracker = DartBoolTracker::new();
        let mut cursor = from;
        while cursor < to {
            if let Some(skip_to) = self.nested_child_end(cursor) {
                cursor = skip_to + 1;
                tracker.clear();
                continue;
            }
            cursor = self.consume_next(cursor, from, to, depth, &mut tracker);
        }
    }

    fn consume_next(
        &mut self,
        cursor: usize,
        from: usize,
        to: usize,
        depth: u32,
        tracker: &mut DartBoolTracker,
    ) -> usize {
        if self.tokens[cursor].kind == DartKind::Keyword
            && !is_dart_member_access(self.tokens, cursor)
            && let Some(next) = self.consume_keyword(cursor, to, depth, tracker)
        {
            return next;
        }
        self.consume_symbol(cursor, from, to, depth, tracker)
    }

    fn consume_keyword(
        &mut self,
        cursor: usize,
        to: usize,
        depth: u32,
        tracker: &mut DartBoolTracker,
    ) -> Option<usize> {
        match self.tokens[cursor].text.as_str() {
            "if" | "for" | "while" | "switch" => {
                self.complexity = self.complexity.saturating_add(depth.saturating_add(1));
                tracker.clear();
                Some(self.consume_paren_and_body(cursor + 1, to, depth))
            }
            "else" => {
                self.complexity = self.complexity.saturating_add(1);
                tracker.clear();
                if self.tokens.get(cursor + 1).is_some_and(|t| is_kw(t, "if")) {
                    Some(self.consume_paren_and_body(cursor + 2, to, depth))
                } else {
                    Some(self.consume_branch_body(cursor + 1, to, depth))
                }
            }
            "do" => {
                self.complexity = self.complexity.saturating_add(depth.saturating_add(1));
                tracker.clear();
                Some(self.consume_do_loop(cursor + 1, to, depth))
            }
            "catch" => {
                self.complexity = self.complexity.saturating_add(depth.saturating_add(1));
                tracker.clear();
                Some(self.consume_catch_clause(cursor + 1, to, depth))
            }
            "on" => self.try_consume_bare_on_clause(cursor, to, depth, tracker),
            "break" | "continue" => {
                if self
                    .tokens
                    .get(cursor + 1)
                    .is_some_and(|t| t.kind == DartKind::Ident)
                {
                    self.complexity = self.complexity.saturating_add(1);
                }
                tracker.clear();
                Some(cursor + 1)
            }
            "try" | "finally" | "return" | "throw" | "rethrow" | "yield" | "case" | "default" => {
                tracker.clear();
                Some(cursor + 1)
            }
            _ => None,
        }
    }

    fn try_consume_bare_on_clause(
        &mut self,
        cursor: usize,
        to: usize,
        depth: u32,
        tracker: &mut DartBoolTracker,
    ) -> Option<usize> {
        let after_type = skip_dart_type(self.tokens, cursor + 1, to);
        if after_type < to && self.tokens[after_type].text == "{" {
            self.complexity = self.complexity.saturating_add(depth.saturating_add(1));
            tracker.clear();
            return Some(self.consume_branch_body(after_type, to, depth));
        }
        None
    }

    fn consume_symbol(
        &mut self,
        cursor: usize,
        from: usize,
        to: usize,
        depth: u32,
        tracker: &mut DartBoolTracker,
    ) -> usize {
        let text = self.tokens[cursor].text.as_str();
        if text == "?" && is_ternary_operator(self.tokens, cursor) {
            self.complexity = self.complexity.saturating_add(depth.saturating_add(1));
            tracker.clear();
            return self.consume_ternary(cursor + 1, to, depth);
        }
        if !self.apply_non_ternary_symbol(text, cursor, from, tracker) {
            self.detect_direct_recursion(cursor);
        }
        cursor + 1
    }

    fn apply_non_ternary_symbol(
        &mut self,
        text: &str,
        cursor: usize,
        from: usize,
        tracker: &mut DartBoolTracker,
    ) -> bool {
        match text {
            "&&" => {
                self.complexity = self
                    .complexity
                    .saturating_add(tracker.track(DartBoolOp::And));
            }
            "||" => {
                self.complexity = self
                    .complexity
                    .saturating_add(tracker.track(DartBoolOp::Or));
            }
            "(" => tracker.enter_paren(preceded_by_dart_callee(self.tokens, cursor, from)),
            "[" | "{" => tracker.enter_nested(),
            ")" | "]" | "}" => tracker.leave(),
            _ if is_dart_reset_punct(text) => tracker.clear(),
            _ => return false,
        }
        true
    }

    fn consume_paren_and_body(&mut self, mut pos: usize, to: usize, depth: u32) -> usize {
        if pos < to
            && self.tokens[pos].text == "("
            && let Some(close) = find_matching_delim(self.tokens, pos)
            && close < to
        {
            self.analyze_span(pos + 1, close, depth);
            pos = close + 1;
        }
        self.consume_branch_body(pos, to, depth)
    }

    fn consume_catch_clause(&mut self, mut pos: usize, to: usize, depth: u32) -> usize {
        if pos < to
            && self.tokens[pos].text == "("
            && let Some(close) = find_matching_delim(self.tokens, pos)
            && close < to
        {
            pos = close + 1;
        }
        self.consume_branch_body(pos, to, depth)
    }

    fn consume_do_loop(&mut self, pos: usize, to: usize, depth: u32) -> usize {
        let after = self.consume_branch_body(pos, to, depth);
        if after < to
            && is_kw(&self.tokens[after], "while")
            && after + 1 < to
            && self.tokens[after + 1].text == "("
            && let Some(close) = find_matching_delim(self.tokens, after + 1)
            && close < to
        {
            self.analyze_span(after + 2, close, depth);
            return close + 1;
        }
        after
    }

    fn consume_branch_body(&mut self, pos: usize, to: usize, depth: u32) -> usize {
        if pos >= to {
            return to;
        }
        if self.tokens[pos].text == "{"
            && let Some(close) = find_matching_delim(self.tokens, pos)
            && close < to
        {
            self.analyze_span(pos + 1, close, depth + 1);
            return close + 1;
        }
        let stmt_end = find_dart_single_body_end(self.tokens, pos, to);
        self.analyze_span(pos, stmt_end, depth + 1);
        stmt_end
    }

    fn consume_ternary(&mut self, start: usize, to: usize, depth: u32) -> usize {
        let Some(colon) = find_dart_ternary_colon(self.tokens, start, to) else {
            return start;
        };
        if start < colon {
            self.analyze_span(start, colon, depth + 1);
        }
        let alt_end = find_dart_ternary_alt_end(self.tokens, colon + 1, to);
        if colon + 1 < alt_end {
            self.analyze_span(colon + 1, alt_end, depth + 1);
        }
        alt_end
    }

    fn detect_direct_recursion(&mut self, cursor: usize) {
        if self.saw_recursion {
            return;
        }
        let Some(expected) = self.simple_name else {
            return;
        };
        let tok = &self.tokens[cursor];
        if tok.kind != DartKind::Ident
            || tok.text != expected
            || !self.tokens.get(cursor + 1).is_some_and(|t| t.text == "(")
        {
            return;
        }
        let direct = if cursor > 0 && matches!(self.tokens[cursor - 1].text.as_str(), "." | "?.") {
            cursor >= 2 && is_kw(&self.tokens[cursor - 2], "this")
        } else {
            true
        };
        if direct {
            self.saw_recursion = true;
            self.complexity = self.complexity.saturating_add(1);
        }
    }

    fn nested_child_end(&self, cursor: usize) -> Option<usize> {
        self.spans
            .iter()
            .enumerate()
            .filter(|&(idx, s)| idx != self.owner && s.header_start == cursor)
            .map(|(_, s)| s.body_end)
            .max()
    }
}

fn preceded_by_dart_callee(tokens: &[DartToken], cursor: usize, from: usize) -> bool {
    tokens
        .get(cursor.wrapping_sub(1))
        .filter(|_| cursor > from)
        .is_some_and(|t| t.kind == DartKind::Ident || matches!(t.text.as_str(), ")" | "]"))
}

fn is_dart_member_access(tokens: &[DartToken], cursor: usize) -> bool {
    tokens
        .get(cursor.wrapping_sub(1))
        .filter(|_| cursor > 0)
        .is_some_and(|t| matches!(t.text.as_str(), "." | "?." | ".."))
}

fn is_dart_reset_punct(text: &str) -> bool {
    matches!(
        text,
        "," | ";" | "=" | "=>" | "+=" | "-=" | "*=" | "/=" | "~/=" | "%=" | "??="
    )
}

fn skip_dart_type(tokens: &[DartToken], mut pos: usize, to: usize) -> usize {
    let mut angle_depth = 0usize;
    while pos < to {
        let tok = &tokens[pos];
        if !step_dart_type_token(tok, &mut angle_depth) {
            break;
        }
        pos += 1;
    }
    pos
}

fn step_dart_type_token(tok: &DartToken, angle_depth: &mut usize) -> bool {
    let text = tok.text.as_str();
    if text == "<" {
        *angle_depth += 1;
        return true;
    }
    let closes = angle_close_count(text);
    if closes > 0 {
        *angle_depth = angle_depth.saturating_sub(closes);
        return true;
    }
    *angle_depth > 0 || tok.kind == DartKind::Ident || matches!(text, "." | "?")
}

fn find_dart_single_body_end(tokens: &[DartToken], start: usize, to: usize) -> usize {
    let mut depth = 0usize;
    let mut pos = start;
    while pos < to {
        if let Some(stop) = check_single_body_boundary(tokens, start, pos, &mut depth) {
            return stop;
        }
        pos += 1;
    }
    to
}

fn check_single_body_boundary(
    tokens: &[DartToken],
    start: usize,
    pos: usize,
    depth: &mut usize,
) -> Option<usize> {
    let tok = &tokens[pos];
    let text = tok.text.as_str();
    if matches!(text, "(" | "[" | "{") {
        *depth += 1;
        return None;
    }
    if *depth > 0 {
        return step_nested_close(tokens, pos, text, depth);
    }
    if text == ";" {
        return Some(pos + 1);
    }
    let is_stop = matches!(text, ")" | "]" | "}" | ",") || (pos > start && is_kw(tok, "else"));
    is_stop.then_some(pos)
}

fn step_nested_close(
    tokens: &[DartToken],
    pos: usize,
    text: &str,
    depth: &mut usize,
) -> Option<usize> {
    if !matches!(text, ")" | "]" | "}") {
        return None;
    }
    *depth -= 1;
    (*depth == 0 && text == "}" && !is_dart_continuation(tokens.get(pos + 1))).then_some(pos + 1)
}

fn is_dart_continuation(next: Option<&DartToken>) -> bool {
    next.is_some_and(|t| {
        t.kind == DartKind::Keyword
            && matches!(
                t.text.as_str(),
                "else" | "catch" | "on" | "finally" | "while"
            )
    })
}

fn find_dart_ternary_colon(tokens: &[DartToken], start: usize, to: usize) -> Option<usize> {
    let mut depth = 0usize;
    let mut nested = 0usize;
    let mut idx = start;
    while idx < to {
        let text = tokens[idx].text.as_str();
        if let Some(hit) = step_ternary_scan(tokens, idx, text, &mut depth, &mut nested) {
            return hit;
        }
        idx += 1;
    }
    None
}

fn step_ternary_scan(
    tokens: &[DartToken],
    idx: usize,
    text: &str,
    depth: &mut usize,
    nested: &mut usize,
) -> Option<Option<usize>> {
    if matches!(text, "(" | "[" | "{") {
        *depth += 1;
        return None;
    }
    if matches!(text, ")" | "]" | "}") {
        if *depth == 0 {
            return Some(None);
        }
        *depth -= 1;
        return None;
    }
    if *depth > 0 {
        return None;
    }
    if text == "?" && is_ternary_operator(tokens, idx) {
        *nested += 1;
    } else if text == ":" {
        if *nested == 0 {
            return Some(Some(idx));
        }
        *nested -= 1;
    } else if matches!(text, ";" | ",") {
        return Some(None);
    }
    None
}

fn find_dart_ternary_alt_end(tokens: &[DartToken], start: usize, to: usize) -> usize {
    let mut depth = 0usize;
    let mut idx = start;
    while idx < to {
        let text = tokens[idx].text.as_str();
        if matches!(text, "(" | "[" | "{") {
            depth += 1;
        } else if matches!(text, ")" | "]" | "}") {
            if depth == 0 {
                return idx;
            }
            depth -= 1;
        } else if depth == 0 && matches!(text, ";" | "," | "=>") {
            return idx;
        }
        idx += 1;
    }
    to
}
