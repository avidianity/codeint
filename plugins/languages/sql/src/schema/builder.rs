use std::collections::BTreeMap;

use sqlparser::ast::{
    AlterColumnOperation, AlterTable, AlterTableOperation, ColumnDef, ColumnOption, CreateIndex,
    CreateTable, DataType, Expr, Ident, IndexColumn, ObjectName, ObjectType, RenameTableNameKind,
    Statement, TableConstraint,
};

use crate::ParsedSqlStatement;
use crate::schema::artifact::{AlterOp, DdlOp};
use crate::schema::types::{
    ColumnTypeClass, IndexSchema, SqlSchema, TableKey, TableSchema, normalize_ident,
    object_name_last_ident, split_object_name,
};

impl SqlSchema {
    pub fn build<'a>(
        files_in_path_order: impl IntoIterator<Item = &'a [ParsedSqlStatement]>,
    ) -> SqlSchema {
        let mut schema = SqlSchema::default();
        for statements in files_in_path_order {
            let ops = extract_ddl_ops(statements);
            schema.apply_ddl_ops(&ops);
        }
        schema
    }

    pub(crate) fn apply_ddl_ops(&mut self, ops: &[DdlOp]) {
        for op in ops {
            apply_ddl_op(self, op);
        }
    }
}

pub(crate) fn extract_ddl_ops(statements: &[ParsedSqlStatement]) -> Vec<DdlOp> {
    let mut ops = Vec::new();
    for parsed in statements {
        extract_statement_ddl(&parsed.statement, &mut ops);
    }
    ops
}

fn extract_statement_ddl(stmt: &Statement, out: &mut Vec<DdlOp>) {
    match stmt {
        Statement::CreateTable(create_table) => {
            if let Some(op) = extract_create_table(create_table) {
                out.push(op);
            }
        }
        Statement::CreateIndex(create_index) => {
            if let Some(op) = extract_create_index(create_index) {
                out.push(op);
            }
        }
        Statement::AlterTable(alter_table) => {
            if let Some(op) = extract_alter_table(alter_table) {
                out.push(op);
            }
        }
        Statement::Drop {
            object_type,
            names,
            table,
            ..
        } => extract_drop(*object_type, names, table.as_ref(), out),
        _ => {}
    }
}

fn extract_create_table(create: &CreateTable) -> Option<DdlOp> {
    if create.temporary || (create.columns.is_empty() && create.query.is_some()) {
        return None;
    }
    let (table_schema, table_name) = split_object_name(&create.name)?;
    let mut table = TableSchema {
        schema: table_schema,
        name: table_name,
        columns: BTreeMap::new(),
        primary_key: Vec::new(),
        indexes: Vec::new(),
    };
    for column in &create.columns {
        apply_column_def(&mut table, column);
    }
    for constraint in &create.constraints {
        apply_table_constraint(&mut table, constraint);
    }
    if table.primary_key.is_empty()
        && let Some(expr) = &create.primary_key
    {
        table.primary_key = extract_pk_expr_columns(expr);
    }
    Some(DdlOp::CreateTable {
        if_not_exists: create.if_not_exists,
        table,
    })
}

fn extract_create_index(create: &CreateIndex) -> Option<DdlOp> {
    let (table_schema, table_name) = split_object_name(&create.table_name)?;
    let columns = extract_index_columns(&create.columns);
    if columns.is_empty() {
        return None;
    }
    let name = create.name.as_ref().and_then(object_name_last_ident);
    Some(DdlOp::CreateIndex {
        table_schema,
        table_name,
        index: IndexSchema { name, columns },
    })
}

fn extract_alter_table(alter: &AlterTable) -> Option<DdlOp> {
    let (table_schema, table_name) = split_object_name(&alter.name)?;
    let mut ops = Vec::new();
    for operation in &alter.operations {
        extract_alter_operation(operation, &mut ops);
    }
    (!ops.is_empty()).then_some(DdlOp::AlterTable {
        table_schema,
        table_name,
        ops,
    })
}

fn extract_column_flags(
    options: &[ColumnOption],
    constraint_names: &[Option<String>],
) -> (bool, Option<Option<String>>) {
    let mut is_pk = false;
    let mut unique_index = None;
    for (idx, option) in options.iter().enumerate() {
        match option {
            ColumnOption::PrimaryKey(_) => is_pk = true,
            ColumnOption::Unique(unique) => {
                let name = unique
                    .name
                    .as_ref()
                    .or(unique.index_name.as_ref())
                    .map(normalize_ident)
                    .or_else(|| constraint_names.get(idx).cloned().flatten());
                unique_index = Some(name);
            }
            _ => {}
        }
    }
    (is_pk, unique_index)
}

fn extract_alter_operation(operation: &AlterTableOperation, out: &mut Vec<AlterOp>) {
    match operation {
        AlterTableOperation::RenameTable { table_name } => {
            let target_name = match table_name {
                RenameTableNameKind::As(name) | RenameTableNameKind::To(name) => name,
            };
            if let Some((new_schema, new_name)) = split_object_name(target_name) {
                out.push(AlterOp::RenameTable {
                    new_schema,
                    new_name,
                });
            }
        }
        AlterTableOperation::AddColumn { column_def, .. } => {
            let opts: Vec<ColumnOption> = column_def
                .options
                .iter()
                .map(|o| o.option.clone())
                .collect();
            let cnames: Vec<Option<String>> = column_def
                .options
                .iter()
                .map(|o| o.name.as_ref().map(normalize_ident))
                .collect();
            let (is_pk, unique_index) = extract_column_flags(&opts, &cnames);
            out.push(AlterOp::AddColumn {
                name: normalize_ident(&column_def.name),
                ty: classify_data_type(&column_def.data_type),
                is_pk,
                unique_index,
            });
        }
        AlterTableOperation::DropColumn { column_names, .. } => {
            for ident in column_names {
                out.push(AlterOp::DropColumn {
                    name: normalize_ident(ident),
                });
            }
        }
        AlterTableOperation::RenameColumn {
            old_column_name,
            new_column_name,
        } => out.push(AlterOp::RenameColumn {
            old: normalize_ident(old_column_name),
            new: normalize_ident(new_column_name),
        }),
        AlterTableOperation::ChangeColumn {
            old_name,
            new_name,
            data_type,
            options,
            ..
        } => {
            let (is_pk, unique_index) = extract_column_flags(options, &[]);
            out.push(AlterOp::ChangeColumn {
                old: normalize_ident(old_name),
                new: normalize_ident(new_name),
                ty: classify_data_type(data_type),
                is_pk,
                unique_index,
            });
        }
        AlterTableOperation::ModifyColumn {
            col_name,
            data_type,
            options,
            ..
        } => {
            let (is_pk, unique_index) = extract_column_flags(options, &[]);
            out.push(AlterOp::ModifyColumn {
                name: normalize_ident(col_name),
                ty: classify_data_type(data_type),
                is_pk,
                unique_index,
            });
        }
        AlterTableOperation::AlterColumn {
            column_name,
            op: AlterColumnOperation::SetDataType { data_type, .. },
        } => out.push(AlterOp::SetColumnType {
            name: normalize_ident(column_name),
            ty: classify_data_type(data_type),
        }),
        _ => extract_constraint_alter(operation, out),
    }
}

fn extract_constraint_alter(operation: &AlterTableOperation, out: &mut Vec<AlterOp>) {
    match operation {
        AlterTableOperation::AddConstraint { constraint, .. } => {
            extract_table_constraint_alter(constraint, out);
        }
        AlterTableOperation::DropConstraint { name, .. }
        | AlterTableOperation::DropIndex { name } => out.push(AlterOp::DropNamedIndex {
            name: normalize_ident(name),
        }),
        AlterTableOperation::DropPrimaryKey { .. } => out.push(AlterOp::DropPrimaryKey),
        _ => {}
    }
}

fn extract_table_constraint_alter(constraint: &TableConstraint, out: &mut Vec<AlterOp>) {
    match constraint {
        TableConstraint::PrimaryKey(pk) => {
            let columns = extract_index_columns(&pk.columns);
            if !columns.is_empty() {
                out.push(AlterOp::SetPrimaryKey { columns });
            }
        }
        TableConstraint::Unique(unique) => {
            let columns = extract_index_columns(&unique.columns);
            if !columns.is_empty() {
                let name = unique
                    .name
                    .as_ref()
                    .or(unique.index_name.as_ref())
                    .map(normalize_ident);
                out.push(AlterOp::AddIndex {
                    index: IndexSchema { name, columns },
                });
            }
        }
        TableConstraint::Index(index) => {
            let columns = extract_index_columns(&index.columns);
            if !columns.is_empty() {
                out.push(AlterOp::AddIndex {
                    index: IndexSchema {
                        name: index.name.as_ref().map(normalize_ident),
                        columns,
                    },
                });
            }
        }
        TableConstraint::FulltextOrSpatial(index) => {
            let columns = extract_index_columns(&index.columns);
            if !columns.is_empty() {
                out.push(AlterOp::AddIndex {
                    index: IndexSchema {
                        name: index.opt_index_name.as_ref().map(normalize_ident),
                        columns,
                    },
                });
            }
        }
        TableConstraint::PrimaryKeyUsingIndex(using_idx) => {
            out.push(AlterOp::PromoteIndexToPk {
                index_name: normalize_ident(&using_idx.index_name),
            });
        }
        _ => {}
    }
}

fn extract_drop(
    object_type: ObjectType,
    names: &[ObjectName],
    table_target: Option<&ObjectName>,
    out: &mut Vec<DdlOp>,
) {
    match object_type {
        ObjectType::Table => extract_drop_tables(names, out),
        ObjectType::Index => extract_drop_indexes(names, table_target, out),
        _ => {}
    }
}

fn extract_drop_tables(names: &[ObjectName], out: &mut Vec<DdlOp>) {
    for (schema, name) in names.iter().filter_map(split_object_name) {
        out.push(DdlOp::DropTable { schema, name });
    }
}

fn extract_drop_indexes(
    names: &[ObjectName],
    table_target: Option<&ObjectName>,
    out: &mut Vec<DdlOp>,
) {
    if let Some(target) = table_target {
        let Some((table_schema, table_name)) = split_object_name(target) else {
            return;
        };
        for index_name in names.iter().filter_map(object_name_last_ident) {
            out.push(DdlOp::DropIndexOnTable {
                table_schema: table_schema.clone(),
                table_name: table_name.clone(),
                index_name,
            });
        }
        return;
    }
    for (index_schema, index_name) in names.iter().filter_map(split_object_name) {
        out.push(DdlOp::DropIndexAcrossTables {
            index_schema,
            index_name,
        });
    }
}

fn apply_ddl_op(schema: &mut SqlSchema, op: &DdlOp) {
    match op {
        DdlOp::CreateTable {
            if_not_exists,
            table,
        } => apply_create_table_op(schema, *if_not_exists, table),
        DdlOp::CreateIndex {
            table_schema,
            table_name,
            index,
        } => apply_create_index_op(schema, table_schema.as_deref(), table_name, index),
        DdlOp::AlterTable {
            table_schema,
            table_name,
            ops,
        } => apply_alter_table_op(schema, table_schema.as_deref(), table_name, ops),
        other => apply_drop_ddl_op(schema, other),
    }
}

fn apply_create_table_op(schema: &mut SqlSchema, if_not_exists: bool, table: &TableSchema) {
    let key = (table.schema.clone(), table.name.clone());
    if !(if_not_exists && schema.tables.contains_key(&key)) {
        schema.tables.insert(key, table.clone());
    }
}

fn apply_create_index_op(
    schema: &mut SqlSchema,
    table_schema: Option<&str>,
    table_name: &str,
    index: &IndexSchema,
) {
    if let Some(key) = schema.resolve_table_key(table_schema, table_name)
        && let Some(table) = schema.tables.get_mut(&key)
    {
        upsert_index(&mut table.indexes, index.clone());
    }
}

fn apply_alter_table_op(
    schema: &mut SqlSchema,
    table_schema: Option<&str>,
    table_name: &str,
    ops: &[AlterOp],
) {
    let Some(mut key) = schema.resolve_table_key(table_schema, table_name) else {
        return;
    };
    for alter_op in ops {
        key = apply_extracted_alter(schema, key, alter_op);
    }
}

fn apply_drop_ddl_op(schema: &mut SqlSchema, op: &DdlOp) {
    match op {
        DdlOp::DropTable { schema: s, name } => {
            if let Some(key) = schema.resolve_table_key(s.as_deref(), name) {
                schema.tables.remove(&key);
            }
        }
        DdlOp::DropIndexOnTable {
            table_schema,
            table_name,
            index_name,
        } => {
            if let Some(key) = schema.resolve_table_key(table_schema.as_deref(), table_name)
                && let Some(table) = schema.tables.get_mut(&key)
            {
                remove_named_index(table, index_name);
            }
        }
        DdlOp::DropIndexAcrossTables {
            index_schema,
            index_name,
        } => {
            for table in schema.tables.values_mut() {
                if index_schema.is_none() || table.schema == *index_schema {
                    remove_named_index(table, index_name);
                }
            }
        }
        _ => {}
    }
}

fn apply_extracted_alter(schema: &mut SqlSchema, key: TableKey, op: &AlterOp) -> TableKey {
    if let AlterOp::RenameTable {
        new_schema,
        new_name,
    } = op
    {
        let Some(mut table) = schema.tables.remove(&key) else {
            return key;
        };
        let effective_schema = new_schema.clone().or(table.schema.clone());
        table.schema = effective_schema.clone();
        table.name = new_name.clone();
        let new_key = (effective_schema, new_name.clone());
        schema.tables.insert(new_key.clone(), table);
        return new_key;
    }
    let Some(table) = schema.tables.get_mut(&key) else {
        return key;
    };
    apply_table_alter_op(table, op);
    key
}

fn apply_column_flags(
    table: &mut TableSchema,
    col: &str,
    ty: ColumnTypeClass,
    is_pk: bool,
    unique_index: &Option<Option<String>>,
) {
    table.columns.insert(col.to_owned(), ty);
    if is_pk {
        table.primary_key = vec![col.to_owned()];
    }
    if let Some(name) = unique_index {
        upsert_index(
            &mut table.indexes,
            IndexSchema {
                name: name.clone(),
                columns: vec![col.to_owned()],
            },
        );
    }
}

fn apply_table_alter_op(table: &mut TableSchema, op: &AlterOp) {
    match op {
        AlterOp::AddColumn {
            name,
            ty,
            is_pk,
            unique_index,
        }
        | AlterOp::ModifyColumn {
            name,
            ty,
            is_pk,
            unique_index,
        } => apply_column_flags(table, name, *ty, *is_pk, unique_index),
        AlterOp::DropColumn { name } => drop_column_from_table(table, name),
        AlterOp::RenameColumn { old, new } => rename_column_in_table(table, old, new),
        AlterOp::ChangeColumn {
            old,
            new,
            ty,
            is_pk,
            unique_index,
        } => {
            rename_column_in_table(table, old, new);
            apply_column_flags(table, new, *ty, *is_pk, unique_index);
        }
        AlterOp::SetColumnType { name, ty } => {
            if table.columns.contains_key(name) {
                table.columns.insert(name.clone(), *ty);
            }
        }
        other => apply_constraint_alter_op(table, other),
    }
}

fn apply_constraint_alter_op(table: &mut TableSchema, op: &AlterOp) {
    match op {
        AlterOp::SetPrimaryKey { columns } => table.primary_key = columns.clone(),
        AlterOp::AddIndex { index } => upsert_index(&mut table.indexes, index.clone()),
        AlterOp::PromoteIndexToPk { index_name } => promote_index_to_pk(table, index_name),
        AlterOp::DropNamedIndex { name } => remove_named_index(table, name),
        AlterOp::DropPrimaryKey => table.primary_key.clear(),
        _ => {}
    }
}

fn apply_column_def(table: &mut TableSchema, column: &ColumnDef) {
    let col_name = normalize_ident(&column.name);
    table
        .columns
        .insert(col_name.clone(), classify_data_type(&column.data_type));
    for opt_def in &column.options {
        apply_column_option(
            table,
            &col_name,
            opt_def.name.as_ref().map(normalize_ident),
            &opt_def.option,
        );
    }
}

fn apply_column_option(
    table: &mut TableSchema,
    col_name: &str,
    constraint_name: Option<String>,
    option: &ColumnOption,
) {
    match option {
        ColumnOption::PrimaryKey(_) => {
            table.primary_key = vec![col_name.to_owned()];
        }
        ColumnOption::Unique(unique) => {
            let name = unique
                .name
                .as_ref()
                .or(unique.index_name.as_ref())
                .map(normalize_ident)
                .or(constraint_name);
            upsert_index(
                &mut table.indexes,
                IndexSchema {
                    name,
                    columns: vec![col_name.to_owned()],
                },
            );
        }
        _ => {}
    }
}

fn apply_table_constraint(table: &mut TableSchema, constraint: &TableConstraint) {
    match constraint {
        TableConstraint::PrimaryKey(pk) => set_primary_key_from_columns(table, &pk.columns),
        TableConstraint::Unique(unique) => {
            let name = unique.name.as_ref().or(unique.index_name.as_ref());
            add_index_from_columns(table, name, &unique.columns);
        }
        TableConstraint::Index(index) => {
            add_index_from_columns(table, index.name.as_ref(), &index.columns);
        }
        TableConstraint::FulltextOrSpatial(index) => {
            add_index_from_columns(table, index.opt_index_name.as_ref(), &index.columns);
        }
        TableConstraint::PrimaryKeyUsingIndex(using_idx) => {
            promote_index_to_pk(table, &normalize_ident(&using_idx.index_name));
        }
        _ => {}
    }
}

fn set_primary_key_from_columns(table: &mut TableSchema, columns: &[IndexColumn]) {
    let cols = extract_index_columns(columns);
    if !cols.is_empty() {
        table.primary_key = cols;
    }
}

fn add_index_from_columns(table: &mut TableSchema, name: Option<&Ident>, columns: &[IndexColumn]) {
    let cols = extract_index_columns(columns);
    if !cols.is_empty() {
        upsert_index(
            &mut table.indexes,
            IndexSchema {
                name: name.map(normalize_ident),
                columns: cols,
            },
        );
    }
}

fn promote_index_to_pk(table: &mut TableSchema, idx_name: &str) {
    if let Some(found) = table
        .indexes
        .iter()
        .find(|idx| idx.name.as_deref() == Some(idx_name))
    {
        table.primary_key = found.columns.clone();
    }
}

fn remove_named_index(table: &mut TableSchema, target: &str) {
    table
        .indexes
        .retain(|idx| idx.name.as_deref() != Some(target));
}

fn drop_column_from_table(table: &mut TableSchema, col: &str) {
    table.columns.remove(col);
    table.primary_key.retain(|c| c != col);
    for idx in &mut table.indexes {
        idx.columns.retain(|c| c != col);
    }
    table.indexes.retain(|idx| !idx.columns.is_empty());
}

fn rename_column_in_table(table: &mut TableSchema, old: &str, new: &str) {
    if let Some(ty) = table.columns.remove(old) {
        table.columns.insert(new.to_owned(), ty);
    }
    for col in &mut table.primary_key {
        if col == old {
            *col = new.to_owned();
        }
    }
    for idx in &mut table.indexes {
        for col in &mut idx.columns {
            if col == old {
                *col = new.to_owned();
            }
        }
    }
}

fn upsert_index(indexes: &mut Vec<IndexSchema>, entry: IndexSchema) {
    if let Some(ref name) = entry.name
        && let Some(existing) = indexes
            .iter_mut()
            .find(|idx| idx.name.as_deref() == Some(name.as_str()))
    {
        *existing = entry;
        return;
    }
    indexes.push(entry);
}

fn extract_index_columns(columns: &[IndexColumn]) -> Vec<String> {
    let mut out = Vec::with_capacity(columns.len());
    for col in columns {
        let Some(name) = extract_expr_column_ident(&col.column.expr) else {
            break;
        };
        out.push(name);
    }
    out
}

fn extract_pk_expr_columns(expr: &Expr) -> Vec<String> {
    match expr {
        Expr::Tuple(items) => items.iter().filter_map(extract_expr_column_ident).collect(),
        _ => extract_expr_column_ident(expr).into_iter().collect(),
    }
}

fn extract_expr_column_ident(mut expr: &Expr) -> Option<String> {
    loop {
        match expr {
            Expr::Nested(inner) | Expr::Collate { expr: inner, .. } => expr = inner,
            Expr::Identifier(ident) => return Some(normalize_ident(ident)),
            Expr::CompoundIdentifier(idents) => return idents.last().map(normalize_ident),
            _ => return None,
        }
    }
}

pub(crate) fn classify_data_type(data_type: &DataType) -> ColumnTypeClass {
    match data_type {
        DataType::Nullable(inner)
        | DataType::LowCardinality(inner)
        | DataType::Collate(inner, _)
        | DataType::Zerofill(inner) => classify_data_type(inner),
        DataType::Character(_)
        | DataType::Char(_)
        | DataType::CharacterVarying(_)
        | DataType::CharVarying(_)
        | DataType::Varchar(_)
        | DataType::Nvarchar(_)
        | DataType::CharacterLargeObject(_)
        | DataType::CharLargeObject(_)
        | DataType::Clob(_)
        | DataType::Text
        | DataType::TinyText
        | DataType::MediumText
        | DataType::LongText
        | DataType::String(_)
        | DataType::FixedString(_) => ColumnTypeClass::Text,
        DataType::Bool | DataType::Boolean => ColumnTypeClass::Boolean,
        DataType::Date
        | DataType::Date32
        | DataType::Time(_, _)
        | DataType::Datetime(_)
        | DataType::Datetime64(_, _)
        | DataType::Timestamp(_, _)
        | DataType::TimestampNtz(_)
        | DataType::Interval { .. } => ColumnTypeClass::Temporal,
        DataType::Binary(_)
        | DataType::Varbinary(_)
        | DataType::Blob(_)
        | DataType::TinyBlob
        | DataType::MediumBlob
        | DataType::LongBlob
        | DataType::Bytes(_)
        | DataType::Bytea => ColumnTypeClass::Binary,
        DataType::Custom(name, _) => classify_custom_type(name),
        other if is_numeric_data_type(other) => ColumnTypeClass::Numeric,
        _ => ColumnTypeClass::Other,
    }
}

fn is_numeric_data_type(data_type: &DataType) -> bool {
    matches!(
        data_type,
        DataType::Numeric(_)
            | DataType::Decimal(_)
            | DataType::DecimalUnsigned(_)
            | DataType::BigNumeric(_)
            | DataType::BigDecimal(_)
            | DataType::Dec(_)
            | DataType::DecUnsigned(_)
            | DataType::Float(_)
            | DataType::FloatUnsigned(_)
            | DataType::Float4
            | DataType::Float32
            | DataType::Float64
            | DataType::Float8
            | DataType::Real
            | DataType::RealUnsigned
            | DataType::Double(_)
            | DataType::DoubleUnsigned(_)
            | DataType::DoublePrecision
            | DataType::DoublePrecisionUnsigned
            | DataType::TinyInt(_)
            | DataType::TinyIntUnsigned(_)
            | DataType::UTinyInt
            | DataType::Int2(_)
            | DataType::Int2Unsigned(_)
            | DataType::SmallInt(_)
            | DataType::SmallIntUnsigned(_)
            | DataType::USmallInt
            | DataType::MediumInt(_)
            | DataType::MediumIntUnsigned(_)
            | DataType::Int(_)
            | DataType::IntUnsigned(_)
            | DataType::Int4(_)
            | DataType::Int4Unsigned(_)
            | DataType::Integer(_)
            | DataType::IntegerUnsigned(_)
            | DataType::Int8(_)
            | DataType::Int8Unsigned(_)
            | DataType::Int16
            | DataType::Int32
            | DataType::Int64
            | DataType::Int128
            | DataType::Int256
            | DataType::HugeInt
            | DataType::UHugeInt
            | DataType::UInt8
            | DataType::UInt16
            | DataType::UInt32
            | DataType::UInt64
            | DataType::UInt128
            | DataType::UInt256
            | DataType::BigInt(_)
            | DataType::BigIntUnsigned(_)
            | DataType::UBigInt
            | DataType::Signed
            | DataType::SignedInteger
            | DataType::Unsigned
            | DataType::UnsignedInteger
    )
}

fn classify_custom_type(name: &ObjectName) -> ColumnTypeClass {
    let Some(last) = object_name_last_ident(name) else {
        return ColumnTypeClass::Other;
    };
    match last.as_str() {
        "citext" | "bpchar" | "varchar2" | "nvarchar2" | "nchar" | "ntext" | "tinytext"
        | "mediumtext" | "longtext" | "text" | "string" | "varchar" | "char" | "character"
        | "clob" => ColumnTypeClass::Text,
        "serial" | "bigserial" | "smallserial" | "serial2" | "serial4" | "serial8" | "money"
        | "smallmoney" | "number" | "int" | "integer" | "bigint" | "smallint" | "tinyint"
        | "mediumint" | "decimal" | "numeric" | "float" | "double" | "real" => {
            ColumnTypeClass::Numeric
        }
        "bool" | "boolean" => ColumnTypeClass::Boolean,
        "date" | "time" | "timetz" | "timestamp" | "timestamptz" | "datetime" | "smalldatetime"
        | "interval" | "year" => ColumnTypeClass::Temporal,
        "blob" | "bytea" | "binary" | "varbinary" | "tinyblob" | "mediumblob" | "longblob" => {
            ColumnTypeClass::Binary
        }
        _ => ColumnTypeClass::Other,
    }
}
