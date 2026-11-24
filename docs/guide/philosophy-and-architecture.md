# Philosophy and Architecture

This page explains why codeint exists, the logic behind its Metrics, and how the tool is implemented.

## Why Software entropy matters

Coding agents write locally valid code very quickly.
Each pull request can pass tests and linters while quietly adding copied blocks, redundant branches, deeper functions, unused dependencies, and tangled imports.
Over dozens of changes, that structural drag compounds until both humans and agents slow down.

codeint measures **Software entropy** directly from the shape of source code so teams see it grow before it compounds.

## Core design principles

### 1. Independent Metrics, never one blended score

codeint reports every Metric side by side and never collapses them into a single grade or index ([ADR 0003](../adr/0003-no-combined-score.md)).
A combined score hides regressions when one Metric improves while another worsens.

Four core Metrics are calibrated against pre-2023 human-written repositories:

- **Erosion**: the share of total **Function mass** (`cyclomatic_complexity * sqrt(source_lines)`) held by functions with cyclomatic complexity over 10, reported with function names.
  Weighting by both complexity and size highlights the few large, tangled functions that hurt maintainability most.
- **Duplication**: the share of source lines inside **Clone groups** (two or more fragments matching at least 75 normalized tokens and 14 distinct normalized tokens, [ADR 0007](../adr/0007-default-minimum-clone-size.md)).
  Normalizing identifiers and literals catches renamed copy-paste while ignoring import blocks and repetitive data tables, clusters three or more overlapping locations into **Clone families**, and supports opt-in shingle-based near-clone detection.
- **Verbosity**: the share of source lines that are **Flagged lines**, exact clone lines, or both.
  Each Flagged-line rule targets a zero-false-positive redundant construct (such as `else` after an unconditional `return` or `if (c) return true; else return false;`).
- **Import cycles**: the share of modules that belong to at least one runtime **Import cycle** (strongly connected component of size 2 or more).

Additional independent Metrics and structural checks run alongside them:

- **Cognitive complexity**: the share of total nesting-weighted cognitive complexity held by functions scoring over 15.
- **Test gap**: the share of total Production Function mass not executed by tests, computed from user-named LCOV or Cobertura XML Coverage reports ([ADR 0017](../adr/0017-test-gap-from-coverage-reports.md), [Test gap](./test-gap.md)).
  codeint never runs tests, leaves Test gap absent without a Coverage report, and publishes no Reference range for it.
- **Orphan files**: Production files unreachable through runtime imports from manifest-declared, program-start, and configured `entry_points`, classified as `unreferenced` or `test-only`.
- **Dependency hygiene**: three independent Metrics (`unused_dependencies`, `unlisted_dependencies`, and `dev_in_production_dependencies`) comparing package manifests against imported external packages while excluding standard-library built-ins.
- **Import boundaries**: directional `allow`/`deny` rules and `public_files` encapsulation across named architectural zones, paired with pre-edit `codeint guard <path>` checks.

### 2. Strict Source-set separation

Every file belongs to exactly one **Source set**: `production`, `test`, `generated`, or `vendored`.
Test fixtures, generated code (`*.g.dart`, `*.pb.dart`, `@generated`), and third-party vendor code are measured separately so they never distort Production ratios.

### 3. Pre-2023 human-written Reference ranges

Raw ratios are hard to interpret without context.
For each built-in language, codeint measures a panel of 20 public repositories pinned before `2023-01-01` (predating widespread coding-agent adoption) and publishes the 10th to 90th percentile as its **Reference range** ([ADR 0008](../adr/0008-typescript-reference-ranges.md), [ADR 0010](../adr/0010-rust-reference-ranges.md), [ADR 0011](../adr/0011-php-reference-ranges.md), [ADR 0012](../adr/0012-dart-reference-ranges.md), [ADR 0013](../adr/0013-python-reference-ranges.md), [ADR 0014](../adr/0014-vue2-reference-ranges.md), [ADR 0015](../adr/0015-vue3-reference-ranges.md), [ADR 0020](../adr/0020-react-reference-ranges.md)).

### 4. Budgets and Regressions

- A **Budget** (`codeint check` and `codeint diff`) sets a hard ceiling for a Metric in a Source set (or for a top-level Dependency hygiene Metric).
  For Import cycles, Orphan files, Import boundaries, and Dependency hygiene, repository totals and Budgets count only languages with imports, and stay absent when no measured language has imports.
- A **Regression** (`codeint diff <base>`) compares the working tree against the Git merge-base and fails when a change introduces new Findings or raises a ratio past its limit.
  Together, Budgets cap absolute entropy while Regressions act as a ratchet on every pull request.

### 5. Version-free language parsing

codeint is a structural entropy tool, not a compiler.
Every built-in Language plugin parses the modern superset of its language family and uses version-agnostic Flagged-line rules, so older and newer codebases work without version pins in `codeint.jsonc` or `codeint.json`.

## How codeint is implemented

codeint is built in safe Rust (`#![forbid(unsafe_code)]`, [ADR 0004](../adr/0004-forbid-unsafe-code.md)) around two clean seams:

```text
Source files + Git history (crates/git)
          │
          ▼  Seam 2: Language plugin (built-in Rust or Wasm component)
Language-neutral Code model (crates/model)
          │
          ▼  Seam 3: Metric plugin (built-in Rust or Wasm component)
Metrics + Findings + Diff + Trend + Graph/History queries (crates/metrics, crates/report)
          │
          ▼
CLI + MCP + Rust SDK (crates/cli)  &  JS/TS SDK (npm/codeint)
```

- **Git-native measurement (`crates/git`)**: reads tracked working-tree files and historical commits directly from Git objects via `gix`, never modifying the working tree during `diff`, `trend`, `churn`, or `coupling`, with a content-addressed result cache for fast repeated runs.
- **Language plugins (`plugins/languages/*`)**: own every language-specific rule (parsing, named functions, decision points, cognitive complexity, tokens, Flagged lines, imports, workspaces, entry points, declared dependencies, Source-set defaults) and lower files into the language-neutral **Code model** (`crates/model`).
  TypeScript/JavaScript (including `.vue` and `.svelte` `<script>` blocks) uses `oxc_parser`; Rust uses `syn` and `proc-macro2`; PHP, Dart, and Python use pure-Rust zero-FFI lexers sharing common helpers in `crates/model/src/scan.rs`.
- **WebAssembly plugin host (`crates/plugin-host`)**: loads third-party Language and Metric plugins as WebAssembly components implementing `codeint:plugin@0.3.0` ([ADR 0002](../adr/0002-plugins-as-wasm-components.md)) with mandatory SHA-256 verification, zero host capabilities (no filesystem, network, env, or clock access), and configurable memory and timeout caps.
- **Zero-duplication programmatic surfaces**: `crates/cli/src/lib.rs` exposes the Rust library API directly, and `npm/codeint` wraps the bundled native binary with zero-dependency ESM, CommonJS, and TypeScript declarations.
