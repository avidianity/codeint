use std::collections::{BTreeMap, BTreeSet};

use sqlparser::ast::{
    AccessExpr, BinaryOperator, CaseWhen, Expr, Function, FunctionArguments, Ident, InTarget,
    Query, UnaryOperator, WindowType,
};
use sqlparser::keywords::Keyword;
use sqlparser::tokenizer::{Token, TokenWithSpan};

use crate::SqlDialect;
use crate::ast_loc::{expr_start_line, function_arg_expr, push_expr_children_rev};
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
    root: &Expr,
    ctx: ExprContext<'_>,
) {
    let mut or_stack = vec![root];
    while let Some(branch) = or_stack.pop() {
        if let Expr::BinaryOp {
            left,
            op: BinaryOperator::Or,
            right,
        } = peel_nested(branch)
        {
            or_stack.push(right);
            or_stack.push(left);
            continue;
        }
        let mut merged = ctx.constrained.clone();
        collect_conjunct_cols(branch, ctx.scope, ctx.excluded, false, &mut merged);
        check_expr(visitor, branch, ctx.with_constrained(&merged));
    }
}

pub(crate) fn collect_conjunct_cols(
    root: &Expr,
    scope: &QueryScope<'_>,
    excluded: &BTreeSet<String>,
    eq_only: bool,
    out: &mut ConstrainedCols,
) {
    let mut stack = vec![root];
    while let Some(expr) = stack.pop() {
        match peel_nested(expr) {
            Expr::BinaryOp {
                left,
                op: BinaryOperator::And,
                right,
            } => {
                stack.push(right);
                stack.push(left);
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

pub(crate) fn check_expr<V: ExprVisitor>(visitor: &mut V, root: &Expr, ctx: ExprContext<'_>) {
    let mut stack = vec![(root, ctx.clause)];
    while let Some((expr, clause)) = stack.pop() {
        let cur_ctx = ctx.with_clause(clause);
        step_check_expr(visitor, expr, cur_ctx, &mut stack);
    }
}

fn step_check_expr<'e, V: ExprVisitor>(
    visitor: &mut V,
    expr: &'e Expr,
    ctx: ExprContext<'_>,
    stack: &mut Vec<(&'e Expr, ClauseKind)>,
) {
    match expr {
        Expr::UnaryOp {
            op: UnaryOperator::PGAbs,
            expr: inner,
        } if matches!(inner.as_ref(), Expr::Identifier(_)) => {}
        Expr::Identifier(ident) => {
            if !is_sqlite_double_quoted_literal(visitor.dialect(), ident) {
                check_column_ref(visitor, expr, ctx);
            }
        }
        Expr::CompoundIdentifier(_) => check_column_ref(visitor, expr, ctx),
        Expr::CompoundFieldAccess { root, access_chain } => {
            check_compound_field_access(visitor, expr, root, access_chain, ctx, stack);
        }
        Expr::BinaryOp { left, op, right } => {
            check_binary_op(visitor, expr, left, op, right, ctx, stack);
        }
        _ => step_check_structured_expr(visitor, expr, ctx, stack),
    }
}

fn step_check_structured_expr<'e, V: ExprVisitor>(
    visitor: &mut V,
    expr: &'e Expr,
    ctx: ExprContext<'_>,
    stack: &mut Vec<(&'e Expr, ClauseKind)>,
) {
    match expr {
        Expr::Subquery(query)
        | Expr::Exists {
            subquery: query, ..
        } => visitor.visit_subquery(query, ctx.scope),
        Expr::Case {
            operand,
            conditions,
            else_result,
            ..
        } => push_case_work(
            operand.as_deref(),
            conditions,
            else_result.as_deref(),
            ctx,
            stack,
        ),
        Expr::In {
            expr: target_expr,
            target,
            ..
        } => push_in_work(visitor, target_expr, target, ctx, stack),
        Expr::Function(func) => push_function_work(visitor, func, ctx, stack),
        _ => push_children_with_clause(expr, ctx.clause, stack),
    }
}

fn push_children_with_clause<'e>(
    expr: &'e Expr,
    clause: ClauseKind,
    stack: &mut Vec<(&'e Expr, ClauseKind)>,
) {
    let mut children = Vec::new();
    push_expr_children_rev(expr, &mut children);
    for child in children {
        stack.push((child, clause));
    }
}

fn is_sqlite_double_quoted_literal(dialect: SqlDialect, ident: &Ident) -> bool {
    dialect == SqlDialect::Sqlite
        && ident.quote_style == Some('"')
        && (ident.value.is_empty() || ident.value.contains("://"))
}

fn check_compound_field_access<'e, V: ExprVisitor>(
    visitor: &mut V,
    whole: &'e Expr,
    root: &'e Expr,
    access_chain: &[AccessExpr],
    ctx: ExprContext<'_>,
    stack: &mut Vec<(&'e Expr, ClauseKind)>,
) {
    let Expr::Identifier(first) = peel_nested(root) else {
        stack.push((root, ctx.clause));
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
        stack.push((root, ctx.clause));
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

fn check_binary_op<'e, V: ExprVisitor>(
    visitor: &mut V,
    whole: &'e Expr,
    left: &'e Expr,
    op: &BinaryOperator,
    right: &'e Expr,
    ctx: ExprContext<'_>,
    stack: &mut Vec<(&'e Expr, ClauseKind)>,
) {
    if matches!(ctx.clause, ClauseKind::Filter | ClauseKind::JoinOn) && *op == BinaryOperator::Or {
        check_filter_expr(visitor, whole, ctx);
        return;
    }
    if is_comparison_operator(op) {
        check_text_compared_to_number(visitor, whole, left, right, ctx);
    }
    stack.push((right, ctx.clause));
    stack.push((left, ctx.clause));
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

fn push_case_work<'e>(
    operand: Option<&'e Expr>,
    conditions: &'e [CaseWhen],
    else_result: Option<&'e Expr>,
    ctx: ExprContext<'_>,
    stack: &mut Vec<(&'e Expr, ClauseKind)>,
) {
    let case_clause = if ctx.clause == ClauseKind::OrderBy && ctx.in_grouped_query {
        ClauseKind::General
    } else {
        ctx.clause
    };
    if let Some(els) = else_result {
        stack.push((els, case_clause));
    }
    for when in conditions.iter().rev() {
        stack.push((&when.result, case_clause));
        stack.push((&when.condition, case_clause));
    }
    if let Some(op) = operand {
        stack.push((op, case_clause));
    }
}

fn push_in_work<'e, V: ExprVisitor>(
    visitor: &mut V,
    target_expr: &'e Expr,
    target: &'e InTarget,
    ctx: ExprContext<'_>,
    stack: &mut Vec<(&'e Expr, ClauseKind)>,
) {
    match target {
        InTarget::List(items) => {
            for item in items.iter().rev() {
                stack.push((item, ctx.clause));
            }
        }
        InTarget::Subquery(query) => visitor.visit_subquery(query, ctx.scope),
        InTarget::Unnest(inner) => stack.push((inner, ctx.clause)),
        InTarget::Table { .. } => {}
    }
    stack.push((target_expr, ctx.clause));
}

fn push_function_work<'e, V: ExprVisitor>(
    visitor: &mut V,
    func: &'e Function,
    ctx: ExprContext<'_>,
    stack: &mut Vec<(&'e Expr, ClauseKind)>,
) {
    let arg_clause = if ctx.clause == ClauseKind::OrderBy && is_aggregate_call(func) {
        ClauseKind::General
    } else {
        ctx.clause
    };
    push_function_modifiers_work(func, stack);
    match &func.args {
        FunctionArguments::Subquery(query) => visitor.visit_subquery(query, ctx.scope),
        FunctionArguments::List(list) => {
            for expr in list.args.iter().rev().filter_map(function_arg_expr) {
                stack.push((expr, arg_clause));
            }
        }
        FunctionArguments::None => {}
    }
}

fn push_function_modifiers_work<'e>(func: &'e Function, stack: &mut Vec<(&'e Expr, ClauseKind)>) {
    if let Some(WindowType::WindowSpec(spec)) = &func.over {
        stack.extend(
            spec.order_by
                .iter()
                .rev()
                .map(|o| (&o.expr, ClauseKind::General)),
        );
        stack.extend(
            spec.partition_by
                .iter()
                .rev()
                .map(|p| (p, ClauseKind::General)),
        );
    }
    stack.extend(
        func.within_group
            .iter()
            .rev()
            .map(|o| (&o.expr, ClauseKind::General)),
    );
    if let Some(filter) = &func.filter {
        stack.push((filter, ClauseKind::General));
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
    let span_line = expr_start_line(expr);
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
