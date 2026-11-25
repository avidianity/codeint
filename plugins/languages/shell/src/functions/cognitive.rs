use crate::lexer::{ShellToken, ShellTokenKind};

use super::SpanIndex;

#[derive(Clone, Copy, PartialEq, Eq)]
enum LogicalOp {
    And,
    Or,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum NestKind {
    If,
    Loop,
    Case,
}

struct CognitiveScorer<'a> {
    tokens: &'a [ShellToken],
    index: &'a SpanIndex,
    fn_name: Option<&'a str>,
    score: u32,
    nesting: u32,
    nest_stack: Vec<(NestKind, bool)>,
    current_logical: Option<LogicalOp>,
    counted_recursion: bool,
}

pub(super) fn score_range(
    tokens: &[ShellToken],
    start: usize,
    end: usize,
    index: &SpanIndex,
    fn_name: Option<&str>,
) -> u32 {
    let mut scorer = CognitiveScorer {
        tokens,
        index,
        fn_name,
        score: 0,
        nesting: 0,
        nest_stack: Vec::new(),
        current_logical: None,
        counted_recursion: false,
    };
    scorer.walk(start, end);
    scorer.score
}

impl CognitiveScorer<'_> {
    fn walk(&mut self, start: usize, end: usize) {
        let mut at_cmd_pos = true;
        let mut idx = start;
        while idx < end {
            if let Some(skip_end) = self.index.skip_end(idx) {
                idx = skip_end + 1;
                at_cmd_pos = true;
                self.current_logical = None;
                continue;
            }
            self.visit_token(idx, end, at_cmd_pos);
            at_cmd_pos = self.tokens[idx].next_is_cmd_pos();
            idx += 1;
        }
    }

    fn visit_token(&mut self, idx: usize, end: usize, at_cmd_pos: bool) {
        let tok = &self.tokens[idx];
        if tok.kind == ShellTokenKind::ArithCmd {
            self.score_arith_logical(&tok.text);
            return;
        }
        if tok.is_op("&&") {
            self.observe_logical(LogicalOp::And);
            return;
        }
        if tok.is_op("||") {
            self.observe_logical(LogicalOp::Or);
            return;
        }
        if tok.is_newline() || tok.kind == ShellTokenKind::Operator {
            self.current_logical = None;
            return;
        }
        if at_cmd_pos && !tok.has_quotes {
            self.visit_cmd_word(idx, end, &tok.text);
        }
        for sub in &tok.sub_tokens {
            let sub_score = score_range(sub, 0, sub.len(), &SpanIndex::empty(), None);
            self.score = self.score.saturating_add(sub_score);
        }
    }

    fn visit_cmd_word(&mut self, idx: usize, end: usize, word: &str) {
        if self.try_visit_block_open(word) || self.try_visit_block_transition(word) {
            return;
        }
        match word {
            "break" | "continue" => self.check_multilevel_jump(idx, end),
            _ => self.check_recursion(word),
        }
    }

    fn try_visit_block_open(&mut self, word: &str) -> bool {
        match word {
            "if" => {
                self.add_nested();
                self.nest_stack.push((NestKind::If, false));
                self.current_logical = None;
                true
            }
            "for" | "while" | "until" | "select" => {
                self.add_nested();
                self.nest_stack.push((NestKind::Loop, false));
                self.current_logical = None;
                true
            }
            "case" => {
                self.add_nested();
                self.nest_stack.push((NestKind::Case, true));
                self.nesting = self.nesting.saturating_add(1);
                self.current_logical = None;
                true
            }
            _ => false,
        }
    }

    fn try_visit_block_transition(&mut self, word: &str) -> bool {
        match word {
            "then" | "do" => self.enter_pending_body(),
            "elif" => {
                self.leave_active_body();
                self.score = self.score.saturating_add(1);
                self.current_logical = None;
            }
            "else" => {
                self.score = self.score.saturating_add(1);
                self.current_logical = None;
            }
            "fi" => self.close_nest(NestKind::If),
            "done" => self.close_nest(NestKind::Loop),
            "esac" => self.close_nest(NestKind::Case),
            _ => return false,
        }
        true
    }

    fn add_nested(&mut self) {
        self.score = self.score.saturating_add(self.nesting.saturating_add(1));
    }

    fn enter_pending_body(&mut self) {
        self.current_logical = None;
        if let Some((_, in_body)) = self.nest_stack.last_mut()
            && !*in_body
        {
            *in_body = true;
            self.nesting = self.nesting.saturating_add(1);
        }
    }

    fn leave_active_body(&mut self) {
        if let Some((_, in_body)) = self.nest_stack.last_mut()
            && *in_body
        {
            *in_body = false;
            self.nesting = self.nesting.saturating_sub(1);
        }
    }

    fn close_nest(&mut self, expected: NestKind) {
        self.current_logical = None;
        if let Some((kind, in_body)) = self.nest_stack.last().copied()
            && kind == expected
        {
            self.nest_stack.pop();
            if in_body {
                self.nesting = self.nesting.saturating_sub(1);
            }
        }
    }

    fn observe_logical(&mut self, op: LogicalOp) {
        if self.current_logical != Some(op) {
            self.score = self.score.saturating_add(1);
            self.current_logical = Some(op);
        }
    }

    fn score_arith_logical(&mut self, text: &str) {
        let bytes = text.as_bytes();
        let mut idx = 0usize;
        while idx + 1 < bytes.len() {
            if &bytes[idx..idx + 2] == b"&&" {
                self.observe_logical(LogicalOp::And);
                idx += 2;
            } else if &bytes[idx..idx + 2] == b"||" {
                self.observe_logical(LogicalOp::Or);
                idx += 2;
            } else {
                idx += 1;
            }
        }
    }

    fn check_multilevel_jump(&mut self, idx: usize, end: usize) {
        self.current_logical = None;
        if idx + 1 < end
            && let Some(arg) = self.tokens.get(idx + 1)
            && arg.kind == ShellTokenKind::Word
            && let Ok(levels) = arg.text.parse::<u32>()
            && levels >= 2
        {
            self.score = self.score.saturating_add(1);
        }
    }

    fn check_recursion(&mut self, word: &str) {
        if !self.counted_recursion && self.fn_name == Some(word) {
            self.counted_recursion = true;
            self.score = self.score.saturating_add(1);
        }
    }
}
