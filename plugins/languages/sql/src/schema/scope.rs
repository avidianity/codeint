use std::collections::BTreeSet;

use sqlparser::ast::{Expr, SelectItem};

use crate::schema::types::normalize_ident;

const SYSTEM_COLUMNS: &[&str] = &[
    "_rowid_", "cmax", "cmin", "ctid", "oid", "rowid", "tableoid", "xmax", "xmin",
];

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
