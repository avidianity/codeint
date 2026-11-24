use sqlparser::ast::{
    Delete, Expr, FromTable, JoinConstraint, JoinOperator, ObjectName, Statement, TableFactor,
    TableWithJoins, Update,
};
use sqlparser::keywords::Keyword;

use super::HazardCtx;
use super::expr_util::last_ident_lower;
use super::locate::first_keyword_line;

pub(super) fn check_statement(ctx: &mut HazardCtx<'_>, stmt: &Statement) {
    match stmt {
        Statement::Update(update) => check_update(ctx, update),
        Statement::Delete(delete) => check_delete(ctx, delete),
        Statement::Truncate(_) => {
            let line = first_keyword_line(ctx.tokens, Keyword::TRUNCATE);
            ctx.emit(line, "truncate");
        }
        Statement::Grant(_) | Statement::GrantRole(_) => {
            let line = first_keyword_line(ctx.tokens, Keyword::GRANT);
            ctx.emit(line, "grant-or-revoke");
        }
        Statement::Revoke(_) | Statement::RevokeRole(_) => {
            let line = first_keyword_line(ctx.tokens, Keyword::REVOKE);
            ctx.emit(line, "grant-or-revoke");
        }
        Statement::Execute {
            name,
            parameters,
            immediate,
            ..
        } => check_execute(ctx, name.as_ref(), parameters, *immediate),
        _ if is_drop_statement(stmt) => {
            let line = first_keyword_line(ctx.tokens, Keyword::DROP);
            ctx.emit(line, "drop");
        }
        _ => {}
    }
}

fn check_update(ctx: &mut HazardCtx<'_>, update: &Update) {
    if update.selection.is_some() || twj_has_filtering_inner_join(&update.table) {
        return;
    }
    let tok_line = update.update_token.0.span.start.line as u32;
    let line = if tok_line > 0 {
        tok_line
    } else {
        first_keyword_line(ctx.tokens, Keyword::UPDATE)
    };
    ctx.emit(line, "update-without-filter");
}

fn check_delete(ctx: &mut HazardCtx<'_>, delete: &Delete) {
    if delete.selection.is_some() || delete_has_filtering_inner_join(delete) {
        return;
    }
    let tok_line = delete.delete_token.0.span.start.line as u32;
    let line = if tok_line > 0 {
        tok_line
    } else {
        first_keyword_line(ctx.tokens, Keyword::DELETE)
    };
    ctx.emit(line, "delete-without-filter");
}

fn delete_has_filtering_inner_join(delete: &Delete) -> bool {
    let from_tables = match &delete.from {
        FromTable::WithFromKeyword(tables) | FromTable::WithoutKeyword(tables) => tables,
    };
    if from_tables.iter().any(twj_has_filtering_inner_join) {
        return true;
    }
    delete
        .using
        .as_deref()
        .is_some_and(|tables| tables.iter().any(twj_has_filtering_inner_join))
}

fn twj_has_filtering_inner_join(twj: &TableWithJoins) -> bool {
    if let TableFactor::NestedJoin {
        table_with_joins, ..
    } = &twj.relation
        && twj_has_filtering_inner_join(table_with_joins)
    {
        return true;
    }
    twj.joins.iter().any(|join| {
        is_filtering_inner_join(&join.join_operator)
            || matches!(
                &join.relation,
                TableFactor::NestedJoin { table_with_joins, .. }
                    if twj_has_filtering_inner_join(table_with_joins)
            )
    })
}

fn is_filtering_inner_join(op: &JoinOperator) -> bool {
    matches!(
        op,
        JoinOperator::Inner(constraint)
            | JoinOperator::Join(constraint)
            | JoinOperator::StraightJoin(constraint)
            if matches!(
                constraint,
                JoinConstraint::On(_)
                    | JoinConstraint::Using(_)
                    | JoinConstraint::UsingAlias(_, _)
            )
    )
}

fn is_drop_statement(stmt: &Statement) -> bool {
    matches!(
        stmt,
        Statement::Drop { .. }
            | Statement::DropFunction(_)
            | Statement::DropDomain(_)
            | Statement::DropProcedure { .. }
            | Statement::DropSecret { .. }
            | Statement::DropPolicy(_)
            | Statement::DropRule(_)
            | Statement::DropConnector { .. }
            | Statement::DropExtension(_)
            | Statement::DropOperator(_)
            | Statement::DropOperatorFamily(_)
            | Statement::DropOperatorClass(_)
            | Statement::DropTrigger(_)
    )
}

fn check_execute(
    ctx: &mut HazardCtx<'_>,
    name: Option<&ObjectName>,
    parameters: &[Expr],
    immediate: bool,
) {
    if is_dynamic_execute(name, parameters, immediate) {
        let line = first_keyword_line(ctx.tokens, Keyword::EXECUTE);
        let fallback = if line > 0 {
            line
        } else {
            first_keyword_line(ctx.tokens, Keyword::EXEC)
        };
        ctx.emit(fallback, "sql-built-at-run-time");
    }
}

fn is_dynamic_execute(name: Option<&ObjectName>, parameters: &[Expr], immediate: bool) -> bool {
    if immediate {
        return true;
    }
    let Some(obj) = name else {
        return !parameters.is_empty();
    };
    if obj.0.len() == 1
        && obj.0[0]
            .as_ident()
            .is_some_and(|id| id.quote_style == Some('\''))
    {
        return true;
    }
    if parameters.is_empty() {
        return false;
    }
    let Some(last) = last_ident_lower(obj) else {
        return false;
    };
    matches!(last.as_str(), "immediate" | "sp_executesql" | "format")
}
