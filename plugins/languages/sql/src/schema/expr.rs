use std::collections::{BTreeMap, BTreeSet};

use sqlparser::ast::{
    AccessExpr, BinaryOperator, CaseWhen, Expr, Function, FunctionArg, FunctionArgExpr,
    FunctionArguments, Ident, InTarget, Query, Spanned, UnaryOperator, WindowType,
};
use sqlparser::keywords::Keyword;
use sqlparser::tokenizer::{Token, TokenWithSpan};

use crate::SqlDialect;
use crate::flagged::{is_aggregate_call, is_bare_number_literal};
use crate::schema::scope::{BindingId, QueryScope, ResolvedColumn, peel_nested};
use crate::schema::types::ColumnTypeClass;

pub(crate) type ConstrainedCols = BTreeMap<BindingId, BTreeSet<String>>;

pub(crate) trait ExprVisitor {
    fn tokens(&self) -> &[TokenWithSpan];
    fn stmt_start_line(&self) -> u32;
    fn dialect(&self) -> SqlDialect;
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
    pub(crate) constrained: &'a ConstrainedCols,
    pub(crate) in_grouped_query: bool,
}

impl<'a> ExprContext<'a> {
    pub(crate) fn new(
        scope: &'a QueryScope<'a>,
        excluded: &'a BTreeSet<String>,
        clause: ClauseKind,
        constrained: &'a ConstrainedCols,
    ) -> ExprContext<'a> {
        ExprContext {
            scope,
            excluded,
            clause,
            constrained,
            in_grouped_query: false,
        }
    }

    pub(crate) fn order_by(
        scope: &'a QueryScope<'a>,
        excluded: &'a BTreeSet<String>,
        constrained: &'a ConstrainedCols,
        in_grouped_query: bool,
    ) -> ExprContext<'a> {
        ExprContext {
            scope,
            excluded,
            clause: ClauseKind::OrderBy,
            constrained,
            in_grouped_query,
        }
    }

    fn with_clause(self, clause: ClauseKind) -> ExprContext<'a> {
        ExprContext { clause, ..self }
    }

    fn with_constrained<'b>(self, constrained: &'b ConstrainedCols) -> ExprContext<'b>
    where
        'a: 'b,
    {
        ExprContext {
            scope: self.scope,
            excluded: self.excluded,
            clause: self.clause,
            constrained,
            in_grouped_query: self.in_grouped_query,
        }
    }
}

pub(crate) fn check_filter_expr<V: ExprVisitor>(
    visitor: &mut V,
    expr: &Expr,
    ctx: ExprContext<'_>,
) {
    let mut merged = ctx.constrained.clone();
    collect_conjunct_cols(expr, ctx.scope, ctx.excluded, false, &mut merged);
    check_expr(visitor, expr, ctx.with_constrained(&merged));
}

pub(crate) fn collect_conjunct_cols(
    expr: &Expr,
    scope: &QueryScope<'_>,
    excluded: &BTreeSet<String>,
    eq_only: bool,
    out: &mut ConstrainedCols,
) {
    match peel_nested(expr) {
        Expr::BinaryOp {
            left,
            op: BinaryOperator::And,
            right,
        } => {
            collect_conjunct_cols(left, scope, excluded, eq_only, out);
            collect_conjunct_cols(right, scope, excluded, eq_only, out);
        }
        Expr::BinaryOp { left, op, right } if is_index_constraint_op(op, eq_only) => {
            record_constrained_col(left, scope, excluded, out);
            record_constrained_col(right, scope, excluded, out);
        }
        Expr::IsNotDistinctFrom(left, right) => {
            record_constrained_col(left, scope, excluded, out);
            record_constrained_col(right, scope, excluded, out);
        }
        Expr::IsNull(target) => {
            record_constrained_col(target, scope, excluded, out);
        }
        Expr::In {
            expr: target,
            negated: false,
            ..
        }
        | Expr::Between {
            expr: target,
            negated: false,
            ..
        } if !eq_only => {
            record_constrained_col(target, scope, excluded, out);
        }
        _ => {}
    }
}

fn is_index_constraint_op(op: &BinaryOperator, eq_only: bool) -> bool {
    if eq_only {
        matches!(op, BinaryOperator::Eq | BinaryOperator::Spaceship)
    } else {
        matches!(
            op,
            BinaryOperator::Eq
                | BinaryOperator::Spaceship
                | BinaryOperator::Lt
                | BinaryOperator::LtEq
                | BinaryOperator::Gt
                | BinaryOperator::GtEq
        )
    }
}

fn record_constrained_col(
    expr: &Expr,
    scope: &QueryScope<'_>,
    excluded: &BTreeSet<String>,
    out: &mut ConstrainedCols,
) {
    if let Some(resolved) = scope.resolve_column_expr(expr, excluded)
        && resolved.table.columns.contains_key(&resolved.column)
    {
        out.entry(resolved.binding_id)
            .or_default()
            .insert(resolved.column);
    }
}

pub(crate) fn check_expr<V: ExprVisitor>(visitor: &mut V, expr: &Expr, ctx: ExprContext<'_>) {
    match expr {
        Expr::UnaryOp {
            op: UnaryOperator::PGAbs,
            expr: inner,
        } if matches!(inner.as_ref(), Expr::Identifier(_)) => {}
        Expr::Identifier(ident) => {
            if is_sqlite_double_quoted_literal(visitor.dialect(), ident) {
                return;
            }
            check_column_ref(visitor, expr, ctx);
        }
        Expr::CompoundIdentifier(_) => {
            check_column_ref(visitor, expr, ctx);
        }
        Expr::CompoundFieldAccess { root, access_chain } => {
            check_compound_field_access(visitor, expr, root, access_chain, ctx);
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

fn is_sqlite_double_quoted_literal(dialect: SqlDialect, ident: &Ident) -> bool {
    dialect == SqlDialect::Sqlite
        && ident.quote_style == Some('"')
        && (ident.value.is_empty() || ident.value.contains("://"))
}

fn check_compound_field_access<V: ExprVisitor>(
    visitor: &mut V,
    whole: &Expr,
    root: &Expr,
    access_chain: &[AccessExpr],
    ctx: ExprContext<'_>,
) {
    let Expr::Identifier(first) = peel_nested(root) else {
        check_expr(visitor, root, ctx);
        return;
    };
    let mut leading = vec![first.clone()];
    for step in access_chain {
        if let AccessExpr::Dot(Expr::Identifier(ident)) = step {
            leading.push(ident.clone());
        } else {
            break;
        }
    }
    if leading.len() == 1 {
        check_expr(visitor, root, ctx);
        return;
    }
    if let Some(resolved) = resolve_leading_idents(&leading, ctx) {
        check_resolved_column(visitor, whole, &resolved, ctx);
    }
}

fn resolve_leading_idents<'a>(
    leading: &[Ident],
    ctx: ExprContext<'a>,
) -> Option<ResolvedColumn<'a>> {
    if leading.len() >= 3
        && let Some(resolved) = ctx.scope.resolve_qualified(&leading[..3])
    {
        return Some(resolved);
    }
    if let Some(resolved) = ctx.scope.resolve_qualified(&leading[..2]) {
        return Some(resolved);
    }
    ctx.scope
        .resolve_unqualified(&leading[0], ctx.excluded)
        .filter(|resolved| resolved.table.columns.contains_key(&resolved.column))
}

fn check_column_ref<V: ExprVisitor>(visitor: &mut V, expr: &Expr, ctx: ExprContext<'_>) {
    let Some(resolved) = ctx.scope.resolve_column_expr(expr, ctx.excluded) else {
        return;
    };
    check_resolved_column(visitor, expr, &resolved, ctx);
}

fn check_resolved_column<V: ExprVisitor>(
    visitor: &mut V,
    expr: &Expr,
    resolved: &ResolvedColumn<'_>,
    ctx: ExprContext<'_>,
) {
    let line = resolve_expr_line(
        visitor,
        expr,
        Some(&resolved.column),
        ctx.clause.fallback_keyword(),
    );
    if !resolved.table.columns.contains_key(&resolved.column) {
        visitor.record(line, "unknown-column");
        return;
    }
    if is_column_indexed_in_context(resolved, ctx) {
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

fn is_column_indexed_in_context(resolved: &ResolvedColumn<'_>, ctx: ExprContext<'_>) -> bool {
    if resolved.table.is_indexed_column(&resolved.column) {
        return true;
    }
    let Some(constrained_cols) = ctx.constrained.get(&resolved.binding_id) else {
        return false;
    };
    match ctx.clause {
        ClauseKind::General => true,
        ClauseKind::OrderBy => resolved
            .table
            .is_covered_by_prefix(&resolved.column, constrained_cols),
        ClauseKind::Filter | ClauseKind::JoinOn => {
            resolved
                .table
                .is_covered_by_prefix(&resolved.column, constrained_cols)
                || resolved.table.has_leading_indexed_in(constrained_cols)
        }
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
    if matches!(ctx.clause, ClauseKind::Filter | ClauseKind::JoinOn) && *op == BinaryOperator::Or {
        check_filter_expr(visitor, left, ctx);
        check_filter_expr(visitor, right, ctx);
        return;
    }
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
    let resolved = ctx.scope.resolve_column_expr(expr, ctx.excluded)?;
    (resolved.table.columns.get(&resolved.column).copied() == Some(ColumnTypeClass::Text))
        .then_some(resolved.column)
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
        | Expr::JsonAccess { value: inner, .. } => Some(inner),
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
    let case_ctx = if ctx.clause == ClauseKind::OrderBy && ctx.in_grouped_query {
        ctx.with_clause(ClauseKind::General)
    } else {
        ctx
    };
    if let Some(op) = operand {
        check_expr(visitor, op, case_ctx);
    }
    for when in conditions {
        check_expr(visitor, &when.condition, case_ctx);
        check_expr(visitor, &when.result, case_ctx);
    }
    if let Some(els) = else_result {
        check_expr(visitor, els, case_ctx);
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
    let arg_ctx = if ctx.clause == ClauseKind::OrderBy && is_aggregate_call(func) {
        ctx.with_clause(ClauseKind::General)
    } else {
        ctx
    };
    match &func.args {
        FunctionArguments::Subquery(query) => visitor.visit_subquery(query, ctx.scope),
        FunctionArguments::List(list) => {
            for arg in &list.args {
                check_function_arg(visitor, arg, arg_ctx);
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
