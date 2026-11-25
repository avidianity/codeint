use std::collections::BTreeSet;

use codeint_model::language::Function;
use codeint_model::scan::{find_matching_delim, paren_then_brace, score_function_spans};

use crate::lexer::{DartKind, DartToken, is_ident_or_contextual_kw, matching_open_paren};

struct DartFunctionSpan {
    header_start: usize,
    body_start: usize,
    body_end: usize,
    start_line: u32,
    end_line: u32,
    name: Option<String>,
    is_block: bool,
}

struct TokenContext {
    enclosing_delim: Vec<Option<u8>>,
    switch_expr_arrows: Vec<bool>,
    switch_expr_arm_counts: Vec<u32>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum DartBoolOp {
    And,
    Or,
}

const NON_FN_PAREN_KEYWORDS: [&str; 13] = [
    "if", "for", "while", "switch", "catch", "assert", "new", "const", "case", "super", "this",
    "on", "when",
];

pub(crate) fn extract(tokens: &[DartToken], code_lines: &BTreeSet<u32>) -> Vec<Function> {
    let ctx = analyze_token_context(tokens);
    let mut spans = Vec::new();
    let mut claimed_block_bodies = BTreeSet::new();
    for (idx, tok) in tokens.iter().enumerate() {
        let found = match (tok.kind, tok.text.as_str()) {
            (DartKind::Punct, "(") => {
                try_paren_function(tokens, idx, &ctx, &mut claimed_block_bodies)
            }
            (DartKind::Keyword, "get") => {
                try_getter_function(tokens, idx, &ctx, &mut claimed_block_bodies)
            }
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
    let ternary_mask = compute_ternary_mask(tokens, &ctx);
    let mut functions = score_function_spans(tokens, code_lines, &raw_spans, |toks, idx| {
        decision_weight(toks, idx, &ternary_mask, &ctx)
    });
    for (owner, func) in functions.iter_mut().enumerate() {
        func.name = spans[owner].name.clone();
        func.cognitive_complexity = compute_dart_cognitive(tokens, &spans, owner, &ternary_mask);
    }
    functions
}

fn analyze_token_context(tokens: &[DartToken]) -> TokenContext {
    let enclosing_delim = compute_enclosing_delims(tokens);
    let mut switch_expr_arrows = vec![false; tokens.len()];
    let mut switch_expr_arm_counts = vec![0u32; tokens.len()];
    for (idx, tok) in tokens.iter().enumerate() {
        if is_kw(tok, "switch")
            && !is_dart_member_access(tokens, idx)
            && let Some((_, _, open_brace, close_brace)) = paren_then_brace(tokens, idx)
        {
            switch_expr_arm_counts[idx] =
                scan_switch_expr_arms(tokens, open_brace, close_brace, &mut switch_expr_arrows);
        }
    }
    TokenContext {
        enclosing_delim,
        switch_expr_arrows,
        switch_expr_arm_counts,
    }
}

fn compute_enclosing_delims(tokens: &[DartToken]) -> Vec<Option<u8>> {
    let mut out = Vec::with_capacity(tokens.len());
    let mut stack: Vec<u8> = Vec::new();
    for tok in tokens {
        let text = tok.text.as_str();
        if is_close_delim(text) {
            stack.pop();
        }
        out.push(stack.last().copied());
        match text {
            "(" => stack.push(b'('),
            "[" => stack.push(b'['),
            "{" => stack.push(b'{'),
            _ => {}
        }
    }
    out
}

struct SwitchArmScan {
    arm_arrows: Vec<usize>,
    level: usize,
    seen_arrow_in_arm: bool,
}

enum SwitchArmStep {
    Advance(usize),
    NotSwitchExpr,
}

fn scan_switch_expr_arms(
    tokens: &[DartToken],
    open_brace: usize,
    close_brace: usize,
    switch_expr_arrows: &mut [bool],
) -> u32 {
    let mut scan = SwitchArmScan {
        arm_arrows: Vec::new(),
        level: 0,
        seen_arrow_in_arm: false,
    };
    let mut idx = open_brace + 1;
    while idx < close_brace {
        match step_switch_expr_token(tokens, idx, close_brace, &mut scan) {
            SwitchArmStep::Advance(next) => idx = next,
            SwitchArmStep::NotSwitchExpr => return 0,
        }
    }
    for arrow_idx in &scan.arm_arrows {
        switch_expr_arrows[*arrow_idx] = true;
    }
    u32::try_from(scan.arm_arrows.len())
        .unwrap_or(u32::MAX)
        .saturating_sub(1)
}

fn step_switch_expr_token(
    tokens: &[DartToken],
    idx: usize,
    close_brace: usize,
    scan: &mut SwitchArmScan,
) -> SwitchArmStep {
    let tok = &tokens[idx];
    let text = tok.text.as_str();
    if is_open_delim(text) {
        scan.level += 1;
        return SwitchArmStep::Advance(idx + 1);
    }
    if is_close_delim(text) {
        scan.level = scan.level.saturating_sub(1);
        return SwitchArmStep::Advance(idx + 1);
    }
    if scan.level > 0 {
        return SwitchArmStep::Advance(idx + 1);
    }
    step_switch_expr_top_level(tokens, idx, close_brace, scan)
}

fn step_switch_expr_top_level(
    tokens: &[DartToken],
    idx: usize,
    close_brace: usize,
    scan: &mut SwitchArmScan,
) -> SwitchArmStep {
    let tok = &tokens[idx];
    let text = tok.text.as_str();
    if is_switch_stmt_label(tokens, idx, tok) {
        return SwitchArmStep::NotSwitchExpr;
    }
    if text == "<"
        && let Some(after) = match_expr_type_args(tokens, idx)
    {
        return SwitchArmStep::Advance(after.min(close_brace));
    }
    if text == "=>" && !scan.seen_arrow_in_arm {
        scan.arm_arrows.push(idx);
        scan.seen_arrow_in_arm = true;
    } else if text == "," {
        scan.seen_arrow_in_arm = false;
    }
    SwitchArmStep::Advance(idx + 1)
}

fn is_switch_stmt_label(tokens: &[DartToken], idx: usize, tok: &DartToken) -> bool {
    (is_kw(tok, "case") || is_kw(tok, "default")) && !is_dart_member_access(tokens, idx)
}

fn try_paren_function(
    tokens: &[DartToken],
    open_paren: usize,
    ctx: &TokenContext,
    claimed_block_bodies: &mut BTreeSet<usize>,
) -> Option<DartFunctionSpan> {
    if open_paren > 0 {
        let prev = &tokens[open_paren - 1];
        if prev.kind == DartKind::Keyword && NON_FN_PAREN_KEYWORDS.contains(&prev.text.as_str()) {
            return None;
        }
    }
    if is_extension_type_header(tokens, open_paren) {
        return None;
    }
    let close_paren = find_matching_delim(tokens, open_paren)?;
    let (name, header_start) = extract_paren_name(tokens, open_paren);
    let can_have_initializer = name.as_deref().is_some_and(|n| !n.starts_with("operator "))
        && is_valid_ctor_prefix(tokens, header_start);
    let (body_start, body_end, is_block) =
        locate_body_after_params(tokens, close_paren + 1, can_have_initializer, ctx)?;
    if is_block && !claimed_block_bodies.insert(body_start) {
        return None;
    }
    Some(DartFunctionSpan {
        header_start,
        body_start,
        body_end,
        start_line: tokens[open_paren].line,
        end_line: tokens[body_end].end_line,
        name,
        is_block,
    })
}

fn is_extension_type_header(tokens: &[DartToken], open_paren: usize) -> bool {
    let cursor = strip_named_ctor_suffix(tokens, skip_trailing_type_params(tokens, open_paren));
    let Some(before_name) = cursor
        .checked_sub(1)
        .filter(|&pos| is_ident_or_contextual_kw(&tokens[pos]))
    else {
        return false;
    };
    let prefix_end = strip_optional_kw_before(tokens, before_name, "const");
    has_extension_type_prefix(tokens, prefix_end)
}

fn strip_named_ctor_suffix(tokens: &[DartToken], cursor: usize) -> usize {
    if cursor >= 2
        && is_ident_or_contextual_kw(&tokens[cursor - 1])
        && tokens[cursor - 2].text == "."
    {
        return skip_trailing_type_params(tokens, cursor - 2);
    }
    cursor
}

fn strip_optional_kw_before(tokens: &[DartToken], cursor: usize, kw: &str) -> usize {
    if cursor > 0 && is_kw(&tokens[cursor - 1], kw) {
        return cursor - 1;
    }
    cursor
}

fn has_extension_type_prefix(tokens: &[DartToken], cursor: usize) -> bool {
    cursor >= 2
        && tokens[cursor - 1].kind == DartKind::Ident
        && tokens[cursor - 1].text == "type"
        && is_kw(&tokens[cursor - 2], "extension")
}

fn is_valid_ctor_prefix(tokens: &[DartToken], header_start: usize) -> bool {
    let mut start = header_start;
    if start > 0 && is_kw(&tokens[start - 1], "const") {
        start -= 1;
    }
    if start == 0 {
        return true;
    }
    let prev = &tokens[start - 1];
    if matches!(
        prev.text.as_str(),
        "." | "?."
            | ".."
            | "="
            | "=>"
            | "?"
            | ":"
            | ","
            | "("
            | "["
            | "&&"
            | "||"
            | "??"
            | "+"
            | "-"
            | "*"
            | "/"
            | "=="
            | "!="
    ) {
        return false;
    }
    !(prev.kind == DartKind::Keyword
        && matches!(
            prev.text.as_str(),
            "case" | "return" | "throw" | "yield" | "await" | "new" | "in" | "is" | "as"
        ))
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
    if cursor == 0 {
        return None;
    }
    if cursor >= 3
        && tokens[cursor - 2].text == "."
        && is_ident_or_contextual_kw(&tokens[cursor - 1])
        && is_ident_or_contextual_kw(&tokens[cursor - 3])
    {
        let full = format!("{}.{}", tokens[cursor - 3].text, tokens[cursor - 1].text);
        return Some((Some(full), cursor - 3));
    }
    let tok = &tokens[cursor - 1];
    if is_callable_name_token(tok) {
        return Some((Some(tok.text.clone()), cursor - 1));
    }
    None
}

fn is_callable_name_token(tok: &DartToken) -> bool {
    is_ident_or_contextual_kw(tok)
        && !matches!(tok.text.as_str(), "await" | "yield" | "as" | "when" | "on")
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

fn try_getter_function(
    tokens: &[DartToken],
    get_idx: usize,
    ctx: &TokenContext,
    claimed_block_bodies: &mut BTreeSet<usize>,
) -> Option<DartFunctionSpan> {
    if is_dart_member_access(tokens, get_idx) {
        return None;
    }
    let name_tok = tokens.get(get_idx + 1)?;
    if !is_ident_or_contextual_kw(name_tok) {
        return None;
    }
    let (body_start, body_end, is_block) =
        locate_body_after_params(tokens, get_idx + 2, false, ctx)?;
    if is_block && !claimed_block_bodies.insert(body_start) {
        return None;
    }
    Some(DartFunctionSpan {
        header_start: get_idx,
        body_start,
        body_end,
        start_line: tokens[get_idx].line,
        end_line: tokens[body_end].end_line,
        name: Some(name_tok.text.clone()),
        is_block,
    })
}

fn locate_body_after_params(
    tokens: &[DartToken],
    start: usize,
    can_have_initializer: bool,
    ctx: &TokenContext,
) -> Option<(usize, usize, bool)> {
    let pos = skip_modifiers_and_initializers(tokens, start, can_have_initializer)?;
    let tok = tokens.get(pos)?;
    if tok.text == "{" {
        return Some((pos, find_matching_delim(tokens, pos)?, true));
    }
    if tok.text == "=>" && !ctx.switch_expr_arrows[pos] {
        let body_start = pos + 1;
        return Some((
            body_start,
            find_dart_arrow_expr_end(tokens, body_start),
            false,
        ));
    }
    None
}

fn skip_modifiers_and_initializers(
    tokens: &[DartToken],
    mut pos: usize,
    can_have_initializer: bool,
) -> Option<usize> {
    if tokens.get(pos).is_some_and(|t| t.text == ":") {
        if !can_have_initializer || !looks_like_initializer_list_start(tokens, pos + 1) {
            return None;
        }
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

fn looks_like_initializer_list_start(tokens: &[DartToken], pos: usize) -> bool {
    let Some(first) = tokens.get(pos) else {
        return false;
    };
    if first.kind == DartKind::Keyword && matches!(first.text.as_str(), "super" | "this" | "assert")
    {
        return true;
    }
    is_ident_or_contextual_kw(first) && tokens.get(pos + 1).is_some_and(|t| t.text == "=")
}

fn skip_initializer_list(tokens: &[DartToken], mut pos: usize) -> Option<usize> {
    let mut depth = 0usize;
    while pos < tokens.len() {
        match tokens[pos].text.as_str() {
            "(" | "[" => depth += 1,
            ")" | "]" => depth = depth.saturating_sub(1),
            "{" if depth == 0 => {
                if is_embedded_initializer_brace(tokens, pos) {
                    pos = find_matching_delim(tokens, pos)? + 1;
                    continue;
                }
                return Some(pos);
            }
            ";" if depth == 0 => return None,
            _ => {}
        }
        pos += 1;
    }
    None
}

fn is_embedded_initializer_brace(tokens: &[DartToken], brace_idx: usize) -> bool {
    if brace_idx > 0 && is_expr_brace_prefix(tokens, brace_idx) {
        return true;
    }
    let Some(close_idx) = find_matching_delim(tokens, brace_idx) else {
        return true;
    };
    tokens.get(close_idx + 1).is_some_and(|next| {
        matches!(
            next.text.as_str(),
            "," | ";"
                | "."
                | "?."
                | ".."
                | ")"
                | "]"
                | "??"
                | "as"
                | "is"
                | "=="
                | "!="
                | "&&"
                | "||"
                | "?"
                | ":"
        )
    })
}

fn is_expr_brace_prefix(tokens: &[DartToken], brace_idx: usize) -> bool {
    let prev = &tokens[brace_idx - 1];
    if matches!(
        prev.text.as_str(),
        "=" | "??" | "?" | ":" | "," | "(" | "[" | "=>" | "const" | ">" | ">>" | ">>>"
    ) {
        return true;
    }
    prev.text == ")"
        && matching_open_paren(tokens, brace_idx - 1)
            .is_some_and(|open| open > 0 && is_kw(&tokens[open - 1], "switch"))
}

enum ArrowExprStep {
    Advance(usize),
    End(usize),
}

fn find_dart_arrow_expr_end(tokens: &[DartToken], start: usize) -> usize {
    let mut depth = 0usize;
    let mut idx = start;
    while idx < tokens.len() {
        match step_arrow_expr_token(tokens, idx, &mut depth) {
            ArrowExprStep::Advance(next) => idx = next,
            ArrowExprStep::End(end) => return end,
        }
    }
    tokens.len().saturating_sub(1)
}

fn step_arrow_expr_token(tokens: &[DartToken], idx: usize, depth: &mut usize) -> ArrowExprStep {
    let text = tokens[idx].text.as_str();
    if is_open_delim(text) {
        *depth += 1;
        return ArrowExprStep::Advance(idx + 1);
    }
    if is_close_delim(text) {
        if *depth == 0 {
            return ArrowExprStep::End(idx.saturating_sub(1));
        }
        *depth -= 1;
        return ArrowExprStep::Advance(idx + 1);
    }
    if *depth > 0 {
        return ArrowExprStep::Advance(idx + 1);
    }
    if matches!(text, ";" | ",") {
        return ArrowExprStep::End(idx.saturating_sub(1));
    }
    if text == "<"
        && let Some(after) = match_expr_type_args(tokens, idx)
    {
        return ArrowExprStep::Advance(after);
    }
    ArrowExprStep::Advance(idx + 1)
}

enum TypeArgStep {
    Continue,
    Done(usize),
    Invalid,
}

fn match_expr_type_args(tokens: &[DartToken], open_angle: usize) -> Option<usize> {
    let mut angle_depth = 1usize;
    let mut group_depth = 0usize;
    for idx in (open_angle + 1)..tokens.len() {
        match step_type_arg_token(tokens, idx, &mut angle_depth, &mut group_depth) {
            TypeArgStep::Continue => {}
            TypeArgStep::Done(after) => return Some(after),
            TypeArgStep::Invalid => return None,
        }
    }
    None
}

fn step_type_arg_token(
    tokens: &[DartToken],
    idx: usize,
    angle_depth: &mut usize,
    group_depth: &mut usize,
) -> TypeArgStep {
    let tok = &tokens[idx];
    let text = tok.text.as_str();
    if matches!(text, "(" | "{") {
        *group_depth += 1;
        return TypeArgStep::Continue;
    }
    if matches!(text, ")" | "}") {
        if *group_depth == 0 {
            return TypeArgStep::Invalid;
        }
        *group_depth -= 1;
        return TypeArgStep::Continue;
    }
    if text == "<" {
        *angle_depth += 1;
        return TypeArgStep::Continue;
    }
    let closes = angle_close_count(text);
    if closes > 0 {
        return step_type_arg_angle_close(tokens, idx, closes, angle_depth, *group_depth);
    }
    if is_type_arg_token(tok) {
        return TypeArgStep::Continue;
    }
    TypeArgStep::Invalid
}

fn step_type_arg_angle_close(
    tokens: &[DartToken],
    idx: usize,
    closes: usize,
    angle_depth: &mut usize,
    group_depth: usize,
) -> TypeArgStep {
    if closes < *angle_depth {
        *angle_depth -= closes;
        return TypeArgStep::Continue;
    }
    if group_depth == 0 && is_valid_after_type_args(tokens.get(idx + 1)) {
        return TypeArgStep::Done(idx + 1);
    }
    TypeArgStep::Invalid
}

fn is_type_arg_token(tok: &DartToken) -> bool {
    match tok.kind {
        DartKind::Ident => true,
        DartKind::Keyword => {
            matches!(
                tok.text.as_str(),
                "dynamic" | "void" | "Function" | "required" | "covariant" | "extends"
            ) || is_ident_or_contextual_kw(tok)
        }
        DartKind::Punct => matches!(tok.text.as_str(), "." | "," | "?" | "[" | "]"),
        DartKind::StringLit | DartKind::NumberLit => false,
    }
}

fn is_valid_after_type_args(next: Option<&DartToken>) -> bool {
    let Some(tok) = next else {
        return true;
    };
    !matches!(
        tok.kind,
        DartKind::Ident | DartKind::StringLit | DartKind::NumberLit
    )
}

fn decision_weight(
    tokens: &[DartToken],
    idx: usize,
    ternary_mask: &[bool],
    ctx: &TokenContext,
) -> u32 {
    let tok = &tokens[idx];
    match (tok.kind, tok.text.as_str()) {
        (DartKind::Keyword, "if" | "for" | "while" | "catch") => 1,
        (DartKind::Keyword, "case") => u32::from(ctx.enclosing_delim[idx] == Some(b'{')),
        (DartKind::Keyword, "switch") => ctx.switch_expr_arm_counts[idx],
        (DartKind::Punct, "&&" | "||" | "??") => 1,
        (DartKind::Punct, "?") if ternary_mask[idx] => 1,
        _ => 0,
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum CaseHeaderContext {
    Outside,
    Pattern,
    Guard,
}

fn compute_ternary_mask(tokens: &[DartToken], ctx: &TokenContext) -> Vec<bool> {
    let mut mask = vec![false; tokens.len()];
    for idx in (0..tokens.len()).rev() {
        if tokens[idx].kind == DartKind::Punct && tokens[idx].text == "?" {
            mask[idx] = is_ternary_operator(tokens, idx, &mask, ctx);
        }
    }
    mask
}

fn is_ternary_operator(
    tokens: &[DartToken],
    q_idx: usize,
    ternary_mask: &[bool],
    ctx: &TokenContext,
) -> bool {
    if tokens.get(q_idx + 1).is_some_and(|tok| tok.text == ":") {
        return false;
    }
    match case_header_context(tokens, q_idx, ctx) {
        CaseHeaderContext::Pattern => false,
        CaseHeaderContext::Guard => {
            has_required_ternary_colons(tokens, q_idx, 2, true, ternary_mask)
        }
        CaseHeaderContext::Outside => {
            has_required_ternary_colons(tokens, q_idx, 1, false, ternary_mask)
        }
    }
}

fn case_header_context(
    tokens: &[DartToken],
    q_idx: usize,
    ctx: &TokenContext,
) -> CaseHeaderContext {
    let mut depth = 0usize;
    let mut saw_when = false;
    let mut idx = q_idx;
    while idx > 0 {
        idx -= 1;
        if let Some(found) = step_case_header_back(tokens, idx, &mut depth, &mut saw_when, ctx) {
            return found;
        }
    }
    CaseHeaderContext::Outside
}

fn step_case_header_back(
    tokens: &[DartToken],
    idx: usize,
    depth: &mut usize,
    saw_when: &mut bool,
    ctx: &TokenContext,
) -> Option<CaseHeaderContext> {
    let tok = &tokens[idx];
    let text = tok.text.as_str();
    if is_close_delim(text) {
        *depth += 1;
        return None;
    }
    if is_open_delim(text) {
        if *depth == 0 {
            return Some(CaseHeaderContext::Outside);
        }
        *depth -= 1;
        return None;
    }
    if *depth > 0 {
        return None;
    }
    classify_case_header_token(tokens, idx, tok, saw_when, ctx)
}

fn classify_case_header_token(
    tokens: &[DartToken],
    idx: usize,
    tok: &DartToken,
    saw_when: &mut bool,
    ctx: &TokenContext,
) -> Option<CaseHeaderContext> {
    if matches!(tok.text.as_str(), ";" | ":") {
        return Some(CaseHeaderContext::Outside);
    }
    if is_kw(tok, "when") {
        *saw_when = true;
        return None;
    }
    if !is_kw(tok, "case") || is_dart_member_access(tokens, idx) {
        return None;
    }
    if !*saw_when {
        return Some(CaseHeaderContext::Pattern);
    }
    Some(if ctx.enclosing_delim[idx] == Some(b'{') {
        CaseHeaderContext::Guard
    } else {
        CaseHeaderContext::Outside
    })
}

struct TernaryColonScan<'a> {
    ternary_mask: &'a [bool],
    depth: usize,
    nested: usize,
    colons: usize,
    required_colons: usize,
    stop_on_stmt_kw: bool,
}

fn has_required_ternary_colons(
    tokens: &[DartToken],
    q_idx: usize,
    required_colons: usize,
    stop_on_stmt_kw: bool,
    ternary_mask: &[bool],
) -> bool {
    let mut scan = TernaryColonScan {
        ternary_mask,
        depth: 0,
        nested: 0,
        colons: 0,
        required_colons,
        stop_on_stmt_kw,
    };
    for idx in (q_idx + 1)..tokens.len() {
        if let Some(done) = step_ternary_colon_scan(tokens, idx, &mut scan) {
            return done;
        }
    }
    false
}

fn step_ternary_colon_scan(
    tokens: &[DartToken],
    idx: usize,
    scan: &mut TernaryColonScan<'_>,
) -> Option<bool> {
    let text = tokens[idx].text.as_str();
    if is_open_delim(text) {
        scan.depth += 1;
        return None;
    }
    if is_close_delim(text) {
        if scan.depth == 0 {
            return Some(false);
        }
        scan.depth -= 1;
        return None;
    }
    if scan.depth > 0 {
        return None;
    }
    if scan.ternary_mask[idx] {
        scan.nested += 1;
        return None;
    }
    if text == ":" {
        return step_ternary_colon_hit(scan);
    }
    is_ternary_colon_boundary(tokens, idx, scan.stop_on_stmt_kw).then_some(false)
}

fn step_ternary_colon_hit(scan: &mut TernaryColonScan<'_>) -> Option<bool> {
    if scan.nested > 0 {
        scan.nested -= 1;
        return None;
    }
    scan.colons += 1;
    (scan.colons >= scan.required_colons).then_some(true)
}

fn is_ternary_colon_boundary(tokens: &[DartToken], idx: usize, stop_on_stmt_kw: bool) -> bool {
    let tok = &tokens[idx];
    let text = tok.text.as_str();
    matches!(text, ";" | "," | "=>" | "=")
        || (is_kw(tok, "when") && !is_dart_member_access(tokens, idx))
        || (stop_on_stmt_kw && is_case_body_boundary_kw(tok))
}

fn is_open_delim(text: &str) -> bool {
    matches!(text, "(" | "[" | "{")
}

fn is_close_delim(text: &str) -> bool {
    matches!(text, ")" | "]" | "}")
}

fn is_case_body_boundary_kw(tok: &DartToken) -> bool {
    tok.kind == DartKind::Keyword
        && matches!(
            tok.text.as_str(),
            "case"
                | "default"
                | "return"
                | "break"
                | "continue"
                | "throw"
                | "if"
                | "for"
                | "while"
                | "do"
                | "switch"
                | "try"
                | "yield"
                | "rethrow"
                | "assert"
        )
}

fn is_kw(tok: &DartToken, kw: &str) -> bool {
    tok.kind == DartKind::Keyword && tok.text == kw
}

fn compute_dart_cognitive(
    tokens: &[DartToken],
    spans: &[DartFunctionSpan],
    owner: usize,
    ternary_mask: &[bool],
) -> u32 {
    let span = &spans[owner];
    let simple_name = span
        .name
        .as_deref()
        .filter(|n| !n.contains(' ') && !n.contains('.'));
    let mut analyzer = DartCognitiveAnalyzer {
        tokens,
        spans,
        ternary_mask,
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
    ternary_mask: &'a [bool],
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
        if self.ternary_mask[cursor] {
            self.complexity = self.complexity.saturating_add(depth.saturating_add(1));
            tracker.clear();
            return self.consume_ternary(cursor + 1, to, depth);
        }
        let text = self.tokens[cursor].text.as_str();
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
        let Some(colon) = find_dart_ternary_colon(self.tokens, self.ternary_mask, start, to) else {
            return start;
        };
        if start < colon {
            self.analyze_span(start, colon, depth + 1);
        }
        let alt_end = find_dart_ternary_alt_end(self.tokens, self.ternary_mask, colon + 1, to);
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

fn find_dart_ternary_colon(
    tokens: &[DartToken],
    ternary_mask: &[bool],
    start: usize,
    to: usize,
) -> Option<usize> {
    let mut depth = 0usize;
    let mut nested = 0usize;
    let mut idx = start;
    while idx < to {
        let text = tokens[idx].text.as_str();
        if let Some(hit) = step_ternary_scan(ternary_mask[idx], idx, text, &mut depth, &mut nested)
        {
            return hit;
        }
        idx += 1;
    }
    None
}

fn step_ternary_scan(
    is_ternary: bool,
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
    if is_ternary {
        *nested += 1;
    } else if text == ":" {
        if *nested == 0 {
            return Some(Some(idx));
        }
        *nested -= 1;
    } else if matches!(text, ";" | "," | "=>") {
        return Some(None);
    }
    None
}

fn find_dart_ternary_alt_end(
    tokens: &[DartToken],
    ternary_mask: &[bool],
    start: usize,
    to: usize,
) -> usize {
    let mut depth = 0usize;
    let mut nested_ternary = 0usize;
    let mut idx = start;
    while idx < to {
        if step_ternary_alt_end(
            tokens[idx].text.as_str(),
            ternary_mask[idx],
            &mut depth,
            &mut nested_ternary,
        ) {
            return idx;
        }
        idx += 1;
    }
    to
}

fn step_ternary_alt_end(
    text: &str,
    is_ternary: bool,
    depth: &mut usize,
    nested_ternary: &mut usize,
) -> bool {
    if is_open_delim(text) {
        *depth += 1;
        return false;
    }
    if is_close_delim(text) {
        if *depth == 0 {
            return true;
        }
        *depth -= 1;
        return false;
    }
    if *depth > 0 {
        return false;
    }
    if is_ternary {
        *nested_ternary += 1;
        return false;
    }
    if text == ":" {
        if *nested_ternary == 0 {
            return true;
        }
        *nested_ternary -= 1;
        return false;
    }
    matches!(text, ";" | "," | "=>")
}
