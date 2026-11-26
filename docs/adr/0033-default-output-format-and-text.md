---
status: accepted
---

# Configurable default output format and concise `text` output

## Context

`codeint` prints TOON (`toon`) by default on `check`, `diff`, `trend`, `report`, `impact`, `trace`, `churn`, and `coupling`.
TOON works well for coding agents and token budgets, but developers running `codeint` interactively in a terminal want a concise, scannable human-readable summary without passing `--format` on every command.
At the same time, `codeint report --format text` previously mapped `text` silently to TOON instead of rendering human text.
Issue #533 asks for a repository-level default output format setting and real concise `text` output for `check`, `diff`, `trend`, and `report`.

## Decision

**Keep `toon` as the built-in default on terminals and pipes alike, add a top-level `format` key to repository configuration, and render real monochrome summary-first `text` output on `check`, `diff`, `trend`, and `report`.**

### Built-in default and configuration key

- `toon` stays the built-in default on interactive terminals and pipes alike; codeint never switches output formats silently based on TTY detection.
- Repository configuration (`codeint.json`, `codeint.jsonc`, `.codeint/config.json`, or `.codeint/config.jsonc`) accepts a top-level `format` key with values `"toon"`, `"text"`, `"json"`, `"sarif"`, or `"markdown"`, defaulting to `"toon"` when omitted.
- CLI `--format` always overrides the configured `format`.
- `codeint config --resolved` reports the effective `format` setting and its provenance (`default` or `file`).
- `codeint init` leaves `format` omitted from the generated starter `codeint.jsonc`.
- Effective configuration serialization excludes `format` unconditionally because `format` only controls output rendering and never affects commit analysis, so changing `format` in repository configuration never splits `ResultCache` keys.

### Command scope and unsupported formats

- Configured `format` applies to the eight analysis and report commands: `check`, `diff`, `trend`, `report`, `impact`, `trace`, `churn`, and `coupling`.
- Utility commands (`doctor`, `guard`, `config`, `ratchet`, `baseline`, `agent`, `plugin test`), `schema`, and MCP tools keep their own defaults and ignore configured `format`.
- When one of the eight analysis and report commands does not support the configured `format` (for example `"sarif"` on `trend`, `churn`, `coupling`, `impact`, or `trace`, or `"markdown"` on `impact` or `trace`), codeint stops with exit code 2 and names the command, the configured format, and the supported formats.
- Passing a supported CLI `--format` still succeeds even when the configured `format` is unsupported for that command.
- When `--format` is omitted on `codeint report`, codeint reads `format` from the current repository's configuration when run inside a Git working tree (failing with exit code 2 on an invalid configuration), and falls back to built-in `toon` when run outside any Git repository or inside a bare Git repository.

### Concise monochrome `text` output

- `check --format text`, `diff --format text`, `trend --format text`, and `report --format text` render plain monochrome text with zero ANSI escape sequences and zero new dependencies, matching the eleven commands that already support `--format text`.
- `codeint report` accepts `--all` so `codeint report <input> --format text --all` expands text output identically to `codeint check --all --format text` and `codeint diff --all --format text` (listing every Finding, passing Workspace packages, and resolved diff Findings).
- `check --format text` (and `report --format text` on a check JSON report) leads with a `PASS` or `FAIL` status header (including Budget, Ratchet, and Baseline hidden counts when active), followed by failure lists, aligned per-Source-set Metric tables (with Reference ranges when single-language), compact per-language summaries when multiple languages are measured, Workspace package lines only when a package fails a limit or `--all` is passed (including any failing non-core Metric ratios on `[FAIL]` lines), up to 5 worst Findings per non-zero Metric (full list with `--all`), and concise notes when present.
- `diff --format text` (and `report --format text` on a diff JSON report) leads with a `PASS` or `FAIL` status header against the base ref (including Regression, per-file, Budget, new-file Budget, and Ratchet counts when present), followed by failure lists, aligned per-Source-set comparison tables, compact one-line listings of `new` Findings (capped at 5 without `--all`), and full `resolved` Findings only when `--all` is passed.
- `trend --format text` prints an aligned per-Source-set commit table with short 7-character commit hashes, UTC dates, file counts, and one column per Metric ratio, oldest first.

## Considered options

- **Switch the built-in default automatically when stdout is a TTY.**
  Rejected: silent format changes between terminals, pipes, CI logs, and agent shells make output unpredictable and break scripts that capture terminal output.
- **Introduce a new format name such as `human` or `pretty` alongside `text`.**
  Rejected: eleven commands already use `--format text` for plain monochrome human-readable output, and adding a second name would fragment the CLI.
- **Apply configured `format` to utility commands (`doctor`, `guard`, `config`, `schema`) and MCP tools.**
  Rejected: `schema` and MCP tools have machine contracts, and utility commands have their own `text` default that does not accept `sarif` or `markdown`.
- **Add ANSI colors or terminal table crates.**
  Rejected: plain monochrome text stays clean across logs, CI, diffs, and pipes with zero added dependencies.

## Consequences

- Repositories without a `format` key keep exact existing TOON output on every command.
- Developers who prefer human-readable terminal summaries can set `"format": "text"` once in `codeint.jsonc` while CI workflows and agents continue passing explicit `--format json`, `--format toon`, or `--format sarif`.
- `codeint report --format text` now renders real monochrome text matching `codeint check --format text` and `codeint diff --format text` byte for byte.
