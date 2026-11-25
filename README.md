<h1 align="center">
  <img src="./assets/branding/banner.webp" alt="codeint: see Software entropy grow before it compounds" width="100%">
</h1>

<p align="center">
  <a href="https://github.com/avidianity/codeint/actions/workflows/ci.yml"><img src="https://github.com/avidianity/codeint/actions/workflows/ci.yml/badge.svg?branch=main" alt="CI status"></a>
  <a href="https://github.com/avidianity/codeint/releases/latest"><img src="https://img.shields.io/github/v/release/avidianity/codeint?color=275e5e" alt="GitHub Release"></a>
  <a href="https://www.npmjs.com/package/codeint"><img src="https://img.shields.io/npm/v/codeint?color=275e5e" alt="npm version"></a>
  <a href="./LICENSE"><img src="https://img.shields.io/github/license/avidianity/codeint?color=275e5e" alt="License: MIT"></a>
</p>

<p align="center">
  <a href="#why-codeint">Why codeint</a> ·
  <a href="#how-it-works">How it works</a> ·
  <a href="#quick-start">Quick start</a> ·
  <a href="./docs/guide/README.md">Guide</a> ·
  <a href="./docs/reference.md">Reference</a> ·
  <a href="./docs/programmatic.md">Programmatic API</a> ·
  <a href="./docs/plugins.md">Plugins</a> ·
  <a href="./llms.txt">llms.txt</a>
</p>

> [!NOTE]
> **Early development**: codeint is still pre-`1.0` and evolving quickly.
> CLI flags, configuration keys, report shapes, and the WebAssembly Plugin contract may change rapidly before `1.0`.

## Why codeint

Coding agents write locally valid code very quickly.
Each pull request can pass tests and linters while quietly adding copied blocks, redundant branches, deeper functions, unused dependencies, and tangled imports.
Over dozens of changes, that structural drag compounds until both humans and agents slow down.

codeint measures **Software entropy** directly from the shape of your source code so you see it grow before it compounds, and reports **Hazards** next to Software entropy ([ADR 0016](./docs/adr/0016-hazards-next-to-software-entropy.md)).
It ships with seven built-in Language plugins covering TypeScript/JavaScript (with built-in `react`, `vue2`, `vue3`, and `svelte` Framework profiles plus framework-blind `.vue` and `.svelte` `<script>` lowering), Rust, PHP, Dart, Python, Go, and SQL, plus sandboxed WebAssembly Plugins for other languages and custom Metrics.

## How it works

### Independent Metrics, never one blended score

codeint reports every Metric side by side and never collapses them into one grade or index ([ADR 0003](./docs/adr/0003-no-combined-score.md)).
A single score hides regressions when one Metric improves while another worsens.

Four core Metrics are calibrated against pre-2023 human-written repositories:

- **Erosion**: share of Function mass (`cyclomatic_complexity * sqrt(source_lines)`) in functions with cyclomatic complexity over 10, reported with function names.
- **Duplication**: share of source lines in exact Clone groups (at least 75 normalized tokens and 14 distinct normalized tokens, catching renamed copy-paste while ignoring import lists and data tables), grouped into Clone families across three or more locations, with opt-in shingle-based near-clone detection.
- **Verbosity**: share of source lines in zero-false-positive Flagged lines (independent of Duplication, [ADR 0024](./docs/adr/0024-verbosity-decoupling.md)).
- **Import cycles**: share of modules in at least one runtime Import cycle.

Additional independent Metrics and structural checks run in the same pass:

- **Cognitive complexity**: share of total nesting-weighted cognitive complexity held by functions scoring over 15.
- **Test gap**: share of total Production Function mass not executed by tests, read from user-named LCOV or Cobertura XML Coverage reports without running your test suite, listing the heaviest untested functions first ([Test gap guide](./docs/guide/test-gap.md), [ADR 0017](./docs/adr/0017-test-gap-from-coverage-reports.md)).
- **Orphan files**: Production files unreachable from manifest-declared, program-start, and configured `entry_points`, classified as `unreferenced` or `test-only`.
- **Dependency hygiene**: three independent Metrics (`unused_dependencies`, `unlisted_dependencies`, and `dev_in_production_dependencies`) comparing `package.json`, `Cargo.toml`, `pyproject.toml`, `composer.json`, `pubspec.yaml`, and `go.mod` against imported external packages while excluding standard-library built-ins.
- **Architectural import boundaries**: named zones, built-in `layered` and `hexagonal` presets, `public_files` encapsulation, directional internal `allow`/`deny` rules, external-package `allow_external`/`deny_external` glob rules, and pre-edit `codeint guard <path>` checks before a file even exists on disk ([Import boundaries guide](./docs/guide/import-boundaries.md)).
- **Hazards**: share of source lines matching AST-only Hazard line rules for patterns likely to cause wrong results, slow execution, or a security exposure, reported next to Software entropy with strict non-leakage (path, line, and rule name only, never source text) and 32 built-in SQL rules including 5 schema-backed rules ([Hazards guide](./docs/guide/hazards.md), [SQL guide](./docs/guide/sql.md), [ADR 0016](./docs/adr/0016-hazards-next-to-software-entropy.md)).

Every file belongs to one **Source set** (`production`, `test`, `generated`, or `vendored`), so fixtures and generated code never distort Production ratios.
**Budgets** cap absolute ratios in `codeint check`, while `codeint diff <base>` ratchets every pull request against its Git merge-base.

### Codebase navigation, Git history, and ownership

Beyond static snapshots, codeint helps humans and coding agents understand how files connect and evolve:

- **`codeint trace <from> <to>`**: find the shortest runtime import paths between two modules.
- **`codeint impact <paths...>`** / **`--changed <base-ref>`**: compute the transitive reverse-dependency blast radius, separating Production and Test dependents and listing affected Workspace packages.
- **`--package <path>`**: scope `check`, `diff`, `impact`, and `trace` to one Workspace package in a monorepo.
- **`codeint churn`**: rank eroded and duplicated files alongside their Git commit churn independently per Metric.
- **`codeint coupling`**: detect pairs of files that repeatedly change together in the same commits and flag whether they share a direct runtime import.
- **`CODEOWNERS` filtering (`--owner <pattern>`)**: filter `check` and `diff` Findings to a specific team or owner.

### Built for CI and coding agents

- **JSONC configuration**: configure codeint in `codeint.jsonc` or `codeint.json` with comments and trailing commas (`codeint init` writes a starter `codeint.jsonc`).
- **Single-scan multi-format reporting**: save `codeint check --format json` or `codeint diff --format json` once, then re-render losslessly into `text`, `toon`, `json`, `sarif`, or `markdown` with `codeint report`.
- **Introspection and diagnostics**: inspect machine-readable CLI/MCP capabilities with `codeint schema`, see effective settings with `default`/`file`/`cli` provenance via `codeint config --resolved`, and verify repository health offline with `codeint doctor`.
- **First-class MCP integration**: run `codeint agent status` and `codeint agent install` to register the local stdio MCP server (`codeint mcp`) for Claude Code, Cursor, and Codex, exposing `check`, `diff`, `trend`, `guard`, `impact`, `trace`, `churn`, and `coupling`.

### Built for speed and safety

codeint is written in safe Rust (`#![forbid(unsafe_code)]`, [ADR 0004](./docs/adr/0004-forbid-unsafe-code.md)) around two clean seams: Language plugins lower source files into a language-neutral Code model, and Metric plugins compute Metrics and Findings over that model.
It reads tracked files and commit history directly from Git objects without modifying your working tree, scanning `laravel/framework` (132k Production lines) in `0.47s` and `flutter/packages` (434k Production lines) in `3.68s` on Apple Silicon.
External WebAssembly Plugins implement the versioned `codeint:plugin@0.4.0` WIT contract with mandatory SHA-256 verification, zero host capabilities, and configurable memory and time caps.

Read [docs/guide/philosophy-and-architecture.md](./docs/guide/philosophy-and-architecture.md) for the full architecture diagram and design rationale.

## Calibrated Reference ranges

10th to 90th percentile across 20 pre-2023 open-source repositories per language or framework:

| Language | Erosion | Duplication | Verbosity | Import cycles | Docs |
| -------- | ------- | ----------- | --------- | ------------- | ---- |
| TypeScript / JavaScript | 35.2% to 60.0% | 1.7% to 34.6% | 1.8% to 34.7% | 0.0% to 30.8% | [ADR 0008](./docs/adr/0008-typescript-reference-ranges.md) |
| React (`.jsx` / `.tsx`) | 10.6% to 54.4% | 4.6% to 36.8% | 4.7% to 36.8% | 0.0% to 14.4% | [ADR 0020](./docs/adr/0020-react-reference-ranges.md) |
| Vue 2 (`.vue`) | 2.0% to 45.2% | 3.6% to 19.2% | 3.6% to 19.2% | 0.0% to 11.4% | [ADR 0014](./docs/adr/0014-vue2-reference-ranges.md) |
| Vue 3 (`.vue`) | 1.9% to 38.8% | 2.3% to 34.5% | 2.4% to 34.6% | 0.0% to 7.7% | [ADR 0015](./docs/adr/0015-vue3-reference-ranges.md) |
| Rust | 38.0% to 64.2% | 5.0% to 32.5% | 6.0% to 32.9% | 2.1% to 50.6% | [ADR 0010](./docs/adr/0010-rust-reference-ranges.md) |
| PHP | 5.5% to 67.7% | 4.1% to 29.0% | 4.2% to 29.2% | 0.0% to 10.7% | [ADR 0011](./docs/adr/0011-php-reference-ranges.md) |
| Dart | 4.7% to 41.9% | 7.3% to 25.6% | 8.0% to 25.9% | 7.3% to 52.4% | [ADR 0012](./docs/adr/0012-dart-reference-ranges.md) |
| Python | 14.4% to 62.4% | 1.2% to 20.1% | 1.4% to 20.4% | 0.0% to 32.1% | [ADR 0013](./docs/adr/0013-python-reference-ranges.md) |
| Go | 37.9% to 69.2% | 4.4% to 18.1% | 4.5% to 18.3% | 0.0% to 0.0% | [ADR 0021](./docs/adr/0021-go-reference-ranges.md) |
| SQL | 9.4% to 97.6% | 4.2% to 92.1% | 4.2% to 92.1% | N/A | [ADR 0022](./docs/adr/0022-sql-reference-ranges.md) |

See [docs/guide/languages.md](./docs/guide/languages.md) for per-language rules and sample repository timings.

## Quick start

Install the standalone binary on macOS or Linux, on Windows, or run directly with `npx`:

```sh
curl -fsSL https://github.com/avidianity/codeint/releases/latest/download/install.sh | bash
```

```powershell
irm https://github.com/avidianity/codeint/releases/latest/download/install.ps1 | iex
```

```sh
npx codeint check
```

Run codeint inside any Git repository:

```sh
codeint init
codeint check
codeint diff main
codeint guard src/domain/order.ts
codeint doctor
codeint agent install
```

- `codeint init` writes a starter `codeint.jsonc` with a versioned `$schema`.
- `codeint check` measures the working tree and prints each Metric with its Findings.
- `codeint diff main` compares the working tree against the merge-base of `main` and fails on any Budget failure or Regression.
- `codeint guard src/domain/order.ts` checks architectural boundary rules for an existing or planned file path.
- `codeint doctor` runs offline diagnostics on Git status, config validity, detected languages, workspaces, and Plugins.
- `codeint agent install` registers `codeint mcp` in detected local coding agents.

Or call codeint programmatically from JavaScript/TypeScript (`npm install codeint`) or Rust (`codeint-cli`):

```ts
import { check, diff } from "codeint";

const checked = await check();
const prDiff = await diff("main");
```

```rust
use std::path::Path;
use codeint_cli::{CheckOptions, DiffOptions, check, diff};

let checked = check(Path::new("."), &CheckOptions::default())?;
let pr_diff = diff(Path::new("."), "main", &DiffOptions::default())?;
```

## Documentation

- [Guide](./docs/guide/README.md): walkthroughs for [Philosophy and Architecture](./docs/guide/philosophy-and-architecture.md), [Getting Started](./docs/guide/getting-started.md), [Configuration](./docs/guide/configuration.md), [Import boundaries](./docs/guide/import-boundaries.md), [Test gap](./docs/guide/test-gap.md), [Hazards](./docs/guide/hazards.md), [Built-in Languages](./docs/guide/languages.md), [CI and GitHub Actions](./docs/guide/ci-and-github-actions.md), and [Coding-Agent Integration (MCP)](./docs/guide/mcp.md).
- [Reference](./docs/reference.md): tested reference for every command, flag, exit code, report format (`toon`, `json`, `sarif`, `markdown`, `text`), and configuration key.
- [Programmatic API](./docs/programmatic.md): JavaScript/TypeScript SDK and Rust library API.
- [Plugins](./docs/plugins.md): build, hash, declare, and test external WebAssembly Language and Metric plugins.
- [Glossary](./GLOSSARY.md): canonical definitions of every domain term on this page.
- [llms.txt](./llms.txt): compact documentation index for coding agents and LLM tools.

## Development

You need Rust through rustup ([rust-toolchain.toml](./rust-toolchain.toml)), Node.js 24 and npm ([package.json](./package.json)), and `gitleaks` plus `lychee` on your `PATH`.

```sh
npm ci
npx lefthook install
cargo xtask affected-tests
```

Read [CONTRIBUTING.md](./CONTRIBUTING.md) for the contributor workflow and local setup.
Agents and people follow the same repository rules: start with [.rules](./.rules) and [docs/checks/README.md](./docs/checks/README.md).

## License

[MIT](./LICENSE)
