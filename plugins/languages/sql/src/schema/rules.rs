use std::collections::BTreeSet;

use codeint_model::HazardLine;
use sqlparser::ast::{
    AssignmentTarget, Delete, Expr, FromTable, GroupByExpr, Insert, Join, JoinConstraint,
    JoinOperator, Merge, ObjectName, OrderBy, OrderByExpr, OrderByKind, Query, Select, SelectItem,
    SetExpr, Statement, TableFactor, TableObject, TableWithJoins, Update, UpdateTableFromKind,
};
use sqlparser::keywords::Keyword;
use sqlparser::tokenizer::TokenWithSpan;

use crate::flagged::expr_contains_aggregate;
use crate::schema::expr::{
    ClauseKind, ConstrainedCols, ExprContext, ExprVisitor, check_expr, check_filter_expr,
    collect_conjunct_cols, resolve_fallback_line,
};
use crate::schema::scope::{QueryScope, collect_select_aliases};
use crate::schema::types::{
    SqlSchema, TableSchema, normalize_ident, object_name_last_ident, split_object_name,
};
use crate::{ParsedSqlStatement, SqlDialect};

const SCHEMA_RULES: &[&str] = &[
    "filter-on-unindexed-column",
    "join-on-unindexed-column",
    "order-by-unindexed-column",
    "text-compared-to-number",
    "unknown-column",
];

pub fn schema_hazard_rules() -> impl Iterator<Item = &'static str> {
    SCHEMA_RULES.iter().copied()
}

pub fn collect_schema_hazards(
    statements: &[ParsedSqlStatement],
    schema: &SqlSchema,
    dialect: SqlDialect,
) -> Vec<HazardLine> {
    if schema.is_empty() {
        return Vec::new();
    }
    let mut hits = BTreeSet::new();
    for parsed in statements {
        let mut checker = SchemaRuleChecker {
            schema,
            dialect,
            tokens: &parsed.tokens,
            stmt_start_line: parsed.start_line,
            hits: &mut hits,
        };
        checker.visit_statement(&parsed.statement);
    }
    hits.into_iter()
        .map(|(line, rule)| HazardLine {
            line,
            rule: rule.to_owned(),
        })
        .collect()
}

struct SchemaRuleChecker<'a> {
    schema: &'a SqlSchema,
    dialect: SqlDialect,
    tokens: &'a [TokenWithSpan],
    stmt_start_line: u32,
    hits: &'a mut BTreeSet<(u32, &'static str)>,
}

impl ExprVisitor for SchemaRuleChecker<'_> {
    fn tokens(&self) -> &[TokenWithSpan] {
        self.tokens
    }

    fn stmt_start_line(&self) -> u32 {
        self.stmt_start_line
    }

    fn dialect(&self) -> SqlDialect {
        self.dialect
    }

    fn record(&mut self, line: u32, rule: &'static str) {
        let target = if line > 0 { line } else { self.stmt_start_line };
        self.hits.insert((target, rule));
    }

    fn visit_subquery(&mut self, query: &Query, parent: &QueryScope<'_>) {
        self.visit_query_in_parent(query, parent);
    }
}

impl<'a> SchemaRuleChecker<'a> {
    fn visit_statement(&mut self, stmt: &Statement) {
        match stmt {
            Statement::Query(query) => self.visit_query(query, BTreeSet::new()),
            Statement::Explain { statement, .. } => self.visit_statement(statement),
            Statement::Insert(insert) => self.visit_insert(insert),
            Statement::Update(update) => self.visit_update(update),
            Statement::Delete(delete) => self.visit_delete(delete),
            Statement::Merge(merge) => self.visit_merge(merge),
            Statement::CreateView(view) => self.visit_query(&view.query, BTreeSet::new()),
            Statement::CreateTable(table) => {
                if let Some(query) = &table.query {
                    self.visit_query(query, BTreeSet::new());
                }
            }
            _ => {}
        }
    }

    fn visit_insert(&mut self, insert: &Insert) {
        if let TableObject::TableName(table_name) = &insert.table
            && let Some((table_schema, name)) = split_object_name(table_name)
            && let Some(table) = self.schema.get_table(table_schema.as_deref(), &name)
        {
            for col_obj in &insert.columns {
                self.check_target_column(table, col_obj);
            }
        }
        if let Some(source) = &insert.source {
            self.visit_query(source, BTreeSet::new());
        }
    }

    fn visit_update(&mut self, update: &Update) {
        let mut scope = QueryScope::root(BTreeSet::new());
        scope.add_table_with_joins(&update.table, self.schema);
        if let Some(from) = &update.from {
            let from_tables = match from {
                UpdateTableFromKind::BeforeSet(tables) | UpdateTableFromKind::AfterSet(tables) => {
                    tables
                }
            };
            for twj in from_tables {
                scope.add_table_with_joins(twj, self.schema);
            }
        }

        let target_table =
            self.resolve_single_factor_table(&update.table.relation, &scope.cte_names);
        let empty = BTreeSet::new();
        let no_constrained = ConstrainedCols::new();
        self.visit_table_with_joins_subqueries(&update.table, &scope);
        self.check_table_with_joins_conditions(&update.table, &scope);

        for assign in &update.assignments {
            if let Some(table) = target_table {
                self.check_assignment_target(table, &assign.target);
            }
            check_expr(
                self,
                &assign.value,
                ExprContext::new(&scope, &empty, ClauseKind::General, &no_constrained),
            );
        }
        self.check_dml_tail(
            update.selection.as_ref(),
            update.returning.as_deref(),
            &update.order_by,
            &scope,
        );
    }

    fn visit_delete(&mut self, delete: &Delete) {
        let mut scope = QueryScope::root(BTreeSet::new());
        let from_tables = match &delete.from {
            FromTable::WithFromKeyword(tables) | FromTable::WithoutKeyword(tables) => tables,
        };
        for twj in from_tables {
            scope.add_table_with_joins(twj, self.schema);
        }
        if let Some(using) = &delete.using {
            for twj in using {
                scope.add_table_with_joins(twj, self.schema);
            }
        }
        for twj in from_tables {
            self.visit_table_with_joins_subqueries(twj, &scope);
            self.check_table_with_joins_conditions(twj, &scope);
        }
        self.check_dml_tail(
            delete.selection.as_ref(),
            delete.returning.as_deref(),
            &delete.order_by,
            &scope,
        );
    }

    fn check_dml_tail(
        &mut self,
        selection: Option<&Expr>,
        returning: Option<&[SelectItem]>,
        order_by: &[OrderByExpr],
        scope: &QueryScope<'_>,
    ) {
        let empty = BTreeSet::new();
        let no_constrained = ConstrainedCols::new();
        let mut order_prefix = ConstrainedCols::new();
        if let Some(filter) = selection {
            check_filter_expr(
                self,
                filter,
                ExprContext::new(scope, &empty, ClauseKind::Filter, &no_constrained),
            );
            collect_conjunct_cols(filter, scope, &empty, true, &mut order_prefix);
        }
        if let Some(items) = returning {
            self.check_select_items(items, scope, &empty);
        }
        self.check_order_by_exprs(order_by, scope, &empty, order_prefix, false);
    }

    fn visit_merge(&mut self, merge: &Merge) {
        let mut scope = QueryScope::root(BTreeSet::new());
        scope.add_table_factor(&merge.table, self.schema);
        scope.add_table_factor(&merge.source, self.schema);
        let empty = BTreeSet::new();
        let no_constrained = ConstrainedCols::new();
        self.visit_table_factor_subqueries(&merge.table, &scope);
        self.visit_table_factor_subqueries(&merge.source, &scope);
        check_filter_expr(
            self,
            &merge.on,
            ExprContext::new(&scope, &empty, ClauseKind::JoinOn, &no_constrained),
        );
        for clause in &merge.clauses {
            if let Some(predicate) = &clause.predicate {
                check_filter_expr(
                    self,
                    predicate,
                    ExprContext::new(&scope, &empty, ClauseKind::Filter, &no_constrained),
                );
            }
        }
    }

    fn visit_query(&mut self, query: &Query, outer_ctes: BTreeSet<String>) {
        let cte_names = self.collect_and_visit_ctes(query, outer_ctes);
        self.visit_set_expr(&query.body, query.order_by.as_ref(), cte_names, None);
    }

    fn visit_query_in_parent(&mut self, query: &Query, parent: &QueryScope<'_>) {
        let cte_names = self.collect_and_visit_ctes(query, parent.cte_names.clone());
        self.visit_set_expr(
            &query.body,
            query.order_by.as_ref(),
            cte_names,
            Some(parent),
        );
    }

    fn collect_and_visit_ctes(
        &mut self,
        query: &Query,
        mut cte_names: BTreeSet<String>,
    ) -> BTreeSet<String> {
        if let Some(with) = &query.with {
            for cte in &with.cte_tables {
                let name = normalize_ident(&cte.alias.name);
                let mut visible = cte_names.clone();
                if with.recursive {
                    visible.insert(name.clone());
                }
                self.visit_query(&cte.query, visible);
                cte_names.insert(name);
            }
        }
        cte_names
    }

    fn visit_set_expr(
        &mut self,
        set_expr: &SetExpr,
        order_by: Option<&OrderBy>,
        cte_names: BTreeSet<String>,
        parent: Option<&QueryScope<'_>>,
    ) {
        let mut stack = vec![(set_expr, order_by)];
        while let Some((current, current_order_by)) = stack.pop() {
            match current {
                SetExpr::Select(select) => {
                    self.visit_select(select, current_order_by, cte_names.clone(), parent);
                }
                SetExpr::Query(inner) => match parent {
                    Some(p) => self.visit_query_in_parent(inner, p),
                    None => self.visit_query(inner, cte_names.clone()),
                },
                SetExpr::SetOperation { left, right, .. } => {
                    stack.push((right, None));
                    stack.push((left, None));
                }
                SetExpr::Insert(stmt)
                | SetExpr::Update(stmt)
                | SetExpr::Delete(stmt)
                | SetExpr::Merge(stmt) => self.visit_statement(stmt),
                _ => {}
            }
        }
    }

    fn visit_select(
        &mut self,
        select: &Select,
        order_by: Option<&OrderBy>,
        cte_names: BTreeSet<String>,
        parent: Option<&QueryScope<'_>>,
    ) {
        let mut scope = match parent {
            Some(p) => {
                let mut child = p.child();
                child.cte_names = cte_names;
                child
            }
            None => QueryScope::root(cte_names),
        };
        for twj in &select.from {
            scope.add_table_with_joins(twj, self.schema);
        }

        let empty = BTreeSet::new();
        let no_constrained = ConstrainedCols::new();
        let select_aliases = collect_select_aliases(&select.projection);

        for twj in &select.from {
            self.visit_table_with_joins_subqueries(twj, &scope);
            self.check_table_with_joins_conditions(twj, &scope);
        }
        self.check_select_items(&select.projection, &scope, &empty);
        if let Some(prewhere) = &select.prewhere {
            check_filter_expr(
                self,
                prewhere,
                ExprContext::new(&scope, &empty, ClauseKind::Filter, &no_constrained),
            );
        }
        if let Some(selection) = &select.selection {
            check_filter_expr(
                self,
                selection,
                ExprContext::new(&scope, &empty, ClauseKind::Filter, &no_constrained),
            );
        }
        self.check_select_post_clauses(select, order_by, &scope, &select_aliases);
    }

    fn check_select_post_clauses(
        &mut self,
        select: &Select,
        order_by: Option<&OrderBy>,
        scope: &QueryScope<'_>,
        select_aliases: &BTreeSet<String>,
    ) {
        let no_constrained = ConstrainedCols::new();
        if let GroupByExpr::Expressions(exprs, _) = &select.group_by {
            let ctx = ExprContext::new(scope, select_aliases, ClauseKind::General, &no_constrained);
            for expr in exprs {
                check_expr(self, expr, ctx);
            }
        }
        if let Some(having) = &select.having {
            check_expr(
                self,
                having,
                ExprContext::new(scope, select_aliases, ClauseKind::General, &no_constrained),
            );
        }
        if let Some(qualify) = &select.qualify {
            check_expr(
                self,
                qualify,
                ExprContext::new(scope, select_aliases, ClauseKind::General, &no_constrained),
            );
        }
        if let Some(order) = order_by
            && let OrderByKind::Expressions(exprs) = &order.kind
        {
            let order_prefix = collect_select_order_prefix(select, scope);
            let grouped = select_has_grouping_or_aggregates(select);
            self.check_order_by_exprs(exprs, scope, select_aliases, order_prefix, grouped);
        }
    }

    fn check_select_items(
        &mut self,
        items: &[SelectItem],
        scope: &QueryScope<'_>,
        excluded: &BTreeSet<String>,
    ) {
        let no_constrained = ConstrainedCols::new();
        let ctx = ExprContext::new(scope, excluded, ClauseKind::General, &no_constrained);
        for item in items {
            match item {
                SelectItem::UnnamedExpr(expr)
                | SelectItem::ExprWithAlias { expr, .. }
                | SelectItem::ExprWithAliases { expr, .. } => {
                    check_expr(self, expr, ctx);
                }
                SelectItem::QualifiedWildcard(_, _) | SelectItem::Wildcard(_) => {}
            }
        }
    }

    fn check_order_by_exprs(
        &mut self,
        exprs: &[OrderByExpr],
        scope: &QueryScope<'_>,
        excluded: &BTreeSet<String>,
        mut prefix: ConstrainedCols,
        in_grouped_query: bool,
    ) {
        for order_expr in exprs {
            let ctx = ExprContext::order_by(scope, excluded, &prefix, in_grouped_query);
            check_expr(self, &order_expr.expr, ctx);
            if let Some(resolved) = scope.resolve_column_expr(&order_expr.expr, excluded)
                && resolved.table.columns.contains_key(&resolved.column)
            {
                prefix
                    .entry(resolved.binding_id)
                    .or_default()
                    .insert(resolved.column);
            }
        }
    }

    fn visit_table_with_joins_subqueries(&mut self, twj: &TableWithJoins, scope: &QueryScope<'_>) {
        self.visit_table_factor_subqueries(&twj.relation, scope);
        for join in &twj.joins {
            self.visit_table_factor_subqueries(&join.relation, scope);
        }
    }

    fn visit_table_factor_subqueries(&mut self, factor: &TableFactor, scope: &QueryScope<'_>) {
        match factor {
            TableFactor::Derived {
                lateral, subquery, ..
            } => {
                if *lateral {
                    self.visit_query_in_parent(subquery, scope);
                } else {
                    self.visit_query(subquery, scope.cte_names.clone());
                }
            }
            TableFactor::NestedJoin {
                table_with_joins, ..
            } => self.visit_table_with_joins_subqueries(table_with_joins, scope),
            _ => {}
        }
    }

    fn check_table_with_joins_conditions(&mut self, twj: &TableWithJoins, scope: &QueryScope<'_>) {
        if let TableFactor::NestedJoin {
            table_with_joins, ..
        } = &twj.relation
        {
            self.check_table_with_joins_conditions(table_with_joins, scope);
        }
        let mut left_scope = QueryScope::root(scope.cte_names.clone());
        left_scope.add_table_factor(&twj.relation, self.schema);
        for join in &twj.joins {
            if let TableFactor::NestedJoin {
                table_with_joins, ..
            } = &join.relation
            {
                self.check_table_with_joins_conditions(table_with_joins, scope);
            }
            self.check_join(join, &left_scope, scope);
            left_scope.add_table_factor(&join.relation, self.schema);
        }
    }

    fn check_join(
        &mut self,
        join: &Join,
        left_scope: &QueryScope<'_>,
        full_scope: &QueryScope<'_>,
    ) {
        let empty = BTreeSet::new();
        match &join.join_operator {
            JoinOperator::Join(constraint)
            | JoinOperator::Inner(constraint)
            | JoinOperator::Left(constraint)
            | JoinOperator::LeftOuter(constraint)
            | JoinOperator::Right(constraint)
            | JoinOperator::RightOuter(constraint)
            | JoinOperator::FullOuter(constraint)
            | JoinOperator::CrossJoin(constraint)
            | JoinOperator::Semi(constraint)
            | JoinOperator::LeftSemi(constraint)
            | JoinOperator::RightSemi(constraint)
            | JoinOperator::Anti(constraint)
            | JoinOperator::LeftAnti(constraint)
            | JoinOperator::RightAnti(constraint) => {
                self.check_join_constraint(constraint, &join.relation, left_scope, full_scope);
            }
            JoinOperator::AsOf {
                match_condition,
                constraint,
            } => {
                let no_constrained = ConstrainedCols::new();
                check_filter_expr(
                    self,
                    match_condition,
                    ExprContext::new(full_scope, &empty, ClauseKind::JoinOn, &no_constrained),
                );
                self.check_join_constraint(constraint, &join.relation, left_scope, full_scope);
            }
            _ => {}
        }
    }

    fn check_join_constraint(
        &mut self,
        constraint: &JoinConstraint,
        right_relation: &TableFactor,
        left_scope: &QueryScope<'_>,
        full_scope: &QueryScope<'_>,
    ) {
        let empty = BTreeSet::new();
        let no_constrained = ConstrainedCols::new();
        match constraint {
            JoinConstraint::On(expr) => {
                check_filter_expr(
                    self,
                    expr,
                    ExprContext::new(full_scope, &empty, ClauseKind::JoinOn, &no_constrained),
                );
            }
            JoinConstraint::Using(columns) | JoinConstraint::UsingAlias(columns, _) => {
                let right_table =
                    self.resolve_single_factor_table(right_relation, &full_scope.cte_names);
                let using_cols: BTreeSet<String> =
                    columns.iter().filter_map(object_name_last_ident).collect();
                for col_obj in columns {
                    self.check_using_column(col_obj, left_scope, right_table, &using_cols);
                }
            }
            JoinConstraint::Natural | JoinConstraint::None => {}
        }
    }

    fn check_using_column(
        &mut self,
        col_obj: &ObjectName,
        left_scope: &QueryScope<'_>,
        right_table: Option<&TableSchema>,
        using_cols: &BTreeSet<String>,
    ) {
        let Some(col) = object_name_last_ident(col_obj) else {
            return;
        };
        let line = self.resolve_object_name_line(col_obj, &col, Some(Keyword::USING));
        if let Some(table) = right_table {
            self.check_join_column_on_table(table, &col, using_cols, line);
        }
        if !left_scope.has_unknown_binding() {
            let left_matches: Vec<&TableSchema> = left_scope
                .bindings
                .iter()
                .filter_map(|b| b.table)
                .filter(|t| t.columns.contains_key(&col))
                .collect();
            if left_matches.len() == 1 {
                self.check_join_column_on_table(left_matches[0], &col, using_cols, line);
            } else if left_matches.is_empty()
                && left_scope.bindings.len() == 1
                && left_scope.bindings[0].table.is_some()
            {
                self.record(line, "unknown-column");
            }
        }
    }

    fn check_join_column_on_table(
        &mut self,
        table: &TableSchema,
        col: &str,
        using_cols: &BTreeSet<String>,
        line: u32,
    ) {
        if !table.columns.contains_key(col) {
            self.record(line, "unknown-column");
        } else if !table.is_indexed_column(col)
            && !table.is_covered_by_prefix(col, using_cols)
            && !table.has_leading_indexed_in(using_cols)
        {
            self.record(line, "join-on-unindexed-column");
        }
    }

    fn check_assignment_target(&mut self, table: &TableSchema, target: &AssignmentTarget) {
        match target {
            AssignmentTarget::ColumnName(col_obj) => self.check_target_column(table, col_obj),
            AssignmentTarget::Tuple(cols) => {
                for col_obj in cols {
                    self.check_target_column(table, col_obj);
                }
            }
        }
    }

    fn check_target_column(&mut self, table: &TableSchema, col_obj: &ObjectName) {
        let Some(col) = object_name_last_ident(col_obj) else {
            return;
        };
        if !table.columns.contains_key(&col) {
            let line = self.resolve_object_name_line(col_obj, &col, None);
            self.record(line, "unknown-column");
        }
    }

    fn resolve_single_factor_table(
        &self,
        factor: &TableFactor,
        cte_names: &BTreeSet<String>,
    ) -> Option<&'a TableSchema> {
        let TableFactor::Table {
            name, args: None, ..
        } = factor
        else {
            return None;
        };
        let (table_schema, table_name) = split_object_name(name)?;
        if table_schema.is_none() && cte_names.contains(&table_name) {
            return None;
        }
        self.schema.get_table(table_schema.as_deref(), &table_name)
    }

    fn resolve_object_name_line(
        &self,
        obj: &ObjectName,
        ident_hint: &str,
        keyword_hint: Option<Keyword>,
    ) -> u32 {
        let span_line = crate::ast_loc::object_name_start_loc(obj).line as u32;
        if span_line > 0 {
            return span_line;
        }
        resolve_fallback_line(
            self.tokens,
            self.stmt_start_line,
            Some(ident_hint),
            keyword_hint,
        )
    }
}

fn select_has_grouping_or_aggregates(select: &Select) -> bool {
    let has_group_by = match &select.group_by {
        GroupByExpr::All(_) => true,
        GroupByExpr::Expressions(exprs, _) => !exprs.is_empty(),
    };
    if has_group_by {
        return true;
    }
    select.projection.iter().any(|item| match item {
        SelectItem::UnnamedExpr(expr)
        | SelectItem::ExprWithAlias { expr, .. }
        | SelectItem::ExprWithAliases { expr, .. } => expr_contains_aggregate(expr),
        SelectItem::Wildcard(_) | SelectItem::QualifiedWildcard(_, _) => false,
    })
}

fn collect_select_order_prefix(select: &Select, scope: &QueryScope<'_>) -> ConstrainedCols {
    let empty = BTreeSet::new();
    let mut prefix = ConstrainedCols::new();
    if let Some(prewhere) = &select.prewhere {
        collect_conjunct_cols(prewhere, scope, &empty, true, &mut prefix);
    }
    if let Some(selection) = &select.selection {
        collect_conjunct_cols(selection, scope, &empty, true, &mut prefix);
    }
    for twj in &select.from {
        collect_inner_join_order_prefix(twj, scope, &empty, &mut prefix);
    }
    prefix
}

fn collect_inner_join_order_prefix(
    twj: &TableWithJoins,
    scope: &QueryScope<'_>,
    excluded: &BTreeSet<String>,
    prefix: &mut ConstrainedCols,
) {
    if let TableFactor::NestedJoin {
        table_with_joins, ..
    } = &twj.relation
    {
        collect_inner_join_order_prefix(table_with_joins, scope, excluded, prefix);
    }
    for join in &twj.joins {
        if let TableFactor::NestedJoin {
            table_with_joins, ..
        } = &join.relation
        {
            collect_inner_join_order_prefix(table_with_joins, scope, excluded, prefix);
        }
        if let JoinOperator::Inner(JoinConstraint::On(expr))
        | JoinOperator::Join(JoinConstraint::On(expr))
        | JoinOperator::StraightJoin(JoinConstraint::On(expr)) = &join.join_operator
        {
            collect_conjunct_cols(expr, scope, excluded, true, prefix);
        }
    }
}
