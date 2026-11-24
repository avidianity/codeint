use std::collections::{BTreeMap, BTreeSet};

use sqlparser::ast::{Ident, ObjectName};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ColumnTypeClass {
    Text,
    Numeric,
    Boolean,
    Temporal,
    Binary,
    Other,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IndexSchema {
    pub name: Option<String>,
    pub columns: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TableSchema {
    pub schema: Option<String>,
    pub name: String,
    pub columns: BTreeMap<String, ColumnTypeClass>,
    pub primary_key: Vec<String>,
    pub indexes: Vec<IndexSchema>,
}

impl TableSchema {
    pub fn has_column(&self, column: &str) -> bool {
        self.columns.contains_key(&column.to_ascii_lowercase())
    }

    pub fn column_type(&self, column: &str) -> Option<ColumnTypeClass> {
        self.columns.get(&column.to_ascii_lowercase()).copied()
    }

    pub fn is_indexed_column(&self, column: &str) -> bool {
        let lower = column.to_ascii_lowercase();
        self.primary_key.first().map(String::as_str) == Some(lower.as_str())
            || self
                .indexes
                .iter()
                .any(|idx| idx.columns.first().map(String::as_str) == Some(lower.as_str()))
    }

    pub(crate) fn is_covered_by_prefix(
        &self,
        column: &str,
        prefix_cols: &BTreeSet<String>,
    ) -> bool {
        let lower = column.to_ascii_lowercase();
        index_covers_with_prefix(&self.primary_key, &lower, prefix_cols)
            || self
                .indexes
                .iter()
                .any(|idx| index_covers_with_prefix(&idx.columns, &lower, prefix_cols))
    }

    pub(crate) fn has_leading_indexed_in(&self, cols: &BTreeSet<String>) -> bool {
        self.primary_key.first().is_some_and(|pk| cols.contains(pk))
            || self
                .indexes
                .iter()
                .any(|idx| idx.columns.first().is_some_and(|c| cols.contains(c)))
    }
}

fn index_covers_with_prefix(cols: &[String], target: &str, prefix_cols: &BTreeSet<String>) -> bool {
    for col in cols {
        if col == target {
            return true;
        }
        if !prefix_cols.contains(col) {
            return false;
        }
    }
    false
}

pub(crate) type TableKey = (Option<String>, String);

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SqlSchema {
    pub(crate) tables: BTreeMap<TableKey, TableSchema>,
}

impl SqlSchema {
    pub fn table_count(&self) -> usize {
        self.tables.len()
    }

    pub fn is_empty(&self) -> bool {
        self.tables.is_empty()
    }

    pub fn get_table(&self, schema: Option<&str>, name: &str) -> Option<&TableSchema> {
        let key = self.resolve_table_key(schema, name)?;
        self.tables.get(&key)
    }

    pub(crate) fn resolve_table_key(&self, schema: Option<&str>, name: &str) -> Option<TableKey> {
        let norm_schema = schema.map(str::to_ascii_lowercase);
        let norm_name = name.to_ascii_lowercase();
        let exact = (norm_schema.clone(), norm_name.clone());
        if self.tables.contains_key(&exact) {
            return Some(exact);
        }
        if norm_schema.is_some() {
            return None;
        }
        let mut matches = self
            .tables
            .keys()
            .filter(|(_, table_name)| table_name == &norm_name);
        let first = matches.next()?;
        if matches.next().is_some() {
            return None;
        }
        Some(first.clone())
    }
}

pub(crate) fn normalize_ident(ident: &Ident) -> String {
    ident.value.to_ascii_lowercase()
}

pub(crate) fn split_object_name(name: &ObjectName) -> Option<(Option<String>, String)> {
    let parts: Vec<String> = name
        .0
        .iter()
        .filter_map(|part| part.as_ident().map(normalize_ident))
        .collect();
    match parts.as_slice() {
        [] => None,
        [table] => Some((None, table.clone())),
        [.., schema, table] => Some((Some(schema.clone()), table.clone())),
    }
}

pub(crate) fn object_name_last_ident(name: &ObjectName) -> Option<String> {
    name.0
        .last()
        .and_then(|part| part.as_ident())
        .map(normalize_ident)
}
