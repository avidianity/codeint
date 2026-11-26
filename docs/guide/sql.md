# SQL

codeint ships a built-in `sql` Language plugin that measures every `.sql` file with no configuration required.
It reads SQL in two layers using `sqlparser` ([ADR 0018](../adr/0018-sql-through-sqlparser-in-two-layers.md)): a tokenizer layer for statements, functions, complexity, and Duplication, and a per-statement parser layer for Flagged lines, Hazard lines, and SQL schemas.
The `sql` plugin claims the `.sql` extension only; SQL inside string literals of another language stays with that language.

## Generic SQL

With no Dialect configuration, codeint reads every `.sql` file as generic SQL and measures Erosion, Cognitive complexity, Duplication, Verbosity, and Hazards.

- **Statements as functions**: each top-level statement counts as one function spanning its first and last source line.
  Blank lines and comment-only lines stay out of source lines, as in every other language.
- **Object names**: a `CREATE` statement for a `TABLE`, `VIEW`, `INDEX`, `FUNCTION`, `PROCEDURE`, `TRIGGER`, `TYPE`, or `MACRO` takes the object's name, keeping schema prefixes such as `analytics.monthly_totals`.
  Other statements, such as standalone `SELECT` or `UPDATE` statements, have no name.
- **Decision points**: each `WHEN` arm of a `CASE`, each `AND` and `OR` in a condition (excluding `BETWEEN ... AND`, transaction `AND CHAIN`, and `CREATE OR REPLACE`), each `JOIN` (including `STRAIGHT_JOIN`), each subquery (`(SELECT ...)` or `(WITH ...)`), and each set operation (`UNION`, `INTERSECT`, or `EXCEPT`, excluding wildcard `EXCEPT`) adds `1`.
  Window functions (`OVER (...)`) and `GROUP BY` add `0`.
- **Cognitive complexity**: nesting depth is subquery depth plus block depth inside a stored routine.
  Each decision point adds `1 + nesting depth`.
- **Duplication**: exact and near-clone detection uses SQL tokens without comments, normalizing identifiers and literals while preserving SQL keywords.
- **Flagged line rules**: two AST-only rules feed Verbosity:
  - `or-chain-collapses-to-in`: three or more `OR` equality comparisons of the same column against constant literals.
  - `having-without-aggregate`: a `HAVING` condition that contains no aggregate function.

## Source sets

After checking any `source_sets` globs in `codeint.jsonc` or `codeint.json` (which always win, in the order `vendored`, `generated`, `test`, `production`) and core Vendored and Generated rules, the `sql` plugin classifies each `.sql` file as follows:

- **Generated**:
  - A database dump header in the first 20 lines: `-- PostgreSQL database dump`, `-- Dumped by pg_dump`, `-- MySQL dump` (followed by a space), `-- MariaDB database dump`, or `-- MariaDB dump` (followed by a space).
  - A `-- Code generated` header with `DO NOT EDIT.` (case-insensitive) in the first 20 lines.
  - A migration tool marker anywhere in the file: `--> statement-breakpoint`, `-- atlas:txmode`, or `-- atlas:nolint`.
  - Any file inside a `prisma/migrations/` or `drizzle/` folder.
- **Test**: any file under a `test`, `tests`, `fixtures`, or `testdata` folder at any depth, or any file named `*.test.sql` or `test_*.sql`.
- **Production**: every other `.sql` file, including hand-written migrations.

Hand-written migrations are measured as Production so complex or destructive migrations receive every Metric and every Hazard line rule.
Generated SQL shows its Metrics and Hazards under the `generated` Source set so dumps and tool-written migrations never distort Production ratios.

## Templated SQL and rejected statements

- **Templated SQL**: codeint skips any `.sql` file containing `{{`, `{%`, `%}`, `<%`, or `%>` outside SQL strings and comments.
  The file appears under `skipped` with reason `templated SQL`, so template files such as dbt models are not measured as raw SQL.
- **Rejected statements**: because the parser reads one statement at a time, a single statement that the parser cannot parse never removes the rest of the file.
  A rejected statement still counts in every token-layer Metric (source lines, Erosion, Cognitive complexity, and Duplication), receives no Flagged line or Hazard line rules, and is listed under `rejected_statements` with its `path`, starting `line`, and `dialect`.

## Absent import Metrics and Workspaces

SQL has no runtime imports, package manifests, or entry points.
Because the `sql` Language plugin reports no imports, Import cycles, Orphan files, Import boundaries, and Dependency hygiene are absent for SQL (never `0`), and reports include a one-line `import_metrics_note` explaining why.

In a monorepo with Workspace packages defined by other languages, each `.sql` file belongs to the Workspace package of its enclosing folder and counts in that package's SQL and repository totals.
Use the `languages` setting in `codeint.jsonc` or `codeint.json` to restrict `sql` to specific folders when needed.

## Dialects

Without configuration, every `.sql` file uses generic SQL.
Map repository-relative folders or file globs to a **Dialect** under `sql.dialects` in `codeint.jsonc` or `codeint.json`:

| Dialect | Covers |
| ------- | ------ |
| `postgresql` | PostgreSQL and PL/pgSQL routines |
| `mysql` | MySQL and MariaDB |
| `sqlite` | SQLite |
| `duckdb` | DuckDB |

Files matching no `sql.dialects` entry stay generic SQL.
ClickHouse, SQL Server, Oracle, BigQuery, and Snowflake stay generic SQL.
An unknown Dialect name, or one file matched by two entries with different Dialects, stops the run with exit code `2` and names the configuration key or conflicting patterns.
When `codeint init` detects `.sql` files, it adds a commented `sql.dialects` hint to the starter `codeint.jsonc`.

In a monorepo that mixes databases, assign each folder its own Dialect:

```jsonc
{
  "sql": {
    "dialects": [
      {
        "dialect": "postgresql",
        "include": ["services/billing/migrations", "services/billing/queries"],
      },
      {
        "dialect": "duckdb",
        "include": ["analytics/**/*.sql"],
      },
    ],
  },
}
```

### Routines and CLI commands

- **Stored routines**: a stored function, procedure, or trigger counts as one function including its whole body.
  Inside a routine, each `IF`, `ELSIF` or `ELSEIF`, loop (`WHILE`, `FOR`, `REPEAT`, `LOOP`), and error handler (`EXCEPTION`, `HANDLER`) adds `1` decision point and deepens Cognitive complexity block nesting.
- **PostgreSQL**: dollar-quoted bodies (`$$ ... $$` or `$tag$ ... $tag$`) on SQL and PL/pgSQL routines and `DO` blocks are tokenized again as SQL for complexity and Duplication.
  `psql` backslash commands (lines starting with `\`, outside `COPY ... FROM STDIN;` payloads) count as source lines and are not parsed as statements.
- **MySQL and MariaDB**: `DELIMITER` directives change the active statement delimiter until the next `DELIMITER` directive so routines with inner semicolons stay one function.
  The `DELIMITER` lines themselves are not statements.
- **SQLite**: `CREATE TRIGGER ... BEGIN ... END;` stays one function spanning its full body.
- **DuckDB**: `CREATE MACRO` counts as one function named after the macro.
  DuckDB CLI dot-commands (lines starting with `.` followed by a letter, such as `.mode` or `.read`) count as source lines and are not parsed as statements.

## SQL Schema

codeint builds an in-memory schema automatically from the repository's own SQL files, with no live database connection and no schema-path configuration.

- **Included statements**: `CREATE TABLE` (excluding `CREATE TEMP` / `CREATE TEMPORARY TABLE`), `CREATE INDEX`, `ALTER TABLE` (including added, modified, renamed, and dropped columns, constraints, and table renames), `DROP TABLE`, and `DROP INDEX`.
- **Included Source sets**: Production, Test, and Generated SQL (including tool-written migrations), never Vendored SQL; rollback down-migration files (`down.sql` or `*.down.sql`) are skipped from schema construction while still checked as SQL files.
- **Ordering**: files are applied in repository-relative path order, and statements in file order, so sequential migrations produce the final table, column, primary key, and index definitions.
- **Scopes**: codeint builds one schema per Workspace package and Dialect, plus one root schema per Dialect for `.sql` files outside every Workspace package.
  Generic SQL is its own Dialect scope.

Every `codeint check` report lists each scope's table count under `sql_schemas` (or under `## SQL schemas` in Markdown).
A scope showing `tables: 0` tells you that no `CREATE TABLE` definitions were found in that package and Dialect.

### Schema-backed Hazard line rules

Five of the 32 SQL Hazard line rules use the built schema:

- `filter-on-unindexed-column`
- `order-by-unindexed-column`
- `join-on-unindexed-column`
- `unknown-column`
- `text-compared-to-number`

A schema rule matches only when the schema knows the table and the column resolves unambiguously to one table.
When a table is not in the schema (such as an external table, CTE, or derived table) or when an unqualified column could belong to more than one table in scope, the schema rules stay silent so an incomplete schema never creates false Findings.

See [Hazards](./hazards.md) for how Hazards works and [docs/reference.md#sql](../reference.md#sql) for the complete 32-rule table.

## Reference ranges

Calibrated SQL Reference ranges come from a stratified panel of 50 current public SQL repositories ([ADR 0028](../adr/0028-stratified-reference-panels.md), source `codeint stratified panel of 50 SQL repositories, ADR 0028, codeint 0.6.0`):

| Metric | Reference range |
| ------ | --------------- |
| Erosion | 0.0% to 79.5% |
| Duplication | 0.1% to 90.9% |
| Verbosity | 0.0% to 0.0% |
| Import cycles | N/A |

`codeint check` displays these Reference ranges next to SQL values for Erosion, Duplication, and Verbosity; Import cycles stays absent because SQL has no runtime imports.
