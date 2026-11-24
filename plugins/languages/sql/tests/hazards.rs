use std::collections::BTreeSet;

use codeint_lang_sql::{SQL_HAZARD_RULES, Sql, SqlDialect, hazard_line_rules};
use codeint_model::{HazardLine, LanguagePlugin, RepoPath};

fn repo_path(path: &str) -> RepoPath {
    RepoPath::new(path).unwrap()
}

fn hazards_for(dialect: SqlDialect, sql: &str) -> Vec<HazardLine> {
    Sql::with_dialect(dialect)
        .lower(&repo_path("query.sql"), sql)
        .unwrap_or_else(|err| panic!("failed to lower SQL ({dialect}): {err}"))
        .hazard_lines
}

fn rule_lines(dialect: SqlDialect, sql: &str, rule: &str) -> Vec<u32> {
    hazards_for(dialect, sql)
        .into_iter()
        .filter(|h| h.rule == rule)
        .map(|h| h.line)
        .collect()
}

fn check_rule(dialect: SqlDialect, rule: &str, hits: &[(&str, &[u32])], misses: &[&str]) {
    for &(sql, expected) in hits {
        let got = rule_lines(dialect, sql, rule);
        assert_eq!(got, expected, "rule {rule} ({dialect}) mismatch on:\n{sql}");
    }
    for &sql in misses {
        let got = rule_lines(dialect, sql, rule);
        assert!(
            got.is_empty(),
            "rule {rule} ({dialect}) unexpectedly matched at {got:?} on:\n{sql}"
        );
    }
}

#[test]
fn plugin_advertises_all_32_unique_hazard_rules_and_skips_rejected_statements() {
    let sql = Sql::new();
    assert!(sql.has_hazard_rules());
    let listed = sql.hazard_rules();
    let exported: Vec<String> = hazard_line_rules().map(str::to_owned).collect();
    assert_eq!(listed, exported);
    assert_eq!(SQL_HAZARD_RULES.len(), 32);
    let unique: BTreeSet<&str> = SQL_HAZARD_RULES.iter().copied().collect();
    assert_eq!(unique.len(), 32);
    for schema_rule in [
        "unknown-column",
        "text-compared-to-number",
        "filter-on-unindexed-column",
        "order-by-unindexed-column",
        "join-on-unindexed-column",
    ] {
        assert!(unique.contains(schema_rule));
    }

    let mixed = "\
SELECT * FROM users WHERE id = 1;

NOT VALID SQL SELECT * FROM users DELETE FROM users DROP TABLE users;

TRUNCATE TABLE logs;
";
    let out = sql
        .lower_with_details(&repo_path("mixed.sql"), mixed)
        .unwrap();
    assert_eq!(out.rejected_statements.len(), 1);
    assert_eq!(out.rejected_statements[0].line, 3);
    let pairs: Vec<(u32, &str)> = out
        .file_model
        .hazard_lines
        .iter()
        .map(|h| (h.line, h.rule.as_str()))
        .collect();
    assert_eq!(pairs, [(1, "select-star"), (5, "truncate")]);
}

#[test]
fn select_projection_and_set_operation_rules() {
    let g = SqlDialect::Generic;
    check_rule(
        g,
        "select-star",
        &[
            ("SELECT *\nFROM users\nWHERE id = 1;", &[1]),
            ("SELECT\n  u.*\nFROM users u\nWHERE u.id = 1;", &[2]),
            (
                "SELECT id\nFROM users\nWHERE id IN (\n  SELECT * FROM admins\n);",
                &[4],
            ),
        ],
        &[
            "SELECT COUNT(*) FROM users WHERE active = true;",
            "SELECT id FROM users u WHERE EXISTS (SELECT * FROM orders o WHERE o.id = 1);",
            "SELECT id, name FROM users WHERE id = 1;",
            "-- SELECT * FROM users\nSELECT 'SELECT * FROM users' FROM dual WHERE 1 < 2;",
        ],
    );
    check_rule(
        g,
        "subquery-in-select-list",
        &[
            (
                "SELECT\n  u.id,\n  (\n    SELECT COUNT(*) FROM orders o WHERE o.id = 1\n  ) AS cnt\nFROM users u\nWHERE u.active = true;",
                &[4],
            ),
            (
                "SELECT u.id,\n  COALESCE((SELECT MAX(total) FROM orders), 0)\nFROM users u\nWHERE u.id = 1;",
                &[2],
            ),
        ],
        &[
            "SELECT sub.id FROM (SELECT id FROM users WHERE active = true) sub WHERE sub.id > 0;",
            "SELECT id FROM users WHERE id IN (SELECT user_id FROM orders WHERE total > 10);",
            "-- SELECT (SELECT 1)\nSELECT '(SELECT 1)' FROM users WHERE id = 1;",
        ],
    );
    check_rule(
        g,
        "union-without-all",
        &[
            (
                "SELECT id FROM users WHERE active = true\nUNION\nSELECT id FROM archived_users WHERE active = true;",
                &[2],
            ),
            (
                "SELECT id FROM a WHERE x = 1\nUNION ALL\nSELECT id FROM b WHERE x = 1\nUNION DISTINCT\nSELECT id FROM c WHERE x = 1;",
                &[4],
            ),
        ],
        &[
            "SELECT id FROM a WHERE x = 1 UNION ALL SELECT id FROM b WHERE x = 1;",
            "SELECT id FROM a WHERE x = 1 INTERSECT SELECT id FROM b WHERE x = 1;",
            "SELECT id FROM a WHERE x = 1 EXCEPT SELECT id FROM b WHERE x = 1;",
            "-- SELECT 1 UNION SELECT 2\nSELECT 'UNION' FROM users WHERE id = 1;",
        ],
    );
    check_rule(
        g,
        "select-without-filter-or-limit",
        &[
            ("SELECT id\nFROM users;", &[1]),
            ("SELECT id\nFROM users\nOFFSET 5;", &[1]),
        ],
        &[
            "SELECT 1;",
            "SELECT id FROM users WHERE active = true;",
            "SELECT id FROM users LIMIT 10;",
            "SELECT id FROM users FETCH FIRST 10 ROWS ONLY;",
            "CREATE VIEW active_users AS SELECT id FROM users;",
            "CREATE MATERIALIZED VIEW mv_users AS SELECT id FROM users;",
            "-- SELECT id FROM users\nSELECT 'SELECT id FROM users';",
        ],
    );
    check_rule(
        g,
        "count-without-filter",
        &[("SELECT\n  COUNT(*)\nFROM users;", &[2])],
        &[
            "SELECT COUNT(*) FROM users WHERE active = true;",
            "SELECT COUNT(*) FILTER (WHERE active = true) FROM users;",
            "SELECT COUNT(id) FROM users;",
            "SELECT COUNT(*);",
            "-- SELECT COUNT(*) FROM users\nSELECT 'COUNT(*)' FROM users WHERE id = 1;",
        ],
    );
}

#[test]
fn ordering_grouping_distinct_and_offset_rules() {
    let g = SqlDialect::Generic;
    check_rule(
        g,
        "large-offset",
        &[
            (
                "SELECT id\nFROM users\nWHERE active = true\nLIMIT 20\nOFFSET 1001;",
                &[5],
            ),
            (
                "SELECT id\nFROM users\nWHERE active = true\nOFFSET 5000 ROWS;",
                &[4],
            ),
        ],
        &[
            "SELECT id FROM users WHERE active = true LIMIT 20 OFFSET 1000;",
            "SELECT id FROM users WHERE active = true LIMIT 20 OFFSET 0;",
            "SELECT id FROM users WHERE active = true LIMIT 20 OFFSET $1;",
            "-- OFFSET 5000\nSELECT 'OFFSET 5000' FROM users WHERE id = 1;",
        ],
    );
    assert_eq!(
        rule_lines(
            SqlDialect::Mysql,
            "SELECT id\nFROM users\nWHERE active = 1\nLIMIT 2000, 10;",
            "large-offset",
        ),
        [4]
    );
    check_rule(
        g,
        "distinct-with-order-by",
        &[(
            "SELECT\n  DISTINCT name\nFROM users\nWHERE active = true\nORDER BY name;",
            &[2],
        )],
        &[
            "SELECT DISTINCT name FROM users WHERE active = true;",
            "SELECT name FROM users WHERE active = true ORDER BY name;",
            "SELECT ALL name FROM users WHERE active = true ORDER BY name;",
            "-- SELECT DISTINCT a ORDER BY a\nSELECT 'DISTINCT ORDER BY' FROM users WHERE id = 1;",
        ],
    );
    check_rule(
        g,
        "column-position",
        &[
            (
                "SELECT dept, COUNT(id)\nFROM emp\nWHERE active = true\nGROUP BY 1;",
                &[4],
            ),
            (
                "SELECT id, name\nFROM users\nWHERE active = true\nORDER BY 2 DESC;",
                &[4],
            ),
            (
                "SELECT id, name\nFROM users\nWHERE active = true\nORDER BY name,\n  1;",
                &[5],
            ),
        ],
        &[
            "SELECT dept, COUNT(id) FROM emp WHERE active = true GROUP BY dept;",
            "SELECT id, name FROM users WHERE active = true ORDER BY name DESC;",
            "SELECT id, a FROM items WHERE active = true ORDER BY a + 1;",
            "SELECT id, a FROM items WHERE active = true GROUP BY a + 1;",
            "-- ORDER BY 1 GROUP BY 2\nSELECT 'ORDER BY 1' FROM users WHERE id = 1;",
        ],
    );
    for (dialect, query) in [
        (
            SqlDialect::Postgresql,
            "SELECT id\nFROM users\nWHERE active = true\nORDER BY RANDOM()\nLIMIT 5;",
        ),
        (
            SqlDialect::Postgresql,
            "SELECT id\nFROM users\nWHERE active = true\nORDER BY gen_random_uuid()\nLIMIT 5;",
        ),
        (
            SqlDialect::Mysql,
            "SELECT id\nFROM users\nWHERE active = 1\nORDER BY RAND()\nLIMIT 5;",
        ),
        (
            SqlDialect::Sqlite,
            "SELECT id\nFROM users\nWHERE active = 1\nORDER BY random()\nLIMIT 5;",
        ),
        (
            SqlDialect::Duckdb,
            "SELECT id\nFROM users\nWHERE active = true\nORDER BY RANDOM()\nLIMIT 5;",
        ),
        (
            SqlDialect::Generic,
            "SELECT id\nFROM users\nWHERE active = true\nORDER BY NEWID();",
        ),
    ] {
        assert_eq!(rule_lines(dialect, query, "order-by-random"), [4]);
    }
    check_rule(
        g,
        "order-by-random",
        &[],
        &[
            "SELECT RANDOM() FROM users WHERE id = 1;",
            "SELECT id FROM users WHERE id = 1 ORDER BY created_at;",
            "SELECT id FROM users WHERE id = 1 ORDER BY random_seed;",
            "-- ORDER BY RANDOM()\nSELECT 'ORDER BY RANDOM()' FROM users WHERE id = 1;",
        ],
    );
}

#[test]
fn join_and_table_access_rules() {
    let g = SqlDialect::Generic;
    check_rule(
        g,
        "tables-without-join-condition",
        &[
            ("SELECT a.id, b.id\nFROM users a, orders b;", &[2]),
            (
                "SELECT a.id, b.id\nFROM users a\nCROSS JOIN orders b\nWHERE a.id = b.user_id;",
                &[3],
            ),
            (
                "SELECT a.id, b.id\nFROM users a, orders b\nWHERE a.active = true;",
                &[2],
            ),
        ],
        &[
            "SELECT a.id, b.id FROM users a, orders b WHERE a.id = b.user_id;",
            "SELECT a.id, b.id FROM users a JOIN orders b ON a.id = b.user_id;",
            "SELECT a.id, b.id FROM users a JOIN orders b USING (id);",
            "SELECT a.id FROM users a NATURAL JOIN orders b;",
            "SELECT id FROM users WHERE id = 1;",
            "-- CROSS JOIN orders\nSELECT 'CROSS JOIN orders' FROM users WHERE id = 1;",
        ],
    );
    check_rule(
        g,
        "distinct-over-join",
        &[(
            "SELECT\n  DISTINCT u.id\nFROM users u\nJOIN orders o ON u.id = o.user_id\nWHERE u.active = true;",
            &[2],
        )],
        &[
            "SELECT DISTINCT id FROM users WHERE active = true;",
            "SELECT u.id FROM users u JOIN orders o ON u.id = o.user_id WHERE u.active = true;",
            "SELECT ALL u.id FROM users u JOIN orders o ON u.id = o.user_id WHERE u.active = true;",
            "-- SELECT DISTINCT a FROM x JOIN y\nSELECT 'DISTINCT JOIN' FROM users WHERE id = 1;",
        ],
    );
    check_rule(
        g,
        "table-read-twice",
        &[
            (
                "SELECT a.id\nFROM users a\nJOIN users b ON a.manager_id = b.id\nWHERE a.active = true;",
                &[3],
            ),
            (
                "SELECT id\nFROM users\nWHERE manager_id IN (\n  SELECT id FROM users WHERE role = 'lead'\n);",
                &[4],
            ),
        ],
        &[
            "SELECT a.id FROM public.users a JOIN archive.users b ON a.id = b.id WHERE a.id = 1;",
            "WITH active_users AS (SELECT id FROM users WHERE active = true) SELECT a.id FROM active_users a JOIN active_users b ON a.id = b.id WHERE a.id = 1;",
            "SELECT a.id FROM users a JOIN orders b ON a.id = b.user_id WHERE a.id = 1;",
            "-- FROM users JOIN users\nSELECT 'users users' FROM users WHERE id = 1;",
        ],
    );
    check_rule(
        g,
        "join-without-aliases",
        &[
            (
                "SELECT users.id, orders.id\nFROM users\nJOIN orders ON users.id = orders.user_id\nWHERE users.active = true;",
                &[3],
            ),
            (
                "SELECT u.id, orders.id\nFROM users u\nJOIN orders ON u.id = orders.user_id\nWHERE u.active = true;",
                &[3],
            ),
        ],
        &[
            "SELECT u.id, o.id FROM users u JOIN orders o ON u.id = o.user_id WHERE u.active = true;",
            "SELECT u.id, o.id FROM users u, orders o WHERE u.id = o.user_id;",
            "SELECT id FROM users WHERE active = true;",
            "-- FROM users JOIN orders\nSELECT 'JOIN orders' FROM users WHERE id = 1;",
        ],
    );
}

#[test]
fn subquery_correlation_and_nesting_rules() {
    let g = SqlDialect::Generic;
    check_rule(
        g,
        "subquery-refers-to-outer-table",
        &[
            (
                "SELECT u.id\nFROM users u\nWHERE EXISTS (\n  SELECT 1\n  FROM orders o\n  WHERE o.user_id = u.id\n);",
                &[4],
            ),
            (
                "SELECT id\nFROM users\nWHERE EXISTS (\n  SELECT 1\n  FROM orders\n  WHERE orders.user_id = users.id\n);",
                &[4],
            ),
            (
                "UPDATE users u\nSET total = (\n  SELECT COUNT(*) FROM orders o WHERE o.user_id = u.id\n)\nWHERE u.id = 1;",
                &[3],
            ),
        ],
        &[
            "SELECT u.id FROM users u WHERE u.id IN (SELECT o.user_id FROM orders o WHERE o.total > 10);",
            "SELECT u.id FROM users u WHERE EXISTS (SELECT 1 FROM orders u WHERE u.total > 10);",
            "-- WHERE o.user_id = u.id\nSELECT 'u.id' FROM users u WHERE u.id = 1;",
        ],
    );
    check_rule(
        g,
        "deep-subquery-nesting",
        &[
            (
                "SELECT id\nFROM a\nWHERE x IN (\n  SELECT id FROM b WHERE y IN (\n    SELECT id FROM c WHERE z IN (\n      SELECT id FROM d WHERE k = 1\n    )\n  )\n);",
                &[6],
            ),
            (
                "SELECT t3.id\nFROM (\n  SELECT t2.id FROM (\n    SELECT t1.id FROM (\n      SELECT id FROM base WHERE id = 1\n    ) t1\n  ) t2\n) t3\nWHERE t3.id = 1;",
                &[5],
            ),
        ],
        &[
            "SELECT id FROM a WHERE x IN (SELECT id FROM b WHERE y IN (SELECT id FROM c WHERE k = 1));",
            "SELECT id FROM a WHERE x IN (SELECT id FROM b WHERE k = 1) AND y IN (SELECT id FROM c WHERE k = 1) AND z IN (SELECT id FROM d WHERE k = 1);",
            "-- (SELECT (SELECT (SELECT 1)))\nSELECT '(SELECT (SELECT (SELECT 1)))' FROM a WHERE id = 1;",
        ],
    );
}

#[test]
fn predicate_and_filter_rules() {
    let g = SqlDialect::Generic;
    check_rule(
        g,
        "leading-wildcard-like",
        &[
            ("SELECT id\nFROM users\nWHERE name LIKE '%smith';", &[3]),
            ("SELECT id\nFROM users\nWHERE code LIKE '_abc';", &[3]),
            (
                "SELECT id\nFROM users\nWHERE email\n  ILIKE '%@example.com';",
                &[4],
            ),
            ("SELECT id\nFROM users\nWHERE name NOT LIKE '%test%';", &[3]),
        ],
        &[
            "SELECT id FROM users WHERE name LIKE 'smith%';",
            "SELECT id FROM users WHERE code LIKE 'abc_';",
            "SELECT id FROM users WHERE name LIKE $1;",
            "SELECT id FROM users WHERE name LIKE ?;",
            "SELECT id FROM users WHERE name LIKE pattern_col;",
            "-- WHERE name LIKE '%foo'\nSELECT 'LIKE ''%foo''' FROM users WHERE id = 1;",
        ],
    );
    check_rule(
        g,
        "function-on-filter-column",
        &[
            (
                "SELECT id\nFROM users\nWHERE LOWER(email) = 'a@example.com';",
                &[3],
            ),
            (
                "SELECT a.id\nFROM users a\nJOIN orders b\n  ON DATE(a.created_at) = b.order_date\nWHERE a.id = 1;",
                &[4],
            ),
            (
                "SELECT dept\nFROM emp\nGROUP BY dept\nHAVING LOWER(dept) = 'sales';",
                &[4],
            ),
            (
                "SELECT id\nFROM users\nWHERE EXTRACT(YEAR FROM created_at) = 2026;",
                &[3],
            ),
            ("SELECT id\nFROM users\nWHERE TRIM(name) = 'alice';", &[3]),
        ],
        &[
            "SELECT LOWER(email) FROM users WHERE id = 1;",
            "SELECT id FROM users WHERE status = LOWER('ACTIVE');",
            "SELECT id FROM users WHERE created_at > NOW();",
            "SELECT dept FROM emp GROUP BY dept HAVING SUM(salary) > 100;",
            "-- WHERE LOWER(email) = 'x'\nSELECT 'LOWER(email)' FROM users WHERE id = 1;",
        ],
    );
    check_rule(
        g,
        "not-in-subquery",
        &[(
            "SELECT id\nFROM users\nWHERE id\n  NOT IN (SELECT user_id FROM banned WHERE active = true);",
            &[4],
        )],
        &[
            "SELECT id FROM users WHERE id NOT IN (1, 2, 3);",
            "SELECT id FROM users WHERE id IN (SELECT user_id FROM admins WHERE active = true);",
            "SELECT u.id FROM users u WHERE NOT EXISTS (SELECT 1 FROM banned b WHERE b.id = 1);",
            "-- NOT IN (SELECT id FROM t)\nSELECT 'NOT IN (SELECT 1)' FROM users WHERE id = 1;",
        ],
    );
    check_rule(
        g,
        "always-true-condition",
        &[
            ("SELECT id\nFROM users\nWHERE 1 = 1;", &[3]),
            (
                "SELECT id\nFROM users\nWHERE name = 'alice'\n   OR 1 = 1;",
                &[4],
            ),
            (
                "SELECT id\nFROM users\nWHERE name = 'alice'\n   OR 'a' = 'a';",
                &[4],
            ),
            ("SELECT id\nFROM users\nWHERE TRUE;", &[3]),
            (
                "SELECT id\nFROM users\nWHERE active = false\n   OR TRUE;",
                &[4],
            ),
            ("SELECT id\nFROM users\nWHERE NOT FALSE;", &[3]),
        ],
        &[
            "SELECT id FROM users WHERE 1 = 2;",
            "SELECT id FROM users WHERE 'a' = 'b';",
            "SELECT id FROM users WHERE a = a;",
            "SELECT id FROM users WHERE active = TRUE;",
            "SELECT id FROM users WHERE NULL = NULL;",
            "SELECT id FROM users WHERE $1 = $1;",
            "SELECT TRUE FROM users WHERE id = 1;",
            "-- WHERE 1 = 1\nSELECT 'OR 1 = 1' FROM users WHERE id = 1;",
        ],
    );

    let items_101: String = (1..=101)
        .map(|n| n.to_string())
        .collect::<Vec<_>>()
        .join(", ");
    let items_100: String = (1..=100)
        .map(|n| n.to_string())
        .collect::<Vec<_>>()
        .join(", ");
    let hit_in = format!("SELECT id\nFROM users\nWHERE id IN ({items_101});");
    let miss_100 = format!("SELECT id\nFROM users\nWHERE id IN ({items_100});");
    let comment_in =
        format!("-- WHERE id IN ({items_101})\nSELECT 'IN ({items_101})' FROM users WHERE id = 1;");
    check_rule(
        g,
        "long-in-list",
        &[(&hit_in, &[3])],
        &[
            &miss_100,
            "SELECT id FROM users WHERE id IN (SELECT user_id FROM orders WHERE total > 0);",
            &comment_in,
        ],
    );
}

#[test]
fn destructive_dml_ddl_dcl_and_dynamic_sql_rules() {
    let g = SqlDialect::Generic;
    check_rule(
        g,
        "update-without-filter",
        &[("\nUPDATE users\nSET active = false;", &[2])],
        &[
            "UPDATE users SET active = false WHERE id = 1;",
            "-- UPDATE users SET active = false\nSELECT 'UPDATE users SET x = 1' FROM users WHERE id = 1;",
        ],
    );
    check_rule(
        g,
        "delete-without-filter",
        &[("\nDELETE FROM users;", &[2])],
        &[
            "DELETE FROM users WHERE id = 1;",
            "-- DELETE FROM users\nSELECT 'DELETE FROM users' FROM users WHERE id = 1;",
        ],
    );
    check_rule(
        g,
        "truncate",
        &[("\nTRUNCATE TABLE audit_log;", &[2])],
        &[
            "SELECT TRUNCATE(3.14, 1) FROM dual WHERE id = 1;",
            "-- TRUNCATE TABLE audit_log\nSELECT 'TRUNCATE TABLE audit_log' FROM users WHERE id = 1;",
        ],
    );
    check_rule(
        g,
        "drop",
        &[
            ("DROP TABLE legacy_users;", &[1]),
            ("\nDROP VIEW IF EXISTS old_report;", &[2]),
            ("DROP INDEX IF EXISTS idx_old;", &[1]),
            ("DROP FUNCTION IF EXISTS old_fn;", &[1]),
            ("DROP TRIGGER IF EXISTS trg_old ON users;", &[1]),
        ],
        &[
            "ALTER TABLE users DROP COLUMN legacy_col;",
            "ALTER TABLE users DROP CONSTRAINT fk_org;",
            "-- DROP TABLE users\nSELECT 'DROP TABLE users' FROM users WHERE id = 1;",
        ],
    );
    check_rule(
        g,
        "grant-or-revoke",
        &[
            ("GRANT SELECT ON users TO analyst;", &[1]),
            ("\nREVOKE INSERT ON users FROM analyst;", &[2]),
        ],
        &[
            "SELECT id FROM grants WHERE id = 1;",
            "-- GRANT ALL ON users TO public\nSELECT 'REVOKE ALL' FROM users WHERE id = 1;",
        ],
    );
    check_rule(
        g,
        "sql-built-at-run-time",
        &[
            ("EXECUTE IMMEDIATE 'SELECT id FROM users';", &[1]),
            ("\nEXECUTE ('SELECT id FROM ' || tbl_name);", &[2]),
            ("EXECUTE ('SELECT 1');", &[1]),
            ("EXEC sp_executesql 'SELECT 1';", &[1]),
        ],
        &[
            "EXECUTE my_stmt(1);",
            "EXECUTE my_stmt;",
            "PREPARE my_stmt AS SELECT id FROM users WHERE id = $1;",
            "-- EXECUTE IMMEDIATE 'SELECT 1'\nSELECT 'EXECUTE IMMEDIATE' FROM users WHERE id = 1;",
        ],
    );
    assert_eq!(
        rule_lines(
            SqlDialect::Postgresql,
            "EXECUTE format('SELECT * FROM %I', tbl);",
            "sql-built-at-run-time",
        ),
        [1]
    );
}

#[test]
fn plain_text_secret_rules_and_no_secret_leakage() {
    let sentinel = "s3cr3t_t0k3n_never_leak_98765";
    let pg_role = format!("CREATE ROLE app_user\nWITH LOGIN PASSWORD '{sentinel}';");
    let pg_alter = format!("ALTER USER app_user\nWITH PASSWORD '{sentinel}';");
    let pg_alter_role = format!("ALTER ROLE app_user\nWITH PASSWORD '{sentinel}';");
    let duck_secret = format!("CREATE SECRET my_s3 (\n  TYPE S3,\n  SECRET '{sentinel}'\n);");
    let insert_pw = format!("INSERT INTO users (name, password)\nVALUES ('alice', '{sentinel}');");
    let insert_api =
        format!("INSERT INTO integrations (service, api_key)\nVALUES ('stripe', '{sentinel}');");
    let update_token = format!("UPDATE sessions\nSET access_key = '{sentinel}'\nWHERE id = 1;");
    let update_priv = format!("UPDATE certs\nSET private_key = '{sentinel}'\nWHERE id = 1;");
    let update_passwd = format!(
        "UPDATE accounts\nSET user_passwd = '{sentinel}',\n    pwd_backup = '{sentinel}',\n    apikey = '{sentinel}',\n    auth_token = '{sentinel}'\nWHERE id = 1;"
    );

    check_rule(
        SqlDialect::Postgresql,
        "plain-text-secret",
        &[(&pg_role, &[2]), (&pg_alter, &[2]), (&pg_alter_role, &[2])],
        &[
            "CREATE ROLE app_user WITH PASSWORD NULL;",
            "ALTER USER app_user WITH PASSWORD NULL;",
            "INSERT INTO users (name, password) VALUES ('alice', $1);",
            "INSERT INTO users (name, password) VALUES ('alice', ?);",
            "INSERT INTO users (name, password) VALUES ('alice', :pwd);",
            "INSERT INTO users (name, password) VALUES ('alice', crypt($1, gen_salt('bf')));",
            "INSERT INTO users (name, password) VALUES ('alice', NULL);",
            "UPDATE users SET password = $1 WHERE id = 1;",
            "UPDATE users SET api_key = sha256(seed) WHERE id = 1;",
            "UPDATE users SET display_name = 'alice' WHERE id = 1;",
            "-- UPDATE users SET password = 'abc'\nSELECT 'PASSWORD ''abc''' FROM users WHERE id = 1;",
        ],
    );
    assert_eq!(
        rule_lines(SqlDialect::Duckdb, &duck_secret, "plain-text-secret"),
        [3]
    );
    check_rule(
        SqlDialect::Generic,
        "plain-text-secret",
        &[
            (&insert_pw, &[2]),
            (&insert_api, &[2]),
            (&update_token, &[2]),
            (&update_priv, &[2]),
            (&update_passwd, &[2, 3, 4, 5]),
        ],
        &[],
    );

    for sql in [&pg_role, &insert_pw, &update_token] {
        let lines = hazards_for(SqlDialect::Postgresql, sql);
        assert!(!lines.is_empty());
        let debug_repr = format!("{lines:?}");
        assert!(!debug_repr.contains(sentinel));
    }
}
