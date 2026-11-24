use sqlparser::tokenizer::{Token, TokenWithSpan};

use crate::tokenize::{ExpandedRoutineBody, TokenChunk, tokenize_pg_routine_body};

pub(crate) struct ExtractedStatement {
    pub(crate) start_line: u32,
    pub(crate) end_line: u32,
    pub(crate) name: Option<String>,
    pub(crate) decision_points: u32,
    pub(crate) cognitive_complexity: u32,
    pub(crate) tokens: Vec<TokenWithSpan>,
    pub(crate) routine_body_tokens: Vec<ExpandedRoutineBody>,
}

pub(crate) fn extract_statements(chunks: &[TokenChunk]) -> Vec<ExtractedStatement> {
    let mut statements = Vec::new();
    for chunk in chunks {
        if chunk.is_custom_delimited {
            if let Some(stmt) = finalize_statement(chunk.tokens.clone()) {
                statements.push(stmt);
            }
        } else {
            for raw_tokens in split_standard_chunk(&chunk.tokens) {
                if let Some(stmt) = finalize_statement(raw_tokens) {
                    statements.push(stmt);
                }
            }
        }
    }
    statements
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SplitBlock {
    Begin,
    Case,
}

struct SplitState {
    paren_depth: usize,
    blocks: Vec<SplitBlock>,
    is_routine_stmt: bool,
    stmt_start: usize,
}

fn split_standard_chunk(tokens: &[TokenWithSpan]) -> Vec<Vec<TokenWithSpan>> {
    let mut result = Vec::new();
    let mut state = SplitState {
        paren_depth: 0,
        blocks: Vec::new(),
        is_routine_stmt: false,
        stmt_start: 0,
    };
    let mut idx = 0usize;

    while idx < tokens.len() {
        idx = step_split_token(tokens, idx, &mut state, &mut result);
    }
    push_trimmed_slice(tokens, state.stmt_start, tokens.len(), &mut result);
    result
}

fn step_split_token(
    tokens: &[TokenWithSpan],
    idx: usize,
    state: &mut SplitState,
    result: &mut Vec<Vec<TokenWithSpan>>,
) -> usize {
    let tok = &tokens[idx].token;
    if matches!(tok, Token::Whitespace(_) | Token::EOF) {
        return idx + 1;
    }
    detect_routine_start(tokens, idx, state);
    match tok {
        Token::LParen => state.paren_depth += 1,
        Token::RParen => state.paren_depth = state.paren_depth.saturating_sub(1),
        Token::SemiColon if state.paren_depth == 0 && state.blocks.is_empty() => {
            finish_top_level_stmt(tokens, idx, state, result);
        }
        Token::Word(_) if state.is_routine_stmt => {
            return update_routine_split_blocks(tokens, idx, &mut state.blocks);
        }
        _ => {}
    }
    idx + 1
}

fn detect_routine_start(tokens: &[TokenWithSpan], idx: usize, state: &mut SplitState) {
    if state.stmt_start <= idx && !has_code_before(tokens, state.stmt_start, idx) {
        state.is_routine_stmt = starts_routine_header(tokens, idx);
    }
}

fn finish_top_level_stmt(
    tokens: &[TokenWithSpan],
    idx: usize,
    state: &mut SplitState,
    result: &mut Vec<Vec<TokenWithSpan>>,
) {
    push_trimmed_slice(tokens, state.stmt_start, idx, result);
    state.stmt_start = idx + 1;
    state.is_routine_stmt = false;
}

fn has_code_before(tokens: &[TokenWithSpan], start: usize, end: usize) -> bool {
    tokens[start..end]
        .iter()
        .any(|t| !matches!(t.token, Token::Whitespace(_) | Token::EOF))
}

fn starts_routine_header(tokens: &[TokenWithSpan], start: usize) -> bool {
    if !is_unquoted_kw(&tokens[start], "CREATE") {
        return false;
    }
    let header_code: Vec<&TokenWithSpan> = tokens[start..]
        .iter()
        .filter(|t| !matches!(t.token, Token::Whitespace(_) | Token::EOF))
        .take_while(|t| !matches!(t.token, Token::SemiColon | Token::LParen))
        .take(32)
        .collect();
    let pos = skip_create_modifiers(&header_code, 1);
    header_code
        .get(pos)
        .copied()
        .is_some_and(is_routine_object_kind)
}

fn is_routine_object_kind(tok: &TokenWithSpan) -> bool {
    is_unquoted_kw(tok, "FUNCTION")
        || is_unquoted_kw(tok, "PROCEDURE")
        || is_unquoted_kw(tok, "TRIGGER")
}

fn update_routine_split_blocks(
    tokens: &[TokenWithSpan],
    idx: usize,
    blocks: &mut Vec<SplitBlock>,
) -> usize {
    let tok = &tokens[idx];
    if is_unquoted_kw(tok, "BEGIN") {
        blocks.push(SplitBlock::Begin);
        return idx + 1;
    }
    if is_unquoted_kw(tok, "CASE") && !blocks.is_empty() {
        blocks.push(SplitBlock::Case);
        return idx + 1;
    }
    if is_unquoted_kw(tok, "END") && !blocks.is_empty() {
        return handle_split_end(tokens, idx, blocks);
    }
    idx + 1
}

fn handle_split_end(tokens: &[TokenWithSpan], idx: usize, blocks: &mut Vec<SplitBlock>) -> usize {
    if let Some(next) = next_code_idx(tokens, idx + 1) {
        if is_unquoted_kw(&tokens[next], "CASE") {
            pop_matching_split_block(blocks, SplitBlock::Case);
            return next + 1;
        }
        if is_compound_end_tail(&tokens[next]) {
            return next + 1;
        }
    }
    blocks.pop();
    idx + 1
}

fn pop_matching_split_block(blocks: &mut Vec<SplitBlock>, target: SplitBlock) {
    if let Some(pos) = blocks.iter().rposition(|&b| b == target) {
        blocks.truncate(pos);
    }
}

fn is_compound_end_tail(tok: &TokenWithSpan) -> bool {
    is_unquoted_kw(tok, "IF")
        || is_unquoted_kw(tok, "LOOP")
        || is_unquoted_kw(tok, "WHILE")
        || is_unquoted_kw(tok, "REPEAT")
}

fn push_trimmed_slice(
    tokens: &[TokenWithSpan],
    start: usize,
    end: usize,
    out: &mut Vec<Vec<TokenWithSpan>>,
) {
    let slice = &tokens[start..end];
    let first = slice
        .iter()
        .position(|t| !matches!(t.token, Token::Whitespace(_) | Token::EOF));
    let last = slice
        .iter()
        .rposition(|t| !matches!(t.token, Token::Whitespace(_) | Token::EOF));
    if let (Some(first), Some(last)) = (first, last) {
        out.push(slice[first..=last].to_vec());
    }
}

fn finalize_statement(tokens: Vec<TokenWithSpan>) -> Option<ExtractedStatement> {
    let code_tokens: Vec<&TokenWithSpan> = tokens
        .iter()
        .filter(|t| !matches!(t.token, Token::Whitespace(_) | Token::EOF))
        .collect();
    let first = *code_tokens.first()?;
    let last = *code_tokens.last()?;
    let start_line = first.span.start.line as u32;
    let end_line = (last.span.end.line as u32).max(start_line);
    let name = extract_object_name(&code_tokens);
    let scored = score_statement(&code_tokens);
    Some(ExtractedStatement {
        start_line,
        end_line,
        name,
        decision_points: scored.decision_points,
        cognitive_complexity: scored.cognitive_complexity,
        tokens,
        routine_body_tokens: scored.routine_body_tokens,
    })
}

fn extract_object_name(code: &[&TokenWithSpan]) -> Option<String> {
    if !code.first().is_some_and(|t| is_unquoted_kw(t, "CREATE")) {
        return None;
    }
    let mut pos = skip_create_modifiers(code, 1);
    let kind_tok = *code.get(pos)?;
    if !is_named_object_kind(kind_tok) {
        return None;
    }
    let is_index = is_unquoted_kw(kind_tok, "INDEX");
    pos += 1;
    if is_index
        && code
            .get(pos)
            .is_some_and(|t| is_unquoted_kw(t, "CONCURRENTLY"))
    {
        pos += 1;
    }
    pos = skip_if_not_exists(code, pos);
    let first = *code.get(pos)?;
    if is_index && is_unquoted_kw(first, "ON") {
        return None;
    }
    read_qualified_name(code, pos)
}

fn skip_create_modifiers(code: &[&TokenWithSpan], mut pos: usize) -> usize {
    while pos < code.len() {
        let tok = code[pos];
        if is_unquoted_kw(tok, "OR")
            && code
                .get(pos + 1)
                .is_some_and(|t| is_unquoted_kw(t, "REPLACE"))
        {
            pos += 2;
        } else if is_single_create_modifier(tok) {
            pos += 1;
        } else if is_unquoted_kw(tok, "DEFINER") {
            pos = skip_definer_clause(code, pos + 1);
        } else if is_unquoted_kw(tok, "ALGORITHM") {
            pos = skip_key_eq_val(code, pos + 1);
        } else if is_unquoted_kw(tok, "SQL")
            && code
                .get(pos + 1)
                .is_some_and(|t| is_unquoted_kw(t, "SECURITY"))
        {
            pos = (pos + 3).min(code.len());
        } else {
            break;
        }
    }
    pos
}

fn is_single_create_modifier(tok: &TokenWithSpan) -> bool {
    let Some(word) = unquoted_word(tok) else {
        return false;
    };
    matches!(
        word.to_ascii_uppercase().as_str(),
        "TEMP"
            | "TEMPORARY"
            | "UNLOGGED"
            | "MATERIALIZED"
            | "UNIQUE"
            | "RECURSIVE"
            | "GLOBAL"
            | "LOCAL"
            | "FULLTEXT"
            | "SPATIAL"
            | "CONSTRAINT"
    )
}

fn skip_key_eq_val(code: &[&TokenWithSpan], mut pos: usize) -> usize {
    if code.get(pos).is_some_and(|t| t.token == Token::Eq) {
        pos += 1;
    }
    (pos + 1).min(code.len())
}

fn skip_definer_clause(code: &[&TokenWithSpan], mut pos: usize) -> usize {
    if code.get(pos).is_some_and(|t| t.token == Token::Eq) {
        pos += 1;
    }
    while pos < code.len() {
        let tok = code[pos];
        if is_named_object_kind(tok)
            || is_unquoted_kw(tok, "SQL")
            || is_unquoted_kw(tok, "ALGORITHM")
        {
            break;
        }
        pos += 1;
    }
    pos
}

fn is_named_object_kind(tok: &TokenWithSpan) -> bool {
    let Some(word) = unquoted_word(tok) else {
        return false;
    };
    matches!(
        word.to_ascii_uppercase().as_str(),
        "TABLE" | "VIEW" | "INDEX" | "FUNCTION" | "PROCEDURE" | "TRIGGER" | "TYPE" | "MACRO"
    )
}

fn skip_if_not_exists(code: &[&TokenWithSpan], pos: usize) -> usize {
    if code.get(pos).is_some_and(|t| is_unquoted_kw(t, "IF"))
        && code.get(pos + 1).is_some_and(|t| is_unquoted_kw(t, "NOT"))
        && code
            .get(pos + 2)
            .is_some_and(|t| is_unquoted_kw(t, "EXISTS"))
    {
        pos + 3
    } else {
        pos
    }
}

fn read_qualified_name(code: &[&TokenWithSpan], mut pos: usize) -> Option<String> {
    let Token::Word(first) = &code.get(pos)?.token else {
        return None;
    };
    let mut name = first.value.clone();
    pos += 1;
    while code.get(pos).is_some_and(|t| t.token == Token::Period)
        && let Some(Token::Word(part)) = code.get(pos + 1).map(|t| &t.token)
    {
        name.push('.');
        name.push_str(&part.value);
        pos += 2;
    }
    Some(name)
}

struct StatementScore {
    decision_points: u32,
    cognitive_complexity: u32,
    routine_body_tokens: Vec<ExpandedRoutineBody>,
}

fn score_statement(code: &[&TokenWithSpan]) -> StatementScore {
    let routine_kind = classify_routine_statement(code);
    let mut scorer = ComplexityScorer::new(routine_kind.is_inline_routine());
    let mut routine_body_tokens = Vec::new();
    let mut idx = 0usize;

    while idx < code.len() {
        if routine_kind.allows_dollar_body()
            && is_routine_dollar_body(code, idx, routine_kind)
            && let Token::DollarQuotedString(dq) = &code[idx].token
        {
            if let Some(inner) =
                tokenize_pg_routine_body(&dq.value, code[idx].span.start.line as u32)
            {
                let inner_code: Vec<&TokenWithSpan> = inner
                    .iter()
                    .filter(|t| !matches!(t.token, Token::Whitespace(_) | Token::EOF))
                    .collect();
                scorer.score_slice(&inner_code, true);
                routine_body_tokens.push(ExpandedRoutineBody {
                    outer_start: code[idx].span.start,
                    tokens: inner,
                });
            }
            idx += 1;
            continue;
        }
        idx = scorer.step(code, idx);
    }

    StatementScore {
        decision_points: scorer.decision_points,
        cognitive_complexity: scorer.cognitive_complexity,
        routine_body_tokens,
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum RoutineKind {
    None,
    CreateRoutine,
    PgDo,
}

impl RoutineKind {
    fn is_inline_routine(self) -> bool {
        self == RoutineKind::CreateRoutine
    }

    fn allows_dollar_body(self) -> bool {
        self != RoutineKind::None
    }
}

fn classify_routine_statement(code: &[&TokenWithSpan]) -> RoutineKind {
    let Some(first) = code.first() else {
        return RoutineKind::None;
    };
    if is_unquoted_kw(first, "DO") {
        return RoutineKind::PgDo;
    }
    if !is_unquoted_kw(first, "CREATE") {
        return RoutineKind::None;
    }
    let pos = skip_create_modifiers(code, 1);
    let is_routine = code.get(pos).copied().is_some_and(is_routine_object_kind);
    if is_routine {
        RoutineKind::CreateRoutine
    } else {
        RoutineKind::None
    }
}

fn is_routine_dollar_body(code: &[&TokenWithSpan], idx: usize, kind: RoutineKind) -> bool {
    if !matches!(code[idx].token, Token::DollarQuotedString(_)) {
        return false;
    }
    if !has_sql_compatible_language(code) {
        return false;
    }
    match kind {
        RoutineKind::PgDo => true,
        RoutineKind::CreateRoutine => idx > 0 && is_unquoted_kw(code[idx - 1], "AS"),
        RoutineKind::None => false,
    }
}

fn has_sql_compatible_language(code: &[&TokenWithSpan]) -> bool {
    let Some(lang_idx) = code.iter().position(|t| is_unquoted_kw(t, "LANGUAGE")) else {
        return true;
    };
    let Some(next) = code.get(lang_idx + 1) else {
        return true;
    };
    let lang = match &next.token {
        Token::Word(w) => w.value.as_str(),
        Token::SingleQuotedString(s) => s.as_str(),
        _ => return true,
    };
    lang.eq_ignore_ascii_case("sql") || lang.eq_ignore_ascii_case("plpgsql")
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ActiveBlock {
    Case,
    Begin { has_exception: bool },
    If,
    Loop,
}

struct ComplexityScorer {
    decision_points: u32,
    cognitive_complexity: u32,
    in_routine: bool,
    paren_subquery_stack: Vec<bool>,
    subquery_depth: u32,
    routine_block_depth: u32,
    blocks: Vec<ActiveBlock>,
    between_paren_depths: Vec<usize>,
    expect_loop_keyword: bool,
}

impl ComplexityScorer {
    fn new(in_routine: bool) -> Self {
        Self {
            decision_points: 0,
            cognitive_complexity: 0,
            in_routine,
            paren_subquery_stack: Vec::new(),
            subquery_depth: 0,
            routine_block_depth: 0,
            blocks: Vec::new(),
            between_paren_depths: Vec::new(),
            expect_loop_keyword: false,
        }
    }

    fn score_slice(&mut self, code: &[&TokenWithSpan], in_routine: bool) {
        let saved = self.in_routine;
        self.in_routine = in_routine;
        let mut idx = 0usize;
        while idx < code.len() {
            idx = self.step(code, idx);
        }
        self.in_routine = saved;
    }

    fn depth(&self) -> u32 {
        self.subquery_depth.saturating_add(self.routine_block_depth)
    }

    fn add_decision(&mut self, depth: u32) {
        self.decision_points = self.decision_points.saturating_add(1);
        self.cognitive_complexity = self
            .cognitive_complexity
            .saturating_add(depth.saturating_add(1));
    }

    fn step(&mut self, code: &[&TokenWithSpan], idx: usize) -> usize {
        let tok = code[idx];
        match &tok.token {
            Token::LParen => {
                self.handle_lparen(code, idx);
                idx + 1
            }
            Token::RParen => {
                self.handle_rparen();
                idx + 1
            }
            Token::Word(_) => self.handle_word(code, idx),
            _ => idx + 1,
        }
    }

    fn handle_lparen(&mut self, code: &[&TokenWithSpan], idx: usize) {
        let is_subquery = code
            .get(idx + 1)
            .is_some_and(|next| is_unquoted_kw(next, "SELECT") || is_unquoted_kw(next, "WITH"));
        if is_subquery {
            self.add_decision(self.depth());
            self.subquery_depth += 1;
        }
        self.paren_subquery_stack.push(is_subquery);
    }

    fn handle_rparen(&mut self) {
        let closing_depth = self.paren_subquery_stack.len();
        while self.between_paren_depths.last() == Some(&closing_depth) {
            self.between_paren_depths.pop();
        }
        if self.paren_subquery_stack.pop() == Some(true) {
            self.subquery_depth = self.subquery_depth.saturating_sub(1);
        }
    }

    fn handle_word(&mut self, code: &[&TokenWithSpan], idx: usize) -> usize {
        let Some(word) = unquoted_word(code[idx]) else {
            return idx + 1;
        };
        let upper = word.to_ascii_uppercase();
        if self.handle_query_keyword(code, idx, &upper) {
            return idx + 1;
        }
        if self.in_routine {
            return self.handle_routine_keyword(code, idx, &upper);
        }
        if upper == "END" {
            self.pop_case_block();
        }
        idx + 1
    }

    fn handle_query_keyword(&mut self, code: &[&TokenWithSpan], idx: usize, upper: &str) -> bool {
        if self.handle_case_or_between(upper) {
            return true;
        }
        match upper {
            "AND" => self.handle_and_keyword(code, idx),
            "OR" if !is_non_condition_or(code, idx) => self.add_decision(self.depth()),
            "OR" => {}
            "JOIN" | "STRAIGHT_JOIN" | "UNION" | "INTERSECT" => self.add_decision(self.depth()),
            "EXCEPT" if !is_wildcard_except(code, idx) => self.add_decision(self.depth()),
            "EXCEPT" => {}
            _ => return false,
        }
        true
    }

    fn handle_case_or_between(&mut self, upper: &str) -> bool {
        match upper {
            "CASE" => self.blocks.push(ActiveBlock::Case),
            "WHEN" if self.blocks.last() == Some(&ActiveBlock::Case) => {
                self.add_decision(self.depth());
            }
            "WHEN" => {}
            "BETWEEN" => self
                .between_paren_depths
                .push(self.paren_subquery_stack.len()),
            _ => return false,
        }
        true
    }

    fn handle_and_keyword(&mut self, code: &[&TokenWithSpan], idx: usize) {
        let current_paren = self.paren_subquery_stack.len();
        if self.between_paren_depths.last() == Some(&current_paren) {
            self.between_paren_depths.pop();
            return;
        }
        if is_transaction_chain_and(code, idx) {
            return;
        }
        self.add_decision(self.depth());
    }

    fn handle_routine_keyword(
        &mut self,
        code: &[&TokenWithSpan],
        idx: usize,
        upper: &str,
    ) -> usize {
        if upper == "END" {
            return self.handle_routine_end(code, idx);
        }
        if !self.handle_routine_branch_keyword(code, idx, upper) {
            self.handle_routine_loop_keyword(code, idx, upper);
        }
        idx + 1
    }

    fn handle_routine_branch_keyword(
        &mut self,
        code: &[&TokenWithSpan],
        idx: usize,
        upper: &str,
    ) -> bool {
        match upper {
            "BEGIN" => self.blocks.push(ActiveBlock::Begin {
                has_exception: false,
            }),
            "IF" => self.handle_routine_if(code, idx),
            "ELSIF" | "ELSEIF" => {
                let d = self.depth().saturating_sub(1);
                self.add_decision(d);
            }
            "EXCEPTION" => self.handle_routine_exception(),
            "HANDLER" => self.add_decision(self.depth()),
            _ => return false,
        }
        true
    }

    fn handle_routine_loop_keyword(&mut self, code: &[&TokenWithSpan], idx: usize, upper: &str) {
        match upper {
            "WHILE" => self.enter_loop_block(true),
            "FOR" if is_routine_for_loop(code, idx, self.paren_subquery_stack.len()) => {
                self.enter_loop_block(true);
            }
            "REPEAT" if !code.get(idx + 1).is_some_and(|t| t.token == Token::LParen) => {
                self.enter_loop_block(false);
            }
            "LOOP" => self.handle_routine_loop(),
            _ => {}
        }
    }

    fn handle_routine_if(&mut self, code: &[&TokenWithSpan], idx: usize) {
        if is_ddl_if_guard(code, idx) || is_scalar_if_call(code, idx) {
            return;
        }
        self.add_decision(self.depth());
        self.blocks.push(ActiveBlock::If);
        self.routine_block_depth += 1;
    }

    fn enter_loop_block(&mut self, expects_loop: bool) {
        self.add_decision(self.depth());
        self.blocks.push(ActiveBlock::Loop);
        self.routine_block_depth += 1;
        self.expect_loop_keyword = expects_loop;
    }

    fn handle_routine_loop(&mut self) {
        if self.expect_loop_keyword {
            self.expect_loop_keyword = false;
        } else {
            self.enter_loop_block(false);
        }
    }

    fn handle_routine_exception(&mut self) {
        self.add_decision(self.depth());
        if let Some(ActiveBlock::Begin { has_exception }) = self
            .blocks
            .iter_mut()
            .rev()
            .find(|b| matches!(b, ActiveBlock::Begin { .. }))
            && !*has_exception
        {
            *has_exception = true;
            self.routine_block_depth += 1;
        }
    }

    fn handle_routine_end(&mut self, code: &[&TokenWithSpan], idx: usize) -> usize {
        if let Some(next) = code.get(idx + 1) {
            if is_unquoted_kw(next, "IF") {
                self.pop_routine_block(ActiveBlock::If);
                return idx + 2;
            }
            if is_unquoted_kw(next, "LOOP")
                || is_unquoted_kw(next, "WHILE")
                || is_unquoted_kw(next, "REPEAT")
            {
                self.pop_routine_block(ActiveBlock::Loop);
                return idx + 2;
            }
            if is_unquoted_kw(next, "CASE") {
                self.pop_case_block();
                return idx + 2;
            }
        }
        match self.blocks.pop() {
            Some(ActiveBlock::Begin {
                has_exception: true,
            }) => {
                self.routine_block_depth = self.routine_block_depth.saturating_sub(1);
            }
            Some(ActiveBlock::If | ActiveBlock::Loop) => {
                self.routine_block_depth = self.routine_block_depth.saturating_sub(1);
            }
            _ => {}
        }
        idx + 1
    }

    fn pop_routine_block(&mut self, target: ActiveBlock) {
        if let Some(pos) = self.blocks.iter().rposition(|&b| b == target) {
            self.blocks.remove(pos);
            self.routine_block_depth = self.routine_block_depth.saturating_sub(1);
        }
    }

    fn pop_case_block(&mut self) {
        if let Some(pos) = self.blocks.iter().rposition(|&b| b == ActiveBlock::Case) {
            self.blocks.remove(pos);
        }
    }
}

fn is_ddl_if_guard(code: &[&TokenWithSpan], idx: usize) -> bool {
    let next_exists = code
        .get(idx + 1)
        .is_some_and(|t| is_unquoted_kw(t, "EXISTS"))
        && !code.get(idx + 2).is_some_and(|t| t.token == Token::LParen);
    let next_not_exists = code.get(idx + 1).is_some_and(|t| is_unquoted_kw(t, "NOT"))
        && code
            .get(idx + 2)
            .is_some_and(|t| is_unquoted_kw(t, "EXISTS"))
        && !code.get(idx + 3).is_some_and(|t| t.token == Token::LParen);
    next_exists || next_not_exists
}

fn is_scalar_if_call(code: &[&TokenWithSpan], idx: usize) -> bool {
    if !code.get(idx + 1).is_some_and(|t| t.token == Token::LParen) {
        return false;
    }
    let Some(close) = find_matching_rparen(code, idx + 1) else {
        return false;
    };
    !code
        .get(close + 1)
        .is_some_and(|t| is_unquoted_kw(t, "THEN"))
}

fn find_matching_rparen(code: &[&TokenWithSpan], open: usize) -> Option<usize> {
    let mut depth = 0usize;
    for (idx, tok) in code.iter().enumerate().skip(open) {
        match tok.token {
            Token::LParen => depth += 1,
            Token::RParen => {
                depth -= 1;
                if depth == 0 {
                    return Some(idx);
                }
            }
            _ => {}
        }
    }
    None
}

fn is_routine_for_loop(code: &[&TokenWithSpan], idx: usize, paren_depth: usize) -> bool {
    if paren_depth > 0 || is_non_loop_for_prefix(code, idx) {
        return false;
    }
    code.get(idx + 1)
        .copied()
        .is_some_and(|next| !is_non_loop_for_target(next))
}

fn is_non_loop_for_prefix(code: &[&TokenWithSpan], idx: usize) -> bool {
    idx > 0 && (is_unquoted_kw(code[idx - 1], "HANDLER") || is_unquoted_kw(code[idx - 1], "CURSOR"))
}

fn is_non_loop_for_target(tok: &TokenWithSpan) -> bool {
    let Some(word) = unquoted_word(tok) else {
        return false;
    };
    matches!(
        word.to_ascii_uppercase().as_str(),
        "EACH" | "UPDATE" | "SHARE" | "NO" | "KEY" | "SELECT" | "EXECUTE" | "VALUES" | "WITH"
    )
}

fn is_non_condition_or(code: &[&TokenWithSpan], idx: usize) -> bool {
    if idx == 0 {
        return false;
    }
    let prev = code[idx - 1];
    let Some(next) = code.get(idx + 1) else {
        return false;
    };
    if is_unquoted_kw(prev, "CREATE") && is_unquoted_kw(next, "REPLACE") {
        return true;
    }
    let is_dml = is_unquoted_kw(prev, "INSERT") || is_unquoted_kw(prev, "UPDATE");
    is_dml && is_conflict_action(next)
}

fn is_conflict_action(tok: &TokenWithSpan) -> bool {
    let Some(word) = unquoted_word(tok) else {
        return false;
    };
    matches!(
        word.to_ascii_uppercase().as_str(),
        "REPLACE" | "IGNORE" | "ABORT" | "FAIL" | "ROLLBACK"
    )
}

fn is_transaction_chain_and(code: &[&TokenWithSpan], idx: usize) -> bool {
    if idx == 0 {
        return false;
    }
    let prev = code[idx - 1];
    if !(is_unquoted_kw(prev, "COMMIT") || is_unquoted_kw(prev, "ROLLBACK")) {
        return false;
    }
    let direct_chain = code
        .get(idx + 1)
        .is_some_and(|t| is_unquoted_kw(t, "CHAIN"));
    let no_chain = code.get(idx + 1).is_some_and(|t| is_unquoted_kw(t, "NO"))
        && code
            .get(idx + 2)
            .is_some_and(|t| is_unquoted_kw(t, "CHAIN"));
    direct_chain || no_chain
}

fn is_wildcard_except(code: &[&TokenWithSpan], idx: usize) -> bool {
    idx > 0 && code[idx - 1].token == Token::Mul
}

fn next_code_idx(tokens: &[TokenWithSpan], mut pos: usize) -> Option<usize> {
    while pos < tokens.len() {
        if !matches!(tokens[pos].token, Token::Whitespace(_) | Token::EOF) {
            return Some(pos);
        }
        pos += 1;
    }
    None
}

fn unquoted_word(tok: &TokenWithSpan) -> Option<&str> {
    match &tok.token {
        Token::Word(w) if w.quote_style.is_none() => Some(w.value.as_str()),
        _ => None,
    }
}

fn is_unquoted_kw(tok: &TokenWithSpan, expected: &str) -> bool {
    unquoted_word(tok).is_some_and(|w| w.eq_ignore_ascii_case(expected))
}
