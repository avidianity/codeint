use codeint_lang_sql::{RejectedSqlStatement, Sql, SqlDialect, SqlLowerOutput, flagged_line_rules};
use codeint_model::{LanguagePlugin, LowerError, RepoPath, SourceSet, TokenKind};

fn repo_path(path: &str) -> RepoPath {
    RepoPath::new(path).unwrap()
}

fn lower_generic(path: &str, src: &str) -> SqlLowerOutput {
    Sql::new()
        .lower_with_details(&repo_path(path), src)
        .unwrap_or_else(|err| panic!("expected {path} to lower cleanly: {err}"))
}

fn lower_dialect(dialect: SqlDialect, path: &str, src: &str) -> SqlLowerOutput {
    Sql::with_dialect(dialect)
        .lower_with_details(&repo_path(path), src)
        .unwrap_or_else(|err| panic!("expected {path} ({dialect}) to lower cleanly: {err}"))
}

#[test]
fn sql_plugin_shell_dialects_empty_comments_and_unparsable_files() {
    let sql = Sql::default();
    assert_eq!(sql.name(), "sql");
    assert_eq!(sql.dialect(), SqlDialect::Generic);
    assert_eq!(sql.extensions(), ["sql"]);
    assert!(!sql.has_imports());
    assert!(!sql.has_hazard_rules());
    assert!(sql.hazard_rules().is_empty());
    assert!(
        sql.reference_ranges()
            .unwrap()
            .for_metric("erosion")
            .is_none()
    );
    assert!(sql.manifest_paths(&[repo_path("schema.sql")]).is_empty());
    assert!(sql.workspace_packages(Vec::new()).unwrap().is_empty());
    assert!(
        sql.resolve_imports(&[], Vec::new(), Vec::new())
            .unwrap()
            .is_empty()
    );
    assert!(sql.declared_dependencies(Vec::new()).unwrap().is_empty());
    assert!(sql.entry_points(&[], Vec::new()).unwrap().is_empty());

    let dialect_labels: Vec<(&str, &str, String)> = [
        SqlDialect::Generic,
        SqlDialect::Postgresql,
        SqlDialect::Mysql,
        SqlDialect::Sqlite,
        SqlDialect::Duckdb,
    ]
    .into_iter()
    .map(|d| (d.as_str(), d.display_name(), d.to_string()))
    .collect();
    assert_eq!(
        dialect_labels,
        [
            ("generic", "generic SQL", "generic SQL".to_owned()),
            ("postgresql", "postgresql", "postgresql".to_owned()),
            ("mysql", "mysql", "mysql".to_owned()),
            ("sqlite", "sqlite", "sqlite".to_owned()),
            ("duckdb", "duckdb", "duckdb".to_owned()),
        ]
    );

    let unsupported = sql.lower(&repo_path("notes.txt"), "SELECT 1;\n");
    assert!(matches!(
        unsupported,
        Err(LowerError::UnsupportedFile { .. })
    ));

    let empty = lower_generic("empty.sql", "");
    assert_eq!(empty.file_model.source_lines, 0);
    assert!(empty.file_model.functions.is_empty());
    assert!(empty.file_model.tokens.is_empty());
    assert_eq!(empty.file_model.source_set, SourceSet::Production);
    assert!(!empty.file_model.program_start);
    assert!(empty.rejected_statements.is_empty());
    assert!(empty.parsed_statements.is_empty());

    let comment_only = lower_generic(
        "comments.sql",
        "-- leading comment\n\n/* multi\n   line\n   comment */\n",
    );
    assert_eq!(comment_only.file_model.source_lines, 0);
    assert!(comment_only.file_model.functions.is_empty());
    assert!(comment_only.file_model.tokens.is_empty());

    let unclosed = sql.lower(&repo_path("broken.sql"), "SELECT 'unclosed string");
    assert!(matches!(unclosed, Err(LowerError::Unparsable { .. })));

    let lf_src = "-- header\n\nSELECT a,\n       b\nFROM items;\n";
    let lf = lower_generic("q.sql", lf_src);
    let crlf = lower_generic("q.sql", &lf_src.replace('\n', "\r\n"));
    assert_eq!(lf.file_model.source_lines, 3);
    assert_eq!(crlf.file_model, lf.file_model);
}

#[test]
fn sql_statements_as_functions_and_object_naming() {
    let src = "\
CREATE TABLE users (
    id INT PRIMARY KEY,
    email TEXT
);

CREATE TABLE IF NOT EXISTS public.orders (
    id INT PRIMARY KEY
);

CREATE TEMP TABLE scratch (x INT);
CREATE UNLOGGED TABLE cache (k TEXT);
CREATE OR REPLACE VIEW analytics.monthly_totals AS SELECT 1;
CREATE MATERIALIZED VIEW mv_sales AS SELECT 1;
CREATE UNIQUE INDEX CONCURRENTLY IF NOT EXISTS public.idx_users_email ON users (email);
CREATE INDEX ON users (id);
CREATE TYPE mood AS ENUM ('sad', 'ok', 'happy');

BEGIN;
INSERT INTO users VALUES (1, 'a@b.com;still-in-string');
COMMIT;

SELECT id FROM users
";
    let out = lower_generic("schema.sql", src);
    let names: Vec<Option<&str>> = out
        .file_model
        .functions
        .iter()
        .map(|f| f.name.as_deref())
        .collect();
    assert_eq!(
        names,
        [
            Some("users"),
            Some("public.orders"),
            Some("scratch"),
            Some("cache"),
            Some("analytics.monthly_totals"),
            Some("mv_sales"),
            Some("public.idx_users_email"),
            None,
            Some("mood"),
            None,
            None,
            None,
            None,
        ]
    );
    assert_eq!(out.file_model.functions[0].line, 1);
    assert_eq!(out.file_model.functions[0].end_line, 4);
    assert_eq!(out.file_model.functions[0].source_lines, 4);
    assert_eq!(out.file_model.functions.last().unwrap().line, 22);
    assert_eq!(out.file_model.functions.last().unwrap().end_line, 22);

    let quoted = lower_generic("quoted.sql", "CREATE TABLE \"public\".\"Users\" (id INT);");
    assert_eq!(
        quoted.file_model.functions[0].name.as_deref(),
        Some("public.Users")
    );
}

#[test]
fn sql_decision_points_cognitive_complexity_and_duplication_tokens() {
    let src = "\
SELECT id,
       ROW_NUMBER() OVER (PARTITION BY dept ORDER BY salary DESC) AS rn,
       COUNT(*) AS total
FROM employees
GROUP BY id, dept, salary;

SELECT CASE
           WHEN score >= 90 THEN 'A'
           WHEN score >= 80 THEN 'B'
           ELSE 'C'
       END AS grade
FROM results
WHERE created_at BETWEEN '2026-01-01' AND '2026-12-31'
  AND active = true
  OR override = true;

SELECT a.id
FROM a
INNER JOIN b ON a.id = b.a_id
LEFT OUTER JOIN c ON b.id = c.b_id
UNION ALL
SELECT d.id FROM d
INTERSECT
SELECT e.id FROM e
EXCEPT
SELECT f.id FROM f;

SELECT id FROM a WHERE x IN (SELECT id FROM b) AND y IN (SELECT id FROM c);

SELECT id FROM a WHERE x IN (SELECT id FROM b WHERE z IN (SELECT id FROM c WHERE k = 1 AND m = 2));
";
    let out = lower_generic("metrics.sql", src);
    let scores: Vec<(u32, u32)> = out
        .file_model
        .functions
        .iter()
        .map(|f| (f.decision_points, f.cognitive_complexity))
        .collect();
    assert_eq!(scores, [(0, 0), (4, 4), (5, 5), (3, 3), (3, 6),]);
    assert!(scores[4].1 > scores[3].1);

    let create_or_replace =
        lower_generic("view.sql", "CREATE OR REPLACE VIEW v AS SELECT 1 FROM t;");
    assert_eq!(create_or_replace.file_model.functions[0].decision_points, 0);

    let upper_q = lower_generic("u.sql", "SELECT col_a FROM tbl_a WHERE col_a = 1;");
    let lower_q = lower_generic("l.sql", "select col_b from tbl_b where col_b = 2;");
    assert_eq!(
        upper_q.file_model.tokens.len(),
        lower_q.file_model.tokens.len()
    );
    for (u, l) in upper_q
        .file_model
        .tokens
        .iter()
        .zip(lower_q.file_model.tokens.iter())
    {
        assert_eq!(u.kind, l.kind);
        if u.kind == TokenKind::Other {
            assert_eq!(u.text_hash, l.text_hash);
        }
    }
    assert!(
        upper_q
            .file_model
            .tokens
            .iter()
            .any(|t| t.kind == TokenKind::Identifier)
    );
    assert!(
        upper_q
            .file_model
            .tokens
            .iter()
            .any(|t| t.kind == TokenKind::Literal)
    );
}

#[test]
fn sql_rejected_statement_keeps_token_metrics_and_does_not_affect_neighbors() {
    let src = "\
SELECT id
FROM orders
WHERE status = 'a'
   OR status = 'b'
   OR status = 'c';

THIS IS NOT VALID SQL SYNTAX AT ALL WHERE x = 1 AND y = 2;

SELECT dept
FROM employees
GROUP BY dept
HAVING dept = 'sales';
";
    let out = lower_generic("mixed.sql", src);
    assert_eq!(out.file_model.functions.len(), 3);
    assert_eq!(out.file_model.functions[1].line, 7);
    assert_eq!(out.file_model.functions[1].decision_points, 1);
    assert_eq!(out.file_model.functions[1].cognitive_complexity, 1);
    assert_eq!(out.parsed_statements.len(), 2);
    assert_eq!(
        out.rejected_statements,
        [RejectedSqlStatement {
            path: repo_path("mixed.sql"),
            line: 7,
            dialect: SqlDialect::Generic,
        }]
    );
    let flagged: Vec<(u32, &str)> = out
        .file_model
        .flagged_lines
        .iter()
        .map(|f| (f.line, f.rule.as_str()))
        .collect();
    assert_eq!(
        flagged,
        [
            (3, "or-chain-collapses-to-in"),
            (12, "having-without-aggregate"),
        ]
    );
}

#[test]
fn sql_flagged_line_rules_hits_and_near_misses() {
    let rules: Vec<&str> = flagged_line_rules().collect();
    assert_eq!(
        rules,
        ["or-chain-collapses-to-in", "having-without-aggregate"]
    );

    let or_hits = [
        (
            "SELECT id\nFROM t\nWHERE status = 'a'\n   OR status = 'b'\n   OR status = 'c';\n",
            3u32,
        ),
        (
            "SELECT id\nFROM t\nWHERE 1 = code OR 2 = code OR 3 = code;\n",
            3u32,
        ),
        (
            "SELECT id\nFROM t\nWHERE t.kind = -1 OR t.kind = 0 OR t.kind = 1;\n",
            3u32,
        ),
        (
            "UPDATE t\nSET active = false\nWHERE role = 'guest' OR role = 'trial' OR role = 'demo';\n",
            3u32,
        ),
    ];
    for (src, expected_line) in or_hits {
        let out = lower_generic("or_hit.sql", src);
        let matches: Vec<u32> = out
            .file_model
            .flagged_lines
            .iter()
            .filter(|f| f.rule == "or-chain-collapses-to-in")
            .map(|f| f.line)
            .collect();
        assert_eq!(matches, [expected_line], "expected hit on:\n{src}");
    }

    let or_misses = [
        "SELECT id FROM t WHERE status = 'a' OR status = 'b';",
        "SELECT id FROM t WHERE a = 1 OR b = 2 OR c = 3;",
        "SELECT id FROM t WHERE a = 1 OR a = 2 OR b = 3;",
        "SELECT id FROM t WHERE a = b OR a = c OR a = d;",
        "SELECT id FROM t WHERE a = f() OR a = g() OR a = h();",
        "SELECT id FROM t WHERE a = $1 OR a = $2 OR a = $3;",
        "SELECT id FROM t WHERE a = NULL OR a = NULL OR a = NULL;",
        "-- WHERE a = 1 OR a = 2 OR a = 3\nSELECT 'a = 1 OR a = 2 OR a = 3' FROM t;",
    ];
    for src in or_misses {
        let out = lower_generic("or_miss.sql", src);
        assert!(
            out.file_model.flagged_lines.is_empty(),
            "expected no flagged lines for:\n{src}\ngot: {:?}",
            out.file_model.flagged_lines
        );
    }

    let having_multiline = "\
SELECT dept
FROM emp
GROUP BY dept
HAVING
    dept = 'sales';
";
    let having_out = lower_generic("having_hit.sql", having_multiline);
    assert_eq!(having_out.file_model.flagged_lines.len(), 1);
    assert_eq!(having_out.file_model.flagged_lines[0].line, 4);
    assert_eq!(
        having_out.file_model.flagged_lines[0].rule,
        "having-without-aggregate"
    );

    let having_misses = [
        "SELECT dept FROM emp GROUP BY dept HAVING COUNT(*) > 5;",
        "SELECT dept FROM emp GROUP BY dept HAVING SUM(salary) > 100 AND dept = 'eng';",
        "SELECT dept FROM emp GROUP BY dept HAVING COALESCE(AVG(salary), 0) > 50;",
        "SELECT dept FROM emp GROUP BY dept HAVING ARRAY_AGG(id) IS NOT NULL;",
        "SELECT dept FROM emp GROUP BY dept HAVING GROUP_CONCAT(name) LIKE '%a%';",
        "SELECT dept FROM emp GROUP BY dept HAVING custom_agg(DISTINCT id) > 1;",
        "-- HAVING dept = 'sales'\nSELECT 'HAVING dept = 1' FROM emp;",
    ];
    for src in having_misses {
        let out = lower_generic("having_miss.sql", src);
        assert!(
            out.file_model.flagged_lines.is_empty(),
            "expected no flagged lines for:\n{src}\ngot: {:?}",
            out.file_model.flagged_lines
        );
    }
}

#[test]
fn sql_source_set_classification_and_templated_sql() {
    let dump_headers = [
        "-- PostgreSQL database dump\nCREATE TABLE a (id INT);\n",
        "-- Dumped by pg_dump version 16.0\nCREATE TABLE a (id INT);\n",
        "-- MySQL dump 10.13\nCREATE TABLE a (id INT);\n",
        "-- MariaDB database dump\nCREATE TABLE a (id INT);\n",
        "-- MariaDB dump 10.19\nCREATE TABLE a (id INT);\n",
    ];
    for src in dump_headers {
        let out = lower_generic("db/dump.sql", src);
        assert_eq!(out.file_model.source_set, SourceSet::Generated);
    }

    let late_dump = format!(
        "{}-- PostgreSQL database dump\nSELECT 1;\n",
        "\n".repeat(25)
    );
    assert_eq!(
        lower_generic("db/late.sql", &late_dump)
            .file_model
            .source_set,
        SourceSet::Production
    );

    for marker in [
        "CREATE TABLE a (id INT);\n--> statement-breakpoint\nCREATE TABLE b (id INT);\n",
        "-- atlas:txmode none\nCREATE TABLE a (id INT);\n",
        "-- atlas:nolint DS103\nDROP TABLE a;\n",
    ] {
        assert_eq!(
            lower_generic("migrations/001.sql", marker)
                .file_model
                .source_set,
            SourceSet::Generated
        );
    }

    for (path, expected) in [
        (
            "prisma/migrations/20260101_init/migration.sql",
            SourceSet::Generated,
        ),
        (
            "apps/web/prisma/migrations/001/migration.sql",
            SourceSet::Generated,
        ),
        ("drizzle/0000_init.sql", SourceSet::Generated),
        ("packages/db/drizzle/0001_add.sql", SourceSet::Generated),
        ("migrations/001_init.sql", SourceSet::Production),
        ("db/migrations/002_users.sql", SourceSet::Production),
        ("test/seed.sql", SourceSet::Test),
        ("db/tests/queries.sql", SourceSet::Test),
        ("sql/fixtures/users.sql", SourceSet::Test),
        ("sql/test.sql", SourceSet::Production),
    ] {
        let out = lower_generic(path, "SELECT 1;\n");
        assert_eq!(out.file_model.source_set, expected, "path {path}");
    }

    for templated in [
        "SELECT * FROM {{ ref('users') }};",
        "{% if target.name == 'prod' %}SELECT 1;{% endif %}",
        "SELECT * FROM <%= table_name %>;",
    ] {
        let err = Sql::new()
            .lower(&repo_path("models/m.sql"), templated)
            .expect_err("expected templated SQL to be rejected");
        assert!(
            matches!(err, LowerError::Unparsable { ref message } if message == "templated SQL"),
            "unexpected error for {templated}: {err:?}"
        );
    }

    let safe_braces = "\
-- {{ comment template }}
/* {% block comment %} */
SELECT '{{ single_quoted }}',
       \"{{ double_quoted }}\",
       $$ {{ dollar_quoted }} $$
FROM items;
";
    let safe_out = lower_dialect(SqlDialect::Postgresql, "safe.sql", safe_braces);
    assert_eq!(safe_out.file_model.functions.len(), 1);

    let safe_backtick = "SELECT `{{ col }}` FROM items;";
    let mysql_safe = lower_dialect(SqlDialect::Mysql, "safe.sql", safe_backtick);
    assert_eq!(mysql_safe.file_model.functions.len(), 1);
}

fn fn_summary(out: &SqlLowerOutput) -> Vec<(Option<&str>, u32, u32, u32, u32)> {
    out.file_model
        .functions
        .iter()
        .map(|f| {
            (
                f.name.as_deref(),
                f.line,
                f.end_line,
                f.decision_points,
                f.cognitive_complexity,
            )
        })
        .collect()
}

#[test]
fn postgresql_dialect_routines_and_backslash_commands() {
    let src = "\
\\set ON_ERROR_STOP on
\\echo 'deploying function'

CREATE OR REPLACE FUNCTION public.compute_tier(p_total INT, p_active BOOLEAN)
RETURNS TEXT
LANGUAGE plpgsql
AS $$
-- comment inside body

DECLARE
    v_i INT := 0;
BEGIN
    IF p_active AND p_total > 100 THEN
        IF p_total > 500 THEN
            RETURN 'platinum';
        END IF;
        RETURN 'gold';
    ELSIF p_total > 50 THEN
        WHILE v_i < 3 LOOP
            v_i := v_i + 1;
        END LOOP;
        RETURN 'silver';
    END IF;
    FOR v_i IN 1..2 LOOP
        PERFORM 1;
    END LOOP;
    RETURN 'bronze';
EXCEPTION
    WHEN division_by_zero THEN
        RETURN 'error';
END;
$$;

DO $block$
BEGIN
    CREATE TEMP TABLE IF NOT EXISTS scratch_pg (id INT);
    PERFORM id FROM users FOR UPDATE;
    IF EXISTS (SELECT 1 FROM users) THEN
        RAISE NOTICE 'ok';
    END IF;
    DROP TABLE IF EXISTS scratch_pg;
END;
$block$;

SELECT id::int, name FROM users WHERE code ~ '^A' AND active;
";
    let out = lower_dialect(SqlDialect::Postgresql, "pg/routines.sql", src);
    assert_eq!(
        fn_summary(&out),
        [
            (Some("public.compute_tier"), 4, 32, 7, 10),
            (None, 34, 43, 2, 3),
            (None, 45, 45, 1, 1),
        ]
    );
    assert_eq!(out.file_model.functions[0].source_lines, 27);
    assert_eq!(out.parsed_statements.len(), 2);
    assert_eq!(
        out.rejected_statements,
        [RejectedSqlStatement {
            path: repo_path("pg/routines.sql"),
            line: 34,
            dialect: SqlDialect::Postgresql,
        }]
    );
    assert_eq!(out.file_model.source_lines, 40);
}

#[test]
fn mysql_dialect_delimiters_routines_and_syntax() {
    let src = "\
# mysql hash comment
DELIMITER //
CREATE DEFINER = 'admin'@'localhost' PROCEDURE refresh_stats(IN p_limit INT)
BEGIN
    DECLARE v_done INT DEFAULT 0;
    DECLARE CONTINUE HANDLER FOR NOT FOUND SET v_done = 1;
    IF p_limit > 0 THEN
        WHILE v_done = 0 DO
            INSERT INTO stats SET total = 1;
            SET v_done = 1;
        END WHILE;
    ELSEIF p_limit = 0 THEN
        DELETE FROM stats WHERE total = 0;
    END IF;
END //
DELIMITER ;

SELECT `id`, `name` FROM `users` LIMIT 10, 20;
";
    let out = lower_dialect(SqlDialect::Mysql, "mysql/proc.sql", src);
    assert_eq!(
        fn_summary(&out),
        [(Some("refresh_stats"), 3, 15, 4, 5), (None, 18, 18, 0, 0),]
    );
    assert_eq!(out.parsed_statements.len(), 1);
    assert_eq!(
        out.rejected_statements,
        [RejectedSqlStatement {
            path: repo_path("mysql/proc.sql"),
            line: 3,
            dialect: SqlDialect::Mysql,
        }]
    );
}

#[test]
fn sqlite_dialect_untyped_tables_and_triggers() {
    let src = "\
CREATE TABLE notes (id, body);

CREATE TEMP TRIGGER notes_after_insert
AFTER INSERT ON notes
FOR EACH ROW
BEGIN
    UPDATE audit
    SET label = CASE
        WHEN NEW.body GLOB 'err*' THEN 'error'
        ELSE 'ok'
    END
    WHERE note_id = NEW.id;
    INSERT OR REPLACE INTO [search_index] VALUES (NEW.id, NEW.body);
END;

SELECT [id] FROM notes WHERE body GLOB 'a*';
";
    let out = lower_dialect(SqlDialect::Sqlite, "sqlite/app.sql", src);
    assert_eq!(
        fn_summary(&out),
        [
            (Some("notes"), 1, 1, 0, 0),
            (Some("notes_after_insert"), 3, 14, 1, 1),
            (None, 16, 16, 0, 0),
        ]
    );
    assert_eq!(out.parsed_statements.len(), 3);
    assert!(out.rejected_statements.is_empty());
}

#[test]
fn duckdb_dialect_macros_dot_commands_and_syntax() {
    let src = "\
.mode box
.headers on
.timer on

CREATE OR REPLACE TEMP MACRO add_one(x) AS x + 1;

CREATE MACRO analytics.top_sales(min_amt) AS TABLE
SELECT * EXCLUDE (internal_notes),
       {'nested': {'k': 1}} AS meta,
FROM sales
WHERE amount > min_amt AND active = true
ORDER BY ALL;

FROM sales SELECT id, amount;
";
    let out = lower_dialect(SqlDialect::Duckdb, "duck/report.sql", src);
    assert_eq!(
        fn_summary(&out),
        [
            (Some("add_one"), 5, 5, 0, 0),
            (Some("analytics.top_sales"), 7, 12, 1, 1),
            (None, 14, 14, 0, 0),
        ]
    );
    assert_eq!(out.parsed_statements.len(), 3);
    assert!(out.rejected_statements.is_empty());
    assert_eq!(out.file_model.source_lines, 11);
}
