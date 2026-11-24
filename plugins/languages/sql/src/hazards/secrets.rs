use sqlparser::ast::helpers::key_value_options::{
    KeyValueOption, KeyValueOptionKind, KeyValueOptions,
};
use sqlparser::ast::{
    AlterRoleOperation, AlterUser, Assignment, AssignmentTarget, CreateRole, CreateServerOption,
    CreateUser, Expr, Insert, ObjectName, OnConflictAction, OnInsert, Password, RoleOption,
    SecretOption, SetExpr, Spanned, Statement,
};
use sqlparser::keywords::Keyword;

use super::HazardCtx;
use super::expr_util::{
    is_non_empty_string_literal_expr, is_non_empty_string_literal_value, last_ident_lower,
    peel_nested, select_item_expr,
};
use super::locate::{first_keyword_line, span_line};

const SENSITIVE_SUBSTRINGS: &[&str] = &[
    "password",
    "passwd",
    "pwd",
    "secret",
    "token",
    "api_key",
    "apikey",
    "private_key",
    "access_key",
];

pub(super) fn is_sensitive_name(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    SENSITIVE_SUBSTRINGS.iter().any(|s| lower.contains(s))
}

pub(super) fn check_statement(ctx: &mut HazardCtx<'_>, stmt: &Statement) {
    match stmt {
        Statement::CreateRole(role) => check_create_role(ctx, role),
        Statement::AlterRole { operation, .. } => check_alter_role(ctx, operation),
        Statement::CreateUser(user) => check_create_user(ctx, user),
        Statement::AlterUser(user) => check_alter_user(ctx, user),
        Statement::CreateSecret { options, .. } => check_create_secret(ctx, options),
        Statement::CreateUserMapping(mapping) => {
            if let Some(opts) = &mapping.options {
                check_server_options(ctx, opts);
            }
        }
        Statement::Insert(insert) => check_insert(ctx, insert),
        Statement::Update(update) => check_assignments(ctx, &update.assignments),
        _ => {}
    }
}

fn check_create_role(ctx: &mut HazardCtx<'_>, role: &CreateRole) {
    if let Some(password) = &role.password {
        check_password(ctx, password);
    }
}

fn check_alter_role(ctx: &mut HazardCtx<'_>, operation: &AlterRoleOperation) {
    if let AlterRoleOperation::WithOptions { options } = operation {
        for opt in options {
            if let RoleOption::Password(password) = opt {
                check_password(ctx, password);
            }
        }
    }
}

fn check_password(ctx: &mut HazardCtx<'_>, password: &Password) {
    if let Password::Password(expr) = password
        && is_non_empty_string_literal_expr(expr)
    {
        let line = password_line(ctx, expr);
        ctx.emit(line, "plain-text-secret");
    }
}

fn password_line(ctx: &HazardCtx<'_>, expr: &Expr) -> u32 {
    let expr_line = span_line(expr.span());
    if expr_line > 0 {
        expr_line
    } else {
        first_keyword_line(ctx.tokens, Keyword::PASSWORD)
    }
}

fn check_create_user(ctx: &mut HazardCtx<'_>, user: &CreateUser) {
    check_key_value_options(ctx, &user.options);
}

fn check_alter_user(ctx: &mut HazardCtx<'_>, user: &AlterUser) {
    if let Some(pw) = &user.password
        && pw.password.as_deref().is_some_and(|s| !s.is_empty())
    {
        let line = first_keyword_line(ctx.tokens, Keyword::PASSWORD);
        ctx.emit(line, "plain-text-secret");
    }
    check_key_value_options(ctx, &user.set_props);
}

fn check_key_value_options(ctx: &mut HazardCtx<'_>, kv: &KeyValueOptions) {
    for opt in &kv.options {
        check_kv_option(ctx, opt);
    }
}

fn check_kv_option(ctx: &mut HazardCtx<'_>, opt: &KeyValueOption) {
    if !is_sensitive_name(&opt.option_name) {
        if let KeyValueOptionKind::KeyValueOptions(nested) = &opt.option_value {
            check_key_value_options(ctx, nested);
        }
        return;
    }
    match &opt.option_value {
        KeyValueOptionKind::Single(val) if is_non_empty_string_literal_value(&val.value) => {
            ctx.emit(span_line(val.span), "plain-text-secret");
        }
        KeyValueOptionKind::Multi(vals) => {
            for val in vals {
                if is_non_empty_string_literal_value(&val.value) {
                    ctx.emit(span_line(val.span), "plain-text-secret");
                }
            }
        }
        KeyValueOptionKind::KeyValueOptions(nested) => check_key_value_options(ctx, nested),
        _ => {}
    }
}

fn check_create_secret(ctx: &mut HazardCtx<'_>, options: &[SecretOption]) {
    for opt in options {
        check_quoted_option(ctx, &opt.key.value, &opt.value, Some(Keyword::SECRET));
    }
}

fn check_server_options(ctx: &mut HazardCtx<'_>, options: &[CreateServerOption]) {
    for opt in options {
        check_quoted_option(ctx, &opt.key.value, &opt.value, None);
    }
}

fn check_quoted_option(
    ctx: &mut HazardCtx<'_>,
    key: &str,
    value: &sqlparser::ast::Ident,
    fallback_kw: Option<Keyword>,
) {
    if value.quote_style != Some('\'') || value.value.is_empty() || !is_sensitive_name(key) {
        return;
    }
    let line = span_line(value.span);
    let resolved = if line > 0 {
        line
    } else if let Some(kw) = fallback_kw {
        first_keyword_line(ctx.tokens, kw)
    } else {
        0
    };
    ctx.emit(resolved, "plain-text-secret");
}

fn check_insert(ctx: &mut HazardCtx<'_>, insert: &Insert) {
    check_assignments(ctx, &insert.assignments);
    check_on_insert(ctx, insert.on.as_ref());
    if insert.columns.is_empty() {
        return;
    }
    let Some(source) = insert.source.as_deref() else {
        return;
    };
    match source.body.as_ref() {
        SetExpr::Values(values) => {
            for row in &values.rows {
                check_column_expr_pairs(ctx, &insert.columns, row.iter());
            }
        }
        SetExpr::Select(select) => {
            let exprs = select.projection.iter().map(select_item_expr);
            for (col, maybe_expr) in insert.columns.iter().zip(exprs) {
                if let Some(expr) = maybe_expr {
                    check_column_value(ctx, col, expr);
                }
            }
        }
        _ => {}
    }
}

fn check_on_insert(ctx: &mut HazardCtx<'_>, on: Option<&OnInsert>) {
    match on {
        Some(OnInsert::DuplicateKeyUpdate(assignments)) => check_assignments(ctx, assignments),
        Some(OnInsert::OnConflict(conflict)) => {
            if let OnConflictAction::DoUpdate(do_update) = &conflict.action {
                check_assignments(ctx, &do_update.assignments);
            }
        }
        _ => {}
    }
}

fn check_column_expr_pairs<'a>(
    ctx: &mut HazardCtx<'_>,
    columns: &[ObjectName],
    values: impl Iterator<Item = &'a Expr>,
) {
    for (col, expr) in columns.iter().zip(values) {
        check_column_value(ctx, col, expr);
    }
}

fn check_assignments(ctx: &mut HazardCtx<'_>, assignments: &[Assignment]) {
    for assign in assignments {
        match &assign.target {
            AssignmentTarget::ColumnName(col) => check_column_value(ctx, col, &assign.value),
            AssignmentTarget::Tuple(cols) => {
                if let Expr::Tuple(vals) = peel_nested(&assign.value) {
                    check_column_expr_pairs(ctx, cols, vals.iter());
                }
            }
        }
    }
}

fn check_column_value(ctx: &mut HazardCtx<'_>, col: &ObjectName, value: &Expr) {
    if !is_sensitive_column(col) || !is_non_empty_string_literal_expr(value) {
        return;
    }
    let val_line = span_line(value.span());
    let line = if val_line > 0 {
        val_line
    } else {
        span_line(col.span())
    };
    ctx.emit(line, "plain-text-secret");
}

fn is_sensitive_column(col: &ObjectName) -> bool {
    last_ident_lower(col).is_some_and(|name| is_sensitive_name(&name))
}
