use std::collections::BTreeSet;

use codeint_model::language::Function;

use crate::lexer::{LogicalLine, PyToken};

struct PyFunctionSpan {
    body_start: usize,
    body_end: usize,
    start_line: u32,
    end_line: u32,
    name: Option<String>,
    def_lines: Option<(usize, usize)>,
    lambda_start: Option<usize>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum BoolOp {
    And,
    Or,
}

enum LineClause {
    NestedHeader(usize),
    FlatHeader(usize),
    CaseHeader,
    Plain,
}

pub(crate) fn extract(
    tokens: &[PyToken],
    logical_lines: &[LogicalLine],
    code_lines: &BTreeSet<u32>,
) -> Vec<Function> {
    let mut spans = Vec::new();
    let case_indices = collect_decision_case_indices(tokens, logical_lines);
    for (line_idx, line) in logical_lines.iter().enumerate() {
        if let Some(span) = extract_def_span(tokens, logical_lines, line_idx) {
            spans.push(span);
        }
        collect_lambda_spans(tokens, line, &mut spans);
    }
    let mut functions: Vec<Function> = spans
        .iter()
        .enumerate()
        .map(|(owner, span)| Function {
            line: span.start_line,
            end_line: span.end_line,
            decision_points: count_decisions(tokens, &spans, owner, &case_indices),
            source_lines: code_lines.range(span.start_line..=span.end_line).count() as u32,
            name: span.name.clone(),
            cognitive_complexity: compute_cognitive(tokens, logical_lines, &spans, owner),
            coverage: None,
        })
        .collect();
    functions.sort_by_key(|f| (f.line, f.end_line));
    functions
}

pub(crate) fn is_def_header(slice: &[PyToken]) -> bool {
    slice.first().is_some_and(|t| t.is_kw("def"))
        || (slice.first().is_some_and(|t| t.is_kw("async"))
            && slice.get(1).is_some_and(|t| t.is_kw("def")))
}

fn def_header_name(slice: &[PyToken]) -> Option<String> {
    let name_idx = if slice.first().is_some_and(|t| t.is_kw("def")) {
        1
    } else if slice.first().is_some_and(|t| t.is_kw("async"))
        && slice.get(1).is_some_and(|t| t.is_kw("def"))
    {
        2
    } else {
        return None;
    };
    slice
        .get(name_idx)
        .filter(|tok| tok.is_ident())
        .map(|tok| tok.text.clone())
}

fn extract_def_span(
    tokens: &[PyToken],
    logical_lines: &[LogicalLine],
    line_idx: usize,
) -> Option<PyFunctionSpan> {
    let line = &logical_lines[line_idx];
    let slice = &tokens[line.tokens.clone()];
    if !is_def_header(slice) {
        return None;
    }
    let colon_idx = find_header_colon(tokens, line.tokens.start, line.tokens.end)?;
    let block_end = block_end_line(logical_lines, line_idx);
    let end_logical = &logical_lines[block_end];
    let body_start = colon_idx + 1;
    let body_end = end_logical.tokens.end.saturating_sub(1);
    (body_start <= body_end).then(|| PyFunctionSpan {
        body_start,
        body_end,
        start_line: line.start_line,
        end_line: end_logical.end_line,
        name: def_header_name(slice),
        def_lines: Some((line_idx, block_end)),
        lambda_start: None,
    })
}

fn find_header_colon(tokens: &[PyToken], start: usize, end: usize) -> Option<usize> {
    let mut depth = 0usize;
    let mut saw_params = false;
    for (idx, tok) in tokens.iter().enumerate().take(end).skip(start) {
        match tok.text.as_str() {
            "(" | "[" | "{" => {
                saw_params |= tok.text == "(" && depth == 0;
                depth += 1;
            }
            ")" | "]" | "}" => depth = depth.saturating_sub(1),
            ":" if depth == 0 && saw_params => return Some(idx),
            _ => {}
        }
    }
    None
}

pub(crate) fn block_end_line(logical_lines: &[LogicalLine], header_idx: usize) -> usize {
    let base_indent = logical_lines[header_idx].indent;
    let mut end = header_idx;
    while let Some(next) = logical_lines.get(end + 1) {
        if next.indent > base_indent {
            end += 1;
        } else {
            break;
        }
    }
    end
}

fn collect_lambda_spans(tokens: &[PyToken], line: &LogicalLine, spans: &mut Vec<PyFunctionSpan>) {
    for idx in line.tokens.clone() {
        if !tokens[idx].is_kw("lambda") {
            continue;
        }
        let Some(colon_idx) = find_lambda_colon(tokens, idx + 1, line.tokens.end) else {
            continue;
        };
        let body_start = colon_idx + 1;
        if body_start >= line.tokens.end {
            continue;
        }
        let body_end = find_lambda_body_end(tokens, body_start, line.tokens.end);
        if body_start <= body_end {
            spans.push(PyFunctionSpan {
                body_start,
                body_end,
                start_line: tokens[idx].line,
                end_line: tokens[body_end].line,
                name: None,
                def_lines: None,
                lambda_start: Some(idx),
            });
        }
    }
}

fn find_lambda_colon(tokens: &[PyToken], start: usize, end: usize) -> Option<usize> {
    let mut depth = 0usize;
    for (idx, tok) in tokens.iter().enumerate().take(end).skip(start) {
        match tok.text.as_str() {
            "(" | "[" | "{" => depth += 1,
            ")" | "]" | "}" if depth == 0 => return None,
            ")" | "]" | "}" => depth -= 1,
            ":" if depth == 0 => return Some(idx),
            _ => {}
        }
    }
    None
}

fn find_lambda_body_end(tokens: &[PyToken], start: usize, end: usize) -> usize {
    let mut depth = 0usize;
    for (idx, tok) in tokens.iter().enumerate().take(end).skip(start) {
        match tok.text.as_str() {
            "(" | "[" | "{" => depth += 1,
            ")" | "]" | "}" if depth == 0 => return idx.saturating_sub(1),
            ")" | "]" | "}" => depth -= 1,
            "," | ";" if depth == 0 => return idx.saturating_sub(1),
            _ => {}
        }
    }
    end.saturating_sub(1)
}

fn collect_decision_case_indices(
    tokens: &[PyToken],
    logical_lines: &[LogicalLine],
) -> BTreeSet<usize> {
    let mut out = BTreeSet::new();
    for line in logical_lines {
        let slice = &tokens[line.tokens.clone()];
        if is_case_header(slice) && !is_wildcard_case(slice) {
            out.insert(line.tokens.start);
        }
    }
    out
}

fn is_case_header(slice: &[PyToken]) -> bool {
    slice
        .first()
        .is_some_and(|t| t.is_ident() && t.text == "case")
        && find_top_level_colon(slice).is_some()
}

fn is_wildcard_case(slice: &[PyToken]) -> bool {
    slice.len() == 3 && slice[1].is_ident() && slice[1].text == "_"
}

fn count_decisions(
    tokens: &[PyToken],
    spans: &[PyFunctionSpan],
    owner: usize,
    case_indices: &BTreeSet<usize>,
) -> u32 {
    let span = &spans[owner];
    let mut decisions = 0u32;
    let mut cursor = span.body_start;
    while cursor <= span.body_end {
        if let Some(child_end) = nested_body_end(spans, owner, cursor) {
            cursor = child_end + 1;
            continue;
        }
        decisions += decision_weight(tokens, cursor, case_indices);
        cursor += 1;
    }
    decisions
}

fn nested_body_end(spans: &[PyFunctionSpan], owner: usize, cursor: usize) -> Option<usize> {
    spans
        .iter()
        .enumerate()
        .filter(|&(idx, s)| idx != owner && s.body_start == cursor)
        .map(|(_, s)| s.body_end)
        .max()
}

fn decision_weight(tokens: &[PyToken], idx: usize, case_indices: &BTreeSet<usize>) -> u32 {
    if case_indices.contains(&idx) {
        return 1;
    }
    let tok = &tokens[idx];
    if !tok.is_kw {
        return 0;
    }
    match tok.text.as_str() {
        "if" | "elif" | "for" | "while" | "except" | "and" | "or" => 1,
        _ => 0,
    }
}

fn compute_cognitive(
    tokens: &[PyToken],
    logical_lines: &[LogicalLine],
    spans: &[PyFunctionSpan],
    owner: usize,
) -> u32 {
    let span = &spans[owner];
    let mut scorer = PyCognitiveScorer {
        tokens,
        spans,
        owner,
        fn_name: span.name.as_deref(),
        total: 0,
        counted_recursion: false,
    };
    let Some((header_line, block_end)) = span.def_lines else {
        scorer.score_expr(span.body_start, span.body_end + 1, 0, false);
        return scorer.total;
    };
    let header_end = logical_lines[header_line].tokens.end;
    if span.body_start < header_end {
        scorer.score_expr(span.body_start, header_end, 0, false);
    }
    let mut indent_stack: Vec<usize> = Vec::new();
    let mut line_idx = header_line + 1;
    while line_idx <= block_end {
        if let Some(child_end_line) = nested_def_end_line(spans, owner, line_idx) {
            line_idx = child_end_line + 1;
            continue;
        }
        let line = &logical_lines[line_idx];
        while indent_stack
            .last()
            .is_some_and(|&frame| frame >= line.indent)
        {
            indent_stack.pop();
        }
        let nesting = indent_stack.len() as u32;
        if scorer.score_statement_line(logical_lines, line_idx, nesting) {
            indent_stack.push(line.indent);
        }
        line_idx += 1;
    }
    scorer.total
}

fn nested_def_end_line(spans: &[PyFunctionSpan], owner: usize, line_idx: usize) -> Option<usize> {
    spans
        .iter()
        .enumerate()
        .filter_map(|(idx, s)| {
            if idx == owner {
                return None;
            }
            let (start, end) = s.def_lines?;
            (start == line_idx).then_some(end)
        })
        .max()
}

fn find_top_level_colon(slice: &[PyToken]) -> Option<usize> {
    let mut depth = 0usize;
    for (idx, tok) in slice.iter().enumerate() {
        match tok.text.as_str() {
            "(" | "[" | "{" => depth += 1,
            ")" | "]" | "}" => depth = depth.saturating_sub(1),
            ":" if depth == 0 => return Some(idx),
            _ => {}
        }
    }
    None
}

fn classify_line(tokens: &[PyToken], logical_lines: &[LogicalLine], line_idx: usize) -> LineClause {
    let slice = &tokens[logical_lines[line_idx].tokens.clone()];
    let Some(first) = slice.first() else {
        return LineClause::Plain;
    };
    if let Some(kw_len) = nested_header_kw_len(slice) {
        return LineClause::NestedHeader(kw_len);
    }
    if first.is_kw && matches!(first.text.as_str(), "elif" | "else") {
        return LineClause::FlatHeader(1);
    }
    if is_match_header(tokens, logical_lines, line_idx) {
        return LineClause::NestedHeader(1);
    }
    if is_case_header(slice) {
        return LineClause::CaseHeader;
    }
    LineClause::Plain
}

fn nested_header_kw_len(slice: &[PyToken]) -> Option<usize> {
    let first = slice.first()?;
    if !first.is_kw {
        return None;
    }
    match first.text.as_str() {
        "if" | "for" | "while" | "except" => Some(1),
        "async" if slice.get(1).is_some_and(|t| t.is_kw("for")) => Some(2),
        _ => None,
    }
}

fn is_match_header(tokens: &[PyToken], logical_lines: &[LogicalLine], line_idx: usize) -> bool {
    let line = &logical_lines[line_idx];
    let slice = &tokens[line.tokens.clone()];
    if slice.len() < 3
        || !slice[0].is_ident()
        || slice[0].text != "match"
        || matches!(slice[1].text.as_str(), ":" | "=" | ".")
    {
        return false;
    }
    find_top_level_colon(slice).is_some()
        && logical_lines
            .get(line_idx + 1)
            .is_some_and(|next| next.indent > line.indent)
}

struct ExprState {
    op_stack: Vec<Option<BoolOp>>,
    group_stack: Vec<(bool, u32)>,
    ternary_depth: u32,
}

impl ExprState {
    fn new() -> Self {
        Self {
            op_stack: vec![None],
            group_stack: Vec::new(),
            ternary_depth: 0,
        }
    }

    fn reset_op(&mut self) {
        if let Some(top) = self.op_stack.last_mut() {
            *top = None;
        }
    }

    fn record_op(&mut self, op: BoolOp) -> u32 {
        let Some(top) = self.op_stack.last_mut() else {
            return 0;
        };
        if *top == Some(op) {
            return 0;
        }
        *top = Some(op);
        1
    }

    fn enter_delim(&mut self, is_group: bool) {
        self.group_stack.push((is_group, self.ternary_depth));
        if !is_group {
            self.op_stack.push(None);
        }
    }

    fn leave_delim(&mut self) {
        if let Some((is_group, saved_ternary)) = self.group_stack.pop() {
            self.ternary_depth = saved_ternary;
            if !is_group && self.op_stack.len() > 1 {
                self.op_stack.pop();
            }
        }
    }
}

struct PyCognitiveScorer<'a> {
    tokens: &'a [PyToken],
    spans: &'a [PyFunctionSpan],
    owner: usize,
    fn_name: Option<&'a str>,
    total: u32,
    counted_recursion: bool,
}

impl PyCognitiveScorer<'_> {
    fn score_statement_line(
        &mut self,
        logical_lines: &[LogicalLine],
        line_idx: usize,
        nesting: u32,
    ) -> bool {
        let line = &logical_lines[line_idx];
        let start = line.tokens.start;
        let end = line.tokens.end;
        let colon_offset = find_top_level_colon(&self.tokens[start..end]);
        match classify_line(self.tokens, logical_lines, line_idx) {
            LineClause::NestedHeader(kw_len) => {
                self.total = self.total.saturating_add(nesting.saturating_add(1));
                self.score_header_and_suite(start + kw_len, start, end, colon_offset, nesting);
                true
            }
            LineClause::FlatHeader(kw_len) => {
                self.total = self.total.saturating_add(1);
                self.score_header_and_suite(start + kw_len, start, end, colon_offset, nesting);
                true
            }
            LineClause::CaseHeader => {
                let cond_end = colon_offset.map_or(end, |off| start + off);
                self.score_expr(start + 1, cond_end, nesting, true);
                self.score_inline_suite(start, end, colon_offset, nesting);
                false
            }
            LineClause::Plain => {
                self.score_expr(start, end, nesting, false);
                false
            }
        }
    }

    fn score_header_and_suite(
        &mut self,
        cond_start: usize,
        line_start: usize,
        line_end: usize,
        colon_offset: Option<usize>,
        nesting: u32,
    ) {
        let cond_end = colon_offset.map_or(line_end, |off| line_start + off);
        if cond_start < cond_end {
            self.score_expr(cond_start, cond_end, nesting, false);
        }
        self.score_inline_suite(line_start, line_end, colon_offset, nesting);
    }

    fn score_inline_suite(
        &mut self,
        line_start: usize,
        line_end: usize,
        colon_offset: Option<usize>,
        nesting: u32,
    ) {
        if let Some(off) = colon_offset
            && line_start + off + 1 < line_end
        {
            self.score_expr(line_start + off + 1, line_end, nesting + 1, false);
        }
    }

    fn score_expr(&mut self, start: usize, end: usize, nesting: u32, ignore_guard_if: bool) {
        let mut state = ExprState::new();
        let mut cursor = start;
        while cursor < end {
            if let Some(skip_end) = self.nested_lambda_end(cursor) {
                cursor = skip_end + 1;
                state.reset_op();
                continue;
            }
            self.score_expr_token(cursor, start, nesting, ignore_guard_if, &mut state);
            cursor += 1;
        }
    }

    fn score_expr_token(
        &mut self,
        cursor: usize,
        range_start: usize,
        nesting: u32,
        ignore_guard_if: bool,
        state: &mut ExprState,
    ) {
        let tok = &self.tokens[cursor];
        if tok.is_kw {
            self.score_expr_kw(tok.text.as_str(), nesting, ignore_guard_if, state);
            return;
        }
        if self.handle_expr_punct(cursor, range_start, state) {
            return;
        }
        self.check_recursion(cursor);
    }

    fn score_expr_kw(
        &mut self,
        kw: &str,
        nesting: u32,
        ignore_guard_if: bool,
        state: &mut ExprState,
    ) {
        match kw {
            "and" => self.total = self.total.saturating_add(state.record_op(BoolOp::And)),
            "or" => self.total = self.total.saturating_add(state.record_op(BoolOp::Or)),
            "if" => {
                if !ignore_guard_if {
                    let weight = nesting
                        .saturating_add(state.ternary_depth)
                        .saturating_add(1);
                    self.total = self.total.saturating_add(weight);
                }
                state.reset_op();
            }
            "for" => {
                self.total = self.total.saturating_add(nesting.saturating_add(1));
                state.reset_op();
            }
            "else" => {
                state.ternary_depth = state.ternary_depth.saturating_add(1);
                state.reset_op();
            }
            "return" | "yield" | "raise" | "assert" | "del" | "in" => state.reset_op(),
            _ => {}
        }
    }

    fn handle_expr_punct(
        &mut self,
        cursor: usize,
        range_start: usize,
        state: &mut ExprState,
    ) -> bool {
        let text = self.tokens[cursor].text.as_str();
        match text {
            "(" => state.enter_delim(!is_call_paren(self.tokens, cursor, range_start)),
            "[" | "{" => state.enter_delim(false),
            ")" | "]" | "}" => state.leave_delim(),
            "," | ";" => {
                state.reset_op();
                state.ternary_depth = 0;
            }
            _ if is_py_reset_punct(text) => state.reset_op(),
            _ => return false,
        }
        true
    }

    fn check_recursion(&mut self, cursor: usize) {
        if self.counted_recursion {
            return;
        }
        if is_py_recursive_call(self.tokens, cursor, self.fn_name) {
            self.counted_recursion = true;
            self.total = self.total.saturating_add(1);
        }
    }

    fn nested_lambda_end(&self, cursor: usize) -> Option<usize> {
        self.spans
            .iter()
            .enumerate()
            .filter_map(|(idx, s)| {
                if idx == self.owner {
                    return None;
                }
                let start = s.lambda_start?;
                (start == cursor).then_some(s.body_end)
            })
            .max()
    }
}

fn is_call_paren(tokens: &[PyToken], cursor: usize, range_start: usize) -> bool {
    if cursor <= range_start {
        return false;
    }
    let prev = &tokens[cursor - 1];
    prev.is_ident() || matches!(prev.text.as_str(), ")" | "]")
}

fn is_py_reset_punct(text: &str) -> bool {
    matches!(
        text,
        ":" | "="
            | ":="
            | "+="
            | "-="
            | "*="
            | "/="
            | "//="
            | "%="
            | "**="
            | "&="
            | "|="
            | "^="
            | "<<="
            | ">>="
            | "@="
    )
}

fn is_py_recursive_call(tokens: &[PyToken], cursor: usize, fn_name: Option<&str>) -> bool {
    let Some(name) = fn_name else {
        return false;
    };
    let tok = &tokens[cursor];
    if !tok.is_ident() || tok.text != name {
        return false;
    }
    if !tokens.get(cursor + 1).is_some_and(|t| t.text == "(") {
        return false;
    }
    if cursor > 0 && tokens[cursor - 1].text == "." {
        return cursor >= 2
            && matches!(tokens[cursor - 2].text.as_str(), "self" | "cls")
            && (cursor < 3 || tokens[cursor - 3].text != ".");
    }
    cursor == 0 || !tokens[cursor - 1].is_kw("def")
}
