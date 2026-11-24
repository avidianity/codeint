use std::collections::{BTreeMap, BTreeSet};

use codeint_model::language::FlaggedLine;
use codeint_model::scan::{finalize_flagged, flag_range};
use sqlparser::ast::{
    BinaryOperator, DuplicateTreatment, Expr, Function, FunctionArguments, Ident, InTarget,
    JoinConstraint, JoinOperator, MacroDefinition, Query, Select, SelectItem, SetExpr, Statement,
    TableFactor, TableWithJoins, UnaryOperator, Value,
};
use sqlparser::keywords::Keyword;
use sqlparser::tokenizer::{Location, Token, TokenWithSpan};

use crate::ParsedSqlStatement;
use crate::ast_loc::{expr_start_line, expr_start_loc, peel_nested, push_expr_children_rev};

const RULES: &[&str] = &["or-chain-collapses-to-in", "having-without-aggregate"];

const AGGREGATE_FUNCTIONS: &[&str] = &[
    "any",
    "any_value",
    "approx_count_distinct",
    "approx_quantile",
    "arg_max",
    "arg_min",
    "argmax",
    "argmin",
    "array_agg",
    "avg",
    "bit_and",
    "bit_or",
    "bit_xor",
    "bitstring_agg",
    "bool_and",
    "bool_or",
    "corr",
    "count",
    "count_if",
    "covar_pop",
    "covar_samp",
    "entropy",
    "every",
    "favg",
    "first",
    "fsum",
    "group_concat",
    "grouping",
    "histogram",
    "json_agg",
    "json_arrayagg",
    "json_group_array",
    "json_group_object",
    "json_object_agg",
    "json_objectagg",
    "jsonb_agg",
    "jsonb_object_agg",
    "kahan_sum",
    "kurtosis",
    "last",
    "list",
    "mad",
    "max",
    "median",
    "min",
    "mode",
    "percentile_cont",
    "percentile_disc",
    "product",
    "quantile",
    "quantile_cont",
    "quantile_disc",
    "range_agg",
    "range_intersect_agg",
    "regr_avgx",
    "regr_avgy",
    "regr_count",
    "regr_intercept",
    "regr_r2",
    "regr_slope",
    "regr_sxx",
    "regr_sxy",
    "regr_syy",
    "reservoir_quantile",
    "skewness",
    "some",
    "std",
    "stddev",
    "stddev_pop",
    "stddev_samp",
    "string_agg",
    "sum",
    "sum_if",
    "total",
    "var_pop",
    "var_samp",
    "variance",
    "xmlagg",
];

pub fn flagged_line_rules() -> impl Iterator<Item = &'static str> {
    RULES.iter().copied()
}

pub(crate) fn evaluate(
    parsed: &[ParsedSqlStatement],
    code_lines: &BTreeSet<u32>,
) -> Vec<FlaggedLine> {
    let mut hits = BTreeSet::new();
    for stmt in parsed {
        let mut checker = StatementChecker {
            tokens: &stmt.tokens,
            stmt_start_line: stmt.start_line,
            code_lines,
            hits: &mut hits,
        };
        checker.visit_statement(&stmt.statement);
    }
    finalize_flagged(hits)
}

struct StatementChecker<'a> {
    tokens: &'a [TokenWithSpan],
    stmt_start_line: u32,
    code_lines: &'a BTreeSet<u32>,
    hits: &'a mut BTreeSet<(u32, &'static str)>,
}

impl StatementChecker<'_> {
    fn flag(&mut self, line: u32, rule: &'static str) {
        let target = if line > 0 { line } else { self.stmt_start_line };
        flag_range(target, target, rule, self.code_lines, self.hits);
    }

    fn visit_statement(&mut self, stmt: &Statement) {
        match stmt {
            Statement::Query(query) => self.visit_query(query),
            Statement::Explain { statement, .. } => self.visit_statement(statement),
            Statement::Insert(_)
            | Statement::Update(_)
            | Statement::Delete(_)
            | Statement::Merge(_) => {
                self.visit_dml_statement(stmt);
            }
            _ => self.visit_ddl_statement(stmt),
        }
    }

    fn visit_dml_statement(&mut self, stmt: &Statement) {
        match stmt {
            Statement::Insert(insert) => self.visit_opt_query(insert.source.as_deref()),
            Statement::Update(update) => self.visit_update(update),
            Statement::Delete(delete) => self.visit_opt_expr(delete.selection.as_ref()),
            Statement::Merge(merge) => self.visit_merge(merge),
            _ => {}
        }
    }

    fn visit_ddl_statement(&mut self, stmt: &Statement) {
        match stmt {
            Statement::CreateView(view) => self.visit_query(&view.query),
            Statement::CreateTable(table) => self.visit_opt_query(table.query.as_deref()),
            Statement::CreateIndex(index) => self.visit_opt_expr(index.predicate.as_ref()),
            Statement::CreateMacro { definition, .. } => self.visit_macro_def(definition),
            _ => {}
        }
    }

    fn visit_opt_query(&mut self, query: Option<&Query>) {
        if let Some(q) = query {
            self.visit_query(q);
        }
    }

    fn visit_opt_expr(&mut self, expr: Option<&Expr>) {
        if let Some(e) = expr {
            self.visit_expr(e);
        }
    }

    fn visit_update(&mut self, update: &sqlparser::ast::Update) {
        self.visit_table_with_joins(&update.table);
        for assign in &update.assignments {
            self.visit_expr(&assign.value);
        }
        self.visit_opt_expr(update.selection.as_ref());
    }

    fn visit_merge(&mut self, merge: &sqlparser::ast::Merge) {
        self.visit_expr(&merge.on);
        for clause in &merge.clauses {
            self.visit_opt_expr(clause.predicate.as_ref());
        }
    }

    fn visit_macro_def(&mut self, definition: &MacroDefinition) {
        match definition {
            MacroDefinition::Expr(expr) => self.visit_expr(expr),
            MacroDefinition::Table(query) => self.visit_query(query),
        }
    }

    fn visit_query(&mut self, query: &Query) {
        if let Some(with) = &query.with {
            for cte in &with.cte_tables {
                self.visit_query(&cte.query);
            }
        }
        self.visit_set_expr(&query.body);
    }

    fn visit_set_expr(&mut self, set_expr: &SetExpr) {
        let mut stack = vec![set_expr];
        while let Some(current) = stack.pop() {
            match current {
                SetExpr::Select(select) => self.visit_select(select),
                SetExpr::Query(query) => self.visit_query(query),
                SetExpr::SetOperation { left, right, .. } => {
                    stack.push(right);
                    stack.push(left);
                }
                SetExpr::Insert(stmt)
                | SetExpr::Update(stmt)
                | SetExpr::Delete(stmt)
                | SetExpr::Merge(stmt) => self.visit_statement(stmt),
                _ => {}
            }
        }
    }

    fn visit_select(&mut self, select: &Select) {
        for item in &select.projection {
            self.visit_select_item(item);
        }
        for table in &select.from {
            self.visit_table_with_joins(table);
        }
        if let Some(prewhere) = &select.prewhere {
            self.visit_expr(prewhere);
        }
        if let Some(selection) = &select.selection {
            self.visit_expr(selection);
        }
        if let Some(having) = &select.having {
            self.check_having_without_aggregate(select, having);
            self.visit_expr(having);
        }
        if let Some(qualify) = &select.qualify {
            self.visit_expr(qualify);
        }
    }

    fn visit_select_item(&mut self, item: &SelectItem) {
        match item {
            SelectItem::UnnamedExpr(expr)
            | SelectItem::ExprWithAlias { expr, .. }
            | SelectItem::ExprWithAliases { expr, .. } => self.visit_expr(expr),
            SelectItem::QualifiedWildcard(_, _) | SelectItem::Wildcard(_) => {}
        }
    }

    fn visit_table_with_joins(&mut self, table: &TableWithJoins) {
        self.visit_table_factor(&table.relation);
        for join in &table.joins {
            self.visit_table_factor(&join.relation);
            self.visit_join_operator(&join.join_operator);
        }
    }

    fn visit_table_factor(&mut self, factor: &TableFactor) {
        match factor {
            TableFactor::Derived { subquery, .. } => self.visit_query(subquery),
            TableFactor::NestedJoin {
                table_with_joins, ..
            } => self.visit_table_with_joins(table_with_joins),
            _ => {}
        }
    }

    fn visit_join_operator(&mut self, op: &JoinOperator) {
        match op {
            JoinOperator::Join(c)
            | JoinOperator::Inner(c)
            | JoinOperator::Left(c)
            | JoinOperator::LeftOuter(c)
            | JoinOperator::Right(c)
            | JoinOperator::RightOuter(c)
            | JoinOperator::FullOuter(c)
            | JoinOperator::CrossJoin(c)
            | JoinOperator::Semi(c)
            | JoinOperator::LeftSemi(c)
            | JoinOperator::RightSemi(c)
            | JoinOperator::Anti(c)
            | JoinOperator::LeftAnti(c)
            | JoinOperator::RightAnti(c) => self.visit_join_constraint(c),
            JoinOperator::AsOf {
                match_condition,
                constraint,
            } => {
                self.visit_expr(match_condition);
                self.visit_join_constraint(constraint);
            }
            _ => {}
        }
    }

    fn visit_join_constraint(&mut self, constraint: &JoinConstraint) {
        if let JoinConstraint::On(expr) = constraint {
            self.visit_expr(expr);
        }
    }

    fn check_having_without_aggregate(&mut self, select: &Select, having: &Expr) {
        if expr_contains_aggregate(having) {
            return;
        }
        let aggregate_aliases = collect_aggregate_aliases(&select.projection);
        if expr_references_any_alias(having, &aggregate_aliases) {
            return;
        }
        let start = expr_start_loc(having);
        let line = find_having_line(self.tokens, start).unwrap_or(start.line as u32);
        self.flag(line, "having-without-aggregate");
    }

    fn visit_expr(&mut self, root: &Expr) {
        let mut stack = vec![root];
        while let Some(expr) = stack.pop() {
            match expr {
                Expr::BinaryOp {
                    op: BinaryOperator::Or,
                    ..
                } => self.inspect_or_chain(expr, &mut stack),
                Expr::Subquery(query)
                | Expr::Exists {
                    subquery: query, ..
                } => self.visit_query(query),
                Expr::In {
                    target: InTarget::Subquery(query),
                    ..
                } => {
                    self.visit_query(query);
                    push_expr_children_rev(expr, &mut stack);
                }
                Expr::Function(func) => {
                    if let FunctionArguments::Subquery(query) = &func.args {
                        self.visit_query(query);
                    }
                    push_expr_children_rev(expr, &mut stack);
                }
                _ => push_expr_children_rev(expr, &mut stack),
            }
        }
    }

    fn inspect_or_chain<'a>(&mut self, root: &'a Expr, stack: &mut Vec<&'a Expr>) {
        let mut operands = Vec::new();
        flatten_or_operands(root, &mut operands);

        let mut counts: BTreeMap<Vec<String>, (usize, u32)> = BTreeMap::new();
        let mut deferred = Vec::new();
        for &operand in &operands {
            if let Some((col_key, line)) = extract_column_eq_literal(operand) {
                counts
                    .entry(col_key)
                    .and_modify(|(count, first_line)| {
                        *count += 1;
                        if line > 0 && (*first_line == 0 || line < *first_line) {
                            *first_line = line;
                        }
                    })
                    .or_insert((1, line));
            } else {
                deferred.push(operand);
            }
        }
        stack.extend(deferred.into_iter().rev());

        for (count, first_line) in counts.into_values() {
            if count >= 3 {
                self.flag(first_line, "or-chain-collapses-to-in");
            }
        }
    }
}

fn collect_aggregate_aliases(projection: &[SelectItem]) -> BTreeSet<String> {
    let mut aliases = BTreeSet::new();
    for item in projection {
        match item {
            SelectItem::ExprWithAlias { expr, alias } if expr_contains_aggregate(expr) => {
                aliases.insert(alias.value.to_ascii_lowercase());
            }
            SelectItem::ExprWithAliases {
                expr,
                aliases: item_aliases,
            } if expr_contains_aggregate(expr) => {
                for alias in item_aliases {
                    aliases.insert(alias.value.to_ascii_lowercase());
                }
            }
            _ => {}
        }
    }
    aliases
}

fn expr_references_any_alias(root: &Expr, aliases: &BTreeSet<String>) -> bool {
    if aliases.is_empty() {
        return false;
    }
    let mut stack = vec![root];
    while let Some(expr) = stack.pop() {
        match expr {
            Expr::Subquery(_) | Expr::Exists { .. } => {}
            Expr::Identifier(ident) if aliases.contains(&ident.value.to_ascii_lowercase()) => {
                return true;
            }
            other => push_expr_children_rev(other, &mut stack),
        }
    }
    false
}

fn flatten_or_operands<'a>(root: &'a Expr, out: &mut Vec<&'a Expr>) {
    let mut stack = vec![root];
    while let Some(expr) = stack.pop() {
        let unwrapped = peel_nested(expr);
        if let Expr::BinaryOp {
            left,
            op: BinaryOperator::Or,
            right,
        } = unwrapped
        {
            stack.push(right);
            stack.push(left);
        } else {
            out.push(unwrapped);
        }
    }
}

fn extract_column_eq_literal(expr: &Expr) -> Option<(Vec<String>, u32)> {
    let Expr::BinaryOp {
        left,
        op: BinaryOperator::Eq,
        right,
    } = peel_nested(expr)
    else {
        return None;
    };
    let line = expr_start_line(expr);
    if let Some(col) = extract_column_key(left)
        && is_constant_literal(right)
    {
        return Some((col, line));
    }
    if let Some(col) = extract_column_key(right)
        && is_constant_literal(left)
    {
        return Some((col, line));
    }
    None
}

fn extract_column_key(expr: &Expr) -> Option<Vec<String>> {
    match peel_nested(expr) {
        Expr::Identifier(ident) => Some(vec![normalize_ident(ident)]),
        Expr::CompoundIdentifier(idents) if !idents.is_empty() => {
            Some(idents.iter().map(normalize_ident).collect())
        }
        _ => None,
    }
}

fn normalize_ident(ident: &Ident) -> String {
    if ident.quote_style.is_none() {
        ident.value.to_ascii_lowercase()
    } else {
        ident.value.clone()
    }
}

pub(crate) fn is_bare_number_literal(expr: &Expr) -> bool {
    match peel_nested(expr) {
        Expr::Value(val) => matches!(val.value, Value::Number(_, _)),
        Expr::UnaryOp {
            op: UnaryOperator::Plus | UnaryOperator::Minus,
            expr: inner,
        } => matches!(
            peel_nested(inner),
            Expr::Value(val) if matches!(val.value, Value::Number(_, _))
        ),
        _ => false,
    }
}

fn is_constant_literal(expr: &Expr) -> bool {
    if is_bare_number_literal(expr) {
        return true;
    }
    match peel_nested(expr) {
        Expr::Value(val) => !matches!(val.value, Value::Null | Value::Placeholder(_)),
        Expr::TypedString(_) => true,
        _ => false,
    }
}

pub(crate) fn expr_contains_aggregate(root: &Expr) -> bool {
    let mut stack = vec![root];
    while let Some(expr) = stack.pop() {
        match expr {
            Expr::Subquery(_) | Expr::Exists { .. } => {}
            Expr::Function(func) => {
                if is_aggregate_call(func) {
                    return true;
                }
                push_expr_children_rev(expr, &mut stack);
            }
            other => push_expr_children_rev(other, &mut stack),
        }
    }
    false
}

pub(crate) fn is_aggregate_call(func: &Function) -> bool {
    if func.over.is_some() {
        return false;
    }
    if func.filter.is_some() || !func.within_group.is_empty() {
        return true;
    }
    if let FunctionArguments::List(list) = &func.args
        && list.duplicate_treatment == Some(DuplicateTreatment::Distinct)
    {
        return true;
    }
    let Some(last_ident) = func.name.0.last().and_then(|part| part.as_ident()) else {
        return false;
    };
    let lower = last_ident.value.to_ascii_lowercase();
    AGGREGATE_FUNCTIONS.binary_search(&lower.as_str()).is_ok()
}

fn find_having_line(tokens: &[TokenWithSpan], expr_start: Location) -> Option<u32> {
    if expr_start.line == 0 {
        return tokens
            .iter()
            .find(|t| matches!(&t.token, Token::Word(w) if w.keyword == Keyword::HAVING))
            .map(|t| t.span.start.line as u32);
    }
    let before = tokens
        .iter()
        .rposition(|t| t.span.start < expr_start && !matches!(t.token, Token::Whitespace(_)))?;
    for tok in tokens[..=before].iter().rev() {
        if matches!(tok.token, Token::Whitespace(_) | Token::LParen) {
            continue;
        }
        if matches!(&tok.token, Token::Word(w) if w.keyword == Keyword::HAVING) {
            return Some(tok.span.start.line as u32);
        }
        break;
    }
    None
}
