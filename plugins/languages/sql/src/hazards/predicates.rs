use sqlparser::ast::{
    BinaryOperator, DuplicateTreatment, Expr, Function, FunctionArguments, InTarget, Spanned,
    UnaryOperator, Value,
};
use sqlparser::keywords::Keyword;

use super::HazardCtx;
use super::expr_util::{
    expr_contains_column, extract_string_literal, for_each_child_expr, function_args_any,
    last_ident_lower, peel_nested, value_as_str,
};
use super::locate::{keyword_line_before, span_line};

const HAVING_AGGREGATES: &[&str] = &[
    "any_value",
    "array_agg",
    "avg",
    "bit_and",
    "bit_or",
    "bit_xor",
    "bool_and",
    "bool_or",
    "count",
    "every",
    "first",
    "group_concat",
    "json_agg",
    "json_arrayagg",
    "json_object_agg",
    "jsonb_agg",
    "jsonb_object_agg",
    "last",
    "list",
    "max",
    "median",
    "min",
    "percentile_cont",
    "percentile_disc",
    "stddev",
    "stddev_pop",
    "stddev_samp",
    "string_agg",
    "sum",
    "total",
    "var_pop",
    "var_samp",
    "variance",
];

pub(super) fn check_leading_wildcard_like(ctx: &mut HazardCtx<'_>, expr: &Expr) {
    let (pattern, kw) = match expr {
        Expr::Like { pattern, .. } => (pattern.as_ref(), Keyword::LIKE),
        Expr::ILike { pattern, .. } => (pattern.as_ref(), Keyword::ILIKE),
        _ => return,
    };
    let Some(lit) = extract_string_literal(pattern) else {
        return;
    };
    if !lit.starts_with('%') && !lit.starts_with('_') {
        return;
    }
    let kw_line = keyword_line_before(ctx.tokens, kw, pattern.span().start);
    let line = if kw_line > 0 {
        kw_line
    } else {
        span_line(expr.span())
    };
    ctx.emit(line, "leading-wildcard-like");
}

pub(super) fn check_in_expr(ctx: &mut HazardCtx<'_>, expr: &Expr) {
    let Expr::In {
        target, negated, ..
    } = expr
    else {
        return;
    };
    match target {
        InTarget::Subquery(subquery) if *negated => {
            let not_line = keyword_line_before(ctx.tokens, Keyword::NOT, subquery.span().start);
            let line = if not_line > 0 {
                not_line
            } else {
                span_line(expr.span())
            };
            ctx.emit(line, "not-in-subquery");
        }
        InTarget::List(items) if items.len() > 100 => {
            let first_loc = items[0].span().start;
            let in_line = keyword_line_before(ctx.tokens, Keyword::IN, first_loc);
            let line = if in_line > 0 {
                in_line
            } else {
                span_line(expr.span())
            };
            ctx.emit(line, "long-in-list");
        }
        _ => {}
    }
}

pub(super) fn check_filter_expr(ctx: &mut HazardCtx<'_>, expr: &Expr, in_having: bool) {
    check_always_true(ctx, expr, true);
    check_function_on_column(ctx, expr, in_having);
}

fn check_always_true(ctx: &mut HazardCtx<'_>, expr: &Expr, in_bool_pos: bool) {
    let unwrapped = peel_nested(expr);
    if in_bool_pos && is_always_true_literal(unwrapped) {
        ctx.emit(span_line(unwrapped.span()), "always-true-condition");
        return;
    }
    match unwrapped {
        Expr::BinaryOp { left, op, right } => {
            check_always_true_binary(ctx, unwrapped, left, op, right);
        }
        Expr::IsNotDistinctFrom(left, right) if same_constant(left, right) => {
            ctx.emit(span_line(unwrapped.span()), "always-true-condition");
        }
        _ => {}
    }
}

fn check_always_true_binary(
    ctx: &mut HazardCtx<'_>,
    whole: &Expr,
    left: &Expr,
    op: &BinaryOperator,
    right: &Expr,
) {
    if matches!(
        op,
        BinaryOperator::And | BinaryOperator::Or | BinaryOperator::Xor
    ) {
        check_always_true(ctx, left, true);
        check_always_true(ctx, right, true);
        return;
    }
    if matches!(
        op,
        BinaryOperator::Eq
            | BinaryOperator::GtEq
            | BinaryOperator::LtEq
            | BinaryOperator::Spaceship
    ) && same_constant(left, right)
    {
        ctx.emit(span_line(whole.span()), "always-true-condition");
    }
}

fn is_always_true_literal(expr: &Expr) -> bool {
    match expr {
        Expr::Value(val) => val.value == Value::Boolean(true),
        Expr::UnaryOp {
            op: UnaryOperator::Not,
            expr: inner,
        } => matches!(peel_nested(inner), Expr::Value(val) if val.value == Value::Boolean(false)),
        _ => false,
    }
}

#[derive(PartialEq, Eq)]
enum ConstVal {
    Num(String),
    Str(String),
    Bool(bool),
}

fn same_constant(left: &Expr, right: &Expr) -> bool {
    match (extract_const(left), extract_const(right)) {
        (Some(a), Some(b)) => a == b,
        _ => false,
    }
}

fn extract_const(expr: &Expr) -> Option<ConstVal> {
    match peel_nested(expr) {
        Expr::Value(val) => value_to_const(&val.value),
        Expr::UnaryOp {
            op: UnaryOperator::Plus,
            expr: inner,
        } => extract_const(inner),
        Expr::UnaryOp {
            op: UnaryOperator::Minus,
            expr: inner,
        } => match extract_const(inner)? {
            ConstVal::Num(n) => Some(ConstVal::Num(format!("-{n}"))),
            _ => None,
        },
        _ => None,
    }
}

fn value_to_const(value: &Value) -> Option<ConstVal> {
    if let Some(s) = value_as_str(value) {
        return Some(ConstVal::Str(s.to_owned()));
    }
    match value {
        Value::Number(raw, _) => Some(ConstVal::Num(raw.clone())),
        Value::Boolean(b) => Some(ConstVal::Bool(*b)),
        _ => None,
    }
}

fn check_function_on_column(ctx: &mut HazardCtx<'_>, expr: &Expr, in_having: bool) {
    match expr {
        Expr::Subquery(_) | Expr::Exists { .. } => {}
        Expr::Function(func) => check_function_call(ctx, expr, func, in_having),
        Expr::Substring { .. }
        | Expr::Trim { .. }
        | Expr::Extract { .. }
        | Expr::Ceil { .. }
        | Expr::Floor { .. }
        | Expr::Position { .. }
        | Expr::Overlay { .. } => {
            if expr_contains_column(expr) {
                ctx.emit(span_line(expr.span()), "function-on-filter-column");
            }
        }
        other => for_each_child_expr(other, &mut |child| {
            check_function_on_column(ctx, child, in_having);
        }),
    }
}

fn check_function_call(ctx: &mut HazardCtx<'_>, expr: &Expr, func: &Function, in_having: bool) {
    if in_having && is_having_aggregate(func) {
        return;
    }
    if function_args_any(func, expr_contains_column) {
        let line = span_line(expr.span());
        ctx.emit(line, "function-on-filter-column");
    }
}

fn is_having_aggregate(func: &Function) -> bool {
    func.over.is_none() && (has_aggregate_modifier(func) || is_aggregate_name(func))
}

fn has_aggregate_modifier(func: &Function) -> bool {
    let is_distinct = matches!(
        &func.args,
        FunctionArguments::List(list) if list.duplicate_treatment == Some(DuplicateTreatment::Distinct)
    );
    func.filter.is_some() || !func.within_group.is_empty() || is_distinct
}

fn is_aggregate_name(func: &Function) -> bool {
    last_ident_lower(&func.name)
        .is_some_and(|name| HAVING_AGGREGATES.binary_search(&name.as_str()).is_ok())
}
