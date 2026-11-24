use std::collections::BTreeSet;

use sqlparser::ast::{
    Expr, GroupByExpr, JoinConstraint, OrderByKind, Query, Select, SelectItem, SetExpr,
    TableFactor, TableWithJoins,
};

use crate::ast_loc::push_expr_children_rev;

use super::HazardCtx;
use super::expr_util::{join_constraint, normalize_ident, object_name_key, select_item_expr};
use super::query_shape::query_first_line;

pub(super) fn collect_twj_bindings(twj: &TableWithJoins, out: &mut BTreeSet<String>) {
    collect_factor_bindings(&twj.relation, out);
    for join in &twj.joins {
        collect_factor_bindings(&join.relation, out);
    }
}

pub(super) fn collect_factor_bindings(factor: &TableFactor, out: &mut BTreeSet<String>) {
    match factor {
        TableFactor::Table { name, alias, .. } => {
            if let Some(a) = alias {
                out.insert(normalize_ident(&a.name));
            } else if let Some(parts) = object_name_key(name) {
                if let Some(last) = parts.last() {
                    out.insert(last.clone());
                }
                if parts.len() >= 2 {
                    out.insert(parts.join("."));
                }
            }
        }
        TableFactor::Derived { alias: Some(a), .. }
        | TableFactor::TableFunction { alias: Some(a), .. }
        | TableFactor::Function { alias: Some(a), .. }
        | TableFactor::UNNEST { alias: Some(a), .. }
        | TableFactor::JsonTable { alias: Some(a), .. }
        | TableFactor::OpenJsonTable { alias: Some(a), .. } => {
            out.insert(normalize_ident(&a.name));
        }
        TableFactor::NestedJoin {
            table_with_joins,
            alias,
        } => {
            if let Some(a) = alias {
                out.insert(normalize_ident(&a.name));
            }
            collect_twj_bindings(table_with_joins, out);
        }
        _ => {}
    }
}

pub(super) fn collect_select_bindings(select: &Select) -> BTreeSet<String> {
    let mut bindings = BTreeSet::new();
    for twj in &select.from {
        collect_twj_bindings(twj, &mut bindings);
    }
    bindings
}

pub(super) fn check_correlated_subquery(
    ctx: &mut HazardCtx<'_>,
    subquery: &Query,
    outer_scopes: &[BTreeSet<String>],
) {
    if outer_scopes.is_empty() {
        return;
    }
    if query_refers_to_outer(subquery, outer_scopes) {
        let line = query_first_line(ctx, subquery, ctx.stmt_start_line);
        ctx.emit(line, "subquery-refers-to-outer-table");
    }
}

pub(super) fn emit_deep_nesting(ctx: &mut HazardCtx<'_>, subquery: &Query) {
    let line = query_first_line(ctx, subquery, ctx.stmt_start_line);
    ctx.emit(line, "deep-subquery-nesting");
}

fn query_refers_to_outer(query: &Query, outer_scopes: &[BTreeSet<String>]) -> bool {
    if set_expr_refers_to_outer(&query.body, outer_scopes) {
        return true;
    }
    let Some(order_by) = &query.order_by else {
        return false;
    };
    let OrderByKind::Expressions(exprs) = &order_by.kind else {
        return false;
    };
    let body_bindings = set_expr_bindings(&query.body);
    exprs
        .iter()
        .any(|o| expr_refers_to_outer(&o.expr, &body_bindings, outer_scopes))
}

fn set_expr_bindings(body: &SetExpr) -> BTreeSet<String> {
    match body {
        SetExpr::Select(sel) => collect_select_bindings(sel),
        SetExpr::Query(q) => set_expr_bindings(&q.body),
        _ => BTreeSet::new(),
    }
}

fn set_expr_refers_to_outer(body: &SetExpr, outer_scopes: &[BTreeSet<String>]) -> bool {
    match body {
        SetExpr::Select(select) => select_refers_to_outer(select, outer_scopes),
        SetExpr::Query(inner) => query_refers_to_outer(inner, outer_scopes),
        SetExpr::SetOperation { left, right, .. } => {
            set_expr_refers_to_outer(left, outer_scopes)
                || set_expr_refers_to_outer(right, outer_scopes)
        }
        _ => false,
    }
}

fn select_refers_to_outer(select: &Select, outer_scopes: &[BTreeSet<String>]) -> bool {
    let local = collect_select_bindings(select);
    if projection_refers_to_outer(&select.projection, &local, outer_scopes) {
        return true;
    }
    if join_ons_refer_to_outer(&select.from, &local, outer_scopes) {
        return true;
    }
    let opt_exprs = [
        select.prewhere.as_ref(),
        select.selection.as_ref(),
        select.having.as_ref(),
        select.qualify.as_ref(),
    ];
    if opt_exprs
        .into_iter()
        .flatten()
        .any(|e| expr_refers_to_outer(e, &local, outer_scopes))
    {
        return true;
    }
    if let GroupByExpr::Expressions(exprs, _) = &select.group_by
        && exprs
            .iter()
            .any(|e| expr_refers_to_outer(e, &local, outer_scopes))
    {
        return true;
    }
    false
}

fn projection_refers_to_outer(
    projection: &[SelectItem],
    local: &BTreeSet<String>,
    outer_scopes: &[BTreeSet<String>],
) -> bool {
    for item in projection {
        if let Some(e) = select_item_expr(item) {
            if expr_refers_to_outer(e, local, outer_scopes) {
                return true;
            }
        } else if let SelectItem::QualifiedWildcard(kind, _) = item {
            let text = kind.to_string().to_ascii_lowercase();
            if !local.contains(&text) && in_outer_scopes(&text, outer_scopes) {
                return true;
            }
        }
    }
    false
}

fn join_ons_refer_to_outer(
    from: &[TableWithJoins],
    local: &BTreeSet<String>,
    outer_scopes: &[BTreeSet<String>],
) -> bool {
    for twj in from {
        for join in &twj.joins {
            if let Some(JoinConstraint::On(expr)) = join_constraint(&join.join_operator)
                && expr_refers_to_outer(expr, local, outer_scopes)
            {
                return true;
            }
        }
    }
    false
}

fn expr_refers_to_outer(
    root: &Expr,
    local: &BTreeSet<String>,
    outer_scopes: &[BTreeSet<String>],
) -> bool {
    let mut stack = vec![root];
    while let Some(expr) = stack.pop() {
        match expr {
            Expr::Subquery(_) | Expr::Exists { .. } => {}
            Expr::CompoundIdentifier(idents) if idents.len() >= 2 => {
                if compound_refers_to_outer(idents, local, outer_scopes) {
                    return true;
                }
            }
            other => push_expr_children_rev(other, &mut stack),
        }
    }
    false
}

fn compound_refers_to_outer(
    idents: &[sqlparser::ast::Ident],
    local: &BTreeSet<String>,
    outer_scopes: &[BTreeSet<String>],
) -> bool {
    if idents.len() >= 3 {
        let two_part = format!(
            "{}.{}",
            normalize_ident(&idents[0]),
            normalize_ident(&idents[1])
        );
        if local.contains(&two_part) {
            return false;
        }
        if in_outer_scopes(&two_part, outer_scopes) {
            return true;
        }
        let table_part = normalize_ident(&idents[1]);
        return !local.contains(&table_part) && in_outer_scopes(&table_part, outer_scopes);
    }
    let qual = normalize_ident(&idents[0]);
    !local.contains(&qual) && in_outer_scopes(&qual, outer_scopes)
}

fn in_outer_scopes(name: &str, outer_scopes: &[BTreeSet<String>]) -> bool {
    outer_scopes.iter().any(|scope| scope.contains(name))
}
