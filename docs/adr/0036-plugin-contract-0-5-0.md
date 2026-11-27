---
status: accepted
---

# Plugin contract 0.5.0 for core neutrality and per-call WebAssembly isolation

The Owner decided this on 2026-11-28 during a full architecture review of codeint v0.8.0.

## Context

[ADR 0002](./0002-plugins-as-wasm-components.md) and `docs/agents/stack.md` require the core crates (`codeint-model`, `codeint-config`, `codeint-metrics`, `codeint-plugin-host`, `codeint-report`, and `codeint-cli`) to stay language-neutral.
Over v0.4.0 through v0.8.0, five language-specific leaks accumulated in the core because `wit/language.wit` (`0.4.0`) lacked seams for them:

1. SQL bypassed the Plugin host across four crates (`crates/config/src/sql.rs`, `crates/plugin-host/src/sql.rs`, `crates/cli/src/check/sql.rs`, and `crates/report/src/lib.rs`).
   Core code configured SQL Dialects, excluded `down.sql`, collected rejected statements, and ran a cross-file schema pass.
2. `crates/cli/src/package_scripts.rs` held 679 lines of `package.json` script parsing and npm binary aliases.
   `crates/cli/src/check/manifests.rs` hardcoded Cargo virtual-workspace filtering along with `Cargo.toml`, `tsconfig*.json`, and `pnpm-workspace.yaml` rules.
3. `crates/metrics/src/dependencies.rs` hardcoded npm root-workspace dependency hoisting by checking `package.json`.
4. `crates/metrics/src/tooling_paths.rs` hardcoded Python, PHP, and JavaScript tooling filenames (`conftest.py`, `setup.py`, `php-cs-fixer`, `.storybook`) inside the metrics crate.
5. `crates/plugin-host/src/builtins.rs` hardcoded Flagged line rules outside the `LanguagePlugin` trait.
   `workspace-packages` returned one broken manifest at a time, forcing an `O(K * M)` retry loop.
   `resolve-imports` accepted one flat `active-profiles` list, forcing repeated resolver rebuilds across heterogeneous monorepo packages.

Separately, `crates/plugin-host` pooled and reused WebAssembly `Store` instances across calls.
That pooling pinned high-water linear memory per thread and allowed stateful third-party Plugins to leak state between files.
HTTPS Plugin downloads also enforced a timeout without a maximum file-size cap.

## Decision

**Batch all core-neutrality contract improvements into `codeint:plugin@0.5.0`, reject `0.4.0` components strictly, and enforce fresh per-call WebAssembly instances with bounded HTTPS downloads.**

1. **Consolidated manifest analysis (`analyze-manifests`)**:
   Replace fragmented manifest calls (`workspace-packages` and `declared-dependencies`) and core manifest heuristics with one deep `analyze-manifests(manifests)` export on `language`.
   It returns discovered Workspace packages, skipped invalid manifests with reasons in a single pass, and dependency-manifest ownership.
   It also returns declared dependencies, manifest-used or script-used dependencies, and root-dependency inheritance.
   This moves `package_scripts.rs` and npm hoisting into `codeint-lang-ts` and moves Cargo virtual-workspace filtering into `codeint-lang-rust`.
2. **Flagged line rule discovery and per-package Framework profiles**:
   Add `flagged-line-rules() -> list<string>` to `language` so built-in and external Language plugins declare Flagged line rules uniformly.
   Pass per-package active Framework profiles into `resolve-imports` so monorepos resolve all imports in one pass without rebuilding resolvers.
3. **Generic Dialects, per-file diagnostics, and single-parse cross-file scope analysis**:
   Add `known-dialects() -> list<string>` and an optional `dialect: option<string>` parameter to `lower`.
   Replace `sql.dialects` outright with generic per-language Dialect configuration (`dialects.<language>`).
   Keep no legacy `sql.dialects` alias while codeint is pre-1.0.
   Add optional non-fatal per-file diagnostics and an optional opaque byte artifact on `FileModel`.
   When a Language plugin opts into cross-file analysis, the Plugin host groups opaque artifacts by `(Workspace package, Dialect)` without inspecting them.
   The Plugin host passes each group back to `analyze-scope` and receives extra Hazard lines and language-neutral scope summaries.
   This moves `down.sql` handling and SQL schema Hazard analysis completely into `codeint-lang-sql`.
   Generalize top-level report keys from `sql_schemas` and `rejected_statements` to `scope_summaries` and `file_diagnostics`.
   Keep legacy deserialization inside `codeint report` so previously saved JSON reports still render.
4. **Explicit Tooling file role**:
   Add a boolean `is-tooling-file` marker to `FileModel`.
   Each Language plugin owns its language-specific tooling rules.
   The core settles universal repository folders (`.github/`, `scripts/`, `docs/`, `examples/`, `tools/`, `benches/`).
   A Tooling file stays in the Production Source set for all Production Metrics and Hazards except Orphan files and Dependency hygiene dev-in-production checks.
   Listing a Tooling file under `entry_points` overrides that exclusion.
5. **Fresh per-call WebAssembly instances and download size cap**:
   Remove cross-call `Store` instance pooling in `crates/plugin-host`.
   Instantiate a fresh `Store` from pre-compiled `InstancePre` per call so linear memory never stays pinned and guest state cannot leak.
   Enforce a configurable 50 MiB default download size cap on HTTPS Plugin downloads before buffering bytes into memory.

## Considered options

- **Work around `WIT 0.4.0` with more host-side special cases.**
  Rejected: SQL, npm scripts, Cargo/tsconfig filtering, npm hoisting, and Python/PHP tooling rules would stay in core crates.
  External WebAssembly Plugins would also lose parity with built-in Plugins.
- **Keep `sql.dialects` as a permanent compatibility alias.**
  Rejected: codeint is pre-1.0 with a single active user.
  Maintaining dual config paths adds permanent schema and validation complexity for minimal benefit.
- **Re-parse raw file text inside `analyze-scope` instead of returning opaque bytes from `lower`.**
  Rejected: double-parsing every SQL file wastes CPU time.
  Passing opaque bytes keeps SQL single-parse while preserving strict Plugin host neutrality.
- **Add a fifth Source set for Tooling files.**
  Rejected: a fifth Source set ripples into Budgets, Ratchets, Baselines, and every report table.
  A boolean file role preserves four Source sets while making the rule explicit in `GLOSSARY.md` and owned by Language plugins.
- **Keep `InstancePool` and evict instances only above a memory threshold.**
  Rejected: soft eviction still lets mutable guest state leak across files under parallel scheduling.
  `InstancePre` instantiation costs microseconds per call.

## Consequences

- The Plugin host rejects `codeint:plugin@0.4.0` components with a contract-version mismatch error; external WebAssembly Plugins must rebuild against `0.5.0`.
- All five language leaks exit `crates/config`, `crates/metrics`, `crates/plugin-host`, `crates/report`, and `crates/cli`, restoring a strictly language-neutral core.
- Existing repositories using `sql.dialects` must migrate to `dialects.sql` when upgrading to Plugin contract `0.5.0`.
- External Language plugins gain parity for Dialects, Flagged line rule declarations, Tooling files, script-used dependencies, and cross-file Hazard passes.
- WebAssembly Plugin execution becomes strictly deterministic and memory-bounded per call.
