mod cognitive;

use std::collections::BTreeSet;

use codeint_model::language::Function;

use crate::dialect::ShellDialect;
use crate::lexer::{ShellToken, ShellTokenKind};

#[derive(Clone, Debug)]
pub(crate) struct FunctionSpan {
    pub(crate) decl_start: usize,
    pub(crate) body_start: usize,
    pub(crate) body_end: usize,
    pub(crate) start_line: u32,
    pub(crate) end_line: u32,
    pub(crate) name: String,
}

pub(crate) struct ExtractedFunctions {
    pub(crate) functions: Vec<Function>,
    pub(crate) spans: Vec<FunctionSpan>,
    pub(crate) index: SpanIndex,
}

pub(crate) struct SpanIndex {
    skip_end: Vec<Option<usize>>,
}

impl SpanIndex {
    pub(crate) fn new(spans: &[FunctionSpan], token_count: usize) -> Self {
        let mut skip_end = vec![None; token_count];
        for span in spans {
            if let Some(slot) = skip_end.get_mut(span.decl_start) {
                *slot = (*slot).max(Some(span.body_end));
            }
        }
        Self { skip_end }
    }

    pub(crate) fn empty() -> Self {
        Self {
            skip_end: Vec::new(),
        }
    }

    pub(crate) fn skip_end(&self, idx: usize) -> Option<usize> {
        self.skip_end.get(idx).copied().flatten()
    }
}

struct HeredocEnds {
    ends: Vec<(usize, u32)>,
}

impl HeredocEnds {
    fn new(tokens: &[ShellToken]) -> Self {
        let mut ends = Vec::new();
        let mut pending = Vec::new();
        for (idx, tok) in tokens.iter().enumerate() {
            if tok.is_op("<<") || tok.is_op("<<-") {
                pending.push(idx);
            } else if tok.is_newline() {
                ends.extend(pending.drain(..).map(|op| (op, tok.end_line)));
            }
        }
        Self { ends }
    }

    fn end_for(&self, idx: usize) -> Option<u32> {
        self.ends
            .binary_search_by_key(&idx, |&(op, _)| op)
            .ok()
            .map(|pos| self.ends[pos].1)
    }

    fn max_in(&self, first: usize, last: usize) -> Option<u32> {
        let from = self.ends.partition_point(|&(op, _)| op < first);
        let to = self.ends.partition_point(|&(op, _)| op <= last);
        self.ends[from..to].iter().map(|&(_, end)| end).max()
    }
}

struct LineCounter {
    lines: Vec<u32>,
}

impl LineCounter {
    fn new(code_lines: &BTreeSet<u32>) -> Self {
        Self {
            lines: code_lines.iter().copied().collect(),
        }
    }

    fn count(&self, first: u32, last: u32) -> u32 {
        let from = self.lines.partition_point(|&line| line < first);
        let to = self.lines.partition_point(|&line| line <= last);
        to.saturating_sub(from) as u32
    }

    fn count_outside(&self, first: u32, last: u32, mut excluded: Vec<(u32, u32)>) -> u32 {
        let total = self.count(first, last);
        excluded.sort_unstable();
        let mut covered = 0u32;
        let mut current: Option<(u32, u32)> = None;
        for (start, end) in excluded {
            let (start, end) = (start.max(first), end.min(last));
            if start > end {
                continue;
            }
            current = match current {
                Some((cur_start, cur_end)) if start <= cur_end => {
                    Some((cur_start, cur_end.max(end)))
                }
                Some((cur_start, cur_end)) => {
                    covered += self.count(cur_start, cur_end);
                    Some((start, end))
                }
                None => Some((start, end)),
            };
        }
        if let Some((cur_start, cur_end)) = current {
            covered += self.count(cur_start, cur_end);
        }
        total - covered
    }
}

fn is_reserved_word(word: &str) -> bool {
    matches!(
        word,
        "if" | "then"
            | "elif"
            | "else"
            | "fi"
            | "for"
            | "in"
            | "do"
            | "done"
            | "while"
            | "until"
            | "select"
            | "case"
            | "esac"
            | "function"
            | "time"
            | "coproc"
            | "{"
            | "}"
            | "[["
            | "]]"
            | "!"
    )
}

pub(crate) fn extract(
    tokens: &[ShellToken],
    code_lines: &BTreeSet<u32>,
    dialect: ShellDialect,
) -> ExtractedFunctions {
    let mut spans = find_named_functions(tokens, dialect);
    let index = SpanIndex::new(&spans, tokens.len());
    let heredocs = HeredocEnds::new(tokens);
    for span in &mut spans {
        if let Some(end) = heredocs.max_in(span.decl_start, span.body_end) {
            span.end_line = span.end_line.max(end);
        }
    }
    let counter = LineCounter::new(code_lines);
    let children = child_line_ranges(&spans);
    let mut functions: Vec<Function> = spans
        .iter()
        .zip(children)
        .map(|(span, child_ranges)| {
            build_named_function(tokens, &counter, &index, span, child_ranges)
        })
        .collect();
    if let Some(script_fn) =
        build_top_level_script_function(tokens, &counter, &spans, &index, &heredocs)
    {
        functions.push(script_fn);
    }
    functions.sort_by_key(|f| (f.line, f.end_line));
    ExtractedFunctions {
        functions,
        spans,
        index,
    }
}

fn build_named_function(
    tokens: &[ShellToken],
    counter: &LineCounter,
    index: &SpanIndex,
    span: &FunctionSpan,
    child_ranges: Vec<(u32, u32)>,
) -> Function {
    let source_lines = counter.count_outside(span.start_line, span.end_line, child_ranges);
    let decision_points =
        count_decisions_in_range(tokens, span.body_start, span.body_end + 1, index);
    let cognitive_complexity = cognitive::score_range(
        tokens,
        span.body_start,
        span.body_end + 1,
        index,
        Some(span.name.as_str()),
    );
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

fn build_top_level_script_function(
    tokens: &[ShellToken],
    counter: &LineCounter,
    spans: &[FunctionSpan],
    index: &SpanIndex,
    heredocs: &HeredocEnds,
) -> Option<Function> {
    let (first_line, last_line) = top_level_line_bounds(tokens, index, heredocs)?;
    let function_ranges = spans
        .iter()
        .map(|span| (span.start_line, span.end_line))
        .collect();
    let source_lines = counter.count_outside(0, u32::MAX, function_ranges);
    let decision_points = count_decisions_in_range(tokens, 0, tokens.len(), index);
    let cognitive_complexity = cognitive::score_range(tokens, 0, tokens.len(), index, None);
    Some(Function {
        line: first_line,
        end_line: last_line,
        decision_points,
        source_lines,
        name: Some("<script>".to_owned()),
        cognitive_complexity,
        coverage: None,
    })
}

fn top_level_line_bounds(
    tokens: &[ShellToken],
    index: &SpanIndex,
    heredocs: &HeredocEnds,
) -> Option<(u32, u32)> {
    let mut first: Option<u32> = None;
    let mut last = 0u32;
    let mut idx = 0usize;
    while idx < tokens.len() {
        if let Some(end) = index.skip_end(idx) {
            idx = end + 1;
            continue;
        }
        let tok = &tokens[idx];
        if !tok.is_newline() {
            first.get_or_insert(tok.line);
            last = last.max(tok.end_line);
            if let Some(heredoc_end) = heredocs.end_for(idx) {
                last = last.max(heredoc_end);
            }
        }
        idx += 1;
    }
    first.map(|start| (start, last.max(start)))
}

fn child_line_ranges(spans: &[FunctionSpan]) -> Vec<Vec<(u32, u32)>> {
    let mut children = vec![Vec::new(); spans.len()];
    let mut open: Vec<usize> = Vec::new();
    for (idx, span) in spans.iter().enumerate() {
        while open
            .last()
            .is_some_and(|&top| spans[top].body_end < span.decl_start)
        {
            open.pop();
        }
        let parent = open.iter().rev().copied().find(|&candidate| {
            span.decl_start >= spans[candidate].body_start
                && span.body_end <= spans[candidate].body_end
        });
        if let Some(parent) = parent {
            children[parent].push((span.start_line, span.end_line));
        }
        open.push(idx);
    }
    children
}

fn find_named_functions(tokens: &[ShellToken], dialect: ShellDialect) -> Vec<FunctionSpan> {
    let mut spans = Vec::new();
    let mut at_cmd_pos = true;
    let mut idx = 0usize;
    while idx < tokens.len() {
        if at_cmd_pos && let Some(span) = try_parse_function_at(tokens, idx, dialect) {
            let next_after_header = span.body_start;
            spans.push(span);
            idx = next_after_header;
            at_cmd_pos = true;
            continue;
        }
        at_cmd_pos = tokens[idx].next_is_cmd_pos();
        idx += 1;
    }
    spans
}

fn try_parse_function_at(
    tokens: &[ShellToken],
    idx: usize,
    dialect: ShellDialect,
) -> Option<FunctionSpan> {
    let (name, header_end) = parse_function_header(tokens, idx)?;
    let mut pos = skip_newlines(tokens, header_end);
    let body_open = pos;
    let body_end = find_compound_body_end(tokens, body_open, dialect)?;
    pos = body_end;
    Some(FunctionSpan {
        decl_start: idx,
        body_start: body_open,
        body_end: pos,
        start_line: tokens[idx].line,
        end_line: tokens[pos].end_line,
        name,
    })
}

fn parse_function_header(tokens: &[ShellToken], idx: usize) -> Option<(String, usize)> {
    let first = tokens.get(idx)?;
    if first.is_word("function") {
        let name_tok = tokens.get(idx + 1)?;
        if !is_valid_fn_name(name_tok) {
            return None;
        }
        let mut after = idx + 2;
        if tokens.get(after).is_some_and(|t| t.is_op("("))
            && tokens.get(after + 1).is_some_and(|t| t.is_op(")"))
        {
            after += 2;
        }
        return Some((name_tok.text.clone(), after));
    }
    if !is_valid_fn_name(first) {
        return None;
    }
    if tokens.get(idx + 1).is_some_and(|t| t.is_op("("))
        && tokens.get(idx + 2).is_some_and(|t| t.is_op(")"))
    {
        return Some((first.text.clone(), idx + 3));
    }
    None
}

fn is_valid_fn_name(tok: &ShellToken) -> bool {
    if tok.kind != ShellTokenKind::Word || tok.has_quotes || tok.has_var_expansion {
        return false;
    }
    if tok.text.is_empty() || is_reserved_word(&tok.text) {
        return false;
    }
    !tok.text
        .bytes()
        .any(|b| matches!(b, b'=' | b'$' | b'*' | b'?' | b'[' | b']'))
}

fn skip_newlines(tokens: &[ShellToken], mut idx: usize) -> usize {
    while tokens.get(idx).is_some_and(ShellToken::is_newline) {
        idx += 1;
    }
    idx
}

fn find_compound_body_end(
    tokens: &[ShellToken],
    open_idx: usize,
    dialect: ShellDialect,
) -> Option<usize> {
    let tok = tokens.get(open_idx)?;
    if tok.is_word("{") {
        return find_matching_brace_group(tokens, open_idx, dialect == ShellDialect::Zsh);
    }
    if tok.is_op("(") {
        return find_matching_subshell(tokens, open_idx);
    }
    if tok.is_word("if") {
        return find_matching_keyword_block(tokens, open_idx, "if", "fi");
    }
    if tok.is_word("case") {
        return find_matching_keyword_block(tokens, open_idx, "case", "esac");
    }
    if tok.is_word("for") || tok.is_word("while") || tok.is_word("until") || tok.is_word("select") {
        return find_matching_loop_block(tokens, open_idx);
    }
    None
}

fn find_matching_brace_group(
    tokens: &[ShellToken],
    open_idx: usize,
    close_brace_ends_any_command: bool,
) -> Option<usize> {
    let mut depth = 0usize;
    let mut at_cmd_pos = true;
    for (idx, tok) in tokens.iter().enumerate().skip(open_idx) {
        match brace_role(tokens, idx, at_cmd_pos, close_brace_ends_any_command) {
            BraceRole::Open => depth += 1,
            BraceRole::Close if depth <= 1 => return Some(idx),
            BraceRole::Close => depth -= 1,
            BraceRole::Other => {}
        }
        at_cmd_pos = tok.next_is_cmd_pos() || (at_cmd_pos && ends_compound_command(tok));
    }
    None
}

enum BraceRole {
    Open,
    Close,
    Other,
}

fn brace_role(
    tokens: &[ShellToken],
    idx: usize,
    at_cmd_pos: bool,
    close_brace_ends_any_command: bool,
) -> BraceRole {
    let tok = &tokens[idx];
    if tok.is_word("{") && (at_cmd_pos || follows_function_header(tokens, idx)) {
        BraceRole::Open
    } else if tok.is_word("}") && (at_cmd_pos || close_brace_ends_any_command) {
        BraceRole::Close
    } else {
        BraceRole::Other
    }
}

fn ends_compound_command(tok: &ShellToken) -> bool {
    ["esac", "fi", "done", "}"]
        .iter()
        .any(|keyword| tok.is_word(keyword))
}

fn follows_function_header(tokens: &[ShellToken], brace_idx: usize) -> bool {
    let Some(prev_idx) = prev_non_newline(tokens, brace_idx) else {
        return false;
    };
    if tokens[prev_idx].is_op(")") {
        return true;
    }
    prev_idx > 0 && tokens[prev_idx - 1].is_word("function") && is_valid_fn_name(&tokens[prev_idx])
}

fn prev_non_newline(tokens: &[ShellToken], mut idx: usize) -> Option<usize> {
    while idx > 0 {
        idx -= 1;
        if !tokens[idx].is_newline() {
            return Some(idx);
        }
    }
    None
}

fn find_matching_subshell(tokens: &[ShellToken], open_idx: usize) -> Option<usize> {
    let mut depth = 0usize;
    for (idx, tok) in tokens.iter().enumerate().skip(open_idx) {
        if tok.is_case_pattern_paren {
            continue;
        }
        if tok.is_op("(") {
            depth += 1;
        } else if tok.is_op(")") {
            depth -= 1;
            if depth == 0 {
                return Some(idx);
            }
        }
    }
    None
}

fn find_matching_keyword_block(
    tokens: &[ShellToken],
    open_idx: usize,
    open_kw: &str,
    close_kw: &str,
) -> Option<usize> {
    let mut depth = 0usize;
    let mut at_cmd_pos = true;
    for (idx, tok) in tokens.iter().enumerate().skip(open_idx) {
        if at_cmd_pos && tok.is_word(open_kw) {
            depth += 1;
        } else if at_cmd_pos && tok.is_word(close_kw) {
            depth -= 1;
            if depth == 0 {
                return Some(idx);
            }
        }
        at_cmd_pos = tok.next_is_cmd_pos();
    }
    None
}

fn find_matching_loop_block(tokens: &[ShellToken], open_idx: usize) -> Option<usize> {
    let mut depth = 0usize;
    let mut at_cmd_pos = true;
    for (idx, tok) in tokens.iter().enumerate().skip(open_idx) {
        if at_cmd_pos
            && (tok.is_word("for")
                || tok.is_word("while")
                || tok.is_word("until")
                || tok.is_word("select"))
        {
            depth += 1;
        } else if at_cmd_pos && tok.is_word("done") {
            depth -= 1;
            if depth == 0 {
                return Some(idx);
            }
        }
        at_cmd_pos = tok.next_is_cmd_pos();
    }
    None
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum CaseTracker {
    WaitingIn,
    InPattern { saw_wildcard: bool, has_word: bool },
    InBody,
}

fn count_decisions_in_range(
    tokens: &[ShellToken],
    start: usize,
    end: usize,
    index: &SpanIndex,
) -> u32 {
    let mut total = 0u32;
    let mut at_cmd_pos = true;
    let mut case_stack: Vec<CaseTracker> = Vec::new();
    let mut idx = start;
    while idx < end {
        if let Some(skip_end) = index.skip_end(idx) {
            idx = skip_end + 1;
            at_cmd_pos = true;
            continue;
        }
        let tok = &tokens[idx];
        total += count_token_decisions(tok, at_cmd_pos, &mut case_stack);
        for sub in &tok.sub_tokens {
            total += count_decisions_in_range(sub, 0, sub.len(), &SpanIndex::empty());
        }
        at_cmd_pos = tok.next_is_cmd_pos();
        idx += 1;
    }
    total
}

fn count_token_decisions(
    tok: &ShellToken,
    at_cmd_pos: bool,
    case_stack: &mut Vec<CaseTracker>,
) -> u32 {
    if let Some(delta) = step_case_tracker(tok, at_cmd_pos, case_stack) {
        return delta;
    }
    if tok.kind == ShellTokenKind::ArithCmd {
        return count_arith_logical_ops(&tok.text);
    }
    if tok.is_op("&&") || tok.is_op("||") {
        return 1;
    }
    if at_cmd_pos && is_decision_keyword(tok) {
        return 1;
    }
    0
}

fn is_decision_keyword(tok: &ShellToken) -> bool {
    tok.kind == ShellTokenKind::Word
        && !tok.has_quotes
        && matches!(
            tok.text.as_str(),
            "if" | "elif" | "for" | "while" | "until" | "select"
        )
}

fn step_case_tracker(
    tok: &ShellToken,
    at_cmd_pos: bool,
    case_stack: &mut Vec<CaseTracker>,
) -> Option<u32> {
    if at_cmd_pos && tok.is_word("case") {
        case_stack.push(CaseTracker::WaitingIn);
        return Some(0);
    }
    match case_stack.last().copied()? {
        CaseTracker::WaitingIn => Some(step_case_waiting_in(tok, case_stack)),
        CaseTracker::InPattern { .. } => Some(step_case_in_pattern(tok, case_stack)),
        CaseTracker::InBody => step_case_in_body(tok, at_cmd_pos, case_stack),
    }
}

fn step_case_waiting_in(tok: &ShellToken, case_stack: &mut [CaseTracker]) -> u32 {
    if tok.is_word("in")
        && let Some(top) = case_stack.last_mut()
    {
        *top = CaseTracker::InPattern {
            saw_wildcard: false,
            has_word: false,
        };
    }
    0
}

fn step_case_in_pattern(tok: &ShellToken, case_stack: &mut Vec<CaseTracker>) -> u32 {
    if tok.is_word("esac") {
        case_stack.pop();
        return 0;
    }
    let Some(CaseTracker::InPattern {
        saw_wildcard,
        has_word,
    }) = case_stack.last_mut()
    else {
        return 0;
    };
    if tok.is_op(")") {
        let is_branch = *has_word && !*saw_wildcard;
        *case_stack.last_mut().unwrap() = CaseTracker::InBody;
        return u32::from(is_branch);
    }
    if tok.kind == ShellTokenKind::Word {
        *has_word = true;
        if !tok.has_quotes && tok.text == "*" {
            *saw_wildcard = true;
        }
    }
    0
}

fn step_case_in_body(
    tok: &ShellToken,
    at_cmd_pos: bool,
    case_stack: &mut Vec<CaseTracker>,
) -> Option<u32> {
    if at_cmd_pos && tok.is_word("esac") {
        case_stack.pop();
        return Some(0);
    }
    if tok.is_op(";;") || tok.is_op(";&") || tok.is_op(";;&") {
        if let Some(top) = case_stack.last_mut() {
            *top = CaseTracker::InPattern {
                saw_wildcard: false,
                has_word: false,
            };
        }
        return Some(0);
    }
    None
}

pub(crate) fn count_arith_logical_ops(text: &str) -> u32 {
    let bytes = text.as_bytes();
    let mut count = 0u32;
    let mut idx = 0usize;
    while idx + 1 < bytes.len() {
        if matches!(&bytes[idx..idx + 2], b"&&" | b"||") {
            count += 1;
            idx += 2;
        } else {
            idx += 1;
        }
    }
    count
}
