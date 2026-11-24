use std::collections::{BTreeMap, BTreeSet};

use sqlparser::ast::{
    BinaryOperator, Distinct, Expr, Join, JoinConstraint, JoinOperator, Select, Spanned,
    TableFactor, TableWithJoins,
};
use sqlparser::keywords::Keyword;

use super::HazardCtx;
use super::expr_util::{
    for_each_child_expr, join_constraint, normalize_ident, object_name_key, peel_nested,
};
use super::locate::{
    first_keyword_line, keyword_line_at_or_after, keyword_pair_line_before, select_line,
    select_start_loc, span_line,
};

pub(super) fn check_tables_without_join_condition(ctx: &mut HazardCtx<'_>, select: &Select) {
    let where_links = select.selection.as_ref().is_some_and(expr_links_tables);
    let from_start = select
        .from
        .first()
        .map_or_else(|| select_start_loc(select), |t| t.span().start);

    if select.from.len() >= 2 && !where_links {
        let from_line =
            keyword_line_at_or_after(ctx.tokens, Keyword::FROM, select_start_loc(select));
        let line = if from_line > 0 {
            from_line
        } else {
            span_line(select.from[0].span())
        };
        ctx.emit(line, "tables-without-join-condition");
    }

    for twj in &select.from {
        check_joins_for_missing_condition(ctx, twj, where_links, from_start);
    }
}

fn check_joins_for_missing_condition(
    ctx: &mut HazardCtx<'_>,
    twj: &TableWithJoins,
    where_links: bool,
    from_start: sqlparser::tokenizer::Location,
) {
    if let TableFactor::NestedJoin {
        table_with_joins, ..
    } = &twj.relation
    {
        check_joins_for_missing_condition(ctx, table_with_joins, where_links, from_start);
    }
    for join in &twj.joins {
        if let TableFactor::NestedJoin {
            table_with_joins, ..
        } = &join.relation
        {
            check_joins_for_missing_condition(ctx, table_with_joins, where_links, from_start);
        }
        check_single_join_condition(ctx, join, where_links, from_start);
    }
}

fn check_single_join_condition(
    ctx: &mut HazardCtx<'_>,
    join: &Join,
    where_links: bool,
    from_start: sqlparser::tokenizer::Location,
) {
    let rel_loc = join.relation.span().start;
    if matches!(join.join_operator, JoinOperator::CrossJoin(_)) {
        let cross = keyword_pair_line_before(ctx.tokens, Keyword::CROSS, Keyword::JOIN, rel_loc);
        let line = if cross > 0 {
            cross
        } else {
            span_line(join.relation.span())
        };
        ctx.emit(line, "tables-without-join-condition");
        return;
    }
    if join_has_no_constraint(&join.join_operator) && !where_links {
        let join_kw = keyword_line_at_or_after(ctx.tokens, Keyword::JOIN, from_start);
        let line = if join_kw > 0 {
            join_kw
        } else {
            span_line(join.relation.span())
        };
        ctx.emit(line, "tables-without-join-condition");
    }
}

fn join_has_no_constraint(op: &JoinOperator) -> bool {
    !matches!(op, JoinOperator::CrossJoin(_) | JoinOperator::AsOf { .. })
        && matches!(join_constraint(op), Some(JoinConstraint::None))
}

fn expr_links_tables(expr: &Expr) -> bool {
    match peel_nested(expr) {
        Expr::BinaryOp { left, op, right } => binary_expr_links_tables(left, op, right),
        Expr::IsDistinctFrom(left, right) | Expr::IsNotDistinctFrom(left, right) => {
            sides_link_tables(left, right)
        }
        Expr::UnaryOp { expr: inner, .. } => expr_links_tables(inner),
        _ => false,
    }
}

fn binary_expr_links_tables(left: &Expr, op: &BinaryOperator, right: &Expr) -> bool {
    if matches!(op, BinaryOperator::And | BinaryOperator::Or) {
        return expr_links_tables(left) || expr_links_tables(right);
    }
    if matches!(
        op,
        BinaryOperator::Eq
            | BinaryOperator::NotEq
            | BinaryOperator::Lt
            | BinaryOperator::LtEq
            | BinaryOperator::Gt
            | BinaryOperator::GtEq
            | BinaryOperator::Spaceship
    ) {
        return sides_link_tables(left, right);
    }
    false
}

fn sides_link_tables(left: &Expr, right: &Expr) -> bool {
    let mut left_quals = BTreeSet::new();
    let mut right_quals = BTreeSet::new();
    let left_has_col = collect_column_qualifiers(left, &mut left_quals);
    let right_has_col = collect_column_qualifiers(right, &mut right_quals);
    if !left_has_col || !right_has_col {
        return false;
    }
    if left_quals.len() == 1 && left_quals == right_quals {
        return false;
    }
    true
}

fn collect_column_qualifiers(expr: &Expr, quals: &mut BTreeSet<String>) -> bool {
    match peel_nested(expr) {
        Expr::Identifier(_) => true,
        Expr::CompoundIdentifier(idents) if idents.len() >= 2 => {
            quals.insert(normalize_ident(&idents[0]));
            true
        }
        Expr::Subquery(_) | Expr::Exists { .. } => false,
        other => {
            let mut found = false;
            for_each_child_expr(other, &mut |child| {
                if collect_column_qualifiers(child, quals) {
                    found = true;
                }
            });
            found
        }
    }
}

pub(super) fn check_distinct_over_join(ctx: &mut HazardCtx<'_>, select: &Select) {
    if !matches!(select.distinct, Some(Distinct::Distinct | Distinct::On(_))) {
        return;
    }
    let table_count = count_from_tables(&select.from);
    if table_count < 2 {
        return;
    }
    let kw = keyword_line_at_or_after(ctx.tokens, Keyword::DISTINCT, select_start_loc(select));
    let line = if kw > 0 {
        kw
    } else {
        select_line(select, ctx.tokens)
    };
    ctx.emit(line, "distinct-over-join");
}

pub(super) fn check_join_without_aliases(ctx: &mut HazardCtx<'_>, select: &Select) {
    let mut total = 0usize;
    let mut missing_alias = false;
    let mut has_explicit_join = false;

    for twj in &select.from {
        if !twj.joins.is_empty() {
            has_explicit_join = true;
        }
        inspect_twj_aliases(twj, &mut total, &mut missing_alias);
    }

    if total < 2 || !missing_alias {
        return;
    }
    let start = select_start_loc(select);
    let kw = if has_explicit_join {
        keyword_line_at_or_after(ctx.tokens, Keyword::JOIN, start)
    } else {
        keyword_line_at_or_after(ctx.tokens, Keyword::FROM, start)
    };
    let line = if kw > 0 {
        kw
    } else {
        span_line(select.from[0].span())
    };
    ctx.emit(line, "join-without-aliases");
}

fn inspect_twj_aliases(twj: &TableWithJoins, total: &mut usize, missing_alias: &mut bool) {
    inspect_factor_alias(&twj.relation, total, missing_alias);
    for join in &twj.joins {
        inspect_factor_alias(&join.relation, total, missing_alias);
    }
}

fn inspect_factor_alias(factor: &TableFactor, total: &mut usize, missing_alias: &mut bool) {
    if let TableFactor::NestedJoin {
        table_with_joins, ..
    } = factor
    {
        inspect_twj_aliases(table_with_joins, total, missing_alias);
        return;
    }
    *total += 1;
    if !table_factor_has_alias(factor) {
        *missing_alias = true;
    }
}

fn table_factor_has_alias(factor: &TableFactor) -> bool {
    match factor {
        TableFactor::Table { alias, .. }
        | TableFactor::Derived { alias, .. }
        | TableFactor::TableFunction { alias, .. }
        | TableFactor::Function { alias, .. }
        | TableFactor::UNNEST { alias, .. }
        | TableFactor::JsonTable { alias, .. }
        | TableFactor::OpenJsonTable { alias, .. }
        | TableFactor::NestedJoin { alias, .. }
        | TableFactor::Pivot { alias, .. }
        | TableFactor::Unpivot { alias, .. }
        | TableFactor::MatchRecognize { alias, .. } => alias.is_some(),
        _ => true,
    }
}

fn count_from_tables(from: &[TableWithJoins]) -> usize {
    from.iter().map(count_twj_tables).sum()
}

fn count_twj_tables(twj: &TableWithJoins) -> usize {
    let base = count_factor_tables(&twj.relation);
    let joined: usize = twj
        .joins
        .iter()
        .map(|j| count_factor_tables(&j.relation))
        .sum();
    base + joined
}

fn count_factor_tables(factor: &TableFactor) -> usize {
    match factor {
        TableFactor::NestedJoin {
            table_with_joins, ..
        } => count_twj_tables(table_with_joins),
        _ => 1,
    }
}

pub(super) fn record_table_factor_read(
    factor: &TableFactor,
    cte_scopes: &[BTreeSet<String>],
    reads: &mut BTreeMap<Vec<String>, Vec<u32>>,
) {
    let TableFactor::Table {
        name, args: None, ..
    } = factor
    else {
        return;
    };
    let Some(key) = object_name_key(name) else {
        return;
    };
    if key.len() == 1 && is_cte_in_scope(&key[0], cte_scopes) {
        return;
    }
    let line = span_line(name.span());
    reads.entry(key).or_default().push(line);
}

fn is_cte_in_scope(name: &str, cte_scopes: &[BTreeSet<String>]) -> bool {
    cte_scopes.iter().any(|scope| scope.contains(name))
}

pub(super) fn emit_table_read_twice(
    ctx: &mut HazardCtx<'_>,
    reads: &BTreeMap<Vec<String>, Vec<u32>>,
) {
    for lines in reads.values() {
        if lines.len() > 1 {
            let line = lines[1];
            let resolved = if line > 0 {
                line
            } else {
                first_keyword_line(ctx.tokens, Keyword::FROM)
            };
            ctx.emit(resolved, "table-read-twice");
        }
    }
}
