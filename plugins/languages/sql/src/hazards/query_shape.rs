use sqlparser::ast::{
    Distinct, Expr, Function, FunctionArg, FunctionArgExpr, FunctionArguments, GroupByExpr,
    InTarget, LimitClause, OrderBy, OrderByExpr, OrderByKind, Query, Select, SelectItem, SetExpr,
    SetOperator, SetQuantifier, Spanned,
};
use sqlparser::keywords::Keyword;

use super::HazardCtx;
use super::expr_util::{
    extract_uint_literal, for_each_child_expr, function_arg_expr, last_ident_lower, peel_nested,
    select_item_expr,
};
use super::locate::{
    first_keyword_line, keyword_line_at_or_after, keyword_line_before, keyword_line_in_range,
    keyword_pair_line_before, select_line, select_start_loc, span_line,
};

const RANDOM_FUNCTIONS: &[&str] = &["gen_random_uuid", "newid", "rand", "random"];

pub(super) fn query_has_row_limit(query: &Query) -> bool {
    if query.fetch.is_some() {
        return true;
    }
    match &query.limit_clause {
        Some(LimitClause::LimitOffset { limit, .. }) => limit.is_some(),
        Some(LimitClause::OffsetCommaLimit { .. }) => true,
        None => false,
    }
}

pub(super) fn check_select_star(ctx: &mut HazardCtx<'_>, select: &Select) {
    for item in &select.projection {
        if matches!(
            item,
            SelectItem::Wildcard(_) | SelectItem::QualifiedWildcard(_, _)
        ) {
            let item_line = span_line(item.span());
            let line = if item_line > 0 {
                item_line
            } else {
                select_line(select, ctx.tokens)
            };
            ctx.emit(line, "select-star");
        }
    }
}

pub(super) fn check_large_offset(ctx: &mut HazardCtx<'_>, limit: &LimitClause) {
    match limit {
        LimitClause::LimitOffset {
            offset: Some(offset),
            ..
        } if extract_uint_literal(&offset.value).is_some_and(|n| n > 1000) => {
            let kw = keyword_line_before(ctx.tokens, Keyword::OFFSET, offset.value.span().start);
            let line = if kw > 0 {
                kw
            } else {
                span_line(offset.value.span())
            };
            ctx.emit(line, "large-offset");
        }
        LimitClause::OffsetCommaLimit { offset, .. }
            if extract_uint_literal(offset).is_some_and(|n| n > 1000) =>
        {
            let line = span_line(offset.span());
            let resolved = if line > 0 {
                line
            } else {
                first_keyword_line(ctx.tokens, Keyword::LIMIT)
            };
            ctx.emit(resolved, "large-offset");
        }
        _ => {}
    }
}

pub(super) fn check_distinct_with_order_by(ctx: &mut HazardCtx<'_>, query: &Query) {
    if !has_non_empty_order_by(query.order_by.as_ref()) {
        return;
    }
    let Some(select) = direct_distinct_select(&query.body) else {
        return;
    };
    let kw = keyword_line_at_or_after(ctx.tokens, Keyword::DISTINCT, select_start_loc(select));
    let line = if kw > 0 {
        kw
    } else {
        select_line(select, ctx.tokens)
    };
    ctx.emit(line, "distinct-with-order-by");
}

fn has_non_empty_order_by(order_by: Option<&OrderBy>) -> bool {
    match order_by.map(|o| &o.kind) {
        Some(OrderByKind::All(_)) => true,
        Some(OrderByKind::Expressions(exprs)) => !exprs.is_empty(),
        None => false,
    }
}

fn direct_distinct_select(body: &SetExpr) -> Option<&Select> {
    match body {
        SetExpr::Select(select) if matches!(select.distinct, Some(Distinct::Distinct)) => {
            Some(select)
        }
        SetExpr::Query(inner) if inner.order_by.is_none() => direct_distinct_select(&inner.body),
        _ => None,
    }
}

pub(super) fn check_subquery_in_select_list(ctx: &mut HazardCtx<'_>, select: &Select) {
    for item in &select.projection {
        if let Some(expr) = select_item_expr(item) {
            emit_subqueries_in_expr(ctx, expr, span_line(item.span()));
        }
    }
}

fn emit_subqueries_in_expr(ctx: &mut HazardCtx<'_>, expr: &Expr, fallback_line: u32) {
    match expr {
        Expr::Subquery(q) | Expr::Exists { subquery: q, .. } => {
            let line = query_first_line(ctx, q, fallback_line);
            ctx.emit(line, "subquery-in-select-list");
        }
        Expr::In {
            expr: lhs,
            target: InTarget::Subquery(q),
            ..
        } => {
            emit_subqueries_in_expr(ctx, lhs, fallback_line);
            let line = query_first_line(ctx, q, fallback_line);
            ctx.emit(line, "subquery-in-select-list");
        }
        _ => for_each_child_expr(expr, &mut |child| {
            emit_subqueries_in_expr(ctx, child, fallback_line);
        }),
    }
}

pub(super) fn query_first_line(ctx: &HazardCtx<'_>, query: &Query, fallback: u32) -> u32 {
    if let SetExpr::Select(sel) = query.body.as_ref() {
        let line = select_line(sel, ctx.tokens);
        if line > 0 {
            return line;
        }
    }
    let span_l = span_line(query.span());
    if span_l > 0 { span_l } else { fallback }
}

pub(super) fn check_union_without_all(
    ctx: &mut HazardCtx<'_>,
    op: SetOperator,
    quantifier: SetQuantifier,
    left: &SetExpr,
    right: &SetExpr,
) {
    if op != SetOperator::Union {
        return;
    }
    if matches!(quantifier, SetQuantifier::All | SetQuantifier::AllByName) {
        return;
    }
    let line = keyword_line_in_range(
        ctx.tokens,
        Keyword::UNION,
        left.span().end,
        right.span().start,
    );
    ctx.emit(line, "union-without-all");
}

pub(super) fn check_select_without_filter_or_limit(ctx: &mut HazardCtx<'_>, select: &Select) {
    if select.from.is_empty()
        || select.selection.is_some()
        || select.prewhere.is_some()
        || select.top.is_some()
    {
        return;
    }
    let line = select_line(select, ctx.tokens);
    ctx.emit(line, "select-without-filter-or-limit");
}

pub(super) fn check_count_without_filter(ctx: &mut HazardCtx<'_>, select: &Select) {
    if select.from.is_empty() || select.selection.is_some() || select.prewhere.is_some() {
        return;
    }
    let sel_line = select_line(select, ctx.tokens);
    for expr in select.projection.iter().filter_map(select_item_expr) {
        emit_count_star_in_expr(ctx, expr, sel_line);
    }
}

fn emit_count_star_in_expr(ctx: &mut HazardCtx<'_>, expr: &Expr, fallback: u32) {
    if matches!(expr, Expr::Subquery(_) | Expr::Exists { .. }) {
        return;
    }
    if let Expr::Function(func) = expr
        && is_unfiltered_count_star(func)
    {
        let line = span_line(expr.span());
        ctx.emit(
            if line > 0 { line } else { fallback },
            "count-without-filter",
        );
        return;
    }
    for_each_child_expr(expr, &mut |child| {
        emit_count_star_in_expr(ctx, child, fallback);
    });
}

fn is_unfiltered_count_star(func: &Function) -> bool {
    if func.filter.is_some() {
        return false;
    }
    if last_ident_lower(&func.name).as_deref() != Some("count") {
        return false;
    }
    let FunctionArguments::List(list) = &func.args else {
        return false;
    };
    matches!(
        list.args.as_slice(),
        [FunctionArg::Unnamed(FunctionArgExpr::Wildcard)]
    )
}

pub(super) fn check_order_by_exprs(ctx: &mut HazardCtx<'_>, exprs: &[OrderByExpr]) {
    let first_loc = exprs.first().map(|e| e.expr.span().start);
    let clause_line = first_loc
        .map(|loc| keyword_pair_line_before(ctx.tokens, Keyword::ORDER, Keyword::BY, loc))
        .unwrap_or(0);
    for item in exprs {
        if expr_contains_random_call(&item.expr) {
            let expr_line = span_line(item.expr.span());
            let line = if clause_line > 0 {
                clause_line
            } else {
                expr_line
            };
            ctx.emit(line, "order-by-random");
        }
        if extract_uint_literal(&item.expr).is_some() {
            let expr_line = span_line(item.expr.span());
            let line = if expr_line > 0 {
                expr_line
            } else {
                clause_line
            };
            ctx.emit(line, "column-position");
        }
    }
}

pub(super) fn check_group_by_column_position(ctx: &mut HazardCtx<'_>, group_by: &GroupByExpr) {
    let GroupByExpr::Expressions(exprs, _) = group_by else {
        return;
    };
    let first_loc = exprs.first().map(|e| e.span().start);
    let clause_line = first_loc
        .map(|loc| keyword_pair_line_before(ctx.tokens, Keyword::GROUP, Keyword::BY, loc))
        .unwrap_or(0);
    for expr in exprs {
        if extract_uint_literal(expr).is_some() {
            let expr_line = span_line(expr.span());
            let line = if expr_line > 0 {
                expr_line
            } else {
                clause_line
            };
            ctx.emit(line, "column-position");
        }
    }
}

fn expr_contains_random_call(expr: &Expr) -> bool {
    match peel_nested(expr) {
        Expr::Subquery(_) | Expr::Exists { .. } => false,
        Expr::Function(func) => {
            if is_random_function(func) {
                return true;
            }
            let FunctionArguments::List(list) = &func.args else {
                return false;
            };
            list.args
                .iter()
                .filter_map(function_arg_expr)
                .any(expr_contains_random_call)
        }
        other => {
            let mut found = false;
            for_each_child_expr(other, &mut |child| {
                if !found && expr_contains_random_call(child) {
                    found = true;
                }
            });
            found
        }
    }
}

fn is_random_function(func: &Function) -> bool {
    let Some(name) = last_ident_lower(&func.name) else {
        return false;
    };
    RANDOM_FUNCTIONS.binary_search(&name.as_str()).is_ok()
}
