use std::collections::BTreeSet;

use sqlparser::ast::{
    AccessExpr, BinaryOperator, CaseWhen, Expr, Function, FunctionArguments, Ident, InTarget,
    Query, UnaryOperator, WindowType,
};
use sqlparser::keywords::Keyword;
use sqlparser::tokenizer::{Token, TokenWithSpan};

use crate::SqlDialect;
use crate::ast_loc::{expr_start_line, function_arg_expr, push_expr_children_rev};
use crate::flagged::{is_aggregate_call, is_bare_number_literal};
use crate::schema::artifact::{ClauseKind, ColumnRefTarget, ConstrainedRef, SchemaCheck};
use crate::schema::scope::peel_nested;
use crate::schema::types::normalize_ident;

pub(crate) trait ExprExtractor {
    fn tokens(&self) -> &[TokenWithSpan];
    fn stmt_start_line(&self) -> u32;
    fn dialect(&self) -> SqlDialect;
    fn push_check(&mut self, check: SchemaCheck);
    fn visit_subquery(&mut self, query: &Query, parent_scope: u32, parent_ctes: &BTreeSet<String>);
}

fn clause_fallback_keyword(clause: ClauseKind) -> Option<Keyword> {
    match clause {
        ClauseKind::General => None,
        ClauseKind::Filter => Some(Keyword::WHERE),
        ClauseKind::JoinOn => Some(Keyword::ON),
        ClauseKind::OrderBy => Some(Keyword::ORDER),
    }
}

#[derive(Clone, Copy)]
pub(crate) struct ExprContext<'a> {
    pub(crate) scope_id: u32,
    pub(crate) cte_names: &'a BTreeSet<String>,
    pub(crate) excluded: &'a BTreeSet<String>,
    pub(crate) clause: ClauseKind,
    pub(crate) constrained: &'a [ConstrainedRef],
    pub(crate) in_grouped_query: bool,
}

impl<'a> ExprContext<'a> {
    pub(crate) fn new(
        scope_id: u32,
        cte_names: &'a BTreeSet<String>,
        excluded: &'a BTreeSet<String>,
        clause: ClauseKind,
        constrained: &'a [ConstrainedRef],
    ) -> ExprContext<'a> {
        ExprContext {
            scope_id,
            cte_names,
            excluded,
            clause,
            constrained,
            in_grouped_query: false,
        }
    }

    pub(crate) fn order_by(
        scope_id: u32,
        cte_names: &'a BTreeSet<String>,
        excluded: &'a BTreeSet<String>,
        constrained: &'a [ConstrainedRef],
        in_grouped_query: bool,
    ) -> ExprContext<'a> {
        ExprContext {
            scope_id,
            cte_names,
            excluded,
            clause: ClauseKind::OrderBy,
            constrained,
            in_grouped_query,
        }
    }

    fn with_clause(self, clause: ClauseKind) -> ExprContext<'a> {
        ExprContext { clause, ..self }
    }

    fn with_constrained<'b>(self, constrained: &'b [ConstrainedRef]) -> ExprContext<'b>
    where
        'a: 'b,
    {
        ExprContext {
            scope_id: self.scope_id,
            cte_names: self.cte_names,
            excluded: self.excluded,
            clause: self.clause,
            constrained,
            in_grouped_query: self.in_grouped_query,
        }
    }
}

pub(crate) fn extract_filter_expr<V: ExprExtractor>(
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
        let mut merged = ctx.constrained.to_vec();
        collect_conjunct_refs(branch, ctx.excluded, false, &mut merged);
        extract_expr(visitor, branch, ctx.with_constrained(&merged));
    }
}

pub(crate) fn collect_conjunct_refs(
    root: &Expr,
    excluded: &BTreeSet<String>,
    eq_only: bool,
    out: &mut Vec<ConstrainedRef>,
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
                push_constrained_ref(left, excluded, out);
                push_constrained_ref(right, excluded, out);
            }
            Expr::IsNotDistinctFrom(left, right) => {
                push_constrained_ref(left, excluded, out);
                push_constrained_ref(right, excluded, out);
            }
            Expr::IsNull(target) => {
                push_constrained_ref(target, excluded, out);
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
                push_constrained_ref(target, excluded, out);
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

pub(crate) fn expr_to_column_target(expr: &Expr) -> Option<ColumnRefTarget> {
    match peel_nested(expr) {
        Expr::Identifier(ident) => Some(ColumnRefTarget::Unqualified(normalize_ident(ident))),
        Expr::CompoundIdentifier(idents) => match idents.as_slice() {
            [qual, col] => Some(ColumnRefTarget::Qualified2(
                normalize_ident(qual),
                normalize_ident(col),
            )),
            [schema, table, col] => Some(ColumnRefTarget::Qualified3(
                normalize_ident(schema),
                normalize_ident(table),
                normalize_ident(col),
            )),
            _ => None,
        },
        _ => None,
    }
}

fn target_column_hint(target: &ColumnRefTarget) -> Option<&str> {
    match target {
        ColumnRefTarget::Unqualified(col)
        | ColumnRefTarget::Qualified2(_, col)
        | ColumnRefTarget::Qualified3(_, _, col) => Some(col.as_str()),
        ColumnRefTarget::Leading(parts) => parts.first().map(|(name, _)| name.as_str()),
    }
}

pub(crate) fn push_constrained_ref(
    expr: &Expr,
    excluded: &BTreeSet<String>,
    out: &mut Vec<ConstrainedRef>,
) {
    if let Some(target) = expr_to_column_target(expr) {
        out.push(ConstrainedRef {
            target,
            excluded: excluded.iter().cloned().collect(),
        });
    }
}

pub(crate) fn extract_expr<V: ExprExtractor>(visitor: &mut V, root: &Expr, ctx: ExprContext<'_>) {
    let mut stack = vec![(root, ctx.clause)];
    while let Some((expr, clause)) = stack.pop() {
        let cur_ctx = ctx.with_clause(clause);
        step_extract_expr(visitor, expr, cur_ctx, &mut stack);
    }
}

fn step_extract_expr<'e, V: ExprExtractor>(
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
                record_column_ref(visitor, expr, ctx);
            }
        }
        Expr::CompoundIdentifier(_) => record_column_ref(visitor, expr, ctx),
        Expr::CompoundFieldAccess { root, access_chain } => {
            extract_compound_field_access(visitor, expr, root, access_chain, ctx, stack);
        }
        Expr::BinaryOp { left, op, right } => {
            extract_binary_op(visitor, expr, left, op, right, ctx, stack);
        }
        _ => step_extract_structured_expr(visitor, expr, ctx, stack),
    }
}

fn step_extract_structured_expr<'e, V: ExprExtractor>(
    visitor: &mut V,
    expr: &'e Expr,
    ctx: ExprContext<'_>,
    stack: &mut Vec<(&'e Expr, ClauseKind)>,
) {
    match expr {
        Expr::Subquery(query)
        | Expr::Exists {
            subquery: query, ..
        } => visitor.visit_subquery(query, ctx.scope_id, ctx.cte_names),
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

fn extract_compound_field_access<'e, V: ExprExtractor>(
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
    let mut leading = vec![normalize_ident(first)];
    for step in access_chain {
        if let AccessExpr::Dot(Expr::Identifier(ident)) = step {
            leading.push(normalize_ident(ident));
        } else {
            break;
        }
    }
    if leading.len() == 1 {
        stack.push((root, ctx.clause));
        return;
    }
    let parts: Vec<(String, u32)> = leading
        .into_iter()
        .map(|name| {
            let line = resolve_expr_line(
                visitor,
                whole,
                Some(&name),
                clause_fallback_keyword(ctx.clause),
            );
            (name, line)
        })
        .collect();
    let fallback_line = parts.first().map_or(visitor.stmt_start_line(), |(_, l)| *l);
    visitor.push_check(SchemaCheck::ColumnRef {
        scope: ctx.scope_id,
        target: ColumnRefTarget::Leading(parts),
        excluded: ctx.excluded.iter().cloned().collect(),
        clause: ctx.clause,
        constrained: ctx.constrained.to_vec(),
        line: fallback_line,
    });
}

fn resolve_target_line<V: ExprExtractor>(
    visitor: &V,
    expr: &Expr,
    target: &ColumnRefTarget,
    clause: ClauseKind,
) -> u32 {
    resolve_expr_line(
        visitor,
        expr,
        target_column_hint(target),
        clause_fallback_keyword(clause),
    )
}

fn record_column_ref<V: ExprExtractor>(visitor: &mut V, expr: &Expr, ctx: ExprContext<'_>) {
    let Some(target) = expr_to_column_target(expr) else {
        return;
    };
    let line = resolve_target_line(visitor, expr, &target, ctx.clause);
    visitor.push_check(SchemaCheck::ColumnRef {
        scope: ctx.scope_id,
        target,
        excluded: ctx.excluded.iter().cloned().collect(),
        clause: ctx.clause,
        constrained: ctx.constrained.to_vec(),
        line,
    });
}

fn extract_binary_op<'e, V: ExprExtractor>(
    visitor: &mut V,
    whole: &'e Expr,
    left: &'e Expr,
    op: &BinaryOperator,
    right: &'e Expr,
    ctx: ExprContext<'_>,
    stack: &mut Vec<(&'e Expr, ClauseKind)>,
) {
    if matches!(ctx.clause, ClauseKind::Filter | ClauseKind::JoinOn) && *op == BinaryOperator::Or {
        extract_filter_expr(visitor, whole, ctx);
        return;
    }
    if is_comparison_operator(op) {
        extract_text_compared_to_number(visitor, whole, left, right, ctx);
    }
    stack.push((right, ctx.clause));
    stack.push((left, ctx.clause));
}

fn extract_text_compared_to_number<V: ExprExtractor>(
    visitor: &mut V,
    whole: &Expr,
    left: &Expr,
    right: &Expr,
    ctx: ExprContext<'_>,
) {
    let col_expr = if is_bare_number_literal(right) {
        Some(left)
    } else if is_bare_number_literal(left) {
        Some(right)
    } else {
        None
    };
    let Some(target) = col_expr.and_then(expr_to_column_target) else {
        return;
    };
    let line = resolve_target_line(visitor, whole, &target, ctx.clause);
    visitor.push_check(SchemaCheck::TextNumber {
        scope: ctx.scope_id,
        target,
        excluded: ctx.excluded.iter().cloned().collect(),
        line,
    });
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

fn push_in_work<'e, V: ExprExtractor>(
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
        InTarget::Subquery(query) => visitor.visit_subquery(query, ctx.scope_id, ctx.cte_names),
        InTarget::Unnest(inner) => stack.push((inner, ctx.clause)),
        InTarget::Table { .. } => {}
    }
    stack.push((target_expr, ctx.clause));
}

fn push_function_work<'e, V: ExprExtractor>(
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
        FunctionArguments::Subquery(query) => {
            visitor.visit_subquery(query, ctx.scope_id, ctx.cte_names);
        }
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

pub(crate) fn resolve_expr_line<V: ExprExtractor>(
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
