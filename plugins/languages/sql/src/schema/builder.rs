use std::collections::BTreeMap;

use sqlparser::ast::{
    AlterColumnOperation, AlterTable, AlterTableOperation, ColumnDef, ColumnOption, CreateIndex,
    CreateTable, DataType, Expr, Ident, IndexColumn, ObjectName, ObjectType, RenameTableNameKind,
    Statement, TableConstraint,
};

use crate::ParsedSqlStatement;
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
            for parsed in statements {
                apply_statement(&mut schema, &parsed.statement);
            }
        }
        schema
    }
}

fn apply_statement(schema: &mut SqlSchema, stmt: &Statement) {
    match stmt {
        Statement::CreateTable(create_table) => apply_create_table(schema, create_table),
        Statement::CreateIndex(create_index) => apply_create_index(schema, create_index),
        Statement::AlterTable(alter_table) => apply_alter_table(schema, alter_table),
        Statement::Drop {
            object_type,
            names,
            table,
            ..
        } => apply_drop(schema, *object_type, names, table.as_ref()),
        _ => {}
    }
}

fn apply_create_table(schema: &mut SqlSchema, create: &CreateTable) {
    if create.columns.is_empty() && create.query.is_some() {
        return;
    }
    let Some((table_schema, table_name)) = split_object_name(&create.name) else {
        return;
    };
    let mut table = TableSchema {
        schema: table_schema.clone(),
        name: table_name.clone(),
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
    schema.tables.insert((table_schema, table_name), table);
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

fn apply_create_index(schema: &mut SqlSchema, create: &CreateIndex) {
    let Some((table_schema, table_name)) = split_object_name(&create.table_name) else {
        return;
    };
    let Some(key) = schema.resolve_table_key(table_schema.as_deref(), &table_name) else {
        return;
    };
    let columns = extract_index_columns(&create.columns);
    if columns.is_empty() {
        return;
    }
    let name = create.name.as_ref().and_then(object_name_last_ident);
    if let Some(table) = schema.tables.get_mut(&key) {
        upsert_index(&mut table.indexes, IndexSchema { name, columns });
    }
}

fn apply_alter_table(schema: &mut SqlSchema, alter: &AlterTable) {
    let Some((table_schema, table_name)) = split_object_name(&alter.name) else {
        return;
    };
    let Some(mut key) = schema.resolve_table_key(table_schema.as_deref(), &table_name) else {
        return;
    };
    for operation in &alter.operations {
        key = apply_alter_operation(schema, key, operation);
    }
}

fn apply_alter_operation(
    schema: &mut SqlSchema,
    key: TableKey,
    operation: &AlterTableOperation,
) -> TableKey {
    if let AlterTableOperation::RenameTable { table_name } = operation {
        return rename_table_entry(schema, key, table_name);
    }
    let Some(table) = schema.tables.get_mut(&key) else {
        return key;
    };
    apply_table_mutation(table, operation);
    key
}

fn rename_table_entry(
    schema: &mut SqlSchema,
    old_key: TableKey,
    target: &RenameTableNameKind,
) -> TableKey {
    let target_name = match target {
        RenameTableNameKind::As(name) | RenameTableNameKind::To(name) => name,
    };
    let Some((new_schema, new_name)) = split_object_name(target_name) else {
        return old_key;
    };
    let Some(mut table) = schema.tables.remove(&old_key) else {
        return old_key;
    };
    let effective_schema = new_schema.or(table.schema.clone());
    table.schema = effective_schema.clone();
    table.name = new_name.clone();
    let new_key = (effective_schema, new_name);
    schema.tables.insert(new_key.clone(), table);
    new_key
}

fn apply_table_mutation(table: &mut TableSchema, operation: &AlterTableOperation) {
    match operation {
        AlterTableOperation::AddColumn { column_def, .. } => apply_column_def(table, column_def),
        AlterTableOperation::DropColumn { column_names, .. } => {
            for ident in column_names {
                drop_column_from_table(table, &normalize_ident(ident));
            }
        }
        AlterTableOperation::RenameColumn {
            old_column_name,
            new_column_name,
        } => rename_column_in_table(
            table,
            &normalize_ident(old_column_name),
            &normalize_ident(new_column_name),
        ),
        AlterTableOperation::ChangeColumn { .. }
        | AlterTableOperation::ModifyColumn { .. }
        | AlterTableOperation::AlterColumn { .. } => apply_column_type_alter(table, operation),
        _ => apply_constraint_alter(table, operation),
    }
}

fn apply_column_type_alter(table: &mut TableSchema, operation: &AlterTableOperation) {
    match operation {
        AlterTableOperation::ChangeColumn {
            old_name,
            new_name,
            data_type,
            options,
            ..
        } => {
            let new = normalize_ident(new_name);
            rename_column_in_table(table, &normalize_ident(old_name), &new);
            update_column_type_and_options(table, &new, data_type, options);
        }
        AlterTableOperation::ModifyColumn {
            col_name,
            data_type,
            options,
            ..
        } => {
            update_column_type_and_options(table, &normalize_ident(col_name), data_type, options);
        }
        AlterTableOperation::AlterColumn {
            column_name,
            op: AlterColumnOperation::SetDataType { data_type, .. },
        } => {
            let col = normalize_ident(column_name);
            if table.columns.contains_key(&col) {
                table.columns.insert(col, classify_data_type(data_type));
            }
        }
        _ => {}
    }
}

fn update_column_type_and_options(
    table: &mut TableSchema,
    col: &str,
    data_type: &DataType,
    options: &[ColumnOption],
) {
    table
        .columns
        .insert(col.to_owned(), classify_data_type(data_type));
    for option in options {
        apply_column_option(table, col, None, option);
    }
}

fn apply_constraint_alter(table: &mut TableSchema, operation: &AlterTableOperation) {
    match operation {
        AlterTableOperation::AddConstraint { constraint, .. } => {
            apply_table_constraint(table, constraint);
        }
        AlterTableOperation::DropConstraint { name, .. }
        | AlterTableOperation::DropIndex { name } => {
            remove_named_index(table, &normalize_ident(name));
        }
        AlterTableOperation::DropPrimaryKey { .. } => table.primary_key.clear(),
        _ => {}
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

fn apply_drop(
    schema: &mut SqlSchema,
    object_type: ObjectType,
    names: &[ObjectName],
    table_target: Option<&ObjectName>,
) {
    match object_type {
        ObjectType::Table => drop_tables(schema, names),
        ObjectType::Index => match table_target {
            Some(target) => drop_indexes_on_table(schema, names, target),
            None => drop_indexes_across_tables(schema, names),
        },
        _ => {}
    }
}

fn drop_tables(schema: &mut SqlSchema, names: &[ObjectName]) {
    for obj_name in names {
        if let Some((table_schema, table_name)) = split_object_name(obj_name)
            && let Some(key) = schema.resolve_table_key(table_schema.as_deref(), &table_name)
        {
            schema.tables.remove(&key);
        }
    }
}

fn drop_indexes_on_table(schema: &mut SqlSchema, names: &[ObjectName], target_obj: &ObjectName) {
    let Some((table_schema, table_name)) = split_object_name(target_obj) else {
        return;
    };
    let Some(key) = schema.resolve_table_key(table_schema.as_deref(), &table_name) else {
        return;
    };
    let Some(table) = schema.tables.get_mut(&key) else {
        return;
    };
    for idx_name in names.iter().filter_map(object_name_last_ident) {
        remove_named_index(table, &idx_name);
    }
}

fn drop_indexes_across_tables(schema: &mut SqlSchema, names: &[ObjectName]) {
    for (idx_schema, idx_name) in names.iter().filter_map(split_object_name) {
        for table in schema.tables.values_mut() {
            if idx_schema.is_none() || table.schema == idx_schema {
                remove_named_index(table, &idx_name);
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

fn extract_expr_column_ident(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Nested(inner) | Expr::Collate { expr: inner, .. } => extract_expr_column_ident(inner),
        Expr::Identifier(ident) => Some(normalize_ident(ident)),
        Expr::CompoundIdentifier(idents) => idents.last().map(normalize_ident),
        _ => None,
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
