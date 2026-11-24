use sqlparser::ast::{
    CaseWhen, Expr, Function, FunctionArg, FunctionArgExpr, FunctionArguments, InTarget,
    ObjectName, ObjectNamePart, Query, Select, SelectItem, SelectItemQualifiedWildcardKind,
    SetExpr, TableFactor, TableWithJoins,
};
use sqlparser::tokenizer::Location;

pub(crate) fn peel_nested(mut expr: &Expr) -> &Expr {
    while let Expr::Nested(inner) = expr {
        expr = inner;
    }
    expr
}

pub(crate) fn function_arg_expr(arg: &FunctionArg) -> Option<&Expr> {
    let slot = match arg {
        FunctionArg::Unnamed(s)
        | FunctionArg::Named { arg: s, .. }
        | FunctionArg::ExprNamed { arg: s, .. } => s,
    };
    match slot {
        FunctionArgExpr::Expr(expr) => Some(expr),
        _ => None,
    }
}

pub(crate) fn object_name_start_loc(name: &ObjectName) -> Location {
    for part in &name.0 {
        let loc = match part {
            ObjectNamePart::Identifier(ident) => ident.span.start,
            ObjectNamePart::Function(func) => func.name.span.start,
        };
        if loc.line > 0 {
            return loc;
        }
    }
    Location::empty()
}

pub(crate) fn expr_start_line(expr: &Expr) -> u32 {
    expr_start_loc(expr).line as u32
}

pub(crate) fn expr_start_loc(expr: &Expr) -> Location {
    let mut stack = vec![expr];
    while let Some(current) = stack.pop() {
        if let Some(loc) = direct_expr_start_loc(current)
            && loc.line > 0
        {
            return loc;
        }
        push_expr_children_rev(current, &mut stack);
    }
    Location::empty()
}

fn direct_expr_start_loc(expr: &Expr) -> Option<Location> {
    if let Some(loc) = direct_leaf_start_loc(expr) {
        return Some(loc);
    }
    match expr {
        Expr::Function(func) => Some(object_name_start_loc(&func.name)),
        Expr::Case { case_token, .. } => Some(case_token.0.span.start),
        Expr::Subquery(q) | Expr::Exists { subquery: q, .. } => Some(query_start_loc(q)),
        _ => None,
    }
}

fn direct_leaf_start_loc(expr: &Expr) -> Option<Location> {
    match expr {
        Expr::Identifier(ident) => Some(ident.span.start),
        Expr::CompoundIdentifier(idents) => {
            idents.iter().map(|i| i.span.start).find(|l| l.line > 0)
        }
        Expr::Value(val) => Some(val.span.start),
        Expr::TypedString(typed) => Some(typed.value.span.start),
        Expr::Wildcard(tok) => Some(tok.0.span.start),
        Expr::QualifiedWildcard(obj, tok) => {
            let loc = object_name_start_loc(obj);
            Some(if loc.line > 0 { loc } else { tok.0.span.start })
        }
        _ => None,
    }
}

pub(crate) fn select_item_start_loc(item: &SelectItem) -> Location {
    match item {
        SelectItem::UnnamedExpr(expr) => expr_start_loc(expr),
        SelectItem::ExprWithAlias { expr, alias } => {
            let loc = expr_start_loc(expr);
            if loc.line > 0 { loc } else { alias.span.start }
        }
        SelectItem::ExprWithAliases { expr, aliases } => {
            let loc = expr_start_loc(expr);
            if loc.line > 0 {
                loc
            } else {
                aliases.first().map_or(Location::empty(), |a| a.span.start)
            }
        }
        SelectItem::Wildcard(opts) => opts.wildcard_token.0.span.start,
        SelectItem::QualifiedWildcard(kind, opts) => {
            let loc = match kind {
                SelectItemQualifiedWildcardKind::ObjectName(obj) => object_name_start_loc(obj),
                SelectItemQualifiedWildcardKind::Expr(expr) => expr_start_loc(expr),
            };
            if loc.line > 0 {
                loc
            } else {
                opts.wildcard_token.0.span.start
            }
        }
    }
}

pub(crate) fn select_start_loc(select: &Select) -> Location {
    let tok_loc = select.select_token.0.span.start;
    if tok_loc.line > 0 {
        return tok_loc;
    }
    if let Some(item) = select.projection.first() {
        let loc = select_item_start_loc(item);
        if loc.line > 0 {
            return loc;
        }
    }
    select
        .from
        .first()
        .map_or(Location::empty(), table_with_joins_start_loc)
}

pub(crate) fn query_start_loc(query: &Query) -> Location {
    if let Some(with) = &query.with {
        let loc = with.with_token.0.span.start;
        if loc.line > 0 {
            return loc;
        }
    }
    set_expr_start_loc(&query.body)
}

pub(crate) fn set_expr_start_loc(mut body: &SetExpr) -> Location {
    loop {
        match body {
            SetExpr::Select(select) => return select_start_loc(select),
            SetExpr::Query(inner) => {
                if let Some(with) = &inner.with {
                    let loc = with.with_token.0.span.start;
                    if loc.line > 0 {
                        return loc;
                    }
                }
                body = &inner.body;
            }
            SetExpr::SetOperation { left, .. } => body = left,
            SetExpr::Values(values) => {
                return values
                    .rows
                    .first()
                    .and_then(|row| row.first())
                    .map_or(Location::empty(), expr_start_loc);
            }
            _ => return Location::empty(),
        }
    }
}

pub(crate) fn table_with_joins_start_loc(twj: &TableWithJoins) -> Location {
    table_factor_start_loc(&twj.relation)
}

pub(crate) fn table_factor_start_loc(mut factor: &TableFactor) -> Location {
    loop {
        match factor {
            TableFactor::Table { name, .. } | TableFactor::Function { name, .. } => {
                return object_name_start_loc(name);
            }
            TableFactor::Derived { subquery, .. } => return query_start_loc(subquery),
            TableFactor::NestedJoin {
                table_with_joins, ..
            } => factor = &table_with_joins.relation,
            TableFactor::TableFunction { expr, .. } => return expr_start_loc(expr),
            TableFactor::UNNEST { alias: Some(a), .. }
            | TableFactor::JsonTable { alias: Some(a), .. }
            | TableFactor::OpenJsonTable { alias: Some(a), .. } => return a.name.span.start,
            _ => return Location::empty(),
        }
    }
}

pub(crate) fn push_expr_children_rev<'a>(expr: &'a Expr, stack: &mut Vec<&'a Expr>) {
    if let Some(child) = single_child_expr(expr) {
        stack.push(child);
        return;
    }
    if let Some((left, right)) = pair_child_exprs(expr) {
        stack.push(right);
        stack.push(left);
        return;
    }
    push_structured_expr_children_rev(expr, stack);
}

fn single_child_expr(expr: &Expr) -> Option<&Expr> {
    match expr {
        Expr::Nested(e)
        | Expr::UnaryOp { expr: e, .. }
        | Expr::Cast { expr: e, .. }
        | Expr::Convert { expr: e, .. }
        | Expr::Extract { expr: e, .. }
        | Expr::Ceil { expr: e, .. }
        | Expr::Floor { expr: e, .. }
        | Expr::Collate { expr: e, .. }
        | Expr::JsonAccess { value: e, .. }
        | Expr::CompoundFieldAccess { root: e, .. }
        | Expr::IsNull(e)
        | Expr::IsNotNull(e)
        | Expr::IsTrue(e)
        | Expr::IsNotTrue(e)
        | Expr::IsFalse(e)
        | Expr::IsNotFalse(e)
        | Expr::IsUnknown(e)
        | Expr::IsNotUnknown(e) => Some(e),
        Expr::Interval(iv) => Some(&iv.value),
        _ => None,
    }
}

fn pair_child_exprs(expr: &Expr) -> Option<(&Expr, &Expr)> {
    match expr {
        Expr::BinaryOp { left, right, .. }
        | Expr::IsDistinctFrom(left, right)
        | Expr::IsNotDistinctFrom(left, right)
        | Expr::AnyOp { left, right, .. }
        | Expr::AllOp { left, right, .. }
        | Expr::Like {
            expr: left,
            pattern: right,
            ..
        }
        | Expr::ILike {
            expr: left,
            pattern: right,
            ..
        }
        | Expr::SimilarTo {
            expr: left,
            pattern: right,
            ..
        }
        | Expr::RLike {
            expr: left,
            pattern: right,
            ..
        }
        | Expr::Position {
            expr: left,
            r#in: right,
        } => Some((left, right)),
        _ => None,
    }
}

fn push_structured_expr_children_rev<'a>(expr: &'a Expr, stack: &mut Vec<&'a Expr>) {
    match expr {
        Expr::Function(func) => push_function_children_rev(func, stack),
        Expr::Between {
            expr: target,
            low,
            high,
            ..
        } => {
            stack.push(high);
            stack.push(low);
            stack.push(target);
        }
        Expr::Case {
            operand,
            conditions,
            else_result,
            ..
        } => push_case_children_rev(
            operand.as_deref(),
            conditions,
            else_result.as_deref(),
            stack,
        ),
        Expr::In {
            expr: lhs, target, ..
        } => push_in_children_rev(lhs, target, stack),
        Expr::Tuple(items) => stack.extend(items.iter().rev()),
        Expr::Array(array) => stack.extend(array.elem.iter().rev()),
        _ => push_string_expr_children_rev(expr, stack),
    }
}

fn push_string_expr_children_rev<'a>(expr: &'a Expr, stack: &mut Vec<&'a Expr>) {
    match expr {
        Expr::Substring {
            expr: target,
            substring_from,
            substring_for,
            ..
        } => {
            if let Some(for_len) = substring_for.as_deref() {
                stack.push(for_len);
            }
            if let Some(from) = substring_from.as_deref() {
                stack.push(from);
            }
            stack.push(target);
        }
        Expr::Trim {
            expr: target,
            trim_what,
            ..
        } => {
            if let Some(what) = trim_what.as_deref() {
                stack.push(what);
            }
            stack.push(target);
        }
        Expr::Overlay {
            expr: target,
            overlay_what,
            overlay_from,
            overlay_for,
        } => {
            if let Some(for_len) = overlay_for.as_deref() {
                stack.push(for_len);
            }
            stack.push(overlay_from);
            stack.push(overlay_what);
            stack.push(target);
        }
        _ => {}
    }
}

fn push_function_children_rev<'a>(func: &'a Function, stack: &mut Vec<&'a Expr>) {
    if let Some(filter) = &func.filter {
        stack.push(filter);
    }
    if let FunctionArguments::List(list) = &func.args {
        stack.extend(list.args.iter().rev().filter_map(function_arg_expr));
    }
}

fn push_case_children_rev<'a>(
    operand: Option<&'a Expr>,
    conditions: &'a [CaseWhen],
    else_result: Option<&'a Expr>,
    stack: &mut Vec<&'a Expr>,
) {
    if let Some(els) = else_result {
        stack.push(els);
    }
    for when in conditions.iter().rev() {
        stack.push(&when.result);
        stack.push(&when.condition);
    }
    if let Some(op) = operand {
        stack.push(op);
    }
}

fn push_in_children_rev<'a>(lhs: &'a Expr, target: &'a InTarget, stack: &mut Vec<&'a Expr>) {
    match target {
        InTarget::List(items) => stack.extend(items.iter().rev()),
        InTarget::Unnest(inner) => stack.push(inner),
        InTarget::Subquery(_) | InTarget::Table { .. } => {}
    }
    stack.push(lhs);
}
