use sqlparser::ast::{
    Expr, Function, FunctionArguments, Ident, JoinConstraint, JoinOperator, ObjectName, SelectItem,
    UnaryOperator, Value,
};

use crate::ast_loc::push_expr_children_rev;
pub(super) use crate::ast_loc::{function_arg_expr, peel_nested};

pub(super) fn normalize_ident(ident: &Ident) -> String {
    if ident.quote_style.is_none() {
        ident.value.to_ascii_lowercase()
    } else {
        ident.value.clone()
    }
}

pub(super) fn object_name_key(name: &ObjectName) -> Option<Vec<String>> {
    let mut parts = Vec::with_capacity(name.0.len());
    for part in &name.0 {
        let ident = part.as_ident()?;
        parts.push(normalize_ident(ident));
    }
    if parts.is_empty() { None } else { Some(parts) }
}

pub(super) fn last_ident_lower(name: &ObjectName) -> Option<String> {
    name.0
        .last()
        .and_then(|part| part.as_ident())
        .map(|ident| ident.value.to_ascii_lowercase())
}

pub(super) fn select_item_expr(item: &SelectItem) -> Option<&Expr> {
    match item {
        SelectItem::UnnamedExpr(e)
        | SelectItem::ExprWithAlias { expr: e, .. }
        | SelectItem::ExprWithAliases { expr: e, .. } => Some(e),
        SelectItem::Wildcard(_) | SelectItem::QualifiedWildcard(_, _) => None,
    }
}

pub(super) fn join_constraint(op: &JoinOperator) -> Option<&JoinConstraint> {
    use JoinOperator::{
        Anti, AsOf, CrossJoin, FullOuter, Inner, Join, Left, LeftAnti, LeftOuter, LeftSemi, Right,
        RightAnti, RightOuter, RightSemi, Semi, StraightJoin,
    };
    match op {
        Join(c) | Inner(c) | Left(c) | LeftOuter(c) | Right(c) | RightOuter(c) | FullOuter(c)
        | CrossJoin(c) | Semi(c) | LeftSemi(c) | RightSemi(c) | Anti(c) | LeftAnti(c)
        | RightAnti(c) | StraightJoin(c) => Some(c),
        AsOf { constraint, .. } => Some(constraint),
        _ => None,
    }
}

pub(super) fn extract_string_literal(expr: &Expr) -> Option<&str> {
    let Expr::Value(val) = peel_nested(expr) else {
        return None;
    };
    value_as_str(&val.value)
}

pub(super) fn value_as_str(value: &Value) -> Option<&str> {
    match value {
        Value::SingleQuotedString(s)
        | Value::DoubleQuotedString(s)
        | Value::TripleSingleQuotedString(s)
        | Value::TripleDoubleQuotedString(s)
        | Value::EscapedStringLiteral(s)
        | Value::UnicodeStringLiteral(s)
        | Value::SingleQuotedByteStringLiteral(s)
        | Value::DoubleQuotedByteStringLiteral(s)
        | Value::TripleSingleQuotedByteStringLiteral(s)
        | Value::TripleDoubleQuotedByteStringLiteral(s)
        | Value::SingleQuotedRawStringLiteral(s)
        | Value::DoubleQuotedRawStringLiteral(s)
        | Value::TripleSingleQuotedRawStringLiteral(s)
        | Value::TripleDoubleQuotedRawStringLiteral(s)
        | Value::NationalStringLiteral(s)
        | Value::HexStringLiteral(s) => Some(s.as_str()),
        Value::DollarQuotedString(s) => Some(s.value.as_str()),
        Value::QuoteDelimitedStringLiteral(s) | Value::NationalQuoteDelimitedStringLiteral(s) => {
            Some(s.value.as_str())
        }
        _ => None,
    }
}

pub(super) fn is_non_empty_string_literal_value(value: &Value) -> bool {
    value_as_str(value).is_some_and(|s| !s.is_empty())
}

pub(super) fn is_non_empty_string_literal_expr(expr: &Expr) -> bool {
    matches!(peel_nested(expr), Expr::Value(v) if is_non_empty_string_literal_value(&v.value))
}

pub(super) fn is_boolean_true_literal(expr: &Expr) -> bool {
    matches!(
        peel_nested(expr),
        Expr::Value(val) if val.value == Value::Boolean(true)
    )
}

pub(super) fn extract_uint_literal(mut expr: &Expr) -> Option<u128> {
    loop {
        match peel_nested(expr) {
            Expr::Value(val) => return parse_uint_value(&val.value),
            Expr::UnaryOp {
                op: UnaryOperator::Plus,
                expr: inner,
            } => expr = inner,
            _ => return None,
        }
    }
}

fn parse_uint_value(value: &Value) -> Option<u128> {
    let Value::Number(raw, _) = value else {
        return None;
    };
    if raw.is_empty() || !raw.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    raw.parse::<u128>().ok()
}

pub(super) fn expr_contains_column(root: &Expr) -> bool {
    let mut stack = vec![root];
    while let Some(expr) = stack.pop() {
        match expr {
            Expr::Identifier(_) | Expr::CompoundIdentifier(_) => return true,
            Expr::Subquery(_) | Expr::Exists { .. } => {}
            other => push_expr_children_rev(other, &mut stack),
        }
    }
    false
}

pub(super) fn function_args_any(func: &Function, pred: fn(&Expr) -> bool) -> bool {
    let FunctionArguments::List(list) = &func.args else {
        return false;
    };
    list.args.iter().filter_map(function_arg_expr).any(pred)
}
