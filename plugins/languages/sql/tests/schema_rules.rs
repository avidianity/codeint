use codeint_lang_sql::schema::{SqlSchema, collect_schema_hazards, schema_hazard_rules};
use codeint_lang_sql::{ParsedSqlStatement, Sql, SqlDialect};
use codeint_model::{HazardLine, RepoPath};

fn parse(dialect: SqlDialect, source: &str) -> Vec<ParsedSqlStatement> {
    let path = RepoPath::new("queries/test.sql").expect("valid path");
    Sql::with_dialect(dialect)
        .lower_with_details(&path, source)
        .expect("lower succeeds")
        .parsed_statements
}

fn assert_hits_dialect(
    dialect: SqlDialect,
    schema_sql: &str,
    query_sql: &str,
    expected: &[(u32, &str)],
) {
    let ddl = parse(dialect, schema_sql);
    let schema = SqlSchema::build([ddl.as_slice()]);
    let stmts = parse(dialect, query_sql);
    let actual = collect_schema_hazards(&stmts, &schema, dialect);
    let expected_lines: Vec<HazardLine> = expected
        .iter()
        .map(|&(line, rule)| HazardLine {
            line,
            rule: rule.to_owned(),
        })
        .collect();
    assert_eq!(actual, expected_lines);
}

fn assert_hits(schema_sql: &str, query_sql: &str, expected: &[(u32, &str)]) {
    assert_hits_dialect(SqlDialect::Generic, schema_sql, query_sql, expected);
}

fn base_schema() -> &'static str {
    r#"
    CREATE TABLE users (
        id INT PRIMARY KEY,
        email VARCHAR(255) NOT NULL,
        code TEXT NOT NULL,
        age INT NOT NULL,
        tenant_id INT NOT NULL,
        created_at TIMESTAMP NOT NULL
    );
    CREATE INDEX idx_users_tenant_created ON users (tenant_id, created_at);

    CREATE TABLE orders (
        id INT PRIMARY KEY,
        user_id INT NOT NULL,
        status TEXT NOT NULL,
        amount NUMERIC(10, 2) NOT NULL
    );
    CREATE INDEX idx_orders_status ON orders (status);
    "#
}

#[test]
fn lists_all_five_schema_hazard_rules() {
    let rules: Vec<&str> = schema_hazard_rules().collect();
    assert_eq!(
        rules,
        vec![
            "filter-on-unindexed-column",
            "join-on-unindexed-column",
            "order-by-unindexed-column",
            "text-compared-to-number",
            "unknown-column",
        ]
    );
}

#[test]
fn unknown_column_matches_missing_column_and_respects_valid_columns() {
    assert_hits(
        base_schema(),
        "SELECT id,\n  nonexistent_col\nFROM users\nWHERE id = 1;",
        &[(2, "unknown-column")],
    );
    assert_hits(
        base_schema(),
        "SELECT id, email FROM users WHERE id = 1;",
        &[],
    );
}

#[test]
fn unknown_column_matches_insert_update_and_delete_targets() {
    assert_hits(
        base_schema(),
        "INSERT INTO users (id, ghost_col) VALUES (1, 'x');\nUPDATE users SET missing_field = 'y' WHERE id = 1;\nDELETE FROM users WHERE bogus = 1;",
        &[
            (1, "unknown-column"),
            (2, "unknown-column"),
            (3, "unknown-column"),
        ],
    );
}

#[test]
fn text_compared_to_number_matches_bare_numbers_and_ignores_strings() {
    assert_hits(
        base_schema(),
        "SELECT id\nFROM orders\nWHERE status = 42\n  AND 7 < status\n  AND status <> -1;",
        &[
            (3, "text-compared-to-number"),
            (4, "text-compared-to-number"),
            (5, "text-compared-to-number"),
        ],
    );
    assert_hits(
        base_schema(),
        "SELECT id FROM orders WHERE status = '42' AND id = 42;",
        &[],
    );
}

#[test]
fn filter_on_unindexed_column_matches_unindexed_and_second_composite_column() {
    assert_hits(
        base_schema(),
        "SELECT id\nFROM users\nWHERE email = 'a@b.com';",
        &[(3, "filter-on-unindexed-column")],
    );
    assert_hits(
        base_schema(),
        "SELECT id\nFROM users\nWHERE created_at > '2026-01-01';",
        &[(3, "filter-on-unindexed-column")],
    );
    assert_hits(
        base_schema(),
        "SELECT id FROM users WHERE id = 1 AND tenant_id = 5;",
        &[],
    );
}

#[test]
fn order_by_unindexed_column_matches_unindexed_and_stays_silent_on_aliases() {
    assert_hits(
        base_schema(),
        "SELECT id\nFROM users\nWHERE id = 1\nORDER BY email DESC;",
        &[(4, "order-by-unindexed-column")],
    );
    assert_hits(
        base_schema(),
        "SELECT id\nFROM users\nORDER BY created_at;",
        &[(3, "order-by-unindexed-column")],
    );
    assert_hits(
        base_schema(),
        "SELECT id, age + 1 AS next_age FROM users ORDER BY id;\nSELECT id, age + 1 AS next_age FROM users ORDER BY tenant_id;\nSELECT id, age + 1 AS next_age FROM users ORDER BY next_age;\nSELECT id FROM users ORDER BY 1;",
        &[],
    );
}

#[test]
fn join_on_unindexed_column_matches_on_and_using_and_respects_pk_and_first_index() {
    assert_hits(
        base_schema(),
        "SELECT u.id, o.id\nFROM users u\nJOIN orders o\n  ON o.user_id = u.id;",
        &[
            (4, "filter-on-unindexed-column"),
            (4, "join-on-unindexed-column"),
        ],
    );
    assert_hits(
        "CREATE TABLE users (id INT PRIMARY KEY);\nCREATE TABLE orders (id INT PRIMARY KEY, user_id INT NOT NULL);\nCREATE INDEX idx_orders_user ON orders (user_id);",
        "SELECT u.id, o.id FROM users u JOIN orders o ON o.user_id = u.id;",
        &[],
    );
    assert_hits(
        "CREATE TABLE a (id INT PRIMARY KEY, shared_key INT NOT NULL);\nCREATE TABLE b (id INT PRIMARY KEY, shared_key INT NOT NULL);",
        "SELECT a.id\nFROM a\nJOIN b USING (shared_key);",
        &[(3, "join-on-unindexed-column")],
    );
}

#[test]
fn stays_silent_on_unknown_tables_ctes_and_derived_subqueries() {
    assert_hits(
        base_schema(),
        "SELECT bogus FROM mystery_table WHERE fake = 123 ORDER BY whatever;",
        &[],
    );
    assert_hits(
        base_schema(),
        "WITH users AS (SELECT 1 AS custom_col)\nSELECT custom_col FROM users WHERE custom_col = 1 ORDER BY custom_col;",
        &[],
    );
    assert_hits(
        base_schema(),
        "SELECT sub.derived_col\nFROM (SELECT id AS derived_col FROM users WHERE id = 1) sub\nWHERE sub.derived_col = 1\nORDER BY sub.derived_col;",
        &[],
    );
}

#[test]
fn stays_silent_on_ambiguous_or_mixed_known_unknown_unqualified_columns() {
    assert_hits(
        "CREATE TABLE a (id INT PRIMARY KEY, status TEXT NOT NULL);\nCREATE TABLE b (id INT PRIMARY KEY, status TEXT NOT NULL);",
        "SELECT id FROM a JOIN b ON a.id = b.id WHERE status = 1 ORDER BY status;",
        &[],
    );
    assert_hits(
        base_schema(),
        "SELECT id\nFROM users\nJOIN external_audit ON users.id = external_audit.user_id\nWHERE email = 'a@b.com'\n  AND unknown_maybe_audit = 1\nORDER BY email;",
        &[],
    );
    assert_hits(
        base_schema(),
        "SELECT users.id\nFROM users\nJOIN external_audit ON users.id = external_audit.user_id\nWHERE users.email = 'a@b.com';",
        &[(4, "filter-on-unindexed-column")],
    );
}

#[test]
fn follows_migration_order_for_schema_backed_hazards() {
    let m1 = parse(
        SqlDialect::Postgresql,
        "CREATE TABLE users (id INT PRIMARY KEY, handle TEXT NOT NULL);",
    );
    let m2 = parse(
        SqlDialect::Postgresql,
        "CREATE INDEX idx_users_handle ON users (handle);",
    );
    let m3 = parse(
        SqlDialect::Postgresql,
        "DROP INDEX idx_users_handle;\nALTER TABLE users DROP COLUMN handle;",
    );
    let query = parse(
        SqlDialect::Postgresql,
        "SELECT id FROM users WHERE handle = 'ada';",
    );

    let s1 = SqlSchema::build([m1.as_slice()]);
    assert_eq!(
        collect_schema_hazards(&query, &s1, SqlDialect::Postgresql),
        vec![HazardLine {
            line: 1,
            rule: "filter-on-unindexed-column".to_owned(),
        }]
    );

    let s2 = SqlSchema::build([m1.as_slice(), m2.as_slice()]);
    assert!(collect_schema_hazards(&query, &s2, SqlDialect::Postgresql).is_empty());

    let s3 = SqlSchema::build([m1.as_slice(), m2.as_slice(), m3.as_slice()]);
    assert_eq!(
        collect_schema_hazards(&query, &s3, SqlDialect::Postgresql),
        vec![HazardLine {
            line: 1,
            rule: "unknown-column".to_owned(),
        }]
    );
}

#[test]
fn deduplicates_and_sorts_findings_by_line_and_rule() {
    assert_hits(
        base_schema(),
        "SELECT u.id\nFROM users u\nWHERE u.code = 10 OR u.code = 20\nORDER BY u.email;",
        &[
            (3, "filter-on-unindexed-column"),
            (3, "text-compared-to-number"),
            (4, "order-by-unindexed-column"),
        ],
    );
}

#[test]
fn correlated_subquery_resolves_outer_column_without_false_unknown() {
    assert_hits(
        "CREATE TABLE users (id INT PRIMARY KEY, name TEXT);\nCREATE TABLE orders (order_id INT PRIMARY KEY, user_id INT NOT NULL);\nCREATE INDEX idx_orders_user ON orders (user_id);",
        "SELECT name FROM users WHERE EXISTS (SELECT 1 FROM orders WHERE user_id = id);",
        &[],
    );
}

#[test]
fn handles_sqlite_and_duckdb_dialects() {
    assert_hits_dialect(
        SqlDialect::Sqlite,
        "CREATE TABLE notes (id INTEGER PRIMARY KEY, body TEXT);",
        "SELECT rowid, id FROM notes WHERE body = 99;",
        &[
            (1, "filter-on-unindexed-column"),
            (1, "text-compared-to-number"),
        ],
    );
    assert_hits_dialect(
        SqlDialect::Duckdb,
        "CREATE TABLE metrics (id INT PRIMARY KEY, tag VARCHAR);",
        "SELECT id FROM metrics ORDER BY tag;",
        &[(1, "order-by-unindexed-column")],
    );
}

#[test]
fn ignores_temporary_tables_in_schema_builder() {
    let persistent = parse(
        SqlDialect::Postgresql,
        "CREATE TABLE sessions (id INT PRIMARY KEY, user_id INT NOT NULL);",
    );
    let temp = parse(
        SqlDialect::Postgresql,
        "CREATE TEMPORARY TABLE sessions (scratch TEXT);\nCREATE TEMP TABLE ephemeral (x INT);",
    );
    let schema = SqlSchema::build([persistent.as_slice(), temp.as_slice()]);
    assert_eq!(schema.table_count(), 1);
    let sessions = schema.get_table(None, "sessions").expect("sessions exists");
    assert!(sessions.has_column("user_id"));
    assert!(!sessions.has_column("scratch"));
    assert!(schema.get_table(None, "ephemeral").is_none());
}

#[test]
fn unknown_column_handles_pg_named_params_duckdb_row_vars_subscripts_and_sqlite_dqs() {
    assert_hits_dialect(
        SqlDialect::Postgresql,
        "CREATE TABLE users (id INT PRIMARY KEY, name TEXT);",
        "SELECT id, name FROM users WHERE id = @user_id;",
        &[],
    );
    assert_hits_dialect(
        SqlDialect::Duckdb,
        "CREATE TABLE users (id INT PRIMARY KEY, name TEXT);",
        "SELECT u FROM users AS u WHERE u.id = 1;\nSELECT users FROM users WHERE id = 1;",
        &[],
    );
    assert_hits_dialect(
        SqlDialect::Postgresql,
        "CREATE TABLE spans (id INT PRIMARY KEY, attributes JSONB);",
        "SELECT s.attributes['env'], s.attributes[$2], attributes[$2]\nFROM spans s\nWHERE s.id = 1;",
        &[],
    );
    assert_hits_dialect(
        SqlDialect::Postgresql,
        "CREATE TABLE spans (id INT PRIMARY KEY, attributes JSONB);",
        "SELECT s.missing_map[$2]\nFROM spans s\nWHERE s.id = 1;",
        &[(1, "unknown-column")],
    );
    assert_hits_dialect(
        SqlDialect::Sqlite,
        "CREATE TABLE links (id INTEGER PRIMARY KEY, url TEXT);",
        "SELECT id FROM links WHERE id = 1 AND url <> \"\" AND url <> \"https://example.com\";",
        &[],
    );
    assert_hits_dialect(
        SqlDialect::Sqlite,
        "CREATE TABLE links (id INTEGER PRIMARY KEY, url TEXT);",
        "SELECT \"missing_col\" FROM links WHERE id = 1;",
        &[(1, "unknown-column")],
    );
}

#[test]
fn order_by_respects_aggregates_grouped_case_and_composite_prefixes() {
    assert_hits(
        base_schema(),
        "SELECT status, COUNT(*)\nFROM orders\nGROUP BY status\nORDER BY MAX(amount) DESC, CASE WHEN status = 'open' THEN 0 ELSE 1 END;",
        &[],
    );
    assert_hits(
        base_schema(),
        "SELECT COUNT(*)\nFROM orders\nORDER BY CASE WHEN amount > 10 THEN 0 ELSE 1 END;",
        &[],
    );
    assert_hits(
        base_schema(),
        "SELECT id\nFROM orders\nORDER BY CASE WHEN amount > 10 THEN 0 ELSE 1 END;",
        &[(3, "order-by-unindexed-column")],
    );
    assert_hits(
        base_schema(),
        "SELECT status\nFROM orders\nGROUP BY status\nORDER BY MAX(bogus_col);",
        &[(4, "unknown-column")],
    );
    assert_hits(
        base_schema(),
        "SELECT id\nFROM users\nWHERE tenant_id = 5\nORDER BY created_at DESC;",
        &[],
    );
    assert_hits(
        base_schema(),
        "SELECT id\nFROM users\nORDER BY tenant_id, created_at;",
        &[],
    );
    assert_hits(
        base_schema(),
        "SELECT id\nFROM users\nWHERE tenant_id > 5\nORDER BY created_at;",
        &[(4, "order-by-unindexed-column")],
    );
    assert_hits(
        "CREATE TABLE events (tenant_id INT, bucket INT, seq INT, payload TEXT, PRIMARY KEY (tenant_id, bucket, seq));",
        "SELECT payload FROM events WHERE tenant_id = 1 AND bucket = 2 ORDER BY seq;",
        &[],
    );
}

#[test]
fn filter_and_join_respect_composite_prefixes_and_leading_index_in_and_chains() {
    assert_hits(
        base_schema(),
        "SELECT id\nFROM users\nWHERE tenant_id = 5\n  AND created_at > '2026-01-01';",
        &[],
    );
    assert_hits(
        base_schema(),
        "SELECT id\nFROM users\nWHERE id = 1\n  AND email = 'a@b.com';",
        &[],
    );
    assert_hits(
        base_schema(),
        "SELECT id\nFROM users\nWHERE id = 1\n  AND email = 42;",
        &[(4, "text-compared-to-number")],
    );
    assert_hits(
        base_schema(),
        "SELECT id\nFROM users\nWHERE id = 1\n   OR email = 'a@b.com';",
        &[(4, "filter-on-unindexed-column")],
    );
    assert_hits(
        base_schema(),
        "SELECT id\nFROM users\nWHERE id <> 1\n  AND email = 'a@b.com';",
        &[(4, "filter-on-unindexed-column")],
    );
    assert_hits(
        base_schema(),
        "SELECT u.id, o.id\nFROM users u\nJOIN orders o\n  ON o.id = u.id\n AND o.user_id = u.id;",
        &[],
    );
    assert_hits(
        base_schema(),
        "SELECT u.id, o.id\nFROM users u\nJOIN orders o ON o.id = u.id\nWHERE u.id = 1\n  AND o.user_id = 5;",
        &[(5, "filter-on-unindexed-column")],
    );
    assert_hits(
        base_schema(),
        "SELECT a.id, b.id\nFROM users a\nJOIN users b\n  ON a.id = 1\n AND b.email = 'x@y.com';",
        &[
            (5, "filter-on-unindexed-column"),
            (5, "join-on-unindexed-column"),
        ],
    );
    assert_hits(
        "CREATE TABLE left_t (a INT, b INT, PRIMARY KEY (a, b));\nCREATE TABLE right_t (a INT, b INT, PRIMARY KEY (a, b));",
        "SELECT l.a\nFROM left_t l\nJOIN right_t r\n  ON r.a = l.a AND r.b = l.b;",
        &[],
    );
}
