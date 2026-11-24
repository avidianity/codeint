use sqlparser::ast::{BinaryOperator, Expr, Function, InTarget, UnaryOperator, Value};
use sqlparser::keywords::Keyword;

use crate::ast_loc::{expr_start_line, expr_start_loc, push_expr_children_rev, query_start_loc};
use crate::flagged::is_aggregate_call;

use super::HazardCtx;
use super::expr_util::{
    expr_contains_column, extract_string_literal, function_args_any, peel_nested, value_as_str,
};
use super::locate::keyword_line_before;

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
    let kw_line = keyword_line_before(ctx.tokens, kw, expr_start_loc(pattern));
    let line = if kw_line > 0 {
        kw_line
    } else {
        expr_start_line(expr)
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
            let not_line = keyword_line_before(ctx.tokens, Keyword::NOT, query_start_loc(subquery));
            let line = if not_line > 0 {
                not_line
            } else {
                expr_start_line(expr)
            };
            ctx.emit(line, "not-in-subquery");
        }
        InTarget::List(items) if items.len() > 100 => {
            let first_loc = expr_start_loc(&items[0]);
            let in_line = keyword_line_before(ctx.tokens, Keyword::IN, first_loc);
            let line = if in_line > 0 {
                in_line
            } else {
                expr_start_line(expr)
            };
            ctx.emit(line, "long-in-list");
        }
        _ => {}
    }
}

pub(super) fn check_filter_expr(ctx: &mut HazardCtx<'_>, expr: &Expr, in_having: bool) {
    check_always_true(ctx, expr);
    check_function_on_column(ctx, expr, in_having);
}

fn check_always_true(ctx: &mut HazardCtx<'_>, root: &Expr) {
    let mut stack = vec![root];
    while let Some(expr) = stack.pop() {
        let unwrapped = peel_nested(expr);
        if is_always_true_literal(unwrapped) {
            ctx.emit(expr_start_line(unwrapped), "always-true-condition");
            continue;
        }
        match unwrapped {
            Expr::BinaryOp {
                left,
                op: BinaryOperator::And | BinaryOperator::Or | BinaryOperator::Xor,
                right,
            } => {
                stack.push(right);
                stack.push(left);
            }
            Expr::BinaryOp { left, op, right }
                if matches!(
                    op,
                    BinaryOperator::Eq
                        | BinaryOperator::GtEq
                        | BinaryOperator::LtEq
                        | BinaryOperator::Spaceship
                ) && same_constant(left, right) =>
            {
                ctx.emit(expr_start_line(unwrapped), "always-true-condition");
            }
            Expr::IsNotDistinctFrom(left, right) if same_constant(left, right) => {
                ctx.emit(expr_start_line(unwrapped), "always-true-condition");
            }
            _ => {}
        }
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

fn extract_const(mut expr: &Expr) -> Option<ConstVal> {
    let mut negated = false;
    loop {
        match peel_nested(expr) {
            Expr::Value(val) => {
                let base = value_to_const(&val.value)?;
                if !negated {
                    return Some(base);
                }
                return match base {
                    ConstVal::Num(n) => Some(ConstVal::Num(format!("-{n}"))),
                    _ => None,
                };
            }
            Expr::UnaryOp {
                op: UnaryOperator::Plus,
                expr: inner,
            } => expr = inner,
            Expr::UnaryOp {
                op: UnaryOperator::Minus,
                expr: inner,
            } => {
                negated = !negated;
                expr = inner;
            }
            _ => return None,
        }
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

fn check_function_on_column(ctx: &mut HazardCtx<'_>, root: &Expr, in_having: bool) {
    let mut stack = vec![root];
    while let Some(expr) = stack.pop() {
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
                    ctx.emit(expr_start_line(expr), "function-on-filter-column");
                }
            }
            other => push_expr_children_rev(other, &mut stack),
        }
    }
}

fn check_function_call(ctx: &mut HazardCtx<'_>, expr: &Expr, func: &Function, in_having: bool) {
    if in_having && is_aggregate_call(func) {
        return;
    }
    if function_args_any(func, expr_contains_column) {
        let line = expr_start_line(expr);
        ctx.emit(line, "function-on-filter-column");
    }
}
