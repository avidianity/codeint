use std::collections::{BTreeSet, HashMap};

use codeint_model::language::Function;
use codeint_model::scan::{find_arrow_expr_end, paren_then_brace, score_function_spans};

use crate::lexer::{RawKind, RawToken, closes_attribute, find_matching_delim};

struct PhpSpan {
    header_idx: usize,
    body_start: usize,
    body_end: usize,
    start_line: u32,
    end_line: u32,
    name: Option<String>,
    is_block: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum PhpBoolOp {
    And,
    Or,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Pending {
    None,
    CondAndBody,
    CatchHeader,
    DoBody,
    DoWhileCond,
    Body,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Frame {
    NestingBrace,
    PlainBrace,
    DoBrace,
    AltColon,
    SingleStmt,
    DoSingleStmt,
    CondParen { after_do: bool },
    CatchParen,
    CallParen,
    GroupParen,
    Bracket,
    Ternary { in_else: bool },
}

pub(crate) fn functions(tokens: &[RawToken], code_lines: &BTreeSet<u32>) -> Vec<Function> {
    let spans = collect_function_spans(tokens);
    let raw_spans: Vec<(usize, usize, u32, u32)> = spans
        .iter()
        .map(|s| (s.body_start, s.body_end, s.start_line, s.end_line))
        .collect();
    let mut scored = score_function_spans(tokens, code_lines, &raw_spans, decision_increment);
    let mut order: Vec<usize> = (0..spans.len()).collect();
    order.sort_by_key(|&i| (spans[i].start_line, spans[i].end_line));
    for (func, owner) in scored.iter_mut().zip(order) {
        func.name = spans[owner].name.clone();
        func.cognitive_complexity = score_php_cognitive(tokens, &spans, owner);
    }
    scored
}

fn collect_function_spans(tokens: &[RawToken]) -> Vec<PhpSpan> {
    let mut spans = Vec::new();
    for (i, token) in tokens.iter().enumerate() {
        if token.kind != RawKind::Keyword {
            continue;
        }
        if token.lower == "function"
            && let Some((name, start, end)) = find_function_body(tokens, i)
        {
            spans.push(PhpSpan {
                header_idx: i,
                body_start: start,
                body_end: end,
                start_line: token.line,
                end_line: tokens[end].line,
                name,
                is_block: true,
            });
        } else if token.lower == "fn"
            && let Some((start, end)) = find_arrow_fn_body(tokens, i)
        {
            spans.push(PhpSpan {
                header_idx: i,
                body_start: start,
                body_end: end,
                start_line: token.line,
                end_line: tokens[end].line,
                name: None,
                is_block: false,
            });
        }
    }
    spans
}

fn skip_fn_params(
    tokens: &[RawToken],
    fn_idx: usize,
    allow_name: bool,
) -> Option<(Option<String>, usize)> {
    let mut cursor = fn_idx + 1;
    if tokens.get(cursor).is_some_and(|t| t.text == "&") {
        cursor += 1;
    }
    let mut name = None;
    if allow_name
        && let Some(candidate) = tokens
            .get(cursor)
            .filter(|t| matches!(t.kind, RawKind::Ident | RawKind::Keyword))
    {
        name = Some(candidate.text.clone());
        cursor += 1;
    }
    if tokens.get(cursor).map(|t| t.text.as_str()) != Some("(") {
        return None;
    }
    Some((name, find_matching_delim(tokens, cursor)?))
}

fn find_function_body(
    tokens: &[RawToken],
    fn_idx: usize,
) -> Option<(Option<String>, usize, usize)> {
    let (name, close_paren) = skip_fn_params(tokens, fn_idx, true)?;
    let mut after = close_paren + 1;
    while after < tokens.len() {
        match tokens[after].text.as_str() {
            "{" => return Some((name, after, find_matching_delim(tokens, after)?)),
            ";" => return None,
            "(" => after = find_matching_delim(tokens, after)? + 1,
            _ => after += 1,
        }
    }
    None
}

fn find_arrow_fn_body(tokens: &[RawToken], fn_idx: usize) -> Option<(usize, usize)> {
    let (_, close_paren) = skip_fn_params(tokens, fn_idx, false)?;
    let mut after = close_paren + 1;
    while after < tokens.len() && tokens[after].text != "=>" {
        if matches!(tokens[after].text.as_str(), ";" | "{") {
            return None;
        }
        after += 1;
    }
    if after >= tokens.len() {
        return None;
    }
    let body_start = after + 1;
    Some((body_start, find_arrow_expr_end(tokens, body_start)))
}

fn decision_increment(tokens: &[RawToken], idx: usize) -> u32 {
    let token = &tokens[idx];
    if token.kind == RawKind::Keyword {
        return match token.lower.as_str() {
            "if" | "elseif" | "for" | "foreach" | "while" | "case" | "catch" | "and" | "or" => 1,
            "match" => count_match_arms(tokens, idx),
            _ => 0,
        };
    }
    match token.text.as_str() {
        "&&" | "||" | "??" => 1,
        "?" if is_ternary_question(tokens, idx) => 1,
        _ => 0,
    }
}

fn is_ternary_question(tokens: &[RawToken], idx: usize) -> bool {
    if tokens.get(idx + 1).is_some_and(|t| t.text == ":") {
        return true;
    }
    let Some(prev) = idx.checked_sub(1).and_then(|i| tokens.get(i)) else {
        return false;
    };
    if prev.text == "]" {
        return !closes_attribute(tokens, idx - 1);
    }
    if prev.text == ")" {
        return !is_asymmetric_visibility_close(tokens, idx - 1);
    }
    if prev.kind == RawKind::Keyword && prev.lower == "static" {
        return idx >= 2
            && tokens[idx - 2].kind == RawKind::Keyword
            && tokens[idx - 2].lower == "instanceof";
    }
    matches!(
        prev.kind,
        RawKind::Variable | RawKind::Ident | RawKind::NumberLit | RawKind::StringLit
    ) || matches!(prev.text.as_str(), "++" | "--")
        || matches!(
            prev.lower.as_str(),
            "true" | "false" | "null" | "self" | "parent"
        )
}

fn is_asymmetric_visibility_close(tokens: &[RawToken], close_idx: usize) -> bool {
    close_idx >= 3
        && tokens[close_idx - 1].lower == "set"
        && tokens[close_idx - 2].text == "("
        && tokens[close_idx - 3].kind == RawKind::Keyword
        && matches!(
            tokens[close_idx - 3].lower.as_str(),
            "public" | "protected" | "private"
        )
}

fn count_match_arms(tokens: &[RawToken], match_idx: usize) -> u32 {
    let Some((_, _, brace_open, close_brace)) = paren_then_brace(tokens, match_idx) else {
        return 0;
    };
    let mut arms = 0u32;
    let mut depth = 0usize;
    for token in &tokens[brace_open + 1..close_brace] {
        match token.text.as_str() {
            "(" | "[" | "{" | "#[" => depth += 1,
            ")" | "]" | "}" => depth = depth.saturating_sub(1),
            "=>" if depth == 0 => arms += 1,
            _ => {}
        }
    }
    arms.saturating_sub(1)
}

fn score_php_cognitive(tokens: &[RawToken], spans: &[PhpSpan], owner: usize) -> u32 {
    let span = &spans[owner];
    let (start, end) = if span.is_block {
        (span.body_start + 1, span.body_end)
    } else {
        (span.body_start, span.body_end + 1)
    };
    let mut child_ends: HashMap<usize, usize> = HashMap::new();
    for (idx, child) in spans.iter().enumerate() {
        if idx != owner && child.header_idx >= start && child.header_idx < end {
            child_ends
                .entry(child.header_idx)
                .and_modify(|e| *e = (*e).max(child.body_end))
                .or_insert(child.body_end);
        }
    }
    let mut machine = PhpStackMachine {
        tokens,
        child_ends,
        fn_lower: span.name.as_deref().map(str::to_ascii_lowercase),
        frames: Vec::new(),
        bool_ops: vec![None],
        nesting: 0,
        score: 0,
        pending: Pending::None,
        expect_do_while: false,
        counted_recursion: false,
    };
    machine.run(start, end);
    machine.score
}

struct PhpStackMachine<'a> {
    tokens: &'a [RawToken],
    child_ends: HashMap<usize, usize>,
    fn_lower: Option<String>,
    frames: Vec<Frame>,
    bool_ops: Vec<Option<PhpBoolOp>>,
    nesting: u32,
    score: u32,
    pending: Pending,
    expect_do_while: bool,
    counted_recursion: bool,
}

impl PhpStackMachine<'_> {
    fn run(&mut self, start: usize, end: usize) {
        let mut idx = start;
        while idx < end {
            if let Some(&child_end) = self.child_ends.get(&idx) {
                self.materialize_pending_single_stmt();
                idx = child_end + 1;
                self.clear_bool_op();
                continue;
            }
            idx = self.process_token(idx, start, end);
        }
    }

    fn process_token(&mut self, idx: usize, start: usize, end: usize) -> usize {
        let tok = &self.tokens[idx];
        if self.pending_expects_body() && !matches!(tok.text.as_str(), "{" | ":") {
            self.materialize_pending_single_stmt();
        }
        if tok.kind == RawKind::Keyword
            && !is_member_access(self.tokens, idx)
            && let Some(next) = self.handle_keyword(idx, end)
        {
            return next;
        }
        self.handle_symbol(idx, start, end);
        idx + 1
    }

    fn handle_keyword(&mut self, idx: usize, end: usize) -> Option<usize> {
        let kw = self.tokens[idx].lower.as_str();
        if kw == "while" && self.expect_do_while {
            self.expect_do_while = false;
            self.pending = Pending::DoWhileCond;
            self.clear_bool_op();
            return Some(idx + 1);
        }
        if let Some(next) = self.handle_branch_keyword(kw, idx, end) {
            return Some(next);
        }
        self.handle_aux_keyword(kw, idx)
    }

    fn handle_branch_keyword(&mut self, kw: &str, idx: usize, end: usize) -> Option<usize> {
        match kw {
            "if" | "for" | "foreach" | "while" | "switch" | "match" => {
                self.add_nested();
                self.pending = Pending::CondAndBody;
                self.clear_bool_op();
                Some(idx + 1)
            }
            "elseif" => {
                self.pop_alt_colon_if_top();
                self.add_flat(1);
                self.pending = Pending::CondAndBody;
                self.clear_bool_op();
                Some(idx + 1)
            }
            "else" => {
                self.pop_alt_colon_if_top();
                self.add_flat(1);
                self.clear_bool_op();
                if idx + 1 < end
                    && self.tokens[idx + 1].kind == RawKind::Keyword
                    && self.tokens[idx + 1].lower == "if"
                {
                    self.pending = Pending::CondAndBody;
                    Some(idx + 2)
                } else {
                    self.pending = Pending::Body;
                    Some(idx + 1)
                }
            }
            "do" => {
                self.add_nested();
                self.pending = Pending::DoBody;
                self.clear_bool_op();
                Some(idx + 1)
            }
            "catch" => {
                self.add_nested();
                self.pending = Pending::CatchHeader;
                self.clear_bool_op();
                Some(idx + 1)
            }
            _ => None,
        }
    }

    fn handle_aux_keyword(&mut self, kw: &str, idx: usize) -> Option<usize> {
        match kw {
            "and" => self.record_bool_op(PhpBoolOp::And),
            "or" => self.record_bool_op(PhpBoolOp::Or),
            "break" | "continue" => {
                if has_multilevel_jump_target(self.tokens, idx) {
                    self.add_flat(1);
                }
                self.clear_bool_op();
            }
            "goto" => {
                if self
                    .tokens
                    .get(idx + 1)
                    .is_some_and(|t| t.kind == RawKind::Ident)
                {
                    self.add_flat(1);
                }
                self.clear_bool_op();
            }
            "endif" | "endfor" | "endforeach" | "endwhile" | "endswitch" => {
                self.pop_alt_colon_if_top();
                self.clear_bool_op();
            }
            "try" | "finally" | "return" | "throw" | "yield" | "case" | "default" => {
                self.clear_bool_op();
            }
            _ => return None,
        }
        Some(idx + 1)
    }

    fn handle_symbol(&mut self, idx: usize, start: usize, end: usize) {
        let text = self.tokens[idx].text.as_str();
        if self.handle_delim_symbol(text, idx, start, end) {
            return;
        }
        match text {
            "&&" => self.record_bool_op(PhpBoolOp::And),
            "||" => self.record_bool_op(PhpBoolOp::Or),
            "?" if is_ternary_question(self.tokens, idx) => {
                self.add_nested();
                self.push_frame(Frame::Ternary { in_else: false });
                self.clear_bool_op();
            }
            ":" => self.handle_colon(),
            ";" => self.handle_semicolon(),
            "," | "=>" => {
                self.pop_else_ternaries();
                self.clear_bool_op();
            }
            "=" | "+=" | "-=" | "*=" | "??=" | "**=" => self.clear_bool_op(),
            _ => self.check_recursion(idx),
        }
    }

    fn handle_delim_symbol(&mut self, text: &str, idx: usize, start: usize, end: usize) -> bool {
        match text {
            "{" => self.handle_open_brace(),
            "}" => self.handle_close_brace(idx, end),
            "(" => self.handle_open_paren(idx, start),
            ")" => self.handle_close_paren(),
            "[" | "#[" => self.push_bracket(),
            "]" => self.pop_bracket(),
            _ => return false,
        }
        true
    }

    fn handle_colon(&mut self) {
        if self.pending_expects_body() {
            self.pending = Pending::None;
            self.push_frame(Frame::AltColon);
            self.clear_bool_op();
            return;
        }
        if let Some(Frame::Ternary { in_else }) = self.frames.last_mut()
            && !*in_else
        {
            *in_else = true;
            self.clear_bool_op();
        }
    }

    fn handle_open_brace(&mut self) {
        let frame = match self.pending {
            Pending::Body => Frame::NestingBrace,
            Pending::DoBody => Frame::DoBrace,
            _ => Frame::PlainBrace,
        };
        self.pending = Pending::None;
        self.push_frame(frame);
        self.bool_ops.push(None);
    }

    fn handle_close_brace(&mut self, idx: usize, end: usize) {
        self.pop_all_ternaries();
        let popped = self.pop_frame();
        if self.bool_ops.len() > 1 {
            self.bool_ops.pop();
        }
        if popped == Some(Frame::DoBrace) {
            self.expect_do_while = true;
        }
        if !has_continuation_after(self.tokens, idx + 1, end) {
            self.unwind_single_stmts();
        }
    }

    fn handle_open_paren(&mut self, idx: usize, start: usize) {
        let frame = match self.pending {
            Pending::CondAndBody => Frame::CondParen { after_do: false },
            Pending::DoWhileCond => Frame::CondParen { after_do: true },
            Pending::CatchHeader => Frame::CatchParen,
            _ if is_php_call_paren(self.tokens, idx, start) => Frame::CallParen,
            _ => Frame::GroupParen,
        };
        self.pending = Pending::None;
        self.frames.push(frame);
        if frame != Frame::GroupParen {
            self.bool_ops.push(None);
        }
    }

    fn handle_close_paren(&mut self) {
        self.pop_all_ternaries();
        let Some(frame) = self.frames.pop() else {
            return;
        };
        if frame != Frame::GroupParen && self.bool_ops.len() > 1 {
            self.bool_ops.pop();
        }
        match frame {
            Frame::CondParen { after_do: false } | Frame::CatchParen => {
                self.pending = Pending::Body;
            }
            _ => {}
        }
    }

    fn push_bracket(&mut self) {
        self.frames.push(Frame::Bracket);
        self.bool_ops.push(None);
    }

    fn pop_bracket(&mut self) {
        self.pop_all_ternaries();
        if self.frames.last() == Some(&Frame::Bracket) {
            self.frames.pop();
            if self.bool_ops.len() > 1 {
                self.bool_ops.pop();
            }
        }
    }

    fn handle_semicolon(&mut self) {
        self.pop_else_ternaries();
        self.clear_bool_op();
        self.unwind_single_stmts();
    }

    fn unwind_single_stmts(&mut self) {
        while matches!(
            self.frames.last(),
            Some(Frame::SingleStmt | Frame::DoSingleStmt)
        ) {
            if self.pop_frame() == Some(Frame::DoSingleStmt) {
                self.expect_do_while = true;
                break;
            }
        }
    }

    fn pending_expects_body(&self) -> bool {
        matches!(self.pending, Pending::Body | Pending::DoBody)
    }

    fn materialize_pending_single_stmt(&mut self) {
        let frame = match self.pending {
            Pending::Body => Frame::SingleStmt,
            Pending::DoBody => Frame::DoSingleStmt,
            _ => return,
        };
        self.pending = Pending::None;
        self.push_frame(frame);
    }

    fn push_frame(&mut self, frame: Frame) {
        if frame_adds_nesting(frame) {
            self.nesting = self.nesting.saturating_add(1);
        }
        self.frames.push(frame);
    }

    fn pop_frame(&mut self) -> Option<Frame> {
        let frame = self.frames.pop()?;
        if frame_adds_nesting(frame) {
            self.nesting = self.nesting.saturating_sub(1);
        }
        Some(frame)
    }

    fn pop_alt_colon_if_top(&mut self) {
        if self.frames.last() == Some(&Frame::AltColon) {
            self.pop_frame();
        }
    }

    fn pop_else_ternaries(&mut self) {
        while self.frames.last() == Some(&Frame::Ternary { in_else: true }) {
            self.pop_frame();
        }
    }

    fn pop_all_ternaries(&mut self) {
        while matches!(self.frames.last(), Some(Frame::Ternary { .. })) {
            self.pop_frame();
        }
    }

    fn add_nested(&mut self) {
        self.score = self.score.saturating_add(self.nesting.saturating_add(1));
    }

    fn add_flat(&mut self, amount: u32) {
        self.score = self.score.saturating_add(amount);
    }

    fn clear_bool_op(&mut self) {
        if let Some(top) = self.bool_ops.last_mut() {
            *top = None;
        }
    }

    fn record_bool_op(&mut self, op: PhpBoolOp) {
        let Some(top) = self.bool_ops.last_mut() else {
            return;
        };
        if *top != Some(op) {
            *top = Some(op);
            self.add_flat(1);
        }
    }

    fn check_recursion(&mut self, idx: usize) {
        if self.counted_recursion {
            return;
        }
        if is_php_recursive_call(self.tokens, idx, self.fn_lower.as_deref()) {
            self.counted_recursion = true;
            self.add_flat(1);
        }
    }
}

fn frame_adds_nesting(frame: Frame) -> bool {
    matches!(
        frame,
        Frame::NestingBrace
            | Frame::DoBrace
            | Frame::AltColon
            | Frame::SingleStmt
            | Frame::DoSingleStmt
            | Frame::Ternary { .. }
    )
}

fn is_member_access(tokens: &[RawToken], idx: usize) -> bool {
    idx > 0 && matches!(tokens[idx - 1].text.as_str(), "->" | "?->" | "::")
}

fn is_php_call_paren(tokens: &[RawToken], idx: usize, start: usize) -> bool {
    if idx <= start {
        return false;
    }
    let prev = &tokens[idx - 1];
    matches!(prev.kind, RawKind::Ident | RawKind::Variable)
        || matches!(prev.text.as_str(), ")" | "]")
}

fn has_multilevel_jump_target(tokens: &[RawToken], idx: usize) -> bool {
    tokens
        .get(idx + 1)
        .filter(|t| t.kind == RawKind::NumberLit)
        .and_then(|t| t.text.parse::<u32>().ok())
        .is_some_and(|level| level > 1)
}

fn has_continuation_after(tokens: &[RawToken], pos: usize, end: usize) -> bool {
    if pos >= end {
        return false;
    }
    let tok = &tokens[pos];
    tok.kind == RawKind::Keyword
        && matches!(
            tok.lower.as_str(),
            "else" | "elseif" | "catch" | "finally" | "while"
        )
}

fn is_php_recursive_call(tokens: &[RawToken], idx: usize, target: Option<&str>) -> bool {
    let Some(expected) = target else {
        return false;
    };
    let tok = &tokens[idx];
    if !matches!(tok.kind, RawKind::Ident | RawKind::Keyword) || tok.lower != expected {
        return false;
    }
    if !tokens.get(idx + 1).is_some_and(|t| t.text == "(") {
        return false;
    }
    has_php_self_receiver(tokens, idx)
}

fn has_php_self_receiver(tokens: &[RawToken], idx: usize) -> bool {
    let Some(prev) = idx.checked_sub(1).and_then(|p| tokens.get(p)) else {
        return true;
    };
    match prev.text.as_str() {
        "->" | "?->" => idx >= 2 && tokens[idx - 2].text == "$this",
        "::" => {
            idx >= 2
                && tokens[idx - 2].kind == RawKind::Keyword
                && matches!(tokens[idx - 2].lower.as_str(), "self" | "static")
        }
        _ => !(prev.kind == RawKind::Keyword && matches!(prev.lower.as_str(), "function" | "new")),
    }
}
