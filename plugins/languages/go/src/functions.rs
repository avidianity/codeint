use std::collections::BTreeSet;

use codeint_model::language::Function;
use codeint_model::scan::find_matching_delim;

use crate::lexer::{GoKind, GoToken};

#[derive(Debug, Clone)]
pub(crate) struct GoFunctionSpan {
    pub(crate) body_open: usize,
    pub(crate) body_close: usize,
    pub(crate) start_line: u32,
    pub(crate) end_line: u32,
    pub(crate) name: String,
    pub(crate) recv_var: Option<String>,
    pub(crate) method_name: Option<String>,
    pub(crate) has_results: bool,
    pub(crate) has_named_results: bool,
}

pub(crate) struct FuncLiteralHeader {
    pub(crate) body_open: usize,
    pub(crate) body_close: usize,
    pub(crate) has_results: bool,
    pub(crate) has_named_results: bool,
}

pub(crate) struct ExtractedFunctions {
    pub(crate) functions: Vec<Function>,
    pub(crate) spans: Vec<GoFunctionSpan>,
}

#[derive(Default)]
struct DelimDepths {
    brace: usize,
    paren: usize,
    bracket: usize,
}

impl DelimDepths {
    fn at_top_level(&self) -> bool {
        self.brace == 0 && self.paren == 0 && self.bracket == 0
    }

    fn advance(&mut self, text: &str) {
        match text {
            "{" => self.brace += 1,
            "}" => self.brace = self.brace.saturating_sub(1),
            "(" => self.paren += 1,
            ")" => self.paren = self.paren.saturating_sub(1),
            "[" => self.bracket += 1,
            "]" => self.bracket = self.bracket.saturating_sub(1),
            _ => {}
        }
    }
}

pub(crate) fn extract(tokens: &[GoToken], code_lines: &BTreeSet<u32>) -> ExtractedFunctions {
    let spans = collect_top_level_functions(tokens);
    let functions = spans
        .iter()
        .map(|span| score_function(tokens, code_lines, span))
        .collect();
    ExtractedFunctions { functions, spans }
}

fn collect_top_level_functions(tokens: &[GoToken]) -> Vec<GoFunctionSpan> {
    let mut spans = Vec::new();
    let mut depths = DelimDepths::default();
    let mut idx = 0usize;

    while idx < tokens.len() {
        if depths.at_top_level()
            && tokens[idx].is_kw("func")
            && is_top_level_decl_start(tokens, idx)
            && let Some(span) = parse_top_level_func(tokens, idx)
        {
            idx = span.body_close + 1;
            spans.push(span);
            continue;
        }
        depths.advance(tokens[idx].text.as_str());
        idx += 1;
    }

    spans
}

fn is_top_level_decl_start(tokens: &[GoToken], idx: usize) -> bool {
    idx == 0 || tokens[idx - 1].is_semi() || tokens[idx - 1].text == "}"
}

fn matching_within(tokens: &[GoToken], open: usize, limit: usize) -> Option<usize> {
    find_matching_delim(tokens, open).filter(|&close| close < limit)
}

fn parse_top_level_func(tokens: &[GoToken], func_idx: usize) -> Option<GoFunctionSpan> {
    let (recv_var, recv_type, pos) = parse_optional_receiver(tokens, func_idx + 1)?;
    let (base_name, after_params) = parse_name_and_params(tokens, pos)?;
    let (after_result, has_results, has_named_results) =
        parse_result_clause(tokens, after_params, tokens.len())?;
    if !tokens.get(after_result).is_some_and(|t| t.text == "{") {
        return None;
    }
    let body_close = find_matching_delim(tokens, after_result)?;
    let (name, method_name) = match recv_type {
        Some(recv) => (format!("{recv}.{base_name}"), Some(base_name)),
        None => (base_name, None),
    };
    Some(GoFunctionSpan {
        body_open: after_result,
        body_close,
        start_line: tokens[func_idx].line,
        end_line: tokens[body_close].line,
        name,
        recv_var,
        method_name,
        has_results,
        has_named_results,
    })
}

fn parse_optional_receiver(
    tokens: &[GoToken],
    pos: usize,
) -> Option<(Option<String>, Option<String>, usize)> {
    if !tokens.get(pos).is_some_and(|t| t.text == "(") {
        return Some((None, None, pos));
    }
    let close = find_matching_delim(tokens, pos)?;
    let (recv_var, recv_type) = parse_receiver(&tokens[pos + 1..close]);
    Some((recv_var, recv_type, close + 1))
}

fn parse_name_and_params(tokens: &[GoToken], mut pos: usize) -> Option<(String, usize)> {
    let name_tok = tokens.get(pos)?;
    if name_tok.kind != GoKind::Ident {
        return None;
    }
    let base_name = name_tok.text.clone();
    pos += 1;
    if tokens.get(pos).is_some_and(|t| t.text == "[") {
        pos = find_matching_delim(tokens, pos)? + 1;
    }
    if !tokens.get(pos).is_some_and(|t| t.text == "(") {
        return None;
    }
    let params_close = find_matching_delim(tokens, pos)?;
    Some((base_name, params_close + 1))
}

fn parse_receiver(inner: &[GoToken]) -> (Option<String>, Option<String>) {
    let mut idx = 0usize;
    let mut var_name = None;
    if inner.first().is_some_and(|t| t.kind == GoKind::Ident)
        && inner
            .get(1)
            .is_some_and(|t| matches!(t.text.as_str(), "*" | "(") || t.kind == GoKind::Ident)
    {
        if inner[0].text != "_" {
            var_name = Some(inner[0].text.clone());
        }
        idx = 1;
    }
    while inner
        .get(idx)
        .is_some_and(|t| matches!(t.text.as_str(), "*" | "("))
    {
        idx += 1;
    }
    let recv_type = inner
        .get(idx)
        .filter(|t| t.kind == GoKind::Ident)
        .map(|t| t.text.clone());
    (var_name, recv_type)
}

pub(crate) fn is_composite_func_type(tokens: &[GoToken], func_idx: usize) -> bool {
    if !tokens.get(func_idx).is_some_and(|t| t.is_kw("func")) {
        return false;
    }
    let mut prev = func_idx;
    while prev > 0 && tokens[prev - 1].text == "*" {
        prev -= 1;
    }
    if prev == 0 || tokens[prev - 1].text != "]" {
        return false;
    }
    if prev == func_idx {
        return true;
    }
    is_bracket_type_prefix(tokens, prev - 1)
}

fn is_bracket_type_prefix(tokens: &[GoToken], mut close_bracket: usize) -> bool {
    loop {
        let Some(open_bracket) = find_opening_bracket(tokens, close_bracket) else {
            return false;
        };
        if open_bracket + 1 == close_bracket {
            return true;
        }
        let Some(before_idx) = open_bracket.checked_sub(1) else {
            return true;
        };
        let before = &tokens[before_idx];
        if before.is_kw("map") {
            return true;
        }
        if before.text == "]" {
            close_bracket = before_idx;
            continue;
        }
        let is_expr_end = matches!(
            before.kind,
            GoKind::Ident | GoKind::NumberLit | GoKind::StringLit | GoKind::RuneLit
        ) || matches!(before.text.as_str(), ")" | "}");
        return !is_expr_end;
    }
}

fn find_opening_bracket(tokens: &[GoToken], close_bracket: usize) -> Option<usize> {
    let mut depth = 0usize;
    let mut idx = close_bracket;
    loop {
        match tokens[idx].text.as_str() {
            "]" => depth += 1,
            "[" => {
                depth -= 1;
                if depth == 0 {
                    return Some(idx);
                }
            }
            _ => {}
        }
        idx = idx.checked_sub(1)?;
    }
}

fn is_func_literal_start(tokens: &[GoToken], func_idx: usize, limit: usize) -> bool {
    let is_func = tokens.get(func_idx).is_some_and(|t| t.is_kw("func"));
    let has_open_paren = tokens
        .get(func_idx + 1)
        .is_some_and(|t| func_idx + 1 < limit && t.text == "(");
    is_func && !is_composite_func_type(tokens, func_idx) && has_open_paren
}

pub(crate) fn parse_func_literal_header(
    tokens: &[GoToken],
    func_idx: usize,
    limit: usize,
) -> Option<FuncLiteralHeader> {
    if !is_func_literal_start(tokens, func_idx, limit) {
        return None;
    }
    let close_paren = matching_within(tokens, func_idx + 1, limit)?;
    let (body_open, has_results, has_named_results) =
        parse_result_clause(tokens, close_paren + 1, limit)?;
    if body_open >= limit || tokens[body_open].text != "{" {
        return None;
    }
    let body_close = matching_within(tokens, body_open, limit)?;
    Some(FuncLiteralHeader {
        body_open,
        body_close,
        has_results,
        has_named_results,
    })
}

fn parse_result_clause(
    tokens: &[GoToken],
    pos: usize,
    limit: usize,
) -> Option<(usize, bool, bool)> {
    if pos >= limit {
        return Some((pos, false, false));
    }
    let tok = &tokens[pos];
    if tok.text == "{" || tok.is_semi() {
        return Some((pos, false, false));
    }
    if tok.text == "(" {
        let close = matching_within(tokens, pos, limit)?;
        let inner = &tokens[pos + 1..close];
        let has_results = !inner.is_empty();
        let has_named = has_results && result_list_is_named(inner);
        return Some((close + 1, has_results, has_named));
    }
    if starts_go_type(tokens, pos, limit) {
        let after = skip_go_type(tokens, pos, limit)?;
        return Some((after, true, false));
    }
    Some((pos, false, false))
}

fn result_list_is_named(inner: &[GoToken]) -> bool {
    let mut items: Vec<&[GoToken]> = Vec::new();
    let mut depth = 0usize;
    let mut start = 0usize;
    for (idx, tok) in inner.iter().enumerate() {
        match tok.text.as_str() {
            "(" | "[" | "{" => depth += 1,
            ")" | "]" | "}" => depth = depth.saturating_sub(1),
            "," if depth == 0 => {
                if start < idx {
                    items.push(&inner[start..idx]);
                }
                start = idx + 1;
            }
            _ => {}
        }
    }
    if start < inner.len() {
        items.push(&inner[start..]);
    }
    items.into_iter().any(result_item_is_named)
}

fn result_item_is_named(item: &[GoToken]) -> bool {
    if item.len() < 2 || item[0].kind != GoKind::Ident {
        return false;
    }
    match item[1].text.as_str() {
        "." => false,
        "[" => find_matching_delim(item, 1).is_some_and(|close| close + 1 < item.len()),
        _ => true,
    }
}

pub(crate) fn starts_go_type(tokens: &[GoToken], pos: usize, limit: usize) -> bool {
    if pos >= limit {
        return false;
    }
    let tok = &tokens[pos];
    matches!(
        (tok.kind, tok.text.as_str()),
        (GoKind::Ident, _)
            | (
                GoKind::Keyword,
                "chan" | "func" | "interface" | "map" | "struct"
            )
            | (GoKind::Punct, "*" | "[" | "(")
    ) || (tok.text == "<-" && tokens.get(pos + 1).is_some_and(|t| t.is_kw("chan")))
}

pub(crate) fn skip_go_type(tokens: &[GoToken], mut pos: usize, limit: usize) -> Option<usize> {
    while let Some(next) = step_type_prefix(tokens, pos, limit)? {
        pos = next;
    }
    skip_type_atom(tokens, pos, limit)
}

fn step_type_prefix(tokens: &[GoToken], pos: usize, limit: usize) -> Option<Option<usize>> {
    if pos >= limit {
        return None;
    }
    let tok = &tokens[pos];
    match tok.text.as_str() {
        "*" => Some(Some(pos + 1)),
        "<-" | "chan" => Some(step_chan_prefix(tokens, pos)),
        "[" => Some(Some(matching_within(tokens, pos, limit)? + 1)),
        "map" if tok.kind == GoKind::Keyword => skip_map_prefix(tokens, pos, limit).map(Some),
        _ => Some(None),
    }
}

fn step_chan_prefix(tokens: &[GoToken], pos: usize) -> Option<usize> {
    let tok = &tokens[pos];
    if tok.text == "<-" {
        return tokens
            .get(pos + 1)
            .is_some_and(|t| t.is_kw("chan"))
            .then_some(pos + 2);
    }
    if tok.is_kw("chan") {
        let arrow = usize::from(tokens.get(pos + 1).is_some_and(|t| t.text == "<-"));
        return Some(pos + 1 + arrow);
    }
    None
}

fn skip_map_prefix(tokens: &[GoToken], pos: usize, limit: usize) -> Option<usize> {
    if pos + 1 >= limit || tokens[pos + 1].text != "[" {
        return None;
    }
    Some(matching_within(tokens, pos + 1, limit)? + 1)
}

fn skip_type_atom(tokens: &[GoToken], pos: usize, limit: usize) -> Option<usize> {
    if pos >= limit {
        return None;
    }
    let tok = &tokens[pos];
    match (tok.kind, tok.text.as_str()) {
        (GoKind::Keyword, "struct" | "interface") => {
            if pos + 1 >= limit || tokens[pos + 1].text != "{" {
                return None;
            }
            Some(matching_within(tokens, pos + 1, limit)? + 1)
        }
        (GoKind::Keyword, "func") => skip_func_type_after_kw(tokens, pos + 1, limit),
        (GoKind::Punct, "(") => Some(matching_within(tokens, pos, limit)? + 1),
        (GoKind::Ident, _) => Some(skip_named_type_tail(tokens, pos + 1, limit)),
        _ => None,
    }
}

fn skip_named_type_tail(tokens: &[GoToken], mut pos: usize, limit: usize) -> usize {
    if pos + 1 < limit && tokens[pos].text == "." && tokens[pos + 1].kind == GoKind::Ident {
        pos += 2;
    }
    if pos < limit
        && tokens[pos].text == "["
        && let Some(close) = matching_within(tokens, pos, limit)
    {
        pos = close + 1;
    }
    pos
}

fn skip_func_type_after_kw(tokens: &[GoToken], pos: usize, limit: usize) -> Option<usize> {
    if pos >= limit || tokens[pos].text != "(" {
        return None;
    }
    let close = matching_within(tokens, pos, limit)?;
    let (after, _, _) = parse_result_clause(tokens, close + 1, limit)?;
    Some(after)
}

enum HeaderStep {
    Advance(usize),
    Body(usize, usize),
    Stop,
}

pub(crate) fn find_control_body(
    tokens: &[GoToken],
    header_start: usize,
    limit: usize,
) -> Option<(usize, usize)> {
    let mut pos = header_start;
    while pos < limit {
        match step_control_header(tokens, pos, limit) {
            HeaderStep::Advance(next) => pos = next,
            HeaderStep::Body(open, close) => return Some((open, close)),
            HeaderStep::Stop => return None,
        }
    }
    None
}

fn step_control_header(tokens: &[GoToken], pos: usize, limit: usize) -> HeaderStep {
    let tok = &tokens[pos];
    if tok.kind == GoKind::AutoSemi {
        return HeaderStep::Stop;
    }
    if matches!(tok.text.as_str(), "(" | "[") {
        return matching_within(tokens, pos, limit)
            .map_or(HeaderStep::Stop, |close| HeaderStep::Advance(close + 1));
    }
    if tok.is_kw("struct") || tok.is_kw("interface") {
        let next = skip_type_brace_in_header(tokens, pos, limit).unwrap_or(pos + 1);
        return HeaderStep::Advance(next);
    }
    if tok.is_kw("func") {
        let next = skip_func_in_header(tokens, pos, limit).unwrap_or(pos + 1);
        return HeaderStep::Advance(next);
    }
    if tok.text == "{" {
        return classify_header_brace(tokens, pos, limit);
    }
    HeaderStep::Advance(pos + 1)
}

fn skip_func_in_header(tokens: &[GoToken], pos: usize, limit: usize) -> Option<usize> {
    if let Some(lit) = parse_func_literal_header(tokens, pos, limit) {
        return Some(lit.body_close + 1);
    }
    skip_go_type(tokens, pos, limit)
}

fn classify_header_brace(tokens: &[GoToken], pos: usize, limit: usize) -> HeaderStep {
    let Some(close) = matching_within(tokens, pos, limit) else {
        return HeaderStep::Stop;
    };
    if is_header_composite_literal(tokens, close, limit) {
        HeaderStep::Advance(close + 1)
    } else {
        HeaderStep::Body(pos, close)
    }
}

fn skip_type_brace_in_header(tokens: &[GoToken], pos: usize, limit: usize) -> Option<usize> {
    if pos + 1 >= limit || tokens[pos + 1].text != "{" {
        return None;
    }
    let close = matching_within(tokens, pos + 1, limit)?;
    let mut next = close + 1;
    if next < limit
        && tokens[next].text == "{"
        && let Some(lit_close) = matching_within(tokens, next, limit)
    {
        next = lit_close + 1;
    }
    Some(next)
}

fn is_header_composite_literal(tokens: &[GoToken], close: usize, limit: usize) -> bool {
    let Some(after) = tokens.get(close + 1).filter(|_| close + 1 < limit) else {
        return false;
    };
    if matches!(
        after.text.as_str(),
        "{" | "."
            | "["
            | "("
            | ","
            | "=="
            | "!="
            | "<"
            | ">"
            | "<="
            | ">="
            | "&&"
            | "||"
            | "+"
            | "-"
            | "*"
            | "/"
            | "%"
            | "&"
            | "|"
            | "^"
            | "<<"
            | ">>"
            | "&^"
            | "<-"
    ) {
        return true;
    }
    after.kind == GoKind::Punct
        && after.text == ";"
        && has_brace_before_auto_semi(tokens, close + 2, limit)
}

fn has_brace_before_auto_semi(tokens: &[GoToken], start: usize, limit: usize) -> bool {
    let mut pos = start;
    while pos < limit {
        let tok = &tokens[pos];
        if tok.kind == GoKind::AutoSemi || tok.text == "}" {
            return false;
        }
        if matches!(tok.text.as_str(), "(" | "[") {
            let Some(close) = find_matching_delim(tokens, pos) else {
                return false;
            };
            pos = close + 1;
            continue;
        }
        if tok.text == "{" {
            return true;
        }
        pos += 1;
    }
    false
}

fn score_function(
    tokens: &[GoToken],
    code_lines: &BTreeSet<u32>,
    span: &GoFunctionSpan,
) -> Function {
    let decision_points = count_decision_points(tokens, span.body_open + 1, span.body_close);
    let cognitive_complexity = compute_cognitive(tokens, span);
    let source_lines = code_lines.range(span.start_line..=span.end_line).count() as u32;
    Function {
        line: span.start_line,
        end_line: span.end_line,
        decision_points,
        source_lines,
        name: Some(span.name.clone()),
        cognitive_complexity,
        coverage: None,
    }
}

fn count_decision_points(tokens: &[GoToken], start: usize, end: usize) -> u32 {
    tokens[start..end]
        .iter()
        .map(|tok| match (tok.kind, tok.text.as_str()) {
            (GoKind::Keyword, "if" | "for" | "case") => 1u32,
            (GoKind::Punct, "&&" | "||") => 1u32,
            _ => 0u32,
        })
        .sum()
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum LogicalOp {
    And,
    Or,
}

#[derive(Default)]
struct LogicalRunStack {
    current: Option<LogicalOp>,
    frames: Vec<Option<Option<LogicalOp>>>,
}

impl LogicalRunStack {
    fn reset(&mut self) {
        self.current = None;
    }

    fn observe(&mut self, op: LogicalOp) -> u32 {
        let incremented = u32::from(self.current != Some(op));
        self.current = Some(op);
        incremented
    }

    fn push_scope(&mut self, isolate: bool) {
        let frame = isolate.then(|| self.current.take());
        self.frames.push(frame);
    }

    fn pop_scope(&mut self) {
        if let Some(Some(restored)) = self.frames.pop() {
            self.current = restored;
        }
    }
}

struct CognitiveScorer<'a> {
    tokens: &'a [GoToken],
    span: &'a GoFunctionSpan,
    score: u32,
    recursion_counted: bool,
}

fn compute_cognitive(tokens: &[GoToken], span: &GoFunctionSpan) -> u32 {
    let mut scorer = CognitiveScorer {
        tokens,
        span,
        score: 0,
        recursion_counted: false,
    };
    if span.body_open + 1 < span.body_close {
        scorer.walk_range(span.body_open + 1, span.body_close, 0);
    }
    scorer.score
}

impl CognitiveScorer<'_> {
    fn walk_range(&mut self, start: usize, end: usize, depth: u32) {
        let mut runs = LogicalRunStack::default();
        let mut cursor = start;
        while cursor < end {
            cursor = self.visit_token(cursor, start, end, depth, &mut runs);
        }
    }

    fn visit_token(
        &mut self,
        cursor: usize,
        start: usize,
        end: usize,
        depth: u32,
        runs: &mut LogicalRunStack,
    ) -> usize {
        if self.tokens[cursor].kind == GoKind::Keyword
            && let Some(next) = self.visit_keyword(cursor, end, depth, runs)
        {
            return next;
        }
        self.visit_non_keyword(cursor, start, runs);
        cursor + 1
    }

    fn visit_keyword(
        &mut self,
        cursor: usize,
        end: usize,
        depth: u32,
        runs: &mut LogicalRunStack,
    ) -> Option<usize> {
        let kw = self.tokens[cursor].text.as_str();
        match kw {
            "if" | "for" | "switch" => {
                self.add_nested(depth);
                runs.reset();
                Some(self.visit_control_with_header(cursor + 1, end, depth))
            }
            "select" => {
                self.add_nested(depth);
                runs.reset();
                Some(self.visit_braced_body(cursor + 1, end, depth))
            }
            "else" => Some(self.visit_else(cursor, end, depth, runs)),
            "func" => self.visit_func_literal(cursor, end, depth, runs),
            "goto" => {
                self.add_flat(1);
                runs.reset();
                Some(cursor + 1)
            }
            "break" | "continue" => {
                if self
                    .tokens
                    .get(cursor + 1)
                    .filter(|_| cursor + 1 < end)
                    .is_some_and(|t| t.kind == GoKind::Ident)
                {
                    self.add_flat(1);
                }
                runs.reset();
                Some(cursor + 1)
            }
            "return" | "case" | "default" | "defer" | "go" | "fallthrough" => {
                runs.reset();
                Some(cursor + 1)
            }
            _ => None,
        }
    }

    fn add_nested(&mut self, depth: u32) {
        self.score = self.score.saturating_add(depth.saturating_add(1));
    }

    fn add_flat(&mut self, delta: u32) {
        self.score = self.score.saturating_add(delta);
    }

    fn visit_else(
        &mut self,
        else_idx: usize,
        end: usize,
        depth: u32,
        runs: &mut LogicalRunStack,
    ) -> usize {
        self.add_flat(1);
        runs.reset();
        if self.tokens.get(else_idx + 1).is_some_and(|t| t.is_kw("if")) {
            self.visit_control_with_header(else_idx + 2, end, depth)
        } else {
            self.visit_braced_body(else_idx + 1, end, depth)
        }
    }

    fn visit_control_with_header(&mut self, header_start: usize, end: usize, depth: u32) -> usize {
        let Some((open, close)) = find_control_body(self.tokens, header_start, end) else {
            return header_start;
        };
        if header_start < open {
            self.walk_range(header_start, open, depth);
        }
        if open + 1 < close {
            self.walk_range(open + 1, close, depth + 1);
        }
        close + 1
    }

    fn visit_braced_body(&mut self, pos: usize, end: usize, depth: u32) -> usize {
        if pos < end
            && self.tokens[pos].text == "{"
            && let Some(close) = matching_within(self.tokens, pos, end)
        {
            if pos + 1 < close {
                self.walk_range(pos + 1, close, depth + 1);
            }
            return close + 1;
        }
        pos
    }

    fn visit_func_literal(
        &mut self,
        cursor: usize,
        end: usize,
        depth: u32,
        runs: &mut LogicalRunStack,
    ) -> Option<usize> {
        if is_composite_func_type(self.tokens, cursor) {
            return skip_go_type(self.tokens, cursor, end);
        }
        let lit = parse_func_literal_header(self.tokens, cursor, end)?;
        runs.reset();
        if lit.body_open + 1 < lit.body_close {
            self.walk_range(lit.body_open + 1, lit.body_close, depth + 1);
        }
        Some(lit.body_close + 1)
    }

    fn visit_non_keyword(&mut self, cursor: usize, start: usize, runs: &mut LogicalRunStack) {
        let tok = &self.tokens[cursor];
        match tok.text.as_str() {
            "&&" => self.add_flat(runs.observe(LogicalOp::And)),
            "||" => self.add_flat(runs.observe(LogicalOp::Or)),
            "(" => runs.push_scope(is_call_open(self.tokens, cursor, start)),
            "[" | "{" => runs.push_scope(true),
            ")" | "]" | "}" => runs.pop_scope(),
            ";" | "," | ":" | ":=" | "=" | "<-" => runs.reset(),
            _ if tok.kind == GoKind::Ident => self.check_recursion(cursor),
            _ => {}
        }
    }

    fn check_recursion(&mut self, cursor: usize) {
        if self.recursion_counted || !is_call_site(self.tokens, cursor) {
            return;
        }
        let matched = match (
            self.span.recv_var.as_deref(),
            self.span.method_name.as_deref(),
        ) {
            (Some(recv), Some(method)) => {
                matches_method_self_call(self.tokens, cursor, recv, method)
            }
            (_, None) => matches_fn_self_call(self.tokens, cursor, &self.span.name),
            _ => false,
        };
        if matched {
            self.recursion_counted = true;
            self.add_flat(1);
        }
    }
}

fn matches_method_self_call(tokens: &[GoToken], cursor: usize, recv: &str, method: &str) -> bool {
    tokens[cursor].text == method
        && cursor >= 2
        && tokens[cursor - 1].text == "."
        && tokens[cursor - 2].kind == GoKind::Ident
        && tokens[cursor - 2].text == recv
}

fn matches_fn_self_call(tokens: &[GoToken], cursor: usize, name: &str) -> bool {
    tokens[cursor].text == name && (cursor == 0 || tokens[cursor - 1].text != ".")
}

fn is_call_open(tokens: &[GoToken], cursor: usize, start: usize) -> bool {
    cursor > start
        && tokens
            .get(cursor - 1)
            .is_some_and(|t| t.kind == GoKind::Ident || matches!(t.text.as_str(), ")" | "]"))
}

fn is_call_site(tokens: &[GoToken], ident_idx: usize) -> bool {
    let Some(next) = tokens.get(ident_idx + 1) else {
        return false;
    };
    if next.text == "(" {
        return true;
    }
    next.text == "["
        && find_matching_delim(tokens, ident_idx + 1)
            .is_some_and(|close| tokens.get(close + 1).is_some_and(|t| t.text == "("))
}
