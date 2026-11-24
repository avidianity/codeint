mod dml_ddl;
mod expr_util;
mod joins;
mod locate;
mod predicates;
mod query_shape;
mod secrets;
mod subqueries;

use std::collections::{BTreeMap, BTreeSet};

use codeint_model::HazardLine;
use sqlparser::ast::{
    Delete, Expr, FromTable, FunctionArguments, InTarget, JoinConstraint, JoinOperator,
    MacroDefinition, Merge, OnConflictAction, OnInsert, OrderByKind, Query, Select, SelectItem,
    SetExpr, Statement, TableFactor, TableWithJoins, Update, UpdateTableFromKind,
};
use sqlparser::tokenizer::TokenWithSpan;

use crate::ast_loc::{function_arg_expr, push_expr_children_rev};
use crate::{ParsedSqlStatement, SqlDialect};

pub const SQL_HAZARD_RULES: &[&str] = &[
    "select-star",
    "leading-wildcard-like",
    "large-offset",
    "tables-without-join-condition",
    "distinct-with-order-by",
    "subquery-in-select-list",
    "function-on-filter-column",
    "not-in-subquery",
    "union-without-all",
    "select-without-filter-or-limit",
    "count-without-filter",
    "order-by-random",
    "distinct-over-join",
    "table-read-twice",
    "subquery-refers-to-outer-table",
    "long-in-list",
    "deep-subquery-nesting",
    "join-without-aliases",
    "column-position",
    "update-without-filter",
    "delete-without-filter",
    "truncate",
    "drop",
    "grant-or-revoke",
    "always-true-condition",
    "sql-built-at-run-time",
    "plain-text-secret",
    "unknown-column",
    "text-compared-to-number",
    "filter-on-unindexed-column",
    "order-by-unindexed-column",
    "join-on-unindexed-column",
];

pub(crate) fn collect_statement_hazards(
    parsed: &[ParsedSqlStatement],
    dialect: SqlDialect,
) -> Vec<HazardLine> {
    let mut hits = BTreeSet::new();
    for stmt in parsed {
        let mut ctx = HazardCtx {
            tokens: &stmt.tokens,
            stmt_start_line: stmt.start_line,
            stmt_end_line: stmt.end_line,
            _dialect: dialect,
            hits: &mut hits,
        };
        let mut walker = StatementWalker::new(&mut ctx);
        walker.visit_statement(&stmt.statement);
        walker.finish();
    }
    hits.into_iter()
        .map(|(line, rule)| HazardLine {
            line,
            rule: rule.to_owned(),
        })
        .collect()
}

pub fn hazard_line_rules() -> impl Iterator<Item = &'static str> {
    SQL_HAZARD_RULES.iter().copied()
}

struct HazardCtx<'a> {
    tokens: &'a [TokenWithSpan],
    stmt_start_line: u32,
    stmt_end_line: u32,
    _dialect: SqlDialect,
    hits: &'a mut BTreeSet<(u32, &'static str)>,
}

impl HazardCtx<'_> {
    fn emit(&mut self, line: u32, rule: &'static str) {
        let max_line = self.stmt_end_line.max(self.stmt_start_line).max(1);
        let resolved = if line >= 1 && line <= max_line {
            line
        } else {
            self.stmt_start_line.clamp(1, max_line)
        };
        self.hits.insert((resolved, rule));
    }
}

struct StatementWalker<'a, 'b> {
    ctx: &'a mut HazardCtx<'b>,
    in_create_view: bool,
    in_exists_direct: bool,
    has_row_limit: bool,
    allow_where_true: bool,
    subquery_depth: u32,
    outer_scopes: Vec<BTreeSet<String>>,
    cte_scopes: Vec<BTreeSet<String>>,
    table_reads: BTreeMap<Vec<String>, Vec<u32>>,
}

impl<'a, 'b> StatementWalker<'a, 'b> {
    fn new(ctx: &'a mut HazardCtx<'b>) -> Self {
        Self {
            ctx,
            in_create_view: false,
            in_exists_direct: false,
            has_row_limit: false,
            allow_where_true: false,
            subquery_depth: 0,
            outer_scopes: Vec::new(),
            cte_scopes: Vec::new(),
            table_reads: BTreeMap::new(),
        }
    }

    fn finish(self) {
        joins::emit_table_read_twice(self.ctx, &self.table_reads);
    }

    fn visit_statement(&mut self, stmt: &Statement) {
        dml_ddl::check_statement(self.ctx, stmt);
        secrets::check_statement(self.ctx, stmt);
        match stmt {
            Statement::Query(query) => self.visit_query(query),
            Statement::Explain { statement, .. } | Statement::Prepare { statement, .. } => {
                self.visit_statement(statement);
            }
            Statement::Insert(insert) => self.visit_insert(insert),
            Statement::Update(update) => self.visit_update(update),
            Statement::Delete(delete) => self.visit_delete(delete),
            Statement::Merge(merge) => self.visit_merge(merge),
            _ => self.visit_ddl_statement(stmt),
        }
    }

    fn visit_ddl_statement(&mut self, stmt: &Statement) {
        match stmt {
            Statement::CreateView(view) => {
                let prev = self.in_create_view;
                self.in_create_view = true;
                self.visit_query(&view.query);
                self.in_create_view = prev;
            }
            Statement::CreateTable(table) => {
                if let Some(q) = table.query.as_deref() {
                    self.visit_query(q);
                }
            }
            Statement::CreateIndex(index) => {
                if let Some(pred) = &index.predicate {
                    self.visit_filter(pred, false);
                }
            }
            Statement::CreateMacro { definition, .. } => match definition {
                MacroDefinition::Expr(e) => self.visit_expr(e),
                MacroDefinition::Table(q) => self.visit_query(q),
            },
            _ => {}
        }
    }

    fn visit_insert(&mut self, insert: &sqlparser::ast::Insert) {
        if let Some(src) = insert.source.as_deref() {
            let prev_allow = self.allow_where_true;
            self.allow_where_true = matches!(insert.on, Some(OnInsert::OnConflict(_)));
            self.visit_query(src);
            self.allow_where_true = prev_allow;
        }
        for assign in &insert.assignments {
            self.visit_expr(&assign.value);
        }
        if let Some(OnInsert::OnConflict(conflict)) = &insert.on
            && let OnConflictAction::DoUpdate(do_update) = &conflict.action
        {
            for assign in &do_update.assignments {
                self.visit_expr(&assign.value);
            }
            if let Some(sel) = &do_update.selection {
                self.visit_filter(sel, false);
            }
        }
    }

    fn visit_update(&mut self, update: &Update) {
        let mut scope = BTreeSet::new();
        self.visit_table_with_joins(&update.table);
        subqueries::collect_twj_bindings(&update.table, &mut scope);
        if let Some(from_kind) = &update.from {
            let from_tables = match from_kind {
                UpdateTableFromKind::BeforeSet(t) | UpdateTableFromKind::AfterSet(t) => t,
            };
            for twj in from_tables {
                self.visit_table_with_joins(twj);
                subqueries::collect_twj_bindings(twj, &mut scope);
            }
        }
        self.outer_scopes.push(scope);
        for assign in &update.assignments {
            self.visit_expr(&assign.value);
        }
        if let Some(sel) = &update.selection {
            self.visit_filter(sel, false);
        }
        query_shape::check_order_by_exprs(self.ctx, &update.order_by);
        self.outer_scopes.pop();
    }

    fn visit_delete(&mut self, delete: &Delete) {
        let mut scope = BTreeSet::new();
        let from_tables = match &delete.from {
            FromTable::WithFromKeyword(t) | FromTable::WithoutKeyword(t) => t,
        };
        for twj in from_tables {
            self.visit_table_with_joins(twj);
            subqueries::collect_twj_bindings(twj, &mut scope);
        }
        if let Some(using) = &delete.using {
            for twj in using {
                self.visit_table_with_joins(twj);
                subqueries::collect_twj_bindings(twj, &mut scope);
            }
        }
        self.outer_scopes.push(scope);
        if let Some(sel) = &delete.selection {
            self.visit_filter(sel, false);
        }
        query_shape::check_order_by_exprs(self.ctx, &delete.order_by);
        self.outer_scopes.pop();
    }

    fn visit_merge(&mut self, merge: &Merge) {
        self.visit_filter(&merge.on, false);
        for clause in &merge.clauses {
            if let Some(pred) = &clause.predicate {
                self.visit_filter(pred, false);
            }
        }
    }

    fn visit_query(&mut self, query: &Query) {
        let pushed_cte = self.visit_with_clause(query);
        if let Some(limit) = &query.limit_clause {
            query_shape::check_large_offset(self.ctx, limit);
        }
        query_shape::check_distinct_with_order_by(self.ctx, query);
        if let Some(order_by) = &query.order_by
            && let OrderByKind::Expressions(exprs) = &order_by.kind
        {
            query_shape::check_order_by_exprs(self.ctx, exprs);
        }

        let prev_limit = self.has_row_limit;
        self.has_row_limit = query_shape::query_has_row_limit(query);
        self.visit_set_expr(&query.body);
        self.has_row_limit = prev_limit;

        if let Some(order_by) = &query.order_by
            && let OrderByKind::Expressions(exprs) = &order_by.kind
        {
            for item in exprs {
                self.visit_expr(&item.expr);
            }
        }
        if pushed_cte {
            self.cte_scopes.pop();
        }
    }

    fn visit_with_clause(&mut self, query: &Query) -> bool {
        let Some(with) = &query.with else {
            return false;
        };
        let mut scope = BTreeSet::new();
        if with.recursive {
            for cte in &with.cte_tables {
                scope.insert(expr_util::normalize_ident(&cte.alias.name));
            }
        }
        self.cte_scopes.push(scope);
        let prev_exists = self.in_exists_direct;
        let prev_limit = self.has_row_limit;
        let prev_allow = self.allow_where_true;
        self.in_exists_direct = false;
        self.has_row_limit = false;
        self.allow_where_true = false;
        for cte in &with.cte_tables {
            self.visit_query(&cte.query);
            if let Some(top) = self.cte_scopes.last_mut() {
                top.insert(expr_util::normalize_ident(&cte.alias.name));
            }
        }
        self.in_exists_direct = prev_exists;
        self.has_row_limit = prev_limit;
        self.allow_where_true = prev_allow;
        true
    }

    fn visit_set_expr(&mut self, set_expr: &SetExpr) {
        let mut stack = vec![(set_expr, self.allow_where_true)];
        self.allow_where_true = false;
        while let Some((current, allow_where_true)) = stack.pop() {
            match current {
                SetExpr::Select(select) => {
                    self.allow_where_true = allow_where_true;
                    self.visit_select(select);
                    self.allow_where_true = false;
                }
                SetExpr::Query(inner) => {
                    self.allow_where_true = allow_where_true;
                    self.visit_query(inner);
                    self.allow_where_true = false;
                }
                SetExpr::SetOperation {
                    left,
                    op,
                    set_quantifier,
                    right,
                } => {
                    query_shape::check_union_without_all(self.ctx, *op, *set_quantifier, right);
                    stack.push((right, allow_where_true));
                    stack.push((left, false));
                }
                SetExpr::Values(values) => {
                    for row in &values.rows {
                        for expr in row.iter() {
                            self.visit_expr(expr);
                        }
                    }
                }
                SetExpr::Insert(s)
                | SetExpr::Update(s)
                | SetExpr::Delete(s)
                | SetExpr::Merge(s) => {
                    self.visit_statement(s);
                }
                SetExpr::Table(_) => {}
            }
        }
    }

    fn visit_select(&mut self, select: &Select) {
        let allow_where_true = self.allow_where_true;
        self.allow_where_true = false;

        if !self.in_exists_direct {
            query_shape::check_select_star(self.ctx, select);
        }
        if !self.in_create_view && !self.has_row_limit {
            query_shape::check_select_without_filter_or_limit(self.ctx, select);
        }
        if !self.in_create_view {
            query_shape::check_count_without_filter(self.ctx, select);
        }
        query_shape::check_subquery_in_select_list(self.ctx, select);
        query_shape::check_group_by_column_position(self.ctx, &select.group_by);
        query_shape::check_order_by_exprs(self.ctx, &select.sort_by);
        joins::check_tables_without_join_condition(self.ctx, select);
        joins::check_distinct_over_join(self.ctx, select);
        joins::check_join_without_aliases(self.ctx, select);

        let saved_exists = self.in_exists_direct;
        self.in_exists_direct = false;

        let mut from_scope = BTreeSet::new();
        for twj in &select.from {
            self.visit_twj_with_lateral_scope(twj, &mut from_scope);
        }

        let local_bindings = subqueries::collect_select_bindings(select);
        self.outer_scopes.push(local_bindings);
        self.visit_select_clauses(select, allow_where_true);
        self.outer_scopes.pop();
        self.in_exists_direct = saved_exists;
    }

    fn visit_twj_with_lateral_scope(
        &mut self,
        twj: &TableWithJoins,
        sibling_scope: &mut BTreeSet<String>,
    ) {
        self.visit_table_factor_with_siblings(&twj.relation, sibling_scope);
        subqueries::collect_factor_bindings(&twj.relation, sibling_scope);
        for join in &twj.joins {
            self.visit_table_factor_with_siblings(&join.relation, sibling_scope);
            subqueries::collect_factor_bindings(&join.relation, sibling_scope);
            self.visit_join_operator(&join.join_operator);
        }
    }

    fn visit_table_factor_with_siblings(
        &mut self,
        factor: &TableFactor,
        sibling_scope: &mut BTreeSet<String>,
    ) {
        joins::record_table_factor_read(factor, &self.cte_scopes, &mut self.table_reads);
        match factor {
            TableFactor::Derived {
                lateral, subquery, ..
            } => {
                if *lateral && !sibling_scope.is_empty() {
                    self.outer_scopes.push(sibling_scope.clone());
                    self.visit_subquery(subquery, false, true);
                    self.outer_scopes.pop();
                } else {
                    self.visit_subquery(subquery, false, false);
                }
            }
            TableFactor::NestedJoin {
                table_with_joins, ..
            } => {
                self.visit_twj_with_lateral_scope(table_with_joins, sibling_scope);
            }
            _ => {}
        }
    }

    fn visit_select_clauses(&mut self, select: &Select, allow_where_true: bool) {
        for item in &select.projection {
            self.visit_select_item(item);
        }
        if let Some(prewhere) = &select.prewhere {
            self.visit_filter(prewhere, false);
        }
        if let Some(selection) = &select.selection {
            if allow_where_true && expr_util::is_boolean_true_literal(selection) {
                self.visit_expr(selection);
            } else {
                self.visit_filter(selection, false);
            }
        }
        if let Some(having) = &select.having {
            self.visit_filter(having, true);
        }
        if let Some(qualify) = &select.qualify {
            self.visit_filter(qualify, false);
        }
    }

    fn visit_select_item(&mut self, item: &SelectItem) {
        if let Some(expr) = expr_util::select_item_expr(item) {
            self.visit_expr(expr);
        }
    }

    fn visit_table_with_joins(&mut self, twj: &TableWithJoins) {
        let mut empty = BTreeSet::new();
        self.visit_twj_with_lateral_scope(twj, &mut empty);
    }

    fn visit_join_operator(&mut self, op: &JoinOperator) {
        if let JoinOperator::AsOf {
            match_condition, ..
        } = op
        {
            self.visit_filter(match_condition, false);
        }
        if let Some(JoinConstraint::On(expr)) = expr_util::join_constraint(op) {
            if is_left_join_on_true(op, expr) {
                self.visit_expr(expr);
            } else {
                self.visit_filter(expr, false);
            }
        }
    }

    fn visit_filter(&mut self, expr: &Expr, in_having: bool) {
        predicates::check_filter_expr(self.ctx, expr, in_having);
        self.visit_expr(expr);
    }

    fn visit_subquery(&mut self, subquery: &Query, is_exists: bool, check_correlation: bool) {
        if check_correlation {
            subqueries::check_correlated_subquery(self.ctx, subquery, &self.outer_scopes);
        }
        self.subquery_depth += 1;
        if self.subquery_depth >= 3 {
            subqueries::emit_deep_nesting(self.ctx, subquery);
        }
        let prev_exists = self.in_exists_direct;
        let prev_limit = self.has_row_limit;
        let prev_allow = self.allow_where_true;
        self.in_exists_direct = is_exists;
        self.has_row_limit = false;
        self.allow_where_true = false;
        self.visit_query(subquery);
        self.in_exists_direct = prev_exists;
        self.has_row_limit = prev_limit;
        self.allow_where_true = prev_allow;
        self.subquery_depth -= 1;
    }

    fn visit_expr(&mut self, root: &Expr) {
        let mut stack = vec![root];
        while let Some(expr) = stack.pop() {
            match expr {
                Expr::Like { .. } | Expr::ILike { .. } => {
                    predicates::check_leading_wildcard_like(self.ctx, expr);
                    push_expr_children_rev(expr, &mut stack);
                }
                Expr::In { target, .. } => {
                    predicates::check_in_expr(self.ctx, expr);
                    if let InTarget::Subquery(q) = target {
                        self.visit_subquery(q, false, true);
                    }
                    push_expr_children_rev(expr, &mut stack);
                }
                Expr::Subquery(q) => self.visit_subquery(q, false, true),
                Expr::Exists { subquery, .. } => self.visit_subquery(subquery, true, true),
                Expr::Function(func) => self.visit_function_iterative(func, &mut stack),
                _ => push_expr_children_rev(expr, &mut stack),
            }
        }
    }

    fn visit_function_iterative<'e>(
        &mut self,
        func: &'e sqlparser::ast::Function,
        stack: &mut Vec<&'e Expr>,
    ) {
        if let Some(filter) = &func.filter {
            predicates::check_filter_expr(self.ctx, filter, false);
            stack.push(filter);
        }
        match &func.args {
            FunctionArguments::Subquery(q) => self.visit_subquery(q, false, true),
            FunctionArguments::List(list) => {
                stack.extend(list.args.iter().rev().filter_map(function_arg_expr));
            }
            FunctionArguments::None => {}
        }
    }
}

fn is_left_join_on_true(op: &JoinOperator, expr: &Expr) -> bool {
    matches!(op, JoinOperator::Left(_) | JoinOperator::LeftOuter(_))
        && expr_util::is_boolean_true_literal(expr)
}
