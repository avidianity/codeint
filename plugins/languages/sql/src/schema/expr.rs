use std::collections::BTreeSet;

use sqlparser::ast::{
    BinaryOperator, CaseWhen, Expr, Function, FunctionArg, FunctionArgExpr, FunctionArguments,
    InTarget, Query, Spanned, UnaryOperator, Value, WindowType,
};
use sqlparser::keywords::Keyword;
use sqlparser::tokenizer::{Token, TokenWithSpan};

use crate::schema::scope::{QueryScope, peel_nested};
use crate::schema::types::ColumnTypeClass;

pub(crate) trait ExprVisitor {
    fn tokens(&self) -> &[TokenWithSpan];
    fn stmt_start_line(&self) -> u32;
    fn record(&mut self, line: u32, rule: &'static str);
    fn visit_subquery(&mut self, query: &Query, parent: &QueryScope<'_>);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ClauseKind {
    General,
    Filter,
    JoinOn,
    OrderBy,
}

impl ClauseKind {
    fn fallback_keyword(self) -> Option<Keyword> {
        match self {
            ClauseKind::General => None,
            ClauseKind::Filter => Some(Keyword::WHERE),
            ClauseKind::JoinOn => Some(Keyword::ON),
            ClauseKind::OrderBy => Some(Keyword::ORDER),
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) struct ExprContext<'a> {
    pub(crate) scope: &'a QueryScope<'a>,
    pub(crate) excluded: &'a BTreeSet<String>,
    pub(crate) clause: ClauseKind,
}

impl<'a> ExprContext<'a> {
    pub(crate) fn new(
        scope: &'a QueryScope<'a>,
        excluded: &'a BTreeSet<String>,
        clause: ClauseKind,
    ) -> ExprContext<'a> {
        ExprContext {
            scope,
            excluded,
            clause,
        }
    }

    fn with_clause(self, clause: ClauseKind) -> ExprContext<'a> {
        ExprContext { clause, ..self }
    }
}

pub(crate) fn check_expr<V: ExprVisitor>(visitor: &mut V, expr: &Expr, ctx: ExprContext<'_>) {
    match expr {
        Expr::Identifier(_) | Expr::CompoundIdentifier(_) => {
            check_column_ref(visitor, expr, ctx);
        }
        Expr::BinaryOp { left, op, right } => {
            check_binary_op(visitor, expr, left, op, right, ctx);
        }
        Expr::Subquery(query)
        | Expr::Exists {
            subquery: query, ..
        } => {
            visitor.visit_subquery(query, ctx.scope);
        }
        _ => check_nested_expr(visitor, expr, ctx),
    }
}

fn check_column_ref<V: ExprVisitor>(visitor: &mut V, expr: &Expr, ctx: ExprContext<'_>) {
    let Some((table, col)) = ctx.scope.resolve_column_expr(expr, ctx.excluded) else {
        return;
    };
    let line = resolve_expr_line(visitor, expr, Some(&col), ctx.clause.fallback_keyword());
    if !table.columns.contains_key(&col) {
        visitor.record(line, "unknown-column");
        return;
    }
    if table.is_indexed_column(&col) {
        return;
    }
    match ctx.clause {
        ClauseKind::General => {}
        ClauseKind::Filter => visitor.record(line, "filter-on-unindexed-column"),
        ClauseKind::JoinOn => {
            visitor.record(line, "join-on-unindexed-column");
            visitor.record(line, "filter-on-unindexed-column");
        }
        ClauseKind::OrderBy => visitor.record(line, "order-by-unindexed-column"),
    }
}

fn check_binary_op<V: ExprVisitor>(
    visitor: &mut V,
    whole: &Expr,
    left: &Expr,
    op: &BinaryOperator,
    right: &Expr,
    ctx: ExprContext<'_>,
) {
    if is_comparison_operator(op) {
        check_text_compared_to_number(visitor, whole, left, right, ctx);
    }
    check_expr(visitor, left, ctx);
    check_expr(visitor, right, ctx);
}

fn check_text_compared_to_number<V: ExprVisitor>(
    visitor: &mut V,
    whole: &Expr,
    left: &Expr,
    right: &Expr,
    ctx: ExprContext<'_>,
) {
    let matched_col = if is_bare_number_literal(right) {
        resolve_text_column(left, ctx)
    } else if is_bare_number_literal(left) {
        resolve_text_column(right, ctx)
    } else {
        None
    };
    if let Some(col) = matched_col {
        let line = resolve_expr_line(visitor, whole, Some(&col), ctx.clause.fallback_keyword());
        visitor.record(line, "text-compared-to-number");
    }
}

fn resolve_text_column(expr: &Expr, ctx: ExprContext<'_>) -> Option<String> {
    let (table, col) = ctx.scope.resolve_column_expr(expr, ctx.excluded)?;
    (table.columns.get(&col).copied() == Some(ColumnTypeClass::Text)).then_some(col)
}

fn check_nested_expr<V: ExprVisitor>(visitor: &mut V, expr: &Expr, ctx: ExprContext<'_>) {
    if let Some(inner) = unwrap_unary_expr(expr).or_else(|| unwrap_predicate_expr(expr)) {
        check_expr(visitor, inner, ctx);
        return;
    }
    if let Some((left, right)) = unwrap_binary_like_expr(expr) {
        check_expr(visitor, left, ctx);
        check_expr(visitor, right, ctx);
        return;
    }
    check_composite_expr(visitor, expr, ctx);
}

fn unwrap_unary_expr(expr: &Expr) -> Option<&Expr> {
    match expr {
        Expr::Nested(inner)
        | Expr::UnaryOp { expr: inner, .. }
        | Expr::Cast { expr: inner, .. }
        | Expr::Convert { expr: inner, .. }
        | Expr::Extract { expr: inner, .. }
        | Expr::Ceil { expr: inner, .. }
        | Expr::Floor { expr: inner, .. }
        | Expr::Collate { expr: inner, .. }
        | Expr::JsonAccess { value: inner, .. }
        | Expr::CompoundFieldAccess { root: inner, .. } => Some(inner),
        _ => None,
    }
}

fn unwrap_predicate_expr(expr: &Expr) -> Option<&Expr> {
    if let Expr::IsNull(target) | Expr::IsNotNull(target) = expr {
        return Some(target);
    }
    if let Expr::IsUnknown(target) | Expr::IsNotUnknown(target) = expr {
        return Some(target);
    }
    match expr {
        Expr::IsFalse(target)
        | Expr::IsNotFalse(target)
        | Expr::IsTrue(target)
        | Expr::IsNotTrue(target) => Some(target),
        _ => None,
    }
}

fn unwrap_binary_like_expr(expr: &Expr) -> Option<(&Expr, &Expr)> {
    match expr {
        Expr::IsDistinctFrom(lhs, rhs)
        | Expr::IsNotDistinctFrom(lhs, rhs)
        | Expr::AnyOp {
            left: lhs,
            right: rhs,
            ..
        }
        | Expr::AllOp {
            left: lhs,
            right: rhs,
            ..
        }
        | Expr::Like {
            expr: lhs,
            pattern: rhs,
            ..
        }
        | Expr::ILike {
            expr: lhs,
            pattern: rhs,
            ..
        }
        | Expr::SimilarTo {
            expr: lhs,
            pattern: rhs,
            ..
        }
        | Expr::RLike {
            expr: lhs,
            pattern: rhs,
            ..
        }
        | Expr::Position {
            expr: lhs,
            r#in: rhs,
        } => Some((lhs, rhs)),
        _ => None,
    }
}

fn check_composite_expr<V: ExprVisitor>(visitor: &mut V, expr: &Expr, ctx: ExprContext<'_>) {
    match expr {
        Expr::Between {
            expr: target,
            low,
            high,
            ..
        } => {
            check_expr_list(visitor, [target.as_ref(), low.as_ref(), high.as_ref()], ctx);
        }
        Expr::Case {
            operand,
            conditions,
            else_result,
            ..
        } => check_case(
            visitor,
            operand.as_deref(),
            conditions,
            else_result.as_deref(),
            ctx,
        ),
        Expr::In {
            expr: target_expr,
            target,
            ..
        } => {
            check_expr(visitor, target_expr, ctx);
            check_in_target(visitor, target, ctx);
        }
        Expr::Function(func) => check_function(visitor, func, ctx),
        Expr::Tuple(items) => check_expr_list(visitor, items.iter(), ctx),
        Expr::Array(array) => check_expr_list(visitor, array.elem.iter(), ctx),
        _ => check_string_expr(visitor, expr, ctx),
    }
}

fn check_string_expr<V: ExprVisitor>(visitor: &mut V, expr: &Expr, ctx: ExprContext<'_>) {
    match expr {
        Expr::Substring {
            expr: inner,
            substring_from,
            substring_for,
            ..
        } => {
            let parts = [
                Some(inner.as_ref()),
                substring_from.as_deref(),
                substring_for.as_deref(),
            ];
            check_expr_list(visitor, parts.into_iter().flatten(), ctx);
        }
        Expr::Trim {
            expr: inner,
            trim_what,
            ..
        } => {
            let parts = [Some(inner.as_ref()), trim_what.as_deref()];
            check_expr_list(visitor, parts.into_iter().flatten(), ctx);
        }
        _ => {}
    }
}

fn check_expr_list<'a, V: ExprVisitor>(
    visitor: &mut V,
    exprs: impl IntoIterator<Item = &'a Expr>,
    ctx: ExprContext<'_>,
) {
    for expr in exprs {
        check_expr(visitor, expr, ctx);
    }
}

fn check_case<V: ExprVisitor>(
    visitor: &mut V,
    operand: Option<&Expr>,
    conditions: &[CaseWhen],
    else_result: Option<&Expr>,
    ctx: ExprContext<'_>,
) {
    if let Some(op) = operand {
        check_expr(visitor, op, ctx);
    }
    for when in conditions {
        check_expr(visitor, &when.condition, ctx);
        check_expr(visitor, &when.result, ctx);
    }
    if let Some(els) = else_result {
        check_expr(visitor, els, ctx);
    }
}

fn check_in_target<V: ExprVisitor>(visitor: &mut V, target: &InTarget, ctx: ExprContext<'_>) {
    match target {
        InTarget::List(items) => check_expr_list(visitor, items.iter(), ctx),
        InTarget::Subquery(query) => visitor.visit_subquery(query, ctx.scope),
        InTarget::Unnest(inner) => check_expr(visitor, inner, ctx),
        InTarget::Table { .. } => {}
    }
}

fn check_function<V: ExprVisitor>(visitor: &mut V, func: &Function, ctx: ExprContext<'_>) {
    match &func.args {
        FunctionArguments::Subquery(query) => visitor.visit_subquery(query, ctx.scope),
        FunctionArguments::List(list) => {
            for arg in &list.args {
                check_function_arg(visitor, arg, ctx);
            }
        }
        FunctionArguments::None => {}
    }
    let general = ctx.with_clause(ClauseKind::General);
    if let Some(filter) = &func.filter {
        check_expr(visitor, filter, general);
    }
    check_expr_list(
        visitor,
        func.within_group.iter().map(|order| &order.expr),
        general,
    );
    if let Some(WindowType::WindowSpec(spec)) = &func.over {
        check_expr_list(visitor, spec.partition_by.iter(), general);
        check_expr_list(
            visitor,
            spec.order_by.iter().map(|order| &order.expr),
            general,
        );
    }
}

fn check_function_arg<V: ExprVisitor>(visitor: &mut V, arg: &FunctionArg, ctx: ExprContext<'_>) {
    let expr_slot = match arg {
        FunctionArg::Unnamed(slot) => slot,
        FunctionArg::Named { arg: slot, .. } => slot,
        FunctionArg::ExprNamed { arg: slot, .. } => slot,
    };
    if let FunctionArgExpr::Expr(expr) = expr_slot {
        check_expr(visitor, expr, ctx);
    }
}

pub(crate) fn is_comparison_operator(op: &BinaryOperator) -> bool {
    matches!(
        op,
        BinaryOperator::Eq
            | BinaryOperator::NotEq
            | BinaryOperator::Lt
            | BinaryOperator::LtEq
            | BinaryOperator::Gt
            | BinaryOperator::GtEq
            | BinaryOperator::Spaceship
    )
}

pub(crate) fn is_bare_number_literal(expr: &Expr) -> bool {
    match peel_nested(expr) {
        Expr::Value(val) => matches!(val.value, Value::Number(_, _)),
        Expr::UnaryOp {
            op: UnaryOperator::Plus | UnaryOperator::Minus,
            expr: inner,
        } => matches!(
            peel_nested(inner),
            Expr::Value(val) if matches!(val.value, Value::Number(_, _))
        ),
        _ => false,
    }
}

pub(crate) fn resolve_expr_line<V: ExprVisitor>(
    visitor: &V,
    expr: &Expr,
    ident_hint: Option<&str>,
    keyword_hint: Option<Keyword>,
) -> u32 {
    let span_line = expr.span().start.line as u32;
    if span_line > 0 {
        return span_line;
    }
    resolve_fallback_line(
        visitor.tokens(),
        visitor.stmt_start_line(),
        ident_hint,
        keyword_hint,
    )
}

pub(crate) fn resolve_fallback_line(
    tokens: &[TokenWithSpan],
    stmt_start_line: u32,
    ident_hint: Option<&str>,
    keyword_hint: Option<Keyword>,
) -> u32 {
    if let Some(name) = ident_hint
        && let Some(line) = find_ident_line(tokens, name)
    {
        return line;
    }
    if let Some(keyword) = keyword_hint
        && let Some(line) = find_keyword_line(tokens, keyword)
    {
        return line;
    }
    stmt_start_line
}

fn find_ident_line(tokens: &[TokenWithSpan], target: &str) -> Option<u32> {
    tokens.iter().find_map(|tok| match &tok.token {
        Token::Word(word) if word.value.eq_ignore_ascii_case(target) => {
            let line = tok.span.start.line as u32;
            (line > 0).then_some(line)
        }
        _ => None,
    })
}

fn find_keyword_line(tokens: &[TokenWithSpan], keyword: Keyword) -> Option<u32> {
    tokens.iter().find_map(|tok| match &tok.token {
        Token::Word(word) if word.keyword == keyword => {
            let line = tok.span.start.line as u32;
            (line > 0).then_some(line)
        }
        _ => None,
    })
}
