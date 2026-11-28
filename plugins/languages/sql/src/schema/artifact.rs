use std::collections::{BTreeMap, BTreeSet};

use codeint_model::{
    FileHazards, FileScopeArtifact, HazardLine, LanguagePluginError, RepoPath, ScopeAnalysis,
    ScopeSummary, SourceSet, SummaryField,
};

use crate::SqlDialect;
use crate::schema::types::{ColumnTypeClass, IndexSchema, SqlSchema, TableSchema};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ClauseKind {
    General,
    Filter,
    JoinOn,
    OrderBy,
}

impl ClauseKind {
    pub(crate) fn to_u8(self) -> u8 {
        match self {
            ClauseKind::General => 0,
            ClauseKind::Filter => 1,
            ClauseKind::JoinOn => 2,
            ClauseKind::OrderBy => 3,
        }
    }

    pub(crate) fn from_u8(tag: u8) -> Option<ClauseKind> {
        match tag {
            0 => Some(ClauseKind::General),
            1 => Some(ClauseKind::Filter),
            2 => Some(ClauseKind::JoinOn),
            3 => Some(ClauseKind::OrderBy),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ColumnAlterOp {
    AddColumn {
        name: String,
        ty: ColumnTypeClass,
        is_pk: bool,
        unique_index: Option<Option<String>>,
    },
    DropColumn {
        name: String,
    },
    RenameColumn {
        old: String,
        new: String,
    },
    ChangeColumn {
        old: String,
        new: String,
        ty: ColumnTypeClass,
        is_pk: bool,
        unique_index: Option<Option<String>>,
    },
    ModifyColumn {
        name: String,
        ty: ColumnTypeClass,
        is_pk: bool,
        unique_index: Option<Option<String>>,
    },
    SetColumnType {
        name: String,
        ty: ColumnTypeClass,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum TableAlterOp {
    SetPrimaryKey {
        columns: Vec<String>,
    },
    AddIndex {
        index: IndexSchema,
    },
    PromoteIndexToPk {
        index_name: String,
    },
    DropNamedIndex {
        name: String,
    },
    DropPrimaryKey,
    RenameTable {
        new_schema: Option<String>,
        new_name: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum AlterOp {
    Column(ColumnAlterOp),
    Table(TableAlterOp),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum DdlOp {
    CreateTable {
        if_not_exists: bool,
        table: TableSchema,
    },
    CreateIndex {
        table_schema: Option<String>,
        table_name: String,
        index: IndexSchema,
    },
    AlterTable {
        table_schema: Option<String>,
        table_name: String,
        ops: Vec<AlterOp>,
    },
    DropTable {
        schema: Option<String>,
        name: String,
    },
    DropIndexOnTable {
        table_schema: Option<String>,
        table_name: String,
        index_name: String,
    },
    DropIndexAcrossTables {
        index_schema: Option<String>,
        index_name: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TableBindingDef {
    pub(crate) schema: Option<String>,
    pub(crate) table_name: Option<String>,
    pub(crate) alias: Option<String>,
    pub(crate) resolvable: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ScopeDef {
    pub(crate) depth: u32,
    pub(crate) parent: Option<u32>,
    pub(crate) bindings: Vec<TableBindingDef>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ColumnRefTarget {
    Unqualified(String),
    Qualified2(String, String),
    Qualified3(String, String, String),
    Leading(Vec<(String, u32)>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ConstrainedRef {
    pub(crate) target: ColumnRefTarget,
    pub(crate) excluded: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum SchemaCheck {
    TargetColumn {
        table_schema: Option<String>,
        table_name: String,
        column: String,
        line: u32,
    },
    UsingColumn {
        left_scope: u32,
        right_table: Option<(Option<String>, String)>,
        column: String,
        using_cols: Vec<String>,
        line: u32,
    },
    ColumnRef {
        scope: u32,
        target: ColumnRefTarget,
        excluded: Vec<String>,
        clause: ClauseKind,
        constrained: Vec<ConstrainedRef>,
        line: u32,
    },
    TextNumber {
        scope: u32,
        target: ColumnRefTarget,
        excluded: Vec<String>,
        line: u32,
    },
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct SqlFileArtifact {
    pub(crate) is_down_migration: bool,
    pub(crate) ddl_ops: Vec<DdlOp>,
    pub(crate) scopes: Vec<ScopeDef>,
    pub(crate) checks: Vec<SchemaCheck>,
}

pub(crate) fn is_down_migration_path(path: &RepoPath) -> bool {
    let file_name = path.file_name();
    file_name.eq_ignore_ascii_case("down.sql")
        || file_name.to_ascii_lowercase().ends_with(".down.sql")
}

fn resolve_scope_dialect(dialect: Option<&str>) -> Result<SqlDialect, LanguagePluginError> {
    match dialect {
        Some(name) => {
            SqlDialect::from_selectable_name(name).ok_or_else(|| LanguagePluginError::Call {
                plugin: "sql".to_owned(),
                call: "analyze-scope",
                reason: format!("unknown SQL dialect {name:?}"),
            })
        }
        None => Ok(SqlDialect::Generic),
    }
}

fn decode_scope_artifacts(
    artifacts: Vec<FileScopeArtifact>,
) -> Result<Vec<(FileScopeArtifact, SqlFileArtifact)>, LanguagePluginError> {
    artifacts
        .into_iter()
        .map(|item| {
            let parsed =
                crate::schema::codec::decode_artifact(&item.artifact).ok_or_else(|| {
                    LanguagePluginError::Call {
                        plugin: "sql".to_owned(),
                        call: "analyze-scope",
                        reason: format!("invalid SQL scope artifact for {}", item.path),
                    }
                })?;
            Ok((item, parsed))
        })
        .collect()
}

pub(crate) fn analyze_scope_artifacts(
    dialect: Option<&str>,
    mut artifacts: Vec<FileScopeArtifact>,
) -> Result<ScopeAnalysis, LanguagePluginError> {
    let resolved_dialect = resolve_scope_dialect(dialect)?;
    artifacts.sort_by(|left, right| left.path.cmp(&right.path));
    let decoded = decode_scope_artifacts(artifacts)?;

    let mut schema = SqlSchema::default();
    for (item, parsed) in &decoded {
        if item.source_set != SourceSet::Vendored && !parsed.is_down_migration {
            schema.apply_ddl_ops(&parsed.ddl_ops);
        }
    }

    let extra_hazards = collect_scope_hazards(&decoded, &schema);
    let summary = ScopeSummary {
        kind: "sql-schema".to_owned(),
        fields: vec![
            SummaryField {
                key: "dialect".to_owned(),
                value: resolved_dialect.display_name().to_owned(),
            },
            SummaryField {
                key: "tables".to_owned(),
                value: schema.table_count().to_string(),
            },
        ],
    };

    Ok(ScopeAnalysis {
        extra_hazards,
        summaries: vec![summary],
    })
}

fn collect_scope_hazards(
    decoded: &[(FileScopeArtifact, SqlFileArtifact)],
    schema: &SqlSchema,
) -> Vec<FileHazards> {
    if schema.is_empty() {
        return Vec::new();
    }
    decoded
        .iter()
        .filter_map(|(item, parsed)| {
            let hazard_lines = evaluate_artifact_checks(parsed, schema);
            (!hazard_lines.is_empty()).then(|| FileHazards {
                path: item.path.clone(),
                hazard_lines,
            })
        })
        .collect()
}

type BindingId = (u32, usize);
type ConstrainedCols = BTreeMap<BindingId, BTreeSet<String>>;

struct ResolvedBinding<'a> {
    schema: Option<&'a str>,
    table_name: Option<&'a str>,
    alias: Option<&'a str>,
    table: Option<&'a TableSchema>,
}

impl ResolvedBinding<'_> {
    fn matches_qualifier(&self, qualifier: &str) -> bool {
        if let Some(alias) = self.alias {
            return alias == qualifier;
        }
        self.table_name == Some(qualifier)
    }

    fn matches_schema_table(&self, schema: &str, table: &str) -> bool {
        self.alias.is_none() && self.schema == Some(schema) && self.table_name == Some(table)
    }
}

struct ResolvedScope<'a> {
    depth: u32,
    parent: Option<usize>,
    bindings: Vec<ResolvedBinding<'a>>,
}

impl<'a> ResolvedScope<'a> {
    fn has_unknown_binding(&self) -> bool {
        self.bindings.iter().any(|binding| binding.table.is_none())
    }
}

struct ResolvedColumn<'a> {
    binding_id: BindingId,
    table: &'a TableSchema,
    column: String,
    line_override: Option<u32>,
}

pub(crate) fn evaluate_artifact_checks(
    artifact: &SqlFileArtifact,
    schema: &SqlSchema,
) -> Vec<HazardLine> {
    if schema.is_empty() || artifact.checks.is_empty() {
        return Vec::new();
    }
    let scopes: Vec<ResolvedScope<'_>> = artifact
        .scopes
        .iter()
        .map(|scope| ResolvedScope {
            depth: scope.depth,
            parent: scope.parent.map(|index| index as usize),
            bindings: scope
                .bindings
                .iter()
                .map(|binding| {
                    let table = if binding.resolvable {
                        binding
                            .table_name
                            .as_deref()
                            .and_then(|name| schema.get_table(binding.schema.as_deref(), name))
                    } else {
                        None
                    };
                    ResolvedBinding {
                        schema: binding.schema.as_deref(),
                        table_name: binding.table_name.as_deref(),
                        alias: binding.alias.as_deref(),
                        table,
                    }
                })
                .collect(),
        })
        .collect();

    let mut hits = BTreeSet::new();
    for check in &artifact.checks {
        evaluate_single_check(check, &scopes, schema, &mut hits);
    }
    hits.into_iter()
        .map(|(line, rule)| HazardLine {
            line,
            rule: rule.to_owned(),
        })
        .collect()
}

fn evaluate_single_check(
    check: &SchemaCheck,
    scopes: &[ResolvedScope<'_>],
    schema: &SqlSchema,
    hits: &mut BTreeSet<(u32, &'static str)>,
) {
    match check {
        SchemaCheck::TargetColumn {
            table_schema,
            table_name,
            column,
            line,
        } => evaluate_target_column(
            schema,
            table_schema.as_deref(),
            table_name,
            column,
            *line,
            hits,
        ),
        SchemaCheck::UsingColumn {
            left_scope,
            right_table,
            column,
            using_cols,
            line,
        } => evaluate_using_column(
            scopes,
            schema,
            UsingColumnEval {
                left_scope: *left_scope as usize,
                right_table: right_table.as_ref(),
                column,
                using_cols,
                line: *line,
            },
            hits,
        ),
        SchemaCheck::ColumnRef {
            scope,
            target,
            excluded,
            clause,
            constrained,
            line,
        } => evaluate_column_ref(
            scopes,
            ColumnRefEval {
                scope_idx: *scope as usize,
                target,
                excluded,
                clause: *clause,
                constrained,
                line: *line,
            },
            hits,
        ),
        SchemaCheck::TextNumber {
            scope,
            target,
            excluded,
            line,
        } => evaluate_text_number(scopes, *scope as usize, target, excluded, *line, hits),
    }
}

fn evaluate_target_column(
    schema: &SqlSchema,
    table_schema: Option<&str>,
    table_name: &str,
    column: &str,
    line: u32,
    hits: &mut BTreeSet<(u32, &'static str)>,
) {
    if let Some(table) = schema.get_table(table_schema, table_name)
        && !table.columns.contains_key(column)
    {
        hits.insert((line, "unknown-column"));
    }
}

struct UsingColumnEval<'a> {
    left_scope: usize,
    right_table: Option<&'a (Option<String>, String)>,
    column: &'a str,
    using_cols: &'a [String],
    line: u32,
}

fn evaluate_using_column(
    scopes: &[ResolvedScope<'_>],
    schema: &SqlSchema,
    eval: UsingColumnEval<'_>,
    hits: &mut BTreeSet<(u32, &'static str)>,
) {
    let using_set: BTreeSet<String> = eval.using_cols.iter().cloned().collect();
    if let Some((r_schema, r_name)) = eval.right_table
        && let Some(table) = schema.get_table(r_schema.as_deref(), r_name)
    {
        check_join_column_on_table(table, eval.column, &using_set, eval.line, hits);
    }
    if let Some(scope) = scopes.get(eval.left_scope) {
        check_using_left_scope(scope, eval.column, &using_set, eval.line, hits);
    }
}

fn evaluate_text_number(
    scopes: &[ResolvedScope<'_>],
    scope_idx: usize,
    target: &ColumnRefTarget,
    excluded: &[String],
    line: u32,
    hits: &mut BTreeSet<(u32, &'static str)>,
) {
    if let Some(resolved) = resolve_target(scopes, scope_idx, target, excluded)
        && resolved.table.columns.get(&resolved.column).copied() == Some(ColumnTypeClass::Text)
    {
        hits.insert((line, "text-compared-to-number"));
    }
}

fn check_using_left_scope(
    left_scope: &ResolvedScope<'_>,
    col: &str,
    using_cols: &BTreeSet<String>,
    line: u32,
    hits: &mut BTreeSet<(u32, &'static str)>,
) {
    if left_scope.has_unknown_binding() {
        return;
    }
    let left_matches: Vec<&TableSchema> = left_scope
        .bindings
        .iter()
        .filter_map(|binding| binding.table)
        .filter(|table| table.columns.contains_key(col))
        .collect();
    if left_matches.len() == 1 {
        check_join_column_on_table(left_matches[0], col, using_cols, line, hits);
    } else if left_matches.is_empty()
        && left_scope.bindings.len() == 1
        && left_scope.bindings[0].table.is_some()
    {
        hits.insert((line, "unknown-column"));
    }
}

fn check_join_column_on_table(
    table: &TableSchema,
    col: &str,
    using_cols: &BTreeSet<String>,
    line: u32,
    hits: &mut BTreeSet<(u32, &'static str)>,
) {
    if !table.columns.contains_key(col) {
        hits.insert((line, "unknown-column"));
    } else if !table.is_indexed_column(col)
        && !table.is_covered_by_prefix(col, using_cols)
        && !table.has_leading_indexed_in(using_cols)
    {
        hits.insert((line, "join-on-unindexed-column"));
    }
}

struct ColumnRefEval<'a> {
    scope_idx: usize,
    target: &'a ColumnRefTarget,
    excluded: &'a [String],
    clause: ClauseKind,
    constrained: &'a [ConstrainedRef],
    line: u32,
}

fn evaluate_column_ref(
    scopes: &[ResolvedScope<'_>],
    eval: ColumnRefEval<'_>,
    hits: &mut BTreeSet<(u32, &'static str)>,
) {
    let Some(resolved) = resolve_target(scopes, eval.scope_idx, eval.target, eval.excluded) else {
        return;
    };
    let effective_line = resolved.line_override.unwrap_or(eval.line);
    if !resolved.table.columns.contains_key(&resolved.column) {
        hits.insert((effective_line, "unknown-column"));
        return;
    }
    let constrained_cols = build_constrained_cols(scopes, eval.scope_idx, eval.constrained);
    if is_indexed_in_clause(&resolved, eval.clause, &constrained_cols) {
        return;
    }
    match eval.clause {
        ClauseKind::General => {}
        ClauseKind::Filter => {
            hits.insert((effective_line, "filter-on-unindexed-column"));
        }
        ClauseKind::JoinOn => {
            hits.insert((effective_line, "join-on-unindexed-column"));
            hits.insert((effective_line, "filter-on-unindexed-column"));
        }
        ClauseKind::OrderBy => {
            hits.insert((effective_line, "order-by-unindexed-column"));
        }
    }
}

fn build_constrained_cols(
    scopes: &[ResolvedScope<'_>],
    scope_idx: usize,
    constrained: &[ConstrainedRef],
) -> ConstrainedCols {
    let mut out = ConstrainedCols::new();
    for item in constrained {
        if let Some(resolved) = resolve_target(scopes, scope_idx, &item.target, &item.excluded)
            && resolved.table.columns.contains_key(&resolved.column)
        {
            out.entry(resolved.binding_id)
                .or_default()
                .insert(resolved.column);
        }
    }
    out
}

fn is_indexed_in_clause(
    resolved: &ResolvedColumn<'_>,
    clause: ClauseKind,
    constrained: &ConstrainedCols,
) -> bool {
    if resolved.table.is_indexed_column(&resolved.column) {
        return true;
    }
    let Some(constrained_cols) = constrained.get(&resolved.binding_id) else {
        return false;
    };
    match clause {
        ClauseKind::General => true,
        ClauseKind::OrderBy => resolved
            .table
            .is_covered_by_prefix(&resolved.column, constrained_cols),
        ClauseKind::Filter | ClauseKind::JoinOn => {
            resolved
                .table
                .is_covered_by_prefix(&resolved.column, constrained_cols)
                || resolved.table.has_leading_indexed_in(constrained_cols)
        }
    }
}

fn resolve_target<'a>(
    scopes: &[ResolvedScope<'a>],
    scope_idx: usize,
    target: &ColumnRefTarget,
    excluded: &[String],
) -> Option<ResolvedColumn<'a>> {
    match target {
        ColumnRefTarget::Unqualified(col) => resolve_unqualified(scopes, scope_idx, col, excluded),
        ColumnRefTarget::Qualified2(qual, col) => resolve_qualified2(scopes, scope_idx, qual, col),
        ColumnRefTarget::Qualified3(schema, table, col) => {
            resolve_qualified3(scopes, scope_idx, schema, table, col)
        }
        ColumnRefTarget::Leading(parts) => resolve_leading(scopes, scope_idx, parts, excluded),
    }
}

fn resolve_leading<'a>(
    scopes: &[ResolvedScope<'a>],
    scope_idx: usize,
    parts: &[(String, u32)],
    excluded: &[String],
) -> Option<ResolvedColumn<'a>> {
    if parts.len() >= 3
        && let Some(mut resolved) =
            resolve_qualified3(scopes, scope_idx, &parts[0].0, &parts[1].0, &parts[2].0)
    {
        resolved.line_override = Some(parts[2].1);
        return Some(resolved);
    }
    if parts.len() >= 2
        && let Some(mut resolved) = resolve_qualified2(scopes, scope_idx, &parts[0].0, &parts[1].0)
    {
        resolved.line_override = Some(parts[1].1);
        return Some(resolved);
    }
    let (first_col, first_line) = parts.first()?;
    resolve_unqualified(scopes, scope_idx, first_col, excluded)
        .filter(|resolved| resolved.table.columns.contains_key(&resolved.column))
        .map(|mut resolved| {
            resolved.line_override = Some(*first_line);
            resolved
        })
}

fn resolve_unqualified<'a>(
    scopes: &[ResolvedScope<'a>],
    scope_idx: usize,
    col: &str,
    excluded: &[String],
) -> Option<ResolvedColumn<'a>> {
    if excluded.iter().any(|name| name == col) || crate::schema::scope::is_system_column(col) {
        return None;
    }
    if let Some((binding_id, table)) = resolve_known_column_in_chain(scopes, scope_idx, col) {
        return Some(ResolvedColumn {
            binding_id,
            table,
            column: col.to_owned(),
            line_override: None,
        });
    }
    if matches_binding_qualifier(scopes, scope_idx, col) {
        return None;
    }
    resolve_single_unambiguous_table(scopes, scope_idx).map(|(binding_id, table)| ResolvedColumn {
        binding_id,
        table,
        column: col.to_owned(),
        line_override: None,
    })
}

fn resolve_qualified2<'a>(
    scopes: &[ResolvedScope<'a>],
    scope_idx: usize,
    qual: &str,
    col: &str,
) -> Option<ResolvedColumn<'a>> {
    if crate::schema::scope::is_system_column(col) {
        return None;
    }
    lookup_binding(scopes, scope_idx, |binding| binding.matches_qualifier(qual)).map(
        |(binding_id, table)| ResolvedColumn {
            binding_id,
            table,
            column: col.to_owned(),
            line_override: None,
        },
    )
}

fn resolve_qualified3<'a>(
    scopes: &[ResolvedScope<'a>],
    scope_idx: usize,
    schema: &str,
    table: &str,
    col: &str,
) -> Option<ResolvedColumn<'a>> {
    if crate::schema::scope::is_system_column(col) {
        return None;
    }
    lookup_binding(scopes, scope_idx, |binding| {
        binding.matches_schema_table(schema, table)
    })
    .map(|(binding_id, table_schema)| ResolvedColumn {
        binding_id,
        table: table_schema,
        column: col.to_owned(),
        line_override: None,
    })
}

fn matches_binding_qualifier(scopes: &[ResolvedScope<'_>], start: usize, name: &str) -> bool {
    let mut cursor = Some(start);
    while let Some(idx) = cursor {
        let Some(scope) = scopes.get(idx) else {
            break;
        };
        if scope
            .bindings
            .iter()
            .any(|binding| binding.matches_qualifier(name))
        {
            return true;
        }
        cursor = scope.parent;
    }
    false
}

fn resolve_known_column_in_chain<'a>(
    scopes: &[ResolvedScope<'a>],
    start: usize,
    col: &str,
) -> Option<(BindingId, &'a TableSchema)> {
    let mut cursor = Some(start);
    while let Some(idx) = cursor {
        let scope = scopes.get(idx)?;
        if !scope.bindings.is_empty() {
            if scope.has_unknown_binding() {
                return None;
            }
            let matches: Vec<(BindingId, &'a TableSchema)> = scope
                .bindings
                .iter()
                .enumerate()
                .filter_map(|(binding_idx, binding)| {
                    binding
                        .table
                        .map(|table| ((scope.depth, binding_idx), table))
                })
                .filter(|(_, table)| table.columns.contains_key(col))
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

fn resolve_single_unambiguous_table<'a>(
    scopes: &[ResolvedScope<'a>],
    start: usize,
) -> Option<(BindingId, &'a TableSchema)> {
    let mut known_table: Option<(BindingId, &'a TableSchema)> = None;
    let mut total_tables = 0usize;
    let mut cursor = Some(start);
    while let Some(idx) = cursor {
        let scope = scopes.get(idx)?;
        if scope.has_unknown_binding() {
            return None;
        }
        for (binding_idx, binding) in scope.bindings.iter().enumerate() {
            total_tables += 1;
            if total_tables > 1 {
                return None;
            }
            known_table = binding
                .table
                .map(|table| ((scope.depth, binding_idx), table));
        }
        cursor = scope.parent;
    }
    known_table
}

fn lookup_binding<'a>(
    scopes: &[ResolvedScope<'a>],
    start: usize,
    pred: impl Fn(&ResolvedBinding<'a>) -> bool,
) -> Option<(BindingId, &'a TableSchema)> {
    let mut cursor = Some(start);
    while let Some(idx) = cursor {
        let scope = scopes.get(idx)?;
        let matches: Vec<(usize, &ResolvedBinding<'a>)> = scope
            .bindings
            .iter()
            .enumerate()
            .filter(|(_, binding)| pred(binding))
            .collect();
        if matches.len() == 1 {
            let (binding_idx, binding) = matches[0];
            return binding
                .table
                .map(|table| ((scope.depth, binding_idx), table));
        }
        if matches.len() > 1 {
            return None;
        }
        cursor = scope.parent;
    }
    None
}
