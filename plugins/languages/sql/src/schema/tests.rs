use codeint_model::RepoPath;

use crate::schema::{ColumnTypeClass, IndexSchema, SqlSchema};
use crate::{ParsedSqlStatement, Sql, SqlDialect};

fn parse_sql(dialect: SqlDialect, source: &str) -> Vec<ParsedSqlStatement> {
    let path = RepoPath::new("migrations/001.sql").expect("valid path");
    Sql::with_dialect(dialect)
        .lower_with_details(&path, source)
        .expect("lower succeeds")
        .parsed_statements
}

#[test]
fn creates_table_with_normalized_columns_and_type_classes() {
    let stmts = parse_sql(
        SqlDialect::Postgresql,
        r#"
        CREATE TABLE Users (
            Id BIGSERIAL PRIMARY KEY,
            Email VARCHAR(255) UNIQUE,
            Bio TEXT,
            Handle CITEXT,
            Code CHARACTER VARYING(16),
            Price NUMERIC(10, 2),
            Balance MONEY,
            Active BOOLEAN,
            CreatedAt TIMESTAMP,
            UpdatedDate DATE,
            Avatar BYTEA,
            Payload JSONB,
            ExternalId UUID
        );
        "#,
    );
    let schema = SqlSchema::build([stmts.as_slice()]);
    assert_eq!(schema.table_count(), 1);
    assert!(!schema.is_empty());

    let table = schema.get_table(None, "USERS").expect("table exists");
    assert_eq!(table.name, "users");
    assert_eq!(table.schema, None);
    assert_eq!(table.primary_key, vec!["id"]);
    assert_eq!(
        table.indexes,
        vec![IndexSchema {
            name: None,
            columns: vec!["email".to_owned()],
        }]
    );
    assert_eq!(table.column_type("id"), Some(ColumnTypeClass::Numeric));
    assert_eq!(table.column_type("email"), Some(ColumnTypeClass::Text));
    assert_eq!(table.column_type("bio"), Some(ColumnTypeClass::Text));
    assert_eq!(table.column_type("handle"), Some(ColumnTypeClass::Text));
    assert_eq!(table.column_type("code"), Some(ColumnTypeClass::Text));
    assert_eq!(table.column_type("price"), Some(ColumnTypeClass::Numeric));
    assert_eq!(table.column_type("balance"), Some(ColumnTypeClass::Numeric));
    assert_eq!(table.column_type("active"), Some(ColumnTypeClass::Boolean));
    assert_eq!(
        table.column_type("createdat"),
        Some(ColumnTypeClass::Temporal)
    );
    assert_eq!(
        table.column_type("updateddate"),
        Some(ColumnTypeClass::Temporal)
    );
    assert_eq!(table.column_type("avatar"), Some(ColumnTypeClass::Binary));
    assert_eq!(table.column_type("payload"), Some(ColumnTypeClass::Other));
    assert_eq!(
        table.column_type("externalid"),
        Some(ColumnTypeClass::Other)
    );
}

#[test]
fn classifies_mysql_text_numeric_temporal_and_binary_types() {
    let stmts = parse_sql(
        SqlDialect::Mysql,
        r#"
        CREATE TABLE articles (
            id INT UNSIGNED PRIMARY KEY,
            tiny_body TINYTEXT,
            med_body MEDIUMTEXT,
            long_body LONGTEXT,
            raw_bin VARBINARY(64),
            blob_data BLOB,
            published_at DATETIME,
            score DOUBLE,
            flags TINYINT,
            INDEX idx_pub_score (published_at, score),
            KEY idx_flags (flags)
        );
        "#,
    );
    let schema = SqlSchema::build([stmts.as_slice()]);
    let table = schema.get_table(None, "articles").expect("table exists");
    assert_eq!(table.column_type("id"), Some(ColumnTypeClass::Numeric));
    assert_eq!(table.column_type("tiny_body"), Some(ColumnTypeClass::Text));
    assert_eq!(table.column_type("med_body"), Some(ColumnTypeClass::Text));
    assert_eq!(table.column_type("long_body"), Some(ColumnTypeClass::Text));
    assert_eq!(table.column_type("raw_bin"), Some(ColumnTypeClass::Binary));
    assert_eq!(
        table.column_type("blob_data"),
        Some(ColumnTypeClass::Binary)
    );
    assert_eq!(
        table.column_type("published_at"),
        Some(ColumnTypeClass::Temporal)
    );
    assert_eq!(table.column_type("score"), Some(ColumnTypeClass::Numeric));
    assert_eq!(table.column_type("flags"), Some(ColumnTypeClass::Numeric));
    assert_eq!(table.indexes.len(), 2);
    assert_eq!(
        table.indexes[0],
        IndexSchema {
            name: Some("idx_pub_score".to_owned()),
            columns: vec!["published_at".to_owned(), "score".to_owned()],
        }
    );
    assert_eq!(
        table.indexes[1],
        IndexSchema {
            name: Some("idx_flags".to_owned()),
            columns: vec!["flags".to_owned()],
        }
    );
}

#[test]
fn records_table_level_primary_key_unique_and_standalone_indexes_in_order() {
    let stmts = parse_sql(
        SqlDialect::Generic,
        r#"
        CREATE TABLE orders (
            tenant_id INT NOT NULL,
            order_id INT NOT NULL,
            customer_id INT NOT NULL,
            created_at TIMESTAMP NOT NULL,
            status VARCHAR(32) NOT NULL,
            PRIMARY KEY (tenant_id, order_id),
            CONSTRAINT uq_customer_created UNIQUE (customer_id, created_at)
        );
        CREATE INDEX idx_orders_status_created ON orders (status DESC, created_at ASC);
        CREATE UNIQUE INDEX idx_missing_table ON ghost_table (col_a);
        "#,
    );
    let schema = SqlSchema::build([stmts.as_slice()]);
    assert_eq!(schema.table_count(), 1);
    let table = schema.get_table(None, "orders").expect("orders exists");
    assert_eq!(table.primary_key, vec!["tenant_id", "order_id"]);
    assert_eq!(
        table.indexes,
        vec![
            IndexSchema {
                name: Some("uq_customer_created".to_owned()),
                columns: vec!["customer_id".to_owned(), "created_at".to_owned()],
            },
            IndexSchema {
                name: Some("idx_orders_status_created".to_owned()),
                columns: vec!["status".to_owned(), "created_at".to_owned()],
            },
        ]
    );
    assert!(table.is_indexed_column("tenant_id"));
    assert!(!table.is_indexed_column("order_id"));
    assert!(table.is_indexed_column("customer_id"));
    assert!(!table.is_indexed_column("created_at"));
    assert!(table.is_indexed_column("status"));
}

#[test]
fn alters_table_add_rename_drop_columns_constraints_and_table_rename() {
    let migration_1 = parse_sql(
        SqlDialect::Postgresql,
        r#"
        CREATE TABLE app.accounts (
            id INT PRIMARY KEY,
            email TEXT NOT NULL,
            legacy_code VARCHAR(20)
        );
        CREATE INDEX idx_accounts_email ON app.accounts (email);
        "#,
    );
    let migration_2 = parse_sql(
        SqlDialect::Postgresql,
        r#"
        ALTER TABLE app.accounts ADD COLUMN region TEXT;
        ALTER TABLE app.accounts RENAME COLUMN email TO primary_email;
        ALTER TABLE app.accounts DROP COLUMN legacy_code;
        ALTER TABLE app.accounts ADD CONSTRAINT uq_region UNIQUE (region);
        "#,
    );
    let migration_3 = parse_sql(
        SqlDialect::Postgresql,
        r#"
        ALTER TABLE app.accounts DROP CONSTRAINT uq_region;
        ALTER TABLE app.accounts RENAME TO members;
        "#,
    );

    let schema = SqlSchema::build([
        migration_1.as_slice(),
        migration_2.as_slice(),
        migration_3.as_slice(),
    ]);
    assert_eq!(schema.table_count(), 1);
    assert!(schema.get_table(Some("app"), "accounts").is_none());

    let table = schema
        .get_table(Some("app"), "members")
        .expect("renamed table exists in same schema");
    assert!(table.has_column("id"));
    assert!(table.has_column("primary_email"));
    assert!(!table.has_column("email"));
    assert!(!table.has_column("legacy_code"));
    assert!(table.has_column("region"));
    assert_eq!(
        table.indexes,
        vec![IndexSchema {
            name: Some("idx_accounts_email".to_owned()),
            columns: vec!["primary_email".to_owned()],
        }]
    );
}

#[test]
fn drops_tables_and_indexes_and_later_files_override_earlier_ones() {
    let v1 = parse_sql(
        SqlDialect::Generic,
        r#"
        CREATE TABLE items (
            id INT PRIMARY KEY,
            sku TEXT NOT NULL,
            notes TEXT
        );
        CREATE INDEX idx_items_sku ON items (sku);
        CREATE TABLE temp_staging (
            id INT PRIMARY KEY
        );
        "#,
    );
    let v2 = parse_sql(
        SqlDialect::Generic,
        r#"
        DROP INDEX idx_items_sku;
        DROP TABLE IF EXISTS temp_staging;
        "#,
    );
    let v3 = parse_sql(
        SqlDialect::Generic,
        r#"
        CREATE TABLE items (
            id INT PRIMARY KEY,
            sku INT NOT NULL
        );
        "#,
    );

    let after_v2 = SqlSchema::build([v1.as_slice(), v2.as_slice()]);
    assert_eq!(after_v2.table_count(), 1);
    assert!(after_v2.get_table(None, "temp_staging").is_none());
    let items_v2 = after_v2.get_table(None, "items").expect("items exists");
    assert!(items_v2.indexes.is_empty());
    assert_eq!(items_v2.column_type("sku"), Some(ColumnTypeClass::Text));

    let after_v3 = SqlSchema::build([v1.as_slice(), v2.as_slice(), v3.as_slice()]);
    let items_v3 = after_v3.get_table(None, "items").expect("items replaced");
    assert_eq!(items_v3.column_type("sku"), Some(ColumnTypeClass::Numeric));
    assert!(!items_v3.has_column("notes"));
}

#[test]
fn drops_mysql_index_on_table_and_handles_multi_schema_lookup() {
    let mysql_stmts = parse_sql(
        SqlDialect::Mysql,
        r#"
        CREATE TABLE events (
            id BIGINT PRIMARY KEY,
            kind VARCHAR(32)
        );
        CREATE INDEX idx_events_kind ON events (kind);
        DROP INDEX idx_events_kind ON events;
        "#,
    );
    let mysql_schema = SqlSchema::build([mysql_stmts.as_slice()]);
    let events = mysql_schema
        .get_table(None, "events")
        .expect("events exists");
    assert!(events.indexes.is_empty());

    let multi_schema_stmts = parse_sql(
        SqlDialect::Postgresql,
        r#"
        CREATE TABLE auth.users (id INT PRIMARY KEY);
        CREATE TABLE billing.users (id INT PRIMARY KEY);
        CREATE TABLE analytics.events (id INT PRIMARY KEY);
        "#,
    );
    let multi = SqlSchema::build([multi_schema_stmts.as_slice()]);
    assert_eq!(multi.table_count(), 3);
    assert!(multi.get_table(Some("auth"), "users").is_some());
    assert!(multi.get_table(Some("billing"), "users").is_some());
    assert!(multi.get_table(None, "users").is_none());
    assert!(multi.get_table(None, "events").is_some());
}

#[test]
fn artifact_codec_round_trips_every_variant_and_rejects_malformed_payloads() {
    use std::collections::BTreeMap;

    use codeint_model::{FileScopeArtifact, LanguagePlugin, LanguagePluginError, SourceSet};

    use crate::schema::artifact::{
        AlterOp, ClauseKind, ColumnAlterOp, ColumnRefTarget, ConstrainedRef, DdlOp, SchemaCheck,
        ScopeDef, SqlFileArtifact, TableAlterOp, TableBindingDef,
    };
    use crate::schema::codec::{decode_artifact, encode_artifact};
    use crate::schema::types::TableSchema;

    let full = SqlFileArtifact {
        is_down_migration: true,
        ddl_ops: vec![
            DdlOp::CreateTable {
                if_not_exists: true,
                table: TableSchema {
                    schema: Some("app".to_owned()),
                    name: "users".to_owned(),
                    columns: BTreeMap::from([
                        ("a".to_owned(), ColumnTypeClass::Text),
                        ("b".to_owned(), ColumnTypeClass::Numeric),
                        ("c".to_owned(), ColumnTypeClass::Boolean),
                        ("d".to_owned(), ColumnTypeClass::Temporal),
                        ("e".to_owned(), ColumnTypeClass::Binary),
                        ("f".to_owned(), ColumnTypeClass::Other),
                    ]),
                    primary_key: vec!["b".to_owned()],
                    indexes: vec![IndexSchema {
                        name: Some("idx_a".to_owned()),
                        columns: vec!["a".to_owned()],
                    }],
                },
            },
            DdlOp::CreateIndex {
                table_schema: None,
                table_name: "users".to_owned(),
                index: IndexSchema {
                    name: None,
                    columns: vec!["c".to_owned(), "d".to_owned()],
                },
            },
            DdlOp::AlterTable {
                table_schema: Some("app".to_owned()),
                table_name: "users".to_owned(),
                ops: vec![
                    AlterOp::Column(ColumnAlterOp::AddColumn {
                        name: "g".to_owned(),
                        ty: ColumnTypeClass::Text,
                        is_pk: false,
                        unique_index: Some(Some("uq_g".to_owned())),
                    }),
                    AlterOp::Column(ColumnAlterOp::DropColumn {
                        name: "f".to_owned(),
                    }),
                    AlterOp::Column(ColumnAlterOp::RenameColumn {
                        old: "a".to_owned(),
                        new: "a2".to_owned(),
                    }),
                    AlterOp::Column(ColumnAlterOp::ChangeColumn {
                        old: "c".to_owned(),
                        new: "c2".to_owned(),
                        ty: ColumnTypeClass::Numeric,
                        is_pk: false,
                        unique_index: Some(None),
                    }),
                    AlterOp::Column(ColumnAlterOp::ModifyColumn {
                        name: "d".to_owned(),
                        ty: ColumnTypeClass::Temporal,
                        is_pk: true,
                        unique_index: None,
                    }),
                    AlterOp::Column(ColumnAlterOp::SetColumnType {
                        name: "e".to_owned(),
                        ty: ColumnTypeClass::Binary,
                    }),
                    AlterOp::Table(TableAlterOp::SetPrimaryKey {
                        columns: vec!["b".to_owned(), "d".to_owned()],
                    }),
                    AlterOp::Table(TableAlterOp::AddIndex {
                        index: IndexSchema {
                            name: Some("idx_e".to_owned()),
                            columns: vec!["e".to_owned()],
                        },
                    }),
                    AlterOp::Table(TableAlterOp::PromoteIndexToPk {
                        index_name: "idx_e".to_owned(),
                    }),
                    AlterOp::Table(TableAlterOp::DropNamedIndex {
                        name: "idx_e".to_owned(),
                    }),
                    AlterOp::Table(TableAlterOp::DropPrimaryKey),
                    AlterOp::Table(TableAlterOp::RenameTable {
                        new_schema: Some("core".to_owned()),
                        new_name: "members".to_owned(),
                    }),
                ],
            },
            DdlOp::DropTable {
                schema: Some("core".to_owned()),
                name: "members".to_owned(),
            },
            DdlOp::DropIndexOnTable {
                table_schema: None,
                table_name: "users".to_owned(),
                index_name: "idx_a".to_owned(),
            },
            DdlOp::DropIndexAcrossTables {
                index_schema: Some("app".to_owned()),
                index_name: "idx_global".to_owned(),
            },
        ],
        scopes: vec![
            ScopeDef {
                depth: 0,
                parent: None,
                bindings: vec![TableBindingDef {
                    schema: Some("app".to_owned()),
                    table_name: Some("users".to_owned()),
                    alias: Some("u".to_owned()),
                    resolvable: true,
                }],
            },
            ScopeDef {
                depth: 1,
                parent: Some(0),
                bindings: vec![TableBindingDef {
                    schema: None,
                    table_name: None,
                    alias: None,
                    resolvable: false,
                }],
            },
        ],
        checks: vec![
            SchemaCheck::TargetColumn {
                table_schema: Some("app".to_owned()),
                table_name: "users".to_owned(),
                column: "missing".to_owned(),
                line: 2,
            },
            SchemaCheck::UsingColumn {
                left_scope: 0,
                right_table: Some((None, "orders".to_owned())),
                column: "user_id".to_owned(),
                using_cols: vec!["user_id".to_owned()],
                line: 3,
            },
            SchemaCheck::ColumnRef {
                scope: 1,
                target: ColumnRefTarget::Unqualified("a".to_owned()),
                excluded: vec!["alias_x".to_owned()],
                clause: ClauseKind::General,
                constrained: vec![ConstrainedRef {
                    target: ColumnRefTarget::Qualified2("u".to_owned(), "b".to_owned()),
                    excluded: Vec::new(),
                }],
                line: 4,
            },
            SchemaCheck::ColumnRef {
                scope: 0,
                target: ColumnRefTarget::Qualified3(
                    "app".to_owned(),
                    "users".to_owned(),
                    "c".to_owned(),
                ),
                excluded: Vec::new(),
                clause: ClauseKind::Filter,
                constrained: Vec::new(),
                line: 5,
            },
            SchemaCheck::ColumnRef {
                scope: 0,
                target: ColumnRefTarget::Leading(vec![("u".to_owned(), 6), ("d".to_owned(), 6)]),
                excluded: Vec::new(),
                clause: ClauseKind::JoinOn,
                constrained: Vec::new(),
                line: 6,
            },
            SchemaCheck::ColumnRef {
                scope: 0,
                target: ColumnRefTarget::Qualified2("u".to_owned(), "e".to_owned()),
                excluded: Vec::new(),
                clause: ClauseKind::OrderBy,
                constrained: Vec::new(),
                line: 7,
            },
            SchemaCheck::TextNumber {
                scope: 0,
                target: ColumnRefTarget::Unqualified("a".to_owned()),
                excluded: Vec::new(),
                line: 8,
            },
        ],
    };

    let encoded = encode_artifact(&full);
    assert_eq!(decode_artifact(&encoded), Some(full));

    let mut bad_magic = encoded.clone();
    bad_magic[0] = b'X';
    assert_eq!(decode_artifact(&bad_magic), None);

    assert_eq!(decode_artifact(&encoded[..encoded.len() - 1]), None);

    let mut trailing = encoded.clone();
    trailing.push(0);
    assert_eq!(decode_artifact(&trailing), None);

    let cyclic = SqlFileArtifact {
        scopes: vec![ScopeDef {
            depth: 0,
            parent: Some(0),
            bindings: Vec::new(),
        }],
        ..SqlFileArtifact::default()
    };
    assert_eq!(decode_artifact(&encode_artifact(&cyclic)), None);

    let forward_parent = SqlFileArtifact {
        scopes: vec![
            ScopeDef {
                depth: 0,
                parent: Some(1),
                bindings: Vec::new(),
            },
            ScopeDef {
                depth: 1,
                parent: None,
                bindings: Vec::new(),
            },
        ],
        ..SqlFileArtifact::default()
    };
    assert_eq!(decode_artifact(&encode_artifact(&forward_parent)), None);

    let sql = Sql::new();
    let valid_artifact = FileScopeArtifact {
        path: "db/empty.sql".to_owned(),
        source_set: SourceSet::Production,
        artifact: encode_artifact(&SqlFileArtifact::default()),
    };
    let ok = sql
        .analyze_scope("", None, vec![valid_artifact.clone()])
        .expect("generic scope analysis succeeds");
    assert_eq!(ok.summaries[0].fields[0].value, "generic SQL");

    for bad_dialect in ["oracle", "generic"] {
        let err = sql
            .analyze_scope("", Some(bad_dialect), vec![valid_artifact.clone()])
            .expect_err("unknown dialect must fail");
        assert!(matches!(err, LanguagePluginError::Call { .. }), "{err}");
    }

    let corrupt = FileScopeArtifact {
        path: "db/corrupt.sql".to_owned(),
        source_set: SourceSet::Production,
        artifact: b"not-a-sql-artifact".to_vec(),
    };
    let err = sql
        .analyze_scope("", Some("postgresql"), vec![corrupt])
        .expect_err("corrupted artifact must fail");
    assert!(matches!(err, LanguagePluginError::Call { .. }), "{err}");
}
