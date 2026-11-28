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
use crate::schema::artifact::{
    ClauseKind, ConstrainedRef, SchemaCheck, ScopeDef, SqlFileArtifact, TableBindingDef,
    evaluate_artifact_checks,
};
use crate::schema::expr::{
    ExprContext, ExprExtractor, collect_conjunct_refs, extract_expr, extract_filter_expr,
    push_constrained_ref, resolve_fallback_line,
};
use crate::schema::scope::collect_select_aliases;
use crate::schema::types::{SqlSchema, normalize_ident, object_name_last_ident, split_object_name};
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
    let (scopes, checks) = extract_scopes_and_checks(statements, dialect);
    let artifact = SqlFileArtifact {
        is_down_migration: false,
        ddl_ops: Vec::new(),
        scopes,
        checks,
    };
    evaluate_artifact_checks(&artifact, schema)
}

pub(crate) fn extract_scopes_and_checks(
    statements: &[ParsedSqlStatement],
    dialect: SqlDialect,
) -> (Vec<ScopeDef>, Vec<SchemaCheck>) {
    let mut scopes = Vec::new();
    let mut checks = Vec::new();
    for parsed in statements {
        let mut extractor = SchemaCheckExtractor {
            dialect,
            tokens: &parsed.tokens,
            stmt_start_line: parsed.start_line,
            scopes: &mut scopes,
            checks: &mut checks,
        };
        extractor.visit_statement(&parsed.statement);
    }
    (scopes, checks)
}

struct SchemaCheckExtractor<'a> {
    dialect: SqlDialect,
    tokens: &'a [TokenWithSpan],
    stmt_start_line: u32,
    scopes: &'a mut Vec<ScopeDef>,
    checks: &'a mut Vec<SchemaCheck>,
}

impl ExprExtractor for SchemaCheckExtractor<'_> {
    fn tokens(&self) -> &[TokenWithSpan] {
        self.tokens
    }

    fn stmt_start_line(&self) -> u32 {
        self.stmt_start_line
    }

    fn dialect(&self) -> SqlDialect {
        self.dialect
    }

    fn push_check(&mut self, check: SchemaCheck) {
        self.checks.push(check);
    }

    fn visit_subquery(&mut self, query: &Query, parent_scope: u32, parent_ctes: &BTreeSet<String>) {
        self.visit_query_in_parent(query, parent_scope, parent_ctes);
    }
}

impl SchemaCheckExtractor<'_> {
    fn alloc_scope(&mut self, parent: Option<u32>, bindings: Vec<TableBindingDef>) -> u32 {
        let depth = parent
            .and_then(|idx| self.scopes.get(idx as usize))
            .map_or(0, |scope| scope.depth + 1);
        let id = self.scopes.len() as u32;
        self.scopes.push(ScopeDef {
            depth,
            parent,
            bindings,
        });
        id
    }

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
        {
            for col_obj in &insert.columns {
                self.record_target_column(&table_schema, &name, col_obj);
            }
        }
        if let Some(source) = &insert.source {
            self.visit_query(source, BTreeSet::new());
        }
    }

    fn visit_update(&mut self, update: &Update) {
        let empty = BTreeSet::new();
        let mut bindings = Vec::new();
        collect_table_with_joins_bindings(&update.table, &empty, &mut bindings);
        if let Some(from) = &update.from {
            let from_tables = match from {
                UpdateTableFromKind::BeforeSet(tables) | UpdateTableFromKind::AfterSet(tables) => {
                    tables
                }
            };
            for twj in from_tables {
                collect_table_with_joins_bindings(twj, &empty, &mut bindings);
            }
        }
        let scope_id = self.alloc_scope(None, bindings);
        let target_table = extract_single_factor_table(&update.table.relation, &empty);
        self.visit_table_with_joins_subqueries(&update.table, scope_id, &empty);
        self.check_table_with_joins_conditions(&update.table, scope_id, &empty);

        for assign in &update.assignments {
            if let Some((ref table_schema, ref table_name)) = target_table {
                self.record_assignment_target(table_schema, table_name, &assign.target);
            }
            extract_expr(
                self,
                &assign.value,
                ExprContext::new(scope_id, &empty, &empty, ClauseKind::General, &[]),
            );
        }
        self.check_dml_tail(
            update.selection.as_ref(),
            update.returning.as_deref(),
            &update.order_by,
            scope_id,
            &empty,
        );
    }

    fn visit_delete(&mut self, delete: &Delete) {
        let empty = BTreeSet::new();
        let mut bindings = Vec::new();
        let from_tables = match &delete.from {
            FromTable::WithFromKeyword(tables) | FromTable::WithoutKeyword(tables) => tables,
        };
        for twj in from_tables {
            collect_table_with_joins_bindings(twj, &empty, &mut bindings);
        }
        if let Some(using) = &delete.using {
            for twj in using {
                collect_table_with_joins_bindings(twj, &empty, &mut bindings);
            }
        }
        let scope_id = self.alloc_scope(None, bindings);
        for twj in from_tables {
            self.visit_table_with_joins_subqueries(twj, scope_id, &empty);
            self.check_table_with_joins_conditions(twj, scope_id, &empty);
        }
        self.check_dml_tail(
            delete.selection.as_ref(),
            delete.returning.as_deref(),
            &delete.order_by,
            scope_id,
            &empty,
        );
    }

    fn check_dml_tail(
        &mut self,
        selection: Option<&Expr>,
        returning: Option<&[SelectItem]>,
        order_by: &[OrderByExpr],
        scope_id: u32,
        cte_names: &BTreeSet<String>,
    ) {
        let empty = BTreeSet::new();
        let mut order_prefix = Vec::new();
        if let Some(filter) = selection {
            extract_filter_expr(
                self,
                filter,
                ExprContext::new(scope_id, cte_names, &empty, ClauseKind::Filter, &[]),
            );
            collect_conjunct_refs(filter, &empty, true, &mut order_prefix);
        }
        if let Some(items) = returning {
            self.check_select_items(items, scope_id, cte_names, &empty);
        }
        self.check_order_by_exprs(order_by, scope_id, cte_names, &empty, order_prefix, false);
    }

    fn visit_merge(&mut self, merge: &Merge) {
        let empty = BTreeSet::new();
        let mut bindings = Vec::new();
        collect_table_factor_bindings(&merge.table, &empty, &mut bindings);
        collect_table_factor_bindings(&merge.source, &empty, &mut bindings);
        let scope_id = self.alloc_scope(None, bindings);
        self.visit_table_factor_subqueries(&merge.table, scope_id, &empty);
        self.visit_table_factor_subqueries(&merge.source, scope_id, &empty);
        extract_filter_expr(
            self,
            &merge.on,
            ExprContext::new(scope_id, &empty, &empty, ClauseKind::JoinOn, &[]),
        );
        for clause in &merge.clauses {
            if let Some(predicate) = &clause.predicate {
                extract_filter_expr(
                    self,
                    predicate,
                    ExprContext::new(scope_id, &empty, &empty, ClauseKind::Filter, &[]),
                );
            }
        }
    }

    fn visit_query(&mut self, query: &Query, outer_ctes: BTreeSet<String>) {
        let cte_names = self.collect_and_visit_ctes(query, outer_ctes);
        self.visit_set_expr(&query.body, query.order_by.as_ref(), cte_names, None);
    }

    fn visit_query_in_parent(
        &mut self,
        query: &Query,
        parent_scope: u32,
        parent_ctes: &BTreeSet<String>,
    ) {
        let cte_names = self.collect_and_visit_ctes(query, parent_ctes.clone());
        self.visit_set_expr(
            &query.body,
            query.order_by.as_ref(),
            cte_names,
            Some(parent_scope),
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
        parent_scope: Option<u32>,
    ) {
        let mut stack = vec![(set_expr, order_by)];
        while let Some((current, current_order_by)) = stack.pop() {
            match current {
                SetExpr::Select(select) => {
                    self.visit_select(select, current_order_by, &cte_names, parent_scope);
                }
                SetExpr::Query(inner) => match parent_scope {
                    Some(parent) => self.visit_query_in_parent(inner, parent, &cte_names),
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
        cte_names: &BTreeSet<String>,
        parent_scope: Option<u32>,
    ) {
        let mut bindings = Vec::new();
        for twj in &select.from {
            collect_table_with_joins_bindings(twj, cte_names, &mut bindings);
        }
        let scope_id = self.alloc_scope(parent_scope, bindings);
        let empty = BTreeSet::new();
        let select_aliases = collect_select_aliases(&select.projection);

        for twj in &select.from {
            self.visit_table_with_joins_subqueries(twj, scope_id, cte_names);
            self.check_table_with_joins_conditions(twj, scope_id, cte_names);
        }
        self.check_select_items(&select.projection, scope_id, cte_names, &empty);
        if let Some(prewhere) = &select.prewhere {
            extract_filter_expr(
                self,
                prewhere,
                ExprContext::new(scope_id, cte_names, &empty, ClauseKind::Filter, &[]),
            );
        }
        if let Some(selection) = &select.selection {
            extract_filter_expr(
                self,
                selection,
                ExprContext::new(scope_id, cte_names, &empty, ClauseKind::Filter, &[]),
            );
        }
        self.check_select_post_clauses(select, order_by, scope_id, cte_names, &select_aliases);
    }

    fn check_select_post_clauses(
        &mut self,
        select: &Select,
        order_by: Option<&OrderBy>,
        scope_id: u32,
        cte_names: &BTreeSet<String>,
        select_aliases: &BTreeSet<String>,
    ) {
        if let GroupByExpr::Expressions(exprs, _) = &select.group_by {
            let ctx = ExprContext::new(
                scope_id,
                cte_names,
                select_aliases,
                ClauseKind::General,
                &[],
            );
            for expr in exprs {
                extract_expr(self, expr, ctx);
            }
        }
        if let Some(having) = &select.having {
            extract_expr(
                self,
                having,
                ExprContext::new(
                    scope_id,
                    cte_names,
                    select_aliases,
                    ClauseKind::General,
                    &[],
                ),
            );
        }
        if let Some(qualify) = &select.qualify {
            extract_expr(
                self,
                qualify,
                ExprContext::new(
                    scope_id,
                    cte_names,
                    select_aliases,
                    ClauseKind::General,
                    &[],
                ),
            );
        }
        if let Some(order) = order_by
            && let OrderByKind::Expressions(exprs) = &order.kind
        {
            let order_prefix = collect_select_order_prefix(select);
            let grouped = select_has_grouping_or_aggregates(select);
            self.check_order_by_exprs(
                exprs,
                scope_id,
                cte_names,
                select_aliases,
                order_prefix,
                grouped,
            );
        }
    }

    fn check_select_items(
        &mut self,
        items: &[SelectItem],
        scope_id: u32,
        cte_names: &BTreeSet<String>,
        excluded: &BTreeSet<String>,
    ) {
        let ctx = ExprContext::new(scope_id, cte_names, excluded, ClauseKind::General, &[]);
        for item in items {
            match item {
                SelectItem::UnnamedExpr(expr)
                | SelectItem::ExprWithAlias { expr, .. }
                | SelectItem::ExprWithAliases { expr, .. } => {
                    extract_expr(self, expr, ctx);
                }
                SelectItem::QualifiedWildcard(_, _) | SelectItem::Wildcard(_) => {}
            }
        }
    }

    fn check_order_by_exprs(
        &mut self,
        exprs: &[OrderByExpr],
        scope_id: u32,
        cte_names: &BTreeSet<String>,
        excluded: &BTreeSet<String>,
        mut prefix: Vec<ConstrainedRef>,
        in_grouped_query: bool,
    ) {
        for order_expr in exprs {
            let ctx =
                ExprContext::order_by(scope_id, cte_names, excluded, &prefix, in_grouped_query);
            extract_expr(self, &order_expr.expr, ctx);
            push_constrained_ref(&order_expr.expr, excluded, &mut prefix);
        }
    }

    fn visit_table_with_joins_subqueries(
        &mut self,
        twj: &TableWithJoins,
        scope_id: u32,
        cte_names: &BTreeSet<String>,
    ) {
        self.visit_table_factor_subqueries(&twj.relation, scope_id, cte_names);
        for join in &twj.joins {
            self.visit_table_factor_subqueries(&join.relation, scope_id, cte_names);
        }
    }

    fn visit_table_factor_subqueries(
        &mut self,
        factor: &TableFactor,
        scope_id: u32,
        cte_names: &BTreeSet<String>,
    ) {
        match factor {
            TableFactor::Derived {
                lateral, subquery, ..
            } => {
                if *lateral {
                    self.visit_query_in_parent(subquery, scope_id, cte_names);
                } else {
                    self.visit_query(subquery, cte_names.clone());
                }
            }
            TableFactor::NestedJoin {
                table_with_joins, ..
            } => self.visit_table_with_joins_subqueries(table_with_joins, scope_id, cte_names),
            _ => {}
        }
    }

    fn check_table_with_joins_conditions(
        &mut self,
        twj: &TableWithJoins,
        scope_id: u32,
        cte_names: &BTreeSet<String>,
    ) {
        if let TableFactor::NestedJoin {
            table_with_joins, ..
        } = &twj.relation
        {
            self.check_table_with_joins_conditions(table_with_joins, scope_id, cte_names);
        }
        let mut left_bindings = Vec::new();
        collect_table_factor_bindings(&twj.relation, cte_names, &mut left_bindings);
        for join in &twj.joins {
            if let TableFactor::NestedJoin {
                table_with_joins, ..
            } = &join.relation
            {
                self.check_table_with_joins_conditions(table_with_joins, scope_id, cte_names);
            }
            self.check_join(join, &left_bindings, scope_id, cte_names);
            collect_table_factor_bindings(&join.relation, cte_names, &mut left_bindings);
        }
    }

    fn check_join(
        &mut self,
        join: &Join,
        left_bindings: &[TableBindingDef],
        full_scope: u32,
        cte_names: &BTreeSet<String>,
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
                self.check_join_constraint(
                    constraint,
                    &join.relation,
                    left_bindings,
                    full_scope,
                    cte_names,
                );
            }
            JoinOperator::AsOf {
                match_condition,
                constraint,
            } => {
                extract_filter_expr(
                    self,
                    match_condition,
                    ExprContext::new(full_scope, cte_names, &empty, ClauseKind::JoinOn, &[]),
                );
                self.check_join_constraint(
                    constraint,
                    &join.relation,
                    left_bindings,
                    full_scope,
                    cte_names,
                );
            }
            _ => {}
        }
    }

    fn check_join_constraint(
        &mut self,
        constraint: &JoinConstraint,
        right_relation: &TableFactor,
        left_bindings: &[TableBindingDef],
        full_scope: u32,
        cte_names: &BTreeSet<String>,
    ) {
        let empty = BTreeSet::new();
        match constraint {
            JoinConstraint::On(expr) => {
                extract_filter_expr(
                    self,
                    expr,
                    ExprContext::new(full_scope, cte_names, &empty, ClauseKind::JoinOn, &[]),
                );
            }
            JoinConstraint::Using(columns) | JoinConstraint::UsingAlias(columns, _) => {
                let left_scope = self.alloc_scope(None, left_bindings.to_vec());
                let right_table = extract_single_factor_table(right_relation, cte_names);
                let using_cols: Vec<String> =
                    columns.iter().filter_map(object_name_last_ident).collect();
                for col_obj in columns {
                    let Some(column) = object_name_last_ident(col_obj) else {
                        continue;
                    };
                    let line =
                        self.resolve_object_name_line(col_obj, &column, Some(Keyword::USING));
                    self.checks.push(SchemaCheck::UsingColumn {
                        left_scope,
                        right_table: right_table.clone(),
                        column,
                        using_cols: using_cols.clone(),
                        line,
                    });
                }
            }
            JoinConstraint::Natural | JoinConstraint::None => {}
        }
    }

    fn record_assignment_target(
        &mut self,
        table_schema: &Option<String>,
        table_name: &str,
        target: &AssignmentTarget,
    ) {
        match target {
            AssignmentTarget::ColumnName(col_obj) => {
                self.record_target_column(table_schema, table_name, col_obj);
            }
            AssignmentTarget::Tuple(cols) => {
                for col_obj in cols {
                    self.record_target_column(table_schema, table_name, col_obj);
                }
            }
        }
    }

    fn record_target_column(
        &mut self,
        table_schema: &Option<String>,
        table_name: &str,
        col_obj: &ObjectName,
    ) {
        let Some(column) = object_name_last_ident(col_obj) else {
            return;
        };
        let line = self.resolve_object_name_line(col_obj, &column, None);
        self.checks.push(SchemaCheck::TargetColumn {
            table_schema: table_schema.clone(),
            table_name: table_name.to_owned(),
            column,
            line,
        });
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

fn collect_table_with_joins_bindings(
    twj: &TableWithJoins,
    cte_names: &BTreeSet<String>,
    out: &mut Vec<TableBindingDef>,
) {
    collect_table_factor_bindings(&twj.relation, cte_names, out);
    for join in &twj.joins {
        collect_table_factor_bindings(&join.relation, cte_names, out);
    }
}

fn collect_table_factor_bindings(
    factor: &TableFactor,
    cte_names: &BTreeSet<String>,
    out: &mut Vec<TableBindingDef>,
) {
    match factor {
        TableFactor::Table {
            name, alias, args, ..
        } => {
            let alias_name = alias.as_ref().map(|a| normalize_ident(&a.name));
            if args.is_some() {
                out.push(TableBindingDef {
                    schema: None,
                    table_name: None,
                    alias: alias_name,
                    resolvable: false,
                });
                return;
            }
            let Some((table_schema, table_name)) = split_object_name(name) else {
                out.push(TableBindingDef {
                    schema: None,
                    table_name: None,
                    alias: alias_name,
                    resolvable: false,
                });
                return;
            };
            let is_cte = table_schema.is_none() && cte_names.contains(&table_name);
            out.push(TableBindingDef {
                schema: table_schema,
                table_name: Some(table_name),
                alias: alias_name,
                resolvable: !is_cte,
            });
        }
        TableFactor::Derived { alias, .. } => {
            out.push(TableBindingDef {
                schema: None,
                table_name: None,
                alias: alias.as_ref().map(|a| normalize_ident(&a.name)),
                resolvable: false,
            });
        }
        TableFactor::NestedJoin {
            table_with_joins, ..
        } => collect_table_with_joins_bindings(table_with_joins, cte_names, out),
        _ => {
            out.push(TableBindingDef {
                schema: None,
                table_name: None,
                alias: None,
                resolvable: false,
            });
        }
    }
}

fn extract_single_factor_table(
    factor: &TableFactor,
    cte_names: &BTreeSet<String>,
) -> Option<(Option<String>, String)> {
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
    Some((table_schema, table_name))
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

fn collect_select_order_prefix(select: &Select) -> Vec<ConstrainedRef> {
    let empty = BTreeSet::new();
    let mut prefix = Vec::new();
    if let Some(prewhere) = &select.prewhere {
        collect_conjunct_refs(prewhere, &empty, true, &mut prefix);
    }
    if let Some(selection) = &select.selection {
        collect_conjunct_refs(selection, &empty, true, &mut prefix);
    }
    for twj in &select.from {
        collect_inner_join_order_prefix(twj, &empty, &mut prefix);
    }
    prefix
}

fn collect_inner_join_order_prefix(
    twj: &TableWithJoins,
    excluded: &BTreeSet<String>,
    prefix: &mut Vec<ConstrainedRef>,
) {
    if let TableFactor::NestedJoin {
        table_with_joins, ..
    } = &twj.relation
    {
        collect_inner_join_order_prefix(table_with_joins, excluded, prefix);
    }
    for join in &twj.joins {
        if let TableFactor::NestedJoin {
            table_with_joins, ..
        } = &join.relation
        {
            collect_inner_join_order_prefix(table_with_joins, excluded, prefix);
        }
        if let JoinOperator::Inner(JoinConstraint::On(expr))
        | JoinOperator::Join(JoinConstraint::On(expr))
        | JoinOperator::StraightJoin(JoinConstraint::On(expr)) = &join.join_operator
        {
            collect_conjunct_refs(expr, excluded, true, prefix);
        }
    }
}
