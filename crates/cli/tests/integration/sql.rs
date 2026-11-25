use std::fs;
use std::path::Path;

use serde_json::{Value, json};

use crate::common::{
    assert_doctor_counts_language_files, assert_import_metrics_absent, check_json_with_config,
    finding_names, init_git_repo, init_git_repo_with, run_check_json as check_json,
    run_codeint_stdout as run_stdout, run_codeint_text,
};

const WORKSPACE_FILES: [(&str, &str); 3] = [
    (
        "package.json",
        r#"{"private":true,"workspaces":["packages/*"]}"#,
    ),
    ("packages/billing/package.json", r#"{"name":"billing"}"#),
    ("packages/analytics/package.json", r#"{"name":"analytics"}"#),
];

fn init_workspace_repo(extra: &[(&str, &str)]) -> tempfile::TempDir {
    init_git_repo_with(&WORKSPACE_FILES, extra)
}

fn assert_format_contains(dir: &Path, format: &str, needles: &[&str]) {
    let output = run_stdout(dir, &["check", "--format", format], 0);
    for needle in needles {
        assert!(
            output.contains(needle),
            "expected {needle:?} in {format} output: {output}"
        );
    }
}

fn dialects_config(entries: &[(&str, &[&str])], disabled_rules: &[&str]) -> String {
    let dialects: Vec<Value> = entries
        .iter()
        .map(|(dialect, include)| json!({ "dialect": dialect, "include": include }))
        .collect();
    if disabled_rules.is_empty() {
        json!({ "sql": { "dialects": dialects } }).to_string()
    } else {
        json!({
            "sql": { "dialects": dialects },
            "hazards": { "disabled_rules": disabled_rules }
        })
        .to_string()
    }
}

fn prod_hazard_findings(report: &Value) -> &[Value] {
    report["source_sets"]["production"]["hazards"]["findings"]
        .as_array()
        .expect("hazard findings")
}

fn has_rejected(report: &Value, path: &str, dialect: Option<&str>) -> bool {
    report["rejected_statements"]
        .as_array()
        .expect("rejected statements")
        .iter()
        .any(|entry| {
            entry["path"].as_str() == Some(path)
                && dialect.is_none_or(|d| entry["dialect"].as_str() == Some(d))
        })
}

fn repeated_report_query() -> &'static str {
    concat!(
        "SELECT o.customer_id, o.region, o.channel, o.currency, o.status, ",
        "SUM(o.subtotal), SUM(o.tax_amount), SUM(o.shipping_fee), SUM(o.discount_amount), COUNT(o.id)\n",
        "FROM orders AS o\n",
        "JOIN customers AS c ON c.id = o.customer_id\n",
        "WHERE o.status = 'completed' AND o.region = 'north' AND o.subtotal > 50 AND c.tier = 'gold'\n",
        "GROUP BY o.customer_id, o.region, o.channel, o.currency, o.status\n",
        "ORDER BY o.customer_id;\n",
    )
}

fn eroded_monthly_totals_view() -> &'static str {
    concat!(
        "CREATE VIEW monthly_totals AS\n",
        "SELECT o.id,\n",
        "  CASE\n",
        "    WHEN o.status = 'a' THEN 1\n",
        "    WHEN o.status = 'b' THEN 2\n",
        "    WHEN o.status = 'c' THEN 3\n",
        "    WHEN o.status = 'd' THEN 4\n",
        "    WHEN o.status = 'e' THEN 5\n",
        "    WHEN o.status = 'f' THEN 6\n",
        "    ELSE 0\n",
        "  END AS bucket\n",
        "FROM orders AS o\n",
        "JOIN accounts AS a ON a.id = o.account_id\n",
        "JOIN regions AS r ON r.id = a.region_id\n",
        "WHERE o.total > 0 AND o.active = 1 AND o.kind = 'web' AND a.tier = 'pro' AND r.enabled = 1;\n",
    )
}

#[test]
fn generic_sql_measures_metrics_object_names_absent_imports_workspaces_and_languages_filter() {
    let verbose_sql = concat!(
        "SELECT id, status\n",
        "FROM orders\n",
        "WHERE status = 'a' OR status = 'b' OR status = 'c' OR status = 'd';\n",
        "SELECT id\n",
        "FROM orders\n",
        "GROUP BY id\n",
        "HAVING id > 10;\n",
    );
    let repo = init_workspace_repo(&[
        (
            "packages/billing/queries/views.sql",
            eroded_monthly_totals_view(),
        ),
        ("packages/billing/queries/verbose.sql", verbose_sql),
        (
            "packages/billing/queries/dup_a.sql",
            repeated_report_query(),
        ),
        (
            "packages/analytics/queries/dup_b.sql",
            repeated_report_query(),
        ),
    ]);

    let report = check_json(repo.path());
    assert_eq!(report["files"], 4, "report: {report}");
    assert_import_metrics_absent(&report);

    let prod = &report["source_sets"]["production"];
    let eroded_names = finding_names(&prod["erosion"]["findings"]);
    assert!(
        eroded_names.contains(&"monthly_totals"),
        "expected monthly_totals in erosion findings: {report}"
    );
    assert!(
        prod["cognitive_complexity"]["total_cognitive_complexity"]
            .as_u64()
            .unwrap_or(0)
            > 0,
        "report: {report}"
    );
    assert!(
        prod["duplication"]["clone_lines"].as_u64().unwrap_or(0) > 0,
        "expected duplication clone lines: {report}"
    );
    let flagged_rules: Vec<&str> = prod["verbosity"]["findings"]
        .as_array()
        .expect("verbosity findings")
        .iter()
        .filter_map(|f| f["rule"].as_str())
        .collect();
    assert!(
        flagged_rules.contains(&"or-chain-collapses-to-in")
            && flagged_rules.contains(&"having-without-aggregate"),
        "expected SQL verbosity rules, got: {flagged_rules:?}"
    );

    let package_paths: Vec<&str> = prod["packages"]
        .as_array()
        .expect("packages")
        .iter()
        .filter_map(|p| p["path"].as_str())
        .collect();
    assert_eq!(
        package_paths,
        [".", "packages/analytics", "packages/billing"]
    );

    let filtered = check_json_with_config(
        repo.path(),
        r#"{"languages":[{"plugin":"sql","include":["packages/billing"],"exclude":["packages/billing/queries/dup_a.sql"]}]}"#,
    );
    assert_eq!(filtered["files"], 2, "filtered: {filtered}");
}

#[test]
fn sql_source_set_rules_and_overrides_classify_dumps_migrations_tests_and_production() {
    let repo = init_git_repo(&[
        (
            "dumps/pg.sql",
            "-- PostgreSQL database dump\nSELECT id FROM accounts WHERE id = 1;\n",
        ),
        (
            "dumps/mysql.sql",
            "-- MySQL dump 10.13\nSELECT id FROM accounts WHERE id = 1;\n",
        ),
        (
            "db/drizzle_marked.sql",
            "CREATE TABLE t1 (id INT);\n--> statement-breakpoint\nCREATE TABLE t2 (id INT);\n",
        ),
        ("drizzle/0001_init.sql", "CREATE TABLE t3 (id INT);\n"),
        (
            "db/atlas.sql",
            "-- atlas:txmode none\nCREATE TABLE t4 (id INT);\n",
        ),
        (
            "prisma/migrations/0001_init/migration.sql",
            "CREATE TABLE t5 (id INT);\n",
        ),
        (
            "migrations/0001_handwritten.sql",
            "CREATE TABLE users (id INT PRIMARY KEY);\n",
        ),
        ("test/seed.sql", "SELECT id FROM users WHERE id = 1;\n"),
        ("tests/verify.sql", "SELECT id FROM users WHERE id = 2;\n"),
        (
            "fixtures/sample.sql",
            "SELECT id FROM users WHERE id = 3;\n",
        ),
    ]);

    let report = check_json(repo.path());
    assert_eq!(report["source_sets"]["production"]["files"], 1, "{report}");
    assert_eq!(report["source_sets"]["generated"]["files"], 6, "{report}");
    assert_eq!(report["source_sets"]["test"]["files"], 3, "{report}");

    let overridden = check_json_with_config(
        repo.path(),
        r#"{"source_sets":{"production":["fixtures/**"]}}"#,
    );
    assert_eq!(
        overridden["source_sets"]["production"]["files"], 2,
        "{overridden}"
    );
    assert_eq!(
        overridden["source_sets"]["test"]["files"], 2,
        "{overridden}"
    );
}

#[test]
fn templated_sql_skipped_and_rejected_statements_render_in_all_formats() {
    let repo = init_git_repo(&[
        (
            "queries/template.sql",
            "SELECT id FROM {{ table_name }} WHERE active = 1;\n",
        ),
        (
            "queries/mixed.sql",
            "SELECT id FROM accounts WHERE id = 1;\nSELECT FROM ???;\n",
        ),
    ]);

    let json = check_json(repo.path());
    assert_eq!(json["files"], 1, "{json}");
    assert_eq!(json["skipped"][0]["path"], "queries/template.sql");
    assert_eq!(json["skipped"][0]["reason"], "templated SQL");
    assert_eq!(json["rejected_statements"][0]["path"], "queries/mixed.sql");
    assert_eq!(json["rejected_statements"][0]["line"], 2);
    assert_eq!(json["rejected_statements"][0]["dialect"], "generic SQL");

    assert_format_contains(
        repo.path(),
        "toon",
        &["templated SQL", "rejected_statements", "generic SQL"],
    );
    assert_format_contains(
        repo.path(),
        "markdown",
        &[
            "templated SQL",
            "## Rejected statements",
            "`queries/mixed.sql:2`",
            "`generic SQL`",
        ],
    );

    let sarif: Value =
        serde_json::from_str(&run_stdout(repo.path(), &["check", "--format", "sarif"], 0))
            .expect("valid sarif");
    assert!(sarif["runs"][0]["results"].is_array());
}

#[test]
fn sql_dialects_monorepo_handles_all_four_dialects_generic_fallback_and_doctor() {
    let pg_sql = concat!(
        "\\dt\n",
        "CREATE FUNCTION compute_tier(val INT) RETURNS INT AS $$\n",
        "BEGIN\n",
        "  IF val > 10 THEN\n",
        "    RETURN 2;\n",
        "  ELSIF val > 5 THEN\n",
        "    RETURN 1;\n",
        "  END IF;\n",
        "  RETURN 0;\n",
        "END;\n",
        "$$ LANGUAGE plpgsql;\n",
        "SELECT FROM ???;\n",
    );
    let mysql_sql = concat!(
        "DELIMITER //\n",
        "CREATE PROCEDURE refresh_totals()\n",
        "BEGIN\n",
        "  IF 1 = 1 THEN\n",
        "    SELECT id FROM orders WHERE id = 1;\n",
        "  END IF;\n",
        "END//\n",
        "DELIMITER ;\n",
    );
    let sqlite_sql = concat!(
        "CREATE TRIGGER audit_insert AFTER INSERT ON orders\n",
        "BEGIN\n",
        "  INSERT INTO audit_log(order_id) VALUES (NEW.id);\n",
        "END;\n",
    );
    let duckdb_sql = concat!(".mode csv\n", "CREATE MACRO double_val(x) AS x * 2;\n",);
    let generic_sql = "SELECT id FROM items WHERE id = 1;\n";

    let config = dialects_config(
        &[
            ("postgresql", &["pg/**"]),
            ("mysql", &["mysql/**"]),
            ("sqlite", &["sqlite/**"]),
            ("duckdb", &["duckdb/**"]),
        ],
        &[],
    );
    let repo = init_git_repo(&[
        ("codeint.json", config.as_str()),
        ("pg/routine.sql", pg_sql),
        ("mysql/proc.sql", mysql_sql),
        ("sqlite/trigger.sql", sqlite_sql),
        ("duckdb/macro.sql", duckdb_sql),
        ("shared/generic.sql", generic_sql),
    ]);

    let report = check_json(repo.path());
    assert_eq!(report["files"], 5, "{report}");
    assert!(
        has_rejected(&report, "pg/routine.sql", Some("postgresql")),
        "expected pg/routine.sql rejected with postgresql dialect: {report}"
    );
    assert!(
        !has_rejected(&report, "duckdb/macro.sql", None),
        "duckdb dot-command + macro should not be rejected: {report}"
    );
    assert!(
        !has_rejected(&report, "sqlite/trigger.sql", None),
        "sqlite trigger should not be rejected: {report}"
    );

    let dialects: Vec<&str> = report["sql_schemas"]
        .as_array()
        .expect("sql_schemas")
        .iter()
        .filter_map(|s| s["dialect"].as_str())
        .collect();
    assert_eq!(
        dialects,
        ["duckdb", "generic SQL", "mysql", "postgresql", "sqlite"]
    );

    assert_doctor_counts_language_files(repo.path(), "sql: 5");
}

#[test]
fn sql_dialects_errors_and_init_behavior() {
    let unknown_repo = init_git_repo(&[
        (
            "codeint.json",
            r#"{"sql":{"dialects":[{"dialect":"oracle","include":["db/**"]}]}}"#,
        ),
        ("db/query.sql", "SELECT 1;\n"),
    ]);
    let (_, unknown_stderr) = run_codeint_text(unknown_repo.path(), &["check"], 2);
    assert!(
        unknown_stderr.contains("postgresql")
            && unknown_stderr.contains("mysql")
            && unknown_stderr.contains("sqlite")
            && unknown_stderr.contains("duckdb"),
        "stderr: {unknown_stderr}"
    );

    let overlap_config = dialects_config(
        &[("postgresql", &["db/**"]), ("duckdb", &["db/analytics/**"])],
        &[],
    );
    let overlap_repo = init_git_repo(&[
        ("codeint.json", overlap_config.as_str()),
        ("db/analytics/report.sql", "SELECT 1;\n"),
    ]);
    let (_, overlap_stderr) = run_codeint_text(overlap_repo.path(), &["check"], 2);
    for expected in [
        "sql.dialects",
        "db/analytics/report.sql",
        "postgresql",
        "duckdb",
        "db/**",
        "db/analytics/**",
    ] {
        assert!(
            overlap_stderr.contains(expected),
            "expected {expected:?} in stderr: {overlap_stderr}"
        );
    }

    let with_sql = init_git_repo(&[("queries/a.sql", "SELECT 1;\n")]);
    run_stdout(with_sql.path(), &["init"], 0);
    let sql_config =
        fs::read_to_string(with_sql.path().join("codeint.jsonc")).expect("read codeint.jsonc");
    assert!(
        sql_config.contains("sql.dialects") && sql_config.contains("\"plugin\": \"sql\""),
        "config: {sql_config}"
    );
    assert!(
        !sql_config.contains("\"dialect\":"),
        "init must not guess a dialect: {sql_config}"
    );
    check_json(with_sql.path());

    let without_sql = init_git_repo(&[("src/app.ts", "export const x = 1;\n")]);
    run_stdout(without_sql.path(), &["init"], 0);
    let ts_config =
        fs::read_to_string(without_sql.path().join("codeint.jsonc")).expect("read codeint.jsonc");
    assert!(
        !ts_config.contains("sql.dialects"),
        "config without sql must not mention sql.dialects: {ts_config}"
    );
}

#[test]
fn sql_schema_pass_handles_migrations_source_sets_scopes_all_five_rules_and_disabled_rules() {
    let schema_dialects: &[(&str, &[&str])] = &[
        (
            "postgresql",
            &["packages/billing/pg/**", "packages/analytics/pg/**"],
        ),
        ("sqlite", &["packages/billing/sqlite/**"]),
        ("duckdb", &["packages/analytics/duckdb/**"]),
    ];
    let config = dialects_config(schema_dialects, &[]);
    let repo = init_workspace_repo(&[
        ("codeint.json", config.as_str()),
        (
            "packages/billing/pg/drizzle/001_create.sql",
            concat!(
                "CREATE TABLE customers (id INT PRIMARY KEY, code TEXT, email TEXT, legacy TEXT);\n",
                "CREATE TABLE invoices (id INT PRIMARY KEY, customer_id INT, status TEXT, total INT);\n",
                "CREATE TABLE temp_notes (id INT PRIMARY KEY);\n",
            ),
        ),
        (
            "packages/billing/pg/migrations/002_alter.sql",
            concat!(
                "ALTER TABLE customers DROP COLUMN legacy;\n",
                "CREATE INDEX idx_customers_email ON customers (email);\n",
                "DROP TABLE temp_notes;\n",
            ),
        ),
        (
            "packages/billing/pg/vendor/000_vendored.sql",
            "CREATE TABLE vendored_only (id INT PRIMARY KEY, name TEXT);\n",
        ),
        (
            "packages/billing/pg/queries.sql",
            concat!(
                "SELECT c.id FROM customers AS c WHERE c.code = 'abc';\n",
                "SELECT c.id FROM customers AS c ORDER BY c.code;\n",
                "SELECT c.id FROM customers AS c JOIN invoices AS i ON i.customer_id = c.id WHERE c.id = 1;\n",
                "SELECT c.legacy FROM customers AS c WHERE c.id = 1;\n",
                "SELECT c.id FROM customers AS c WHERE c.email = 42;\n",
                "SELECT v.missing_col FROM vendored_only AS v WHERE v.name = 'x';\n",
                "SELECT g.whatever FROM unknown_table AS g WHERE g.col = 1;\n",
                "SELECT c.id FROM customers AS c JOIN invoices AS i ON c.id = i.id WHERE id = 1;\n",
            ),
        ),
        (
            "packages/billing/sqlite/schema.sql",
            "CREATE TABLE local_cache (k TEXT PRIMARY KEY);\n",
        ),
        (
            "packages/analytics/pg/schema.sql",
            "CREATE TABLE events (id INT PRIMARY KEY);\nCREATE TABLE sessions (id INT PRIMARY KEY);\nCREATE TABLE metrics (id INT PRIMARY KEY);\n",
        ),
        (
            "packages/analytics/duckdb/schema.sql",
            "CREATE TABLE rollups (id INT PRIMARY KEY);\n",
        ),
    ]);

    let report = check_json(repo.path());
    let schemas: Vec<(Option<&str>, &str, u64)> = report["sql_schemas"]
        .as_array()
        .expect("sql_schemas")
        .iter()
        .map(|s| {
            (
                s.get("package").and_then(Value::as_str),
                s["dialect"].as_str().expect("dialect"),
                s["tables"].as_u64().expect("tables"),
            )
        })
        .collect();
    assert_eq!(
        schemas,
        [
            (Some("packages/analytics"), "duckdb", 1),
            (Some("packages/analytics"), "postgresql", 3),
            (Some("packages/billing"), "postgresql", 2),
            (Some("packages/billing"), "sqlite", 1),
        ],
        "report: {report}"
    );

    let query_hits: Vec<(u64, &str)> = prod_hazard_findings(&report)
        .iter()
        .filter(|f| f["path"] == "packages/billing/pg/queries.sql")
        .map(|f| {
            (
                f["line"].as_u64().expect("line"),
                f["rule"].as_str().expect("rule"),
            )
        })
        .collect();
    for expected in [
        (1, "filter-on-unindexed-column"),
        (2, "order-by-unindexed-column"),
        (3, "join-on-unindexed-column"),
        (4, "unknown-column"),
        (5, "text-compared-to-number"),
    ] {
        assert!(
            query_hits.contains(&expected),
            "expected {expected:?} in query_hits: {query_hits:?}"
        );
    }
    for silent_line in [6u64, 7, 8] {
        assert!(
            !query_hits.iter().any(|(line, rule)| {
                *line == silent_line
                    && matches!(
                        *rule,
                        "filter-on-unindexed-column"
                            | "order-by-unindexed-column"
                            | "join-on-unindexed-column"
                            | "unknown-column"
                            | "text-compared-to-number"
                    )
            }),
            "expected schema silence on line {silent_line}, got: {query_hits:?}"
        );
    }

    assert_format_contains(
        repo.path(),
        "markdown",
        &["## SQL schemas", "`packages/billing`"],
    );
    assert_format_contains(repo.path(), "toon", &["sql_schemas"]);

    fs::write(
        repo.path().join("codeint.json"),
        dialects_config(
            schema_dialects,
            &["unknown-column", "text-compared-to-number"],
        ),
    )
    .expect("update config");
    let disabled = check_json(repo.path());
    let disabled_rules: Vec<&str> = prod_hazard_findings(&disabled)
        .iter()
        .filter_map(|f| f["rule"].as_str())
        .collect();
    assert!(!disabled_rules.contains(&"unknown-column"));
    assert!(!disabled_rules.contains(&"text-compared-to-number"));
    assert!(disabled_rules.contains(&"filter-on-unindexed-column"));
}

#[test]
fn sql_schema_skips_down_migration_files_while_still_running_rules_on_them() {
    let repo = init_git_repo(&[
        (
            "migrations/001_init/up.sql",
            "CREATE TABLE accounts (id INT PRIMARY KEY, email TEXT NOT NULL);\n",
        ),
        ("migrations/001_init/down.sql", "DROP TABLE accounts;\n"),
        (
            "migrations/002_add_handle.up.sql",
            "ALTER TABLE accounts ADD COLUMN handle TEXT;\n",
        ),
        (
            "migrations/002_add_handle.down.sql",
            "ALTER TABLE accounts DROP COLUMN handle;\nDROP TABLE IF EXISTS legacy_audit;\n",
        ),
        (
            "queries/fetch.sql",
            "SELECT id, handle FROM accounts WHERE id = 1;\n",
        ),
    ]);

    let report = check_json(repo.path());
    assert_eq!(report["sql_schemas"][0]["tables"], 1, "report: {report}");

    let findings = prod_hazard_findings(&report);
    let query_findings: Vec<&Value> = findings
        .iter()
        .filter(|f| f["path"] == "queries/fetch.sql")
        .collect();
    assert!(
        query_findings.is_empty(),
        "down migrations must not drop forward schema columns: {query_findings:?}"
    );
    let down_drops: Vec<&str> = findings
        .iter()
        .filter(|f| f["rule"] == "drop")
        .filter_map(|f| f["path"].as_str())
        .collect();
    assert!(
        down_drops.contains(&"migrations/001_init/down.sql")
            && down_drops.contains(&"migrations/002_add_handle.down.sql"),
        "down migrations must still run statement hazard rules: {down_drops:?}"
    );
}
