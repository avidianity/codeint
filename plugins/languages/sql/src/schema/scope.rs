use std::collections::BTreeSet;

use sqlparser::ast::{Expr, Ident, ObjectName, SelectItem, TableFactor, TableWithJoins};

use crate::schema::types::{SqlSchema, TableSchema, normalize_ident, split_object_name};

const SYSTEM_COLUMNS: &[&str] = &[
    "_rowid_", "cmax", "cmin", "ctid", "oid", "rowid", "tableoid", "xmax", "xmin",
];

#[derive(Clone, Debug)]
pub(crate) struct TableBinding<'a> {
    pub(crate) schema: Option<String>,
    pub(crate) table_name: Option<String>,
    pub(crate) alias: Option<String>,
    pub(crate) table: Option<&'a TableSchema>,
}

impl<'a> TableBinding<'a> {
    fn matches_qualifier(&self, qualifier: &str) -> bool {
        if let Some(alias) = &self.alias {
            return alias == qualifier;
        }
        self.table_name.as_deref() == Some(qualifier)
    }

    fn matches_schema_table(&self, schema: &str, table: &str) -> bool {
        self.alias.is_none()
            && self.schema.as_deref() == Some(schema)
            && self.table_name.as_deref() == Some(table)
    }
}

#[derive(Clone, Debug)]
pub(crate) struct QueryScope<'a> {
    pub(crate) bindings: Vec<TableBinding<'a>>,
    pub(crate) cte_names: BTreeSet<String>,
    pub(crate) parent: Option<&'a QueryScope<'a>>,
}

impl<'a> QueryScope<'a> {
    pub(crate) fn root(cte_names: BTreeSet<String>) -> QueryScope<'a> {
        QueryScope {
            bindings: Vec::new(),
            cte_names,
            parent: None,
        }
    }

    pub(crate) fn child(&'a self) -> QueryScope<'a> {
        QueryScope {
            bindings: Vec::new(),
            cte_names: self.cte_names.clone(),
            parent: Some(self),
        }
    }

    pub(crate) fn add_table_with_joins(&mut self, twj: &TableWithJoins, sql_schema: &'a SqlSchema) {
        self.add_table_factor(&twj.relation, sql_schema);
        for join in &twj.joins {
            self.add_table_factor(&join.relation, sql_schema);
        }
    }

    pub(crate) fn add_table_factor(&mut self, factor: &TableFactor, sql_schema: &'a SqlSchema) {
        match factor {
            TableFactor::Table {
                name, alias, args, ..
            } => {
                let alias_name = alias.as_ref().map(|a| normalize_ident(&a.name));
                if args.is_some() {
                    self.bindings.push(TableBinding {
                        schema: None,
                        table_name: None,
                        alias: alias_name,
                        table: None,
                    });
                    return;
                }
                self.add_named_table(name, alias_name, sql_schema);
            }
            TableFactor::Derived { alias, .. } => {
                self.bindings.push(TableBinding {
                    schema: None,
                    table_name: None,
                    alias: alias.as_ref().map(|a| normalize_ident(&a.name)),
                    table: None,
                });
            }
            TableFactor::NestedJoin {
                table_with_joins, ..
            } => {
                self.add_table_with_joins(table_with_joins, sql_schema);
            }
            _ => {
                self.bindings.push(TableBinding {
                    schema: None,
                    table_name: None,
                    alias: None,
                    table: None,
                });
            }
        }
    }

    pub(crate) fn add_named_table(
        &mut self,
        name: &ObjectName,
        alias: Option<String>,
        sql_schema: &'a SqlSchema,
    ) {
        let Some((table_schema, table_name)) = split_object_name(name) else {
            self.bindings.push(TableBinding {
                schema: None,
                table_name: None,
                alias,
                table: None,
            });
            return;
        };
        let is_cte = table_schema.is_none() && self.cte_names.contains(&table_name);
        let resolved = if is_cte {
            None
        } else {
            sql_schema.get_table(table_schema.as_deref(), &table_name)
        };
        self.bindings.push(TableBinding {
            schema: table_schema,
            table_name: Some(table_name),
            alias,
            table: resolved,
        });
    }

    pub(crate) fn resolve_column_expr(
        &self,
        expr: &Expr,
        excluded_names: &BTreeSet<String>,
    ) -> Option<(&'a TableSchema, String)> {
        match peel_nested(expr) {
            Expr::Identifier(ident) => self.resolve_unqualified(ident, excluded_names),
            Expr::CompoundIdentifier(idents) => self.resolve_qualified(idents),
            _ => None,
        }
    }

    pub(crate) fn resolve_unqualified(
        &self,
        ident: &Ident,
        excluded_names: &BTreeSet<String>,
    ) -> Option<(&'a TableSchema, String)> {
        let col = normalize_ident(ident);
        if excluded_names.contains(&col) || is_system_column(&col) {
            return None;
        }
        if let Some(found) = self.resolve_known_column_in_chain(&col) {
            return Some((found, col));
        }
        self.resolve_single_unambiguous_table()
            .map(|table| (table, col))
    }

    fn resolve_known_column_in_chain(&self, col: &str) -> Option<&'a TableSchema> {
        let mut cursor = Some(self);
        while let Some(scope) = cursor {
            if !scope.bindings.is_empty() {
                if scope.has_unknown_binding() {
                    return None;
                }
                let matches: Vec<&'a TableSchema> = scope
                    .bindings
                    .iter()
                    .filter_map(|b| b.table)
                    .filter(|t| t.columns.contains_key(col))
                    .collect();
                if matches.len() == 1 {
                    return Some(matches[0]);
                }
                if matches.len() > 1 {
                    return None;
                }
            }
            cursor = scope.parent;
        }
        None
    }

    fn resolve_single_unambiguous_table(&self) -> Option<&'a TableSchema> {
        let mut known_table: Option<&'a TableSchema> = None;
        let mut total_tables = 0usize;
        let mut cursor = Some(self);
        while let Some(scope) = cursor {
            if scope.has_unknown_binding() {
                return None;
            }
            for binding in &scope.bindings {
                total_tables += 1;
                if total_tables > 1 {
                    return None;
                }
                known_table = binding.table;
            }
            cursor = scope.parent;
        }
        known_table
    }

    pub(crate) fn resolve_qualified(&self, idents: &[Ident]) -> Option<(&'a TableSchema, String)> {
        match idents {
            [qual, col] => {
                let q = normalize_ident(qual);
                let c = normalize_ident(col);
                if is_system_column(&c) {
                    return None;
                }
                self.lookup_by_qualifier(&q).map(|table| (table, c))
            }
            [schema, table, col] => {
                let s = normalize_ident(schema);
                let t = normalize_ident(table);
                let c = normalize_ident(col);
                if is_system_column(&c) {
                    return None;
                }
                self.lookup_by_schema_table(&s, &t).map(|ts| (ts, c))
            }
            _ => None,
        }
    }

    fn lookup_by_qualifier(&self, qualifier: &str) -> Option<&'a TableSchema> {
        let mut cursor = Some(self);
        while let Some(scope) = cursor {
            let matches: Vec<&TableBinding<'a>> = scope
                .bindings
                .iter()
                .filter(|b| b.matches_qualifier(qualifier))
                .collect();
            if matches.len() == 1 {
                return matches[0].table;
            }
            if matches.len() > 1 {
                return None;
            }
            cursor = scope.parent;
        }
        None
    }

    fn lookup_by_schema_table(&self, schema: &str, table: &str) -> Option<&'a TableSchema> {
        let mut cursor = Some(self);
        while let Some(scope) = cursor {
            let matches: Vec<&TableBinding<'a>> = scope
                .bindings
                .iter()
                .filter(|b| b.matches_schema_table(schema, table))
                .collect();
            if matches.len() == 1 {
                return matches[0].table;
            }
            if matches.len() > 1 {
                return None;
            }
            cursor = scope.parent;
        }
        None
    }

    pub(crate) fn has_unknown_binding(&self) -> bool {
        self.bindings.iter().any(|b| b.table.is_none())
    }
}

pub(crate) fn peel_nested(mut expr: &Expr) -> &Expr {
    while let Expr::Nested(inner) = expr {
        expr = inner;
    }
    expr
}

pub(crate) fn collect_select_aliases(projection: &[SelectItem]) -> BTreeSet<String> {
    let mut aliases = BTreeSet::new();
    for item in projection {
        match item {
            SelectItem::ExprWithAlias { alias, .. } => {
                aliases.insert(normalize_ident(alias));
            }
            SelectItem::ExprWithAliases {
                aliases: item_aliases,
                ..
            } => {
                for alias in item_aliases {
                    aliases.insert(normalize_ident(alias));
                }
            }
            _ => {}
        }
    }
    aliases
}

pub(crate) fn is_system_column(col: &str) -> bool {
    SYSTEM_COLUMNS.binary_search(&col).is_ok()
}
