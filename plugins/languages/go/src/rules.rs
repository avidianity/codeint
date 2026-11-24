use std::collections::{BTreeSet, HashSet};

use codeint_model::language::FlaggedLine;
use codeint_model::scan::{finalize_flagged, find_matching_delim, flag_range};

use crate::functions::{GoFunctionSpan, find_control_body, parse_func_literal_header};
use crate::lexer::{GoKind, GoToken};

const RULES: &[&str] = &[
    "conditional-yields-boolean",
    "else-after-return",
    "if-else-returns-boolean",
    "redundant-break",
    "trailing-bare-return",
];

pub fn flagged_line_rules() -> impl Iterator<Item = &'static str> {
    RULES.iter().copied()
}

#[derive(Clone, Copy)]
struct FuncContext {
    has_results: bool,
    has_named_results: bool,
}

struct IfHeader {
    if_idx: usize,
    then_open: usize,
    then_close: usize,
    is_else_if: bool,
    has_init: bool,
}

struct ElseBlock {
    else_idx: usize,
    else_open: usize,
    else_close: usize,
    all_prior_return: bool,
}

struct IfChainState {
    is_else_if: bool,
    prior_branches_return: bool,
    scope_vars: HashSet<String>,
}

pub(crate) fn evaluate(
    tokens: &[GoToken],
    spans: &[GoFunctionSpan],
    code_lines: &BTreeSet<u32>,
) -> Vec<FlaggedLine> {
    let mut evaluator = RuleEvaluator {
        tokens,
        code_lines,
        hits: BTreeSet::new(),
    };
    for span in spans {
        let ctx = FuncContext {
            has_results: span.has_results,
            has_named_results: span.has_named_results,
        };
        evaluator.inspect_function_body(span.body_open, span.body_close, ctx);
    }
    evaluator.inspect_package_level_literals(spans);
    finalize_flagged(evaluator.hits)
}

struct RuleEvaluator<'a> {
    tokens: &'a [GoToken],
    code_lines: &'a BTreeSet<u32>,
    hits: BTreeSet<(u32, &'static str)>,
}

impl RuleEvaluator<'_> {
    fn inspect_package_level_literals(&mut self, spans: &[GoFunctionSpan]) {
        let mut cursor = 0usize;
        while cursor < self.tokens.len() {
            if let Some(span) = spans.iter().find(|s| s.body_open == cursor) {
                cursor = span.body_close + 1;
                continue;
            }
            if let Some(next) = self.try_inspect_func_literal(cursor, self.tokens.len()) {
                cursor = next;
                continue;
            }
            cursor += 1;
        }
    }

    fn try_inspect_func_literal(&mut self, cursor: usize, limit: usize) -> Option<usize> {
        if !self.tokens[cursor].is_kw("func") {
            return None;
        }
        let lit = parse_func_literal_header(self.tokens, cursor, limit)?;
        let ctx = FuncContext {
            has_results: lit.has_results,
            has_named_results: lit.has_named_results,
        };
        self.inspect_function_body(lit.body_open, lit.body_close, ctx);
        Some(lit.body_close + 1)
    }

    fn inspect_function_body(&mut self, body_open: usize, body_close: usize, ctx: FuncContext) {
        if body_open + 1 >= body_close {
            return;
        }
        let stmts = split_statements(self.tokens, body_open + 1, body_close);
        if !ctx.has_results {
            self.flag_last_matching_stmt(&stmts, "trailing-bare-return", is_unlabeled_bare_return);
        }
        self.inspect_statements(&stmts, ctx);
    }

    fn flag_last_matching_stmt(
        &mut self,
        stmts: &[(usize, usize)],
        rule: &'static str,
        matcher: fn(&[GoToken]) -> bool,
    ) {
        if let Some(&(last_start, last_end)) = stmts.last()
            && matcher(&self.tokens[last_start..last_end])
        {
            let line = self.tokens[last_start].line;
            flag_range(line, line, rule, self.code_lines, &mut self.hits);
        }
    }

    fn inspect_block(&mut self, start: usize, end: usize, ctx: FuncContext) {
        if start >= end {
            return;
        }
        let stmts = split_statements(self.tokens, start, end);
        self.inspect_statements(&stmts, ctx);
    }

    fn inspect_statements(&mut self, stmts: &[(usize, usize)], ctx: FuncContext) {
        for (idx, &(raw_start, end)) in stmts.iter().enumerate() {
            let start = skip_leading_label(self.tokens, raw_start, end);
            if start >= end {
                continue;
            }
            let next_stmt = stmts.get(idx + 1).copied();
            self.inspect_statement(start, end, next_stmt, ctx);
        }
    }

    fn inspect_statement(
        &mut self,
        start: usize,
        end: usize,
        next_stmt: Option<(usize, usize)>,
        ctx: FuncContext,
    ) {
        let first = &self.tokens[start];
        if first.is_kw("if") {
            let mut chain = IfChainState {
                is_else_if: false,
                prior_branches_return: true,
                scope_vars: HashSet::new(),
            };
            self.inspect_if_chain(start, end, next_stmt, ctx, &mut chain);
        } else if first.is_kw("switch") || first.is_kw("select") {
            self.inspect_switch_or_select(start, end, ctx);
        } else if first.is_kw("for") {
            self.inspect_for(start, end, ctx);
        } else if first.text == "{" {
            if let Some(close) = find_matching_delim(self.tokens, start)
                && close < end
            {
                self.inspect_block(start + 1, close, ctx);
            }
        } else {
            self.inspect_nested_literals(start, end);
        }
    }

    fn inspect_if_chain(
        &mut self,
        if_idx: usize,
        limit: usize,
        next_stmt: Option<(usize, usize)>,
        ctx: FuncContext,
        chain: &mut IfChainState,
    ) {
        let Some((then_open, then_close)) = find_control_body(self.tokens, if_idx + 1, limit)
        else {
            self.inspect_nested_literals(if_idx + 1, limit);
            return;
        };
        let header = &self.tokens[if_idx + 1..then_open];
        let branch = IfHeader {
            if_idx,
            then_open,
            then_close,
            is_else_if: chain.is_else_if,
            has_init: header_has_init_stmt(header),
        };
        collect_header_short_vars(header, &mut chain.scope_vars);
        self.inspect_nested_literals(if_idx + 1, then_open);

        let else_idx = then_close + 1;
        if else_idx < limit && self.tokens[else_idx].is_kw("else") {
            self.inspect_if_with_else(&branch, else_idx, limit, ctx, chain);
        } else if !branch.is_else_if && !branch.has_init && !ctx.has_named_results {
            self.check_if_followed_by_bool_return(&branch, next_stmt);
        }
        self.inspect_block(then_open + 1, then_close, ctx);
    }

    fn inspect_if_with_else(
        &mut self,
        branch: &IfHeader,
        else_idx: usize,
        limit: usize,
        ctx: FuncContext,
        chain: &mut IfChainState,
    ) {
        let after_else = else_idx + 1;
        if after_else >= limit {
            return;
        }
        let then_stmts = split_statements(self.tokens, branch.then_open + 1, branch.then_close);
        let all_prior_return =
            chain.prior_branches_return && block_always_returns(self.tokens, &then_stmts);
        if self.tokens[after_else].is_kw("if") {
            chain.is_else_if = true;
            chain.prior_branches_return = all_prior_return;
            self.inspect_if_chain(after_else, limit, None, ctx, chain);
            return;
        }
        if self.tokens[after_else].text != "{" {
            return;
        }
        let Some(else_close) = find_matching_delim(self.tokens, after_else) else {
            return;
        };
        if else_close >= limit {
            return;
        }
        let else_block = ElseBlock {
            else_idx,
            else_open: after_else,
            else_close,
            all_prior_return,
        };
        self.check_if_else_rules(branch, &then_stmts, &else_block, ctx, &chain.scope_vars);
        self.inspect_block(after_else + 1, else_close, ctx);
    }

    fn check_if_else_rules(
        &mut self,
        branch: &IfHeader,
        then_stmts: &[(usize, usize)],
        else_block: &ElseBlock,
        ctx: FuncContext,
        scope_vars: &HashSet<String>,
    ) {
        let else_stmts =
            split_statements(self.tokens, else_block.else_open + 1, else_block.else_close);

        if !branch.is_else_if
            && !branch.has_init
            && (self.try_flag_if_else_bool_return(
                branch,
                else_block.else_close,
                ctx,
                then_stmts,
                &else_stmts,
            ) || self.try_flag_conditional_bool_assign(
                branch,
                else_block.else_close,
                then_stmts,
                &else_stmts,
            ))
        {
            return;
        }

        if else_block.all_prior_return
            && !uses_any_var(
                &self.tokens[else_block.else_open + 1..else_block.else_close],
                scope_vars,
            )
        {
            flag_range(
                self.tokens[else_block.else_idx].line,
                self.tokens[else_block.else_close].line,
                "else-after-return",
                self.code_lines,
                &mut self.hits,
            );
        }
    }

    fn try_flag_if_else_bool_return(
        &mut self,
        branch: &IfHeader,
        else_close: usize,
        ctx: FuncContext,
        then_stmts: &[(usize, usize)],
        else_stmts: &[(usize, usize)],
    ) -> bool {
        if ctx.has_named_results {
            return false;
        }
        let Some(a) = single_bool_return(self.tokens, then_stmts) else {
            return false;
        };
        let Some(b) = single_bool_return(self.tokens, else_stmts) else {
            return false;
        };
        if a == b {
            return false;
        }
        flag_range(
            self.tokens[branch.if_idx].line,
            self.tokens[else_close].line,
            "if-else-returns-boolean",
            self.code_lines,
            &mut self.hits,
        );
        true
    }

    fn try_flag_conditional_bool_assign(
        &mut self,
        branch: &IfHeader,
        else_close: usize,
        then_stmts: &[(usize, usize)],
        else_stmts: &[(usize, usize)],
    ) -> bool {
        let Some((lhs_a, val_a)) = single_bool_assign(self.tokens, then_stmts) else {
            return false;
        };
        let Some((lhs_b, val_b)) = single_bool_assign(self.tokens, else_stmts) else {
            return false;
        };
        if val_a == val_b || !same_token_texts(lhs_a, lhs_b) {
            return false;
        }
        flag_range(
            self.tokens[branch.if_idx].line,
            self.tokens[else_close].line,
            "conditional-yields-boolean",
            self.code_lines,
            &mut self.hits,
        );
        true
    }

    fn check_if_followed_by_bool_return(
        &mut self,
        branch: &IfHeader,
        next_stmt: Option<(usize, usize)>,
    ) {
        let Some((next_start, next_end)) = next_stmt else {
            return;
        };
        let then_stmts = split_statements(self.tokens, branch.then_open + 1, branch.then_close);
        let Some(then_bool) = single_bool_return(self.tokens, &then_stmts) else {
            return;
        };
        let Some(next_bool) = bool_return_stmt(&self.tokens[next_start..next_end]) else {
            return;
        };
        if then_bool != next_bool {
            flag_range(
                self.tokens[branch.if_idx].line,
                self.tokens[next_end - 1].line,
                "if-else-returns-boolean",
                self.code_lines,
                &mut self.hits,
            );
        }
    }

    fn inspect_for(&mut self, start: usize, end: usize, ctx: FuncContext) {
        let Some((body_open, body_close)) = find_control_body(self.tokens, start + 1, end) else {
            self.inspect_nested_literals(start + 1, end);
            return;
        };
        self.inspect_nested_literals(start + 1, body_open);
        self.inspect_block(body_open + 1, body_close, ctx);
    }

    fn inspect_switch_or_select(&mut self, start: usize, end: usize, ctx: FuncContext) {
        let body = if self.tokens[start].is_kw("select") {
            let open = start + 1;
            (open < end && self.tokens[open].text == "{")
                .then(|| find_matching_delim(self.tokens, open).filter(|&close| close < end))
                .flatten()
                .map(|close| (open, close))
        } else {
            find_control_body(self.tokens, start + 1, end)
        };
        let Some((body_open, body_close)) = body else {
            self.inspect_nested_literals(start + 1, end);
            return;
        };
        self.inspect_nested_literals(start + 1, body_open);
        let clauses = split_switch_clauses(self.tokens, body_open + 1, body_close);
        for (clause_start, clause_end) in clauses {
            self.inspect_case_clause(clause_start, clause_end, ctx);
        }
    }

    fn inspect_case_clause(&mut self, start: usize, end: usize, ctx: FuncContext) {
        let Some(rel_colon) = find_at_depth_zero(&self.tokens[start + 1..end], |t| t.text == ":")
        else {
            return;
        };
        let colon = start + 1 + rel_colon;
        self.inspect_nested_literals(start + 1, colon);
        let stmts = split_statements(self.tokens, colon + 1, end);
        self.flag_last_matching_stmt(&stmts, "redundant-break", is_unlabeled_bare_break);
        self.inspect_statements(&stmts, ctx);
    }

    fn inspect_nested_literals(&mut self, start: usize, end: usize) {
        let mut cursor = start;
        while cursor < end {
            if let Some(next) = self.try_inspect_func_literal(cursor, end) {
                cursor = next;
                continue;
            }
            cursor += 1;
        }
    }
}

fn split_statements(tokens: &[GoToken], start: usize, end: usize) -> Vec<(usize, usize)> {
    let mut stmts = Vec::new();
    let mut depth = 0usize;
    let mut stmt_start = start;
    let mut idx = start;
    while idx < end {
        idx = step_statement_split(tokens, idx, end, &mut depth, &mut stmt_start, &mut stmts);
    }
    if stmt_start < end {
        stmts.push((stmt_start, end));
    }
    stmts
}

fn step_statement_split(
    tokens: &[GoToken],
    idx: usize,
    end: usize,
    depth: &mut usize,
    stmt_start: &mut usize,
    stmts: &mut Vec<(usize, usize)>,
) -> usize {
    let tok = &tokens[idx];
    if *depth == 0
        && is_header_control_kw(tok)
        && let Some((_, body_close)) = find_control_body(tokens, idx + 1, end)
    {
        return body_close + 1;
    }
    if *depth == 0 && tok.is_semi() {
        if *stmt_start < idx {
            stmts.push((*stmt_start, idx));
        }
        *stmt_start = idx + 1;
        return idx + 1;
    }
    *depth = update_nest_depth(*depth, tok.text.as_str());
    idx + 1
}

fn is_header_control_kw(tok: &GoToken) -> bool {
    tok.kind == GoKind::Keyword && matches!(tok.text.as_str(), "if" | "for" | "switch")
}

fn update_nest_depth(depth: usize, text: &str) -> usize {
    match text {
        "(" | "[" | "{" => depth + 1,
        ")" | "]" | "}" => depth.saturating_sub(1),
        _ => depth,
    }
}

fn find_at_depth_zero(slice: &[GoToken], pred: impl Fn(&GoToken) -> bool) -> Option<usize> {
    let mut depth = 0usize;
    for (idx, tok) in slice.iter().enumerate() {
        if depth == 0 && pred(tok) {
            return Some(idx);
        }
        depth = update_nest_depth(depth, tok.text.as_str());
    }
    None
}

fn skip_leading_label(tokens: &[GoToken], mut start: usize, end: usize) -> usize {
    while start + 1 < end && tokens[start].kind == GoKind::Ident && tokens[start + 1].text == ":" {
        start += 2;
    }
    start
}

fn split_switch_clauses(tokens: &[GoToken], start: usize, end: usize) -> Vec<(usize, usize)> {
    let mut starts = Vec::new();
    let mut depth = 0usize;
    for (idx, tok) in tokens.iter().enumerate().take(end).skip(start) {
        if depth == 0 && (tok.is_kw("case") || tok.is_kw("default")) {
            starts.push(idx);
        }
        depth = update_nest_depth(depth, tok.text.as_str());
    }
    starts
        .iter()
        .enumerate()
        .map(|(i, &clause_start)| {
            let clause_end = starts.get(i + 1).copied().unwrap_or(end);
            (clause_start, clause_end)
        })
        .collect()
}

fn is_unlabeled_bare_return(stmt: &[GoToken]) -> bool {
    stmt.len() == 1 && stmt[0].is_kw("return")
}

fn is_unlabeled_bare_break(stmt: &[GoToken]) -> bool {
    stmt.len() == 1 && stmt[0].is_kw("break")
}

fn bool_ident(tok: &GoToken) -> Option<bool> {
    if tok.kind != GoKind::Ident {
        return None;
    }
    match tok.text.as_str() {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    }
}

fn bool_return_stmt(stmt: &[GoToken]) -> Option<bool> {
    if stmt.len() == 2 && stmt[0].is_kw("return") {
        bool_ident(&stmt[1])
    } else {
        None
    }
}

fn single_bool_return(tokens: &[GoToken], stmts: &[(usize, usize)]) -> Option<bool> {
    if stmts.len() != 1 {
        return None;
    }
    let (start, end) = stmts[0];
    bool_return_stmt(&tokens[start..end])
}

fn single_bool_assign<'a>(
    tokens: &'a [GoToken],
    stmts: &[(usize, usize)],
) -> Option<(&'a [GoToken], bool)> {
    if stmts.len() != 1 {
        return None;
    }
    let (start, end) = stmts[0];
    let slice = &tokens[start..end];
    if slice.len() < 3 {
        return None;
    }
    let rhs_bool = bool_ident(slice.last()?)?;
    if slice[slice.len() - 2].text != "=" {
        return None;
    }
    let lhs = &slice[..slice.len() - 2];
    if lhs
        .iter()
        .any(|t| t.text == "," || t.kind == GoKind::Keyword)
    {
        return None;
    }
    Some((lhs, rhs_bool))
}

fn same_token_texts(a: &[GoToken], b: &[GoToken]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.text == y.text)
}

fn block_always_returns(tokens: &[GoToken], stmts: &[(usize, usize)]) -> bool {
    let Some(&(raw_start, end)) = stmts.last() else {
        return false;
    };
    let start = skip_leading_label(tokens, raw_start, end);
    if start >= end {
        return false;
    }
    let first = &tokens[start];
    if first.is_kw("return") {
        return true;
    }
    first.is_kw("if") && if_always_returns(tokens, start, end)
}

fn if_always_returns(tokens: &[GoToken], if_idx: usize, limit: usize) -> bool {
    let Some((then_open, then_close)) = find_control_body(tokens, if_idx + 1, limit) else {
        return false;
    };
    let then_stmts = split_statements(tokens, then_open + 1, then_close);
    if !block_always_returns(tokens, &then_stmts) {
        return false;
    }
    let else_idx = then_close + 1;
    if else_idx + 1 >= limit || !tokens[else_idx].is_kw("else") {
        return false;
    }
    let after = else_idx + 1;
    if tokens[after].is_kw("if") {
        return if_always_returns(tokens, after, limit);
    }
    if tokens[after].text == "{"
        && let Some(else_close) = find_matching_delim(tokens, after)
        && else_close < limit
    {
        let else_stmts = split_statements(tokens, after + 1, else_close);
        return block_always_returns(tokens, &else_stmts);
    }
    false
}

fn header_has_init_stmt(header: &[GoToken]) -> bool {
    find_at_depth_zero(header, |t| t.text == ";").is_some()
}

fn collect_header_short_vars(header: &[GoToken], out: &mut HashSet<String>) {
    let Some(semi) = find_at_depth_zero(header, |t| t.text == ";") else {
        return;
    };
    let init = &header[..semi];
    let Some(decl_pos) = find_at_depth_zero(init, |t| t.text == ":=") else {
        return;
    };
    for tok in &init[..decl_pos] {
        if tok.kind == GoKind::Ident && tok.text != "_" {
            out.insert(tok.text.clone());
        }
    }
}

fn uses_any_var(slice: &[GoToken], vars: &HashSet<String>) -> bool {
    if vars.is_empty() {
        return false;
    }
    slice.iter().enumerate().any(|(idx, tok)| {
        tok.kind == GoKind::Ident
            && vars.contains(&tok.text)
            && !(idx > 0 && slice[idx - 1].text == ".")
    })
}
