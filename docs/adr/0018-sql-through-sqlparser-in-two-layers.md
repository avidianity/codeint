---
status: accepted
---

# The SQL Language plugin reads SQL with sqlparser in two layers

The Owner decided this on 2026-11-24 while adding SQL support.

## Context

The SQL Language plugin needs tokens with line numbers for every Metric, and parsed statements for its Hazard line and Flagged line rules.
The PHP, Dart, and Python Language plugins use hand-written lexers, which is enough for their rules.
The SQL rules compare tables, columns, joins, and subqueries, across several Dialects, which needs a real SQL parser.
SQL also differs from every earlier language: it has no runtime imports, no package manifest, and few functions.
Checked on 2026-11-24: `sqlparser` 0.64.0, published 2026-11-09 on crates.io, Apache-2.0, with `log` as its only mandatory dependency.
It knows 16 dialects, including PostgreSQL, MySQL, SQLite, and DuckDB.
Its tokenizer gives a line and a column for every token and keeps comments.
Its parser fails a whole input on the first statement that it cannot parse.
Its statement positions are empty for about 100 statement kinds.
It keeps the body of a PostgreSQL function as an unparsed string, and it rejects MySQL `CREATE FUNCTION`.
Every release changes its syntax tree; 0.64.0 lists 19 breaking changes, and releases arrive every 2 to 4 months.

## Decision

**The SQL Language plugin uses the `sqlparser` tokenizer for every file, and its parser for one statement at a time.**
Statements, functions, decision points, Duplication, and Verbosity come from tokens.
The Hazard line and Flagged line rules read one parsed statement at a time.
A statement that the parser rejects still counts in every Metric that comes from tokens, gets no rules, and is listed in the report.
Each top-level statement is one function, named after the object it creates when it creates one.
A stored routine is one function that includes its whole body.
A decision point is each `WHEN` arm of a `CASE`, each `AND` and `OR` in a condition, each `JOIN`, each subquery, and each set operation such as `UNION`.
In a stored routine, each `IF`, `ELSIF`, loop, and error handler also counts.
Every decision point counts 1.
The plugin reads files with the `.sql` extension only.
The first Dialects are PostgreSQL, MySQL with MariaDB, SQLite, and DuckDB.
With no configuration, every file is read as generic SQL, and the configuration maps a folder pattern to one Dialect.
A Language plugin may state that its language has no imports.
Import cycles, Orphan files, and the three Dependency hygiene Metrics are then absent for that language, never 0.

## Considered options

- **The `sqlparser` parser alone.**
  Rejected: one statement that it cannot parse would remove a whole file from every Metric, and routine bodies in PostgreSQL and MySQL would have no measured complexity.
- **A hand-written lexer and rules on tokens.**
  Rejected: the rules need joins, subqueries, and column references, so this means writing a SQL parser for every Dialect.
- **Rules that match text.**
  Rejected: a match would not be exactly what the rule names.
- **Weights for joins, subqueries, and window functions.**
  Rejected: weights cannot be calibrated honestly, as ADR 0003 says.
- **Invent imports for SQL, from one file that includes another.**
  Rejected: few SQL files include another, and a 0 would claim a clean result for a question that does not apply.
- **Read SQL inside the strings of other languages.**
  Rejected: codeint assigns each file to one Language plugin by its extension.

## Consequences

- A repository with SQL files works with no configuration.
- The pin `sqlparser = "=0.64.0"` needs an upgrade by hand for each release, and each upgrade can change the rules' code.
- The conformance test of Language plugins no longer requires imports, a manifest, or a Workspace package from every language.
- SQL files belong to the Workspace package of the folder that holds them.
- Templated SQL, such as dbt models with template marks, is skipped and listed in the report.
- ClickHouse, SQL Server, Oracle, BigQuery, and Snowflake are read as generic SQL until each one is added and proven on public repositories.
- The decision points define Erosion and Cognitive complexity for SQL, so a change to them invalidates the SQL Reference range.
