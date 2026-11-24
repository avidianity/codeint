use sqlparser::ast::{
    CaseWhen, Expr, Function, FunctionArg, FunctionArgExpr, FunctionArguments, Ident, InTarget,
    JoinConstraint, JoinOperator, ObjectName, SelectItem, UnaryOperator, Value,
};

pub(super) fn peel_nested(mut expr: &Expr) -> &Expr {
    while let Expr::Nested(inner) = expr {
        expr = inner;
    }
    expr
}

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

pub(super) fn is_string_literal_value(value: &Value) -> bool {
    value_as_str(value).is_some()
}

pub(super) fn is_string_literal_expr(expr: &Expr) -> bool {
    matches!(peel_nested(expr), Expr::Value(v) if is_string_literal_value(&v.value))
}

pub(super) fn extract_uint_literal(expr: &Expr) -> Option<u128> {
    match peel_nested(expr) {
        Expr::Value(val) => parse_uint_value(&val.value),
        Expr::UnaryOp {
            op: UnaryOperator::Plus,
            expr: inner,
        } => extract_uint_literal(inner),
        _ => None,
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

pub(super) fn function_arg_expr(arg: &FunctionArg) -> Option<&Expr> {
    match arg {
        FunctionArg::Unnamed(FunctionArgExpr::Expr(e))
        | FunctionArg::Named {
            arg: FunctionArgExpr::Expr(e),
            ..
        }
        | FunctionArg::ExprNamed {
            arg: FunctionArgExpr::Expr(e),
            ..
        } => Some(e),
        _ => None,
    }
}

pub(super) fn expr_contains_column(expr: &Expr) -> bool {
    match expr {
        Expr::Identifier(_) | Expr::CompoundIdentifier(_) => true,
        Expr::Subquery(_) | Expr::Exists { .. } => false,
        other => {
            let mut found = false;
            for_each_child_expr(other, &mut |child| {
                if !found && expr_contains_column(child) {
                    found = true;
                }
            });
            found
        }
    }
}

pub(super) fn function_args_any(func: &Function, pred: fn(&Expr) -> bool) -> bool {
    let FunctionArguments::List(list) = &func.args else {
        return false;
    };
    list.args.iter().filter_map(function_arg_expr).any(pred)
}

pub(super) fn for_each_child_expr<'a>(expr: &'a Expr, visit: &mut impl FnMut(&'a Expr)) {
    if let Some(child) = unary_or_test_child(expr) {
        visit(child);
        return;
    }
    if let Some((a, b)) = binary_or_match_children(expr) {
        visit(a);
        visit(b);
        return;
    }
    visit_structured_expr_children(expr, visit);
}

fn unary_or_test_child(expr: &Expr) -> Option<&Expr> {
    match expr {
        Expr::Nested(e) | Expr::UnaryOp { expr: e, .. } | Expr::Collate { expr: e, .. } => Some(e),
        Expr::Cast { expr: e, .. }
        | Expr::Convert { expr: e, .. }
        | Expr::Extract { expr: e, .. }
        | Expr::Ceil { expr: e, .. }
        | Expr::Floor { expr: e, .. }
        | Expr::Trim { expr: e, .. } => Some(e),
        other => predicate_test_child(other),
    }
}

fn predicate_test_child(expr: &Expr) -> Option<&Expr> {
    use Expr::{
        IsFalse, IsNotFalse, IsNotNull, IsNotTrue, IsNotUnknown, IsNull, IsTrue, IsUnknown,
    };
    match expr {
        IsNull(e) | IsNotNull(e) | IsTrue(e) | IsNotTrue(e) | IsFalse(e) | IsNotFalse(e)
        | IsUnknown(e) | IsNotUnknown(e) => Some(e),
        _ => None,
    }
}

fn binary_or_match_children(expr: &Expr) -> Option<(&Expr, &Expr)> {
    match expr {
        Expr::BinaryOp { left, right, .. } => Some((left, right)),
        Expr::Like {
            expr: lhs,
            pattern: rhs,
            ..
        }
        | Expr::ILike {
            expr: lhs,
            pattern: rhs,
            ..
        }
        | Expr::Position {
            expr: lhs,
            r#in: rhs,
        } => Some((lhs, rhs)),
        Expr::IsDistinctFrom(a, b)
        | Expr::IsNotDistinctFrom(a, b)
        | Expr::AnyOp {
            left: a, right: b, ..
        }
        | Expr::AllOp {
            left: a, right: b, ..
        } => Some((a, b)),
        _ => None,
    }
}

fn visit_structured_expr_children<'a>(expr: &'a Expr, visit: &mut impl FnMut(&'a Expr)) {
    match expr {
        Expr::Function(func) => visit_function_children(func, visit),
        Expr::Between {
            expr: target,
            low,
            high,
            ..
        } => {
            visit(target);
            visit(low);
            visit(high);
        }
        Expr::Case {
            operand,
            conditions,
            else_result,
            ..
        } => visit_case_children(
            operand.as_deref(),
            conditions,
            else_result.as_deref(),
            visit,
        ),
        Expr::In {
            expr: lhs, target, ..
        } => visit_in_children(lhs, target, visit),
        Expr::Tuple(items) => items.iter().for_each(visit),
        Expr::Substring {
            expr: target,
            substring_from,
            substring_for,
            ..
        } => {
            visit(target);
            substring_from.as_deref().into_iter().for_each(&mut *visit);
            substring_for.as_deref().into_iter().for_each(visit);
        }
        Expr::Overlay {
            expr: target,
            overlay_what,
            overlay_from,
            overlay_for,
        } => {
            visit(target);
            visit(overlay_what);
            visit(overlay_from);
            overlay_for.as_deref().into_iter().for_each(visit);
        }
        _ => {}
    }
}

fn visit_function_children<'a>(func: &'a Function, visit: &mut impl FnMut(&'a Expr)) {
    if let FunctionArguments::List(list) = &func.args {
        list.args
            .iter()
            .filter_map(function_arg_expr)
            .for_each(&mut *visit);
    }
    if let Some(filter) = &func.filter {
        visit(filter);
    }
}

fn visit_case_children<'a>(
    operand: Option<&'a Expr>,
    conditions: &'a [CaseWhen],
    else_result: Option<&'a Expr>,
    visit: &mut impl FnMut(&'a Expr),
) {
    if let Some(op) = operand {
        visit(op);
    }
    for when in conditions {
        visit(&when.condition);
        visit(&when.result);
    }
    if let Some(els) = else_result {
        visit(els);
    }
}

fn visit_in_children<'a>(lhs: &'a Expr, target: &'a InTarget, visit: &mut impl FnMut(&'a Expr)) {
    visit(lhs);
    match target {
        InTarget::List(items) => items.iter().for_each(visit),
        InTarget::Unnest(inner) => visit(inner),
        InTarget::Subquery(_) | InTarget::Table { .. } => {}
    }
}
