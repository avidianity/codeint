use std::collections::BTreeMap;

use crate::schema::artifact::{
    AlterOp, ClauseKind, ColumnAlterOp, ColumnRefTarget, ConstrainedRef, DdlOp, SchemaCheck,
    ScopeDef, SqlFileArtifact, TableAlterOp, TableBindingDef,
};
use crate::schema::types::{ColumnTypeClass, IndexSchema, TableSchema};

const MAGIC: [u8; 4] = *b"SQL1";

const COL_TYPES: [ColumnTypeClass; 6] = [
    ColumnTypeClass::Text,
    ColumnTypeClass::Numeric,
    ColumnTypeClass::Boolean,
    ColumnTypeClass::Temporal,
    ColumnTypeClass::Binary,
    ColumnTypeClass::Other,
];

pub(crate) fn encode_artifact(artifact: &SqlFileArtifact) -> Vec<u8> {
    let mut enc = Encoder::new();
    enc.bytes(&MAGIC);
    enc.bool(artifact.is_down_migration);
    enc.vec(&artifact.ddl_ops, encode_ddl_op);
    enc.vec(&artifact.scopes, encode_scope_def);
    enc.vec(&artifact.checks, encode_schema_check);
    enc.out
}

pub(crate) fn decode_artifact(bytes: &[u8]) -> Option<SqlFileArtifact> {
    let mut dec = Decoder::new(bytes);
    if dec.fixed_4()? != MAGIC {
        return None;
    }
    let is_down_migration = dec.bool()?;
    let ddl_ops = dec.vec(decode_ddl_op)?;
    let scopes = decode_scopes(&mut dec)?;
    let checks = dec.vec(decode_schema_check)?;
    if dec.pos != dec.input.len() {
        return None;
    }
    Some(SqlFileArtifact {
        is_down_migration,
        ddl_ops,
        scopes,
        checks,
    })
}

struct Encoder {
    out: Vec<u8>,
}

impl Encoder {
    fn new() -> Encoder {
        Encoder { out: Vec::new() }
    }

    fn bytes(&mut self, data: &[u8]) {
        self.out.extend_from_slice(data);
    }

    fn u8(&mut self, value: u8) {
        self.out.push(value);
    }

    fn bool(&mut self, value: bool) {
        self.u8(u8::from(value));
    }

    fn u32(&mut self, value: u32) {
        self.out.extend_from_slice(&value.to_le_bytes());
    }

    fn str(&mut self, value: &str) {
        let len = u32::try_from(value.len()).unwrap_or(u32::MAX);
        self.u32(len);
        self.bytes(&value.as_bytes()[..len as usize]);
    }

    fn opt<T>(&mut self, value: &Option<T>, write: fn(&mut Encoder, &T)) {
        match value {
            Some(inner) => {
                self.u8(1);
                write(self, inner);
            }
            None => self.u8(0),
        }
    }

    fn opt_str(&mut self, value: &Option<String>) {
        self.opt(value, |enc, s| enc.str(s));
    }

    fn vec<T>(&mut self, items: &[T], write: fn(&mut Encoder, &T)) {
        let len = u32::try_from(items.len()).unwrap_or(u32::MAX);
        self.u32(len);
        for item in items.iter().take(len as usize) {
            write(self, item);
        }
    }

    fn vec_str(&mut self, items: &[String]) {
        self.vec(items, |enc, s| enc.str(s));
    }
}

struct Decoder<'a> {
    input: &'a [u8],
    pos: usize,
}

impl<'a> Decoder<'a> {
    fn new(input: &'a [u8]) -> Decoder<'a> {
        Decoder { input, pos: 0 }
    }

    fn take(&mut self, count: usize) -> Option<&'a [u8]> {
        let end = self.pos.checked_add(count)?;
        let slice = self.input.get(self.pos..end)?;
        self.pos = end;
        Some(slice)
    }

    fn fixed_4(&mut self) -> Option<[u8; 4]> {
        self.take(4)?.try_into().ok()
    }

    fn u8(&mut self) -> Option<u8> {
        self.take(1).map(|slice| slice[0])
    }

    fn bool(&mut self) -> Option<bool> {
        match self.u8()? {
            0 => Some(false),
            1 => Some(true),
            _ => None,
        }
    }

    fn u32(&mut self) -> Option<u32> {
        self.fixed_4().map(u32::from_le_bytes)
    }

    fn str(&mut self) -> Option<String> {
        let len = self.u32()? as usize;
        let bytes = self.take(len)?;
        std::str::from_utf8(bytes).ok().map(str::to_owned)
    }

    fn opt<T>(&mut self, mut read: impl FnMut(&mut Decoder<'a>) -> Option<T>) -> Option<Option<T>> {
        match self.u8()? {
            0 => Some(None),
            1 => read(self).map(Some),
            _ => None,
        }
    }

    fn opt_str(&mut self) -> Option<Option<String>> {
        self.opt(Decoder::str)
    }

    fn vec<T>(&mut self, mut read: impl FnMut(&mut Decoder<'a>) -> Option<T>) -> Option<Vec<T>> {
        let len = self.u32()? as usize;
        let mut out = Vec::with_capacity(len.min(1024));
        for _ in 0..len {
            out.push(read(self)?);
        }
        Some(out)
    }

    fn vec_str(&mut self) -> Option<Vec<String>> {
        self.vec(Decoder::str)
    }

    fn dispatch<T>(&mut self, table: &[for<'b> fn(&mut Decoder<'b>) -> Option<T>]) -> Option<T> {
        let decode = *table.get(self.u8()? as usize)?;
        decode(self)
    }
}

fn encode_col_type(enc: &mut Encoder, ty: &ColumnTypeClass) {
    let tag = match ty {
        ColumnTypeClass::Text => 0,
        ColumnTypeClass::Numeric => 1,
        ColumnTypeClass::Boolean => 2,
        ColumnTypeClass::Temporal => 3,
        ColumnTypeClass::Binary => 4,
        ColumnTypeClass::Other => 5,
    };
    enc.u8(tag);
}

fn decode_col_type(dec: &mut Decoder<'_>) -> Option<ColumnTypeClass> {
    COL_TYPES.get(dec.u8()? as usize).copied()
}

fn encode_col_spec(
    enc: &mut Encoder,
    ty: &ColumnTypeClass,
    is_pk: bool,
    unique_index: &Option<Option<String>>,
) {
    encode_col_type(enc, ty);
    enc.bool(is_pk);
    enc.opt(unique_index, Encoder::opt_str);
}

type DecodedColSpec = (ColumnTypeClass, bool, Option<Option<String>>);

fn decode_col_spec(dec: &mut Decoder<'_>) -> Option<DecodedColSpec> {
    let ty = decode_col_type(dec)?;
    let is_pk = dec.bool()?;
    let unique_index = dec.opt(Decoder::opt_str)?;
    Some((ty, is_pk, unique_index))
}

fn encode_index(enc: &mut Encoder, index: &IndexSchema) {
    enc.opt_str(&index.name);
    enc.vec_str(&index.columns);
}

fn decode_index(dec: &mut Decoder<'_>) -> Option<IndexSchema> {
    Some(IndexSchema {
        name: dec.opt_str()?,
        columns: dec.vec_str()?,
    })
}

fn encode_table(enc: &mut Encoder, table: &TableSchema) {
    enc.opt_str(&table.schema);
    enc.str(&table.name);
    let len = u32::try_from(table.columns.len()).unwrap_or(u32::MAX);
    enc.u32(len);
    for (name, ty) in table.columns.iter().take(len as usize) {
        enc.str(name);
        encode_col_type(enc, ty);
    }
    enc.vec_str(&table.primary_key);
    enc.vec(&table.indexes, encode_index);
}

fn decode_table(dec: &mut Decoder<'_>) -> Option<TableSchema> {
    let schema = dec.opt_str()?;
    let name = dec.str()?;
    let col_count = dec.u32()? as usize;
    let mut columns = BTreeMap::new();
    for _ in 0..col_count {
        let col_name = dec.str()?;
        let ty = decode_col_type(dec)?;
        columns.insert(col_name, ty);
    }
    let primary_key = dec.vec_str()?;
    let indexes = dec.vec(decode_index)?;
    Some(TableSchema {
        schema,
        name,
        columns,
        primary_key,
        indexes,
    })
}

fn encode_ddl_op(enc: &mut Encoder, op: &DdlOp) {
    match op {
        DdlOp::CreateTable {
            if_not_exists,
            table,
        } => {
            enc.u8(0);
            enc.bool(*if_not_exists);
            encode_table(enc, table);
        }
        DdlOp::CreateIndex {
            table_schema,
            table_name,
            index,
        } => {
            enc.u8(1);
            enc.opt_str(table_schema);
            enc.str(table_name);
            encode_index(enc, index);
        }
        DdlOp::AlterTable {
            table_schema,
            table_name,
            ops,
        } => {
            enc.u8(2);
            enc.opt_str(table_schema);
            enc.str(table_name);
            enc.vec(ops, encode_alter_op);
        }
        DdlOp::DropTable { schema, name } => {
            enc.u8(3);
            enc.opt_str(schema);
            enc.str(name);
        }
        DdlOp::DropIndexOnTable {
            table_schema,
            table_name,
            index_name,
        } => {
            enc.u8(4);
            enc.opt_str(table_schema);
            enc.str(table_name);
            enc.str(index_name);
        }
        DdlOp::DropIndexAcrossTables {
            index_schema,
            index_name,
        } => {
            enc.u8(5);
            enc.opt_str(index_schema);
            enc.str(index_name);
        }
    }
}

fn decode_ddl_op(dec: &mut Decoder<'_>) -> Option<DdlOp> {
    dec.dispatch(&[
        decode_create_table_op,
        decode_create_index_op,
        decode_alter_table_op,
        decode_drop_table_op,
        decode_drop_index_on_table_op,
        decode_drop_index_across_tables_op,
    ])
}

fn decode_qualified_name(dec: &mut Decoder<'_>) -> Option<(Option<String>, String)> {
    Some((dec.opt_str()?, dec.str()?))
}

fn decode_create_table_op(dec: &mut Decoder<'_>) -> Option<DdlOp> {
    let if_not_exists = dec.bool()?;
    let table = decode_table(dec)?;
    Some(DdlOp::CreateTable {
        if_not_exists,
        table,
    })
}

fn decode_create_index_op(dec: &mut Decoder<'_>) -> Option<DdlOp> {
    let (table_schema, table_name) = decode_qualified_name(dec)?;
    let index = decode_index(dec)?;
    Some(DdlOp::CreateIndex {
        table_schema,
        table_name,
        index,
    })
}

fn decode_alter_table_op(dec: &mut Decoder<'_>) -> Option<DdlOp> {
    let (table_schema, table_name) = decode_qualified_name(dec)?;
    let ops = dec.vec(decode_alter_op)?;
    Some(DdlOp::AlterTable {
        table_schema,
        table_name,
        ops,
    })
}

fn decode_drop_table_op(dec: &mut Decoder<'_>) -> Option<DdlOp> {
    let (schema, name) = decode_qualified_name(dec)?;
    Some(DdlOp::DropTable { schema, name })
}

fn decode_drop_index_on_table_op(dec: &mut Decoder<'_>) -> Option<DdlOp> {
    let (table_schema, table_name) = decode_qualified_name(dec)?;
    let index_name = dec.str()?;
    Some(DdlOp::DropIndexOnTable {
        table_schema,
        table_name,
        index_name,
    })
}

fn decode_drop_index_across_tables_op(dec: &mut Decoder<'_>) -> Option<DdlOp> {
    let (index_schema, index_name) = decode_qualified_name(dec)?;
    Some(DdlOp::DropIndexAcrossTables {
        index_schema,
        index_name,
    })
}

fn encode_alter_op(enc: &mut Encoder, op: &AlterOp) {
    match op {
        AlterOp::Column(col_op) => encode_column_alter_op(enc, col_op),
        AlterOp::Table(table_op) => encode_table_alter_op(enc, table_op),
    }
}

fn encode_column_alter_op(enc: &mut Encoder, op: &ColumnAlterOp) {
    match op {
        ColumnAlterOp::AddColumn {
            name,
            ty,
            is_pk,
            unique_index,
        } => {
            enc.u8(0);
            enc.str(name);
            encode_col_spec(enc, ty, *is_pk, unique_index);
        }
        ColumnAlterOp::DropColumn { name } => {
            enc.u8(1);
            enc.str(name);
        }
        ColumnAlterOp::RenameColumn { old, new } => {
            enc.u8(2);
            enc.str(old);
            enc.str(new);
        }
        ColumnAlterOp::ChangeColumn {
            old,
            new,
            ty,
            is_pk,
            unique_index,
        } => {
            enc.u8(3);
            enc.str(old);
            enc.str(new);
            encode_col_spec(enc, ty, *is_pk, unique_index);
        }
        ColumnAlterOp::ModifyColumn {
            name,
            ty,
            is_pk,
            unique_index,
        } => {
            enc.u8(4);
            enc.str(name);
            encode_col_spec(enc, ty, *is_pk, unique_index);
        }
        ColumnAlterOp::SetColumnType { name, ty } => {
            enc.u8(5);
            enc.str(name);
            encode_col_type(enc, ty);
        }
    }
}

fn encode_table_alter_op(enc: &mut Encoder, op: &TableAlterOp) {
    match op {
        TableAlterOp::SetPrimaryKey { columns } => {
            enc.u8(6);
            enc.vec_str(columns);
        }
        TableAlterOp::AddIndex { index } => {
            enc.u8(7);
            encode_index(enc, index);
        }
        TableAlterOp::PromoteIndexToPk { index_name } => {
            enc.u8(8);
            enc.str(index_name);
        }
        TableAlterOp::DropNamedIndex { name } => {
            enc.u8(9);
            enc.str(name);
        }
        TableAlterOp::DropPrimaryKey => enc.u8(10),
        TableAlterOp::RenameTable {
            new_schema,
            new_name,
        } => {
            enc.u8(11);
            enc.opt_str(new_schema);
            enc.str(new_name);
        }
    }
}

fn decode_alter_op(dec: &mut Decoder<'_>) -> Option<AlterOp> {
    match dec.u8()? {
        tag @ 0..=2 => decode_column_alter_basic(tag, dec).map(AlterOp::Column),
        tag @ 3..=5 => decode_column_alter_typed(tag, dec).map(AlterOp::Column),
        tag @ 6..=8 => decode_table_alter_add(tag, dec).map(AlterOp::Table),
        tag @ 9..=11 => decode_table_alter_drop_or_rename(tag, dec).map(AlterOp::Table),
        _ => None,
    }
}

fn decode_column_alter_basic(tag: u8, dec: &mut Decoder<'_>) -> Option<ColumnAlterOp> {
    match tag {
        0 => {
            let name = dec.str()?;
            let (ty, is_pk, unique_index) = decode_col_spec(dec)?;
            Some(ColumnAlterOp::AddColumn {
                name,
                ty,
                is_pk,
                unique_index,
            })
        }
        1 => Some(ColumnAlterOp::DropColumn { name: dec.str()? }),
        2 => Some(ColumnAlterOp::RenameColumn {
            old: dec.str()?,
            new: dec.str()?,
        }),
        _ => None,
    }
}

fn decode_column_alter_typed(tag: u8, dec: &mut Decoder<'_>) -> Option<ColumnAlterOp> {
    match tag {
        3 => decode_change_column_op(dec),
        4 => {
            let name = dec.str()?;
            let (ty, is_pk, unique_index) = decode_col_spec(dec)?;
            Some(ColumnAlterOp::ModifyColumn {
                name,
                ty,
                is_pk,
                unique_index,
            })
        }
        5 => Some(ColumnAlterOp::SetColumnType {
            name: dec.str()?,
            ty: decode_col_type(dec)?,
        }),
        _ => None,
    }
}

fn decode_change_column_op(dec: &mut Decoder<'_>) -> Option<ColumnAlterOp> {
    let old = dec.str()?;
    let new = dec.str()?;
    let (ty, is_pk, unique_index) = decode_col_spec(dec)?;
    Some(ColumnAlterOp::ChangeColumn {
        old,
        new,
        ty,
        is_pk,
        unique_index,
    })
}

fn decode_table_alter_add(tag: u8, dec: &mut Decoder<'_>) -> Option<TableAlterOp> {
    match tag {
        6 => Some(TableAlterOp::SetPrimaryKey {
            columns: dec.vec_str()?,
        }),
        7 => Some(TableAlterOp::AddIndex {
            index: decode_index(dec)?,
        }),
        8 => Some(TableAlterOp::PromoteIndexToPk {
            index_name: dec.str()?,
        }),
        _ => None,
    }
}

fn decode_table_alter_drop_or_rename(tag: u8, dec: &mut Decoder<'_>) -> Option<TableAlterOp> {
    match tag {
        9 => Some(TableAlterOp::DropNamedIndex { name: dec.str()? }),
        10 => Some(TableAlterOp::DropPrimaryKey),
        11 => Some(TableAlterOp::RenameTable {
            new_schema: dec.opt_str()?,
            new_name: dec.str()?,
        }),
        _ => None,
    }
}

fn encode_scope_def(enc: &mut Encoder, scope: &ScopeDef) {
    enc.u32(scope.depth);
    enc.opt(&scope.parent, |e, p| e.u32(*p));
    enc.vec(&scope.bindings, |e, binding| {
        e.opt_str(&binding.schema);
        e.opt_str(&binding.table_name);
        e.opt_str(&binding.alias);
        e.bool(binding.resolvable);
    });
}

fn decode_scopes(dec: &mut Decoder<'_>) -> Option<Vec<ScopeDef>> {
    let len = dec.u32()? as usize;
    let mut out = Vec::with_capacity(len.min(1024));
    for scope_idx in 0..len {
        out.push(decode_scope_def(dec, scope_idx)?);
    }
    Some(out)
}

fn decode_scope_def(dec: &mut Decoder<'_>, scope_idx: usize) -> Option<ScopeDef> {
    let depth = dec.u32()?;
    let parent = dec.opt(Decoder::u32)?;
    if parent.is_some_and(|p| (p as usize) >= scope_idx) {
        return None;
    }
    Some(ScopeDef {
        depth,
        parent,
        bindings: dec.vec(|d| {
            Some(TableBindingDef {
                schema: d.opt_str()?,
                table_name: d.opt_str()?,
                alias: d.opt_str()?,
                resolvable: d.bool()?,
            })
        })?,
    })
}

fn encode_target(enc: &mut Encoder, target: &ColumnRefTarget) {
    match target {
        ColumnRefTarget::Unqualified(col) => {
            enc.u8(0);
            enc.str(col);
        }
        ColumnRefTarget::Qualified2(qual, col) => {
            enc.u8(1);
            enc.str(qual);
            enc.str(col);
        }
        ColumnRefTarget::Qualified3(schema, table, col) => {
            enc.u8(2);
            enc.str(schema);
            enc.str(table);
            enc.str(col);
        }
        ColumnRefTarget::Leading(parts) => {
            enc.u8(3);
            enc.vec(parts, |e, (name, line)| {
                e.str(name);
                e.u32(*line);
            });
        }
    }
}

fn decode_target(dec: &mut Decoder<'_>) -> Option<ColumnRefTarget> {
    match dec.u8()? {
        0 => dec.str().map(ColumnRefTarget::Unqualified),
        1 => decode_qualified_2(dec),
        2 => decode_qualified_3(dec),
        3 => decode_leading_target(dec),
        _ => None,
    }
}

fn decode_qualified_2(dec: &mut Decoder<'_>) -> Option<ColumnRefTarget> {
    Some(ColumnRefTarget::Qualified2(dec.str()?, dec.str()?))
}

fn decode_qualified_3(dec: &mut Decoder<'_>) -> Option<ColumnRefTarget> {
    Some(ColumnRefTarget::Qualified3(
        dec.str()?,
        dec.str()?,
        dec.str()?,
    ))
}

fn decode_leading_target(dec: &mut Decoder<'_>) -> Option<ColumnRefTarget> {
    dec.vec(|d| Some((d.str()?, d.u32()?)))
        .map(ColumnRefTarget::Leading)
}

fn encode_constrained(enc: &mut Encoder, item: &ConstrainedRef) {
    encode_target(enc, &item.target);
    enc.vec_str(&item.excluded);
}

fn decode_constrained(dec: &mut Decoder<'_>) -> Option<ConstrainedRef> {
    Some(ConstrainedRef {
        target: decode_target(dec)?,
        excluded: dec.vec_str()?,
    })
}

fn encode_schema_check(enc: &mut Encoder, check: &SchemaCheck) {
    match check {
        SchemaCheck::TargetColumn {
            table_schema,
            table_name,
            column,
            line,
        } => {
            enc.u8(0);
            enc.opt_str(table_schema);
            enc.str(table_name);
            enc.str(column);
            enc.u32(*line);
        }
        SchemaCheck::UsingColumn {
            left_scope,
            right_table,
            column,
            using_cols,
            line,
        } => {
            enc.u8(1);
            enc.u32(*left_scope);
            enc.opt(right_table, |e, (schema, name)| {
                e.opt_str(schema);
                e.str(name);
            });
            enc.str(column);
            enc.vec_str(using_cols);
            enc.u32(*line);
        }
        SchemaCheck::ColumnRef {
            scope,
            target,
            excluded,
            clause,
            constrained,
            line,
        } => {
            enc.u8(2);
            enc.u32(*scope);
            encode_target(enc, target);
            enc.vec_str(excluded);
            enc.u8(clause.to_u8());
            enc.vec(constrained, encode_constrained);
            enc.u32(*line);
        }
        SchemaCheck::TextNumber {
            scope,
            target,
            excluded,
            line,
        } => {
            enc.u8(3);
            enc.u32(*scope);
            encode_target(enc, target);
            enc.vec_str(excluded);
            enc.u32(*line);
        }
    }
}

fn decode_schema_check(dec: &mut Decoder<'_>) -> Option<SchemaCheck> {
    dec.dispatch(&[
        decode_target_column_check,
        decode_using_column_check,
        decode_column_ref_check,
        decode_text_number_check,
    ])
}

fn decode_target_column_check(dec: &mut Decoder<'_>) -> Option<SchemaCheck> {
    Some(SchemaCheck::TargetColumn {
        table_schema: dec.opt_str()?,
        table_name: dec.str()?,
        column: dec.str()?,
        line: dec.u32()?,
    })
}

fn decode_using_column_check(dec: &mut Decoder<'_>) -> Option<SchemaCheck> {
    Some(SchemaCheck::UsingColumn {
        left_scope: dec.u32()?,
        right_table: dec.opt(|d| Some((d.opt_str()?, d.str()?)))?,
        column: dec.str()?,
        using_cols: dec.vec_str()?,
        line: dec.u32()?,
    })
}

fn decode_column_ref_check(dec: &mut Decoder<'_>) -> Option<SchemaCheck> {
    Some(SchemaCheck::ColumnRef {
        scope: dec.u32()?,
        target: decode_target(dec)?,
        excluded: dec.vec_str()?,
        clause: ClauseKind::from_u8(dec.u8()?)?,
        constrained: dec.vec(decode_constrained)?,
        line: dec.u32()?,
    })
}

fn decode_text_number_check(dec: &mut Decoder<'_>) -> Option<SchemaCheck> {
    Some(SchemaCheck::TextNumber {
        scope: dec.u32()?,
        target: decode_target(dec)?,
        excluded: dec.vec_str()?,
        line: dec.u32()?,
    })
}
