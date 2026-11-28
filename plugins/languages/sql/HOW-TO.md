# How to use and change the SQL Language plugin

## Call the plugin

```rust
use codeint_lang_sql::{Sql, SqlDialect};
use codeint_model::{FileScopeArtifact, LanguagePlugin, RepoPath};

let plugin = Sql::new();
let path = RepoPath::new("migrations/001_init.sql")?;
let model = plugin.lower(&path, "CREATE TABLE users (id INT PRIMARY KEY);\n", Some("postgresql"))?;
let artifacts = vec![FileScopeArtifact {
    path: path.to_string(),
    source_set: model.source_set,
    artifact: model.scope_artifact.unwrap_or_default(),
}];
let scope = plugin.analyze_scope("", Some("postgresql"), artifacts)?;
```

- `plugins/languages/sql` (`codeint-lang-sql`, plugin name `sql`) is a built-in Language plugin for `.sql` files under `#![forbid(unsafe_code)]`.
- It lowers SQL in two layers via `sqlparser` ([ADR 0018](../../../docs/adr/0018-sql-through-sqlparser-in-two-layers.md)).
- `has_imports` is `false`, `has_hazard_rules` is `true`, `has_scope_analysis` is `true`, and `known_framework_profiles` is empty.
- Call `Sql::with_dialect(SqlDialect::...)` and `Sql::lower_with_details` when you need `SqlLowerOutput` (`file_model`, `parsed_statements`, and `rejected_statements`) directly in Rust.

## Dialects and lowering

- `known_dialects` returns `["postgresql", "mysql", "sqlite", "duckdb"]`.
- Passing `None` to `lower` uses the `Sql` instance's default Dialect (`SqlDialect::Generic` for `Sql::new()`).
- Passing an unknown Dialect name returns `LowerError::UnsupportedFile` from `lower` and `LanguagePluginError::Call` from `analyze_scope`.
- `lower` returns `LowerError::UnsupportedFile` for non-`.sql` paths and `LowerError::Unparsable` for templated SQL containing `{{`, `{%`, `%}`, `<%`, or `%>` outside comments and string literals.
- `src/statements.rs` splits top-level statements and maps each statement to one `Function` entry with decision points and cognitive complexity.
- `src/parser.rs` parses each statement individually with `sqlparser` and skips statements over 12,000 non-whitespace tokens; unparsed statements record a `rejected-statement` `FileDiagnostic` instead of failing the file.

## Source sets

- `src/source_set.rs` classifies a `.sql` file in this priority order:
  1. `Generated` when the first 20 lines carry a database dump header or both `code generated` and `do not edit` in `--` comments, when any line starts with a migration marker (`--> statement-breakpoint`, `-- atlas:txmode`, `-- atlas:nolint`), or when the path is under `drizzle/` or `prisma/migrations/`.
  2. `Test` when the filename ends with `.test.sql` or starts with `test_` and ends with `.sql`, or when any folder segment is `test`, `tests`, `fixtures`, or `testdata`.
  3. `Production` otherwise.

## Flagged lines and Hazard lines

- `src/flagged.rs` implements two Flagged line rules returned by `flagged_line_rules()`:
  - `or-chain-collapses-to-in`
  - `having-without-aggregate`
- `src/hazards.rs` exports `SQL_HAZARD_RULES` and `hazard_line_rules()` with 32 Hazard line rules:
  - 27 per-statement rules (`select-star`, `leading-wildcard-like`, `large-offset`, `tables-without-join-condition`, `distinct-with-order-by`, `subquery-in-select-list`, `function-on-filter-column`, `not-in-subquery`, `union-without-all`, `select-without-filter-or-limit`, `count-without-filter`, `order-by-random`, `distinct-over-join`, `table-read-twice`, `subquery-refers-to-outer-table`, `long-in-list`, `deep-subquery-nesting`, `join-without-aliases`, `column-position`, `update-without-filter`, `delete-without-filter`, `truncate`, `drop`, `grant-or-revoke`, `always-true-condition`, `sql-built-at-run-time`, `plain-text-secret`).
  - 5 cross-file schema rules evaluated in `analyze_scope` (also callable directly via `schema::collect_schema_hazards` and listed by `schema::schema_hazard_rules()`): `unknown-column`, `text-compared-to-number`, `filter-on-unindexed-column`, `order-by-unindexed-column`, and `join-on-unindexed-column`.

## Cross-file scope analysis and `SQL1` codec

- During `lower`, `schema::encode_file_artifact` serializes `is_down_migration`, extracted DDL operations, query scope trees, and deferred schema checks into `FileModel::scope_artifact` using the binary `SQL1` codec in `src/schema/codec.rs`.
- `is_down_migration` is `true` for files named `down.sql` or ending with `.down.sql` (case-insensitive).
- `analyze_scope` (`src/schema/artifact.rs`) sorts incoming `FileScopeArtifact` entries by `path`, decodes each `SQL1` payload (returning `LanguagePluginError::Call` on invalid bytes), and replays DDL operations from every file that is neither `SourceSet::Vendored` nor a down migration into `SqlSchema`.
- When the resulting `SqlSchema` is non-empty, `analyze_scope` evaluates all deferred schema checks against it and returns `ScopeAnalysis` with per-file `extra_hazards` and one `sql-schema` `ScopeSummary` containing `dialect` and `tables`.

## Reference ranges

- `reference_ranges` returns the 10th to 90th percentile bands from the stratified 77-repository SQL panel under [ADR 0035](../../../docs/adr/0035-stratified-100-repository-reference-panels.md).
- Its source label is `codeint stratified panel of 77 SQL repositories, ADR 0035, codeint 0.8.0`.
