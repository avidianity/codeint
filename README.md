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
It ships with eight built-in Language plugins covering TypeScript/JavaScript (with built-in `react`, `vue2`, `vue3`, `svelte`, `nextjs`, `nuxt`, `sveltekit`, `angular`, and `nestjs` Framework profiles plus framework-blind `.vue` and `.svelte` `<script>` lowering), Rust, PHP, Dart, Python, Go, SQL, and Shell (`.sh`, `.bash`, `.zsh`), plus sandboxed WebAssembly Plugins for other languages and custom Metrics.

## How it works

### Independent Metrics, never one blended score

codeint reports every Metric side by side and never collapses them into one grade or index ([ADR 0003](./docs/adr/0003-no-combined-score.md)).
A single score hides regressions when one Metric improves while another worsens.

Five core Metrics are calibrated against stratified panels of current public repositories ([ADR 0035](./docs/adr/0035-stratified-100-repository-reference-panels.md)):

- **Erosion**: share of Function mass (`cyclomatic_complexity * sqrt(source_lines)`) in functions with cyclomatic complexity over 10, reported with function names.
- **Cognitive complexity**: share of total nesting-weighted cognitive complexity held by functions scoring over 15.
- **Duplication**: share of source lines in per-language exact Clone groups (at least 75 normalized tokens and 14 distinct normalized tokens, catching renamed copy-paste while ignoring import lists and data tables), grouped into Clone families across three or more locations, with opt-in anchor-checked near-clone detection ([ADR 0037](./docs/adr/0037-metric-v2-duplication-and-boundaries.md)).
- **Verbosity**: share of source lines in zero-false-positive Flagged lines (independent of Duplication, [ADR 0024](./docs/adr/0024-verbosity-decoupling.md)).
- **Import cycles**: share of modules in at least one runtime Import cycle.

Additional independent Metrics and structural checks run in the same pass:

- **Test gap**: share of total Production Function mass not executed by tests, read from user-named LCOV or Cobertura XML Coverage reports without running your test suite, listing the heaviest untested functions first ([Test gap guide](./docs/guide/test-gap.md), [ADR 0017](./docs/adr/0017-test-gap-from-coverage-reports.md)).
- **Orphan files**: Production files unreachable from manifest-declared, program-start, and configured `entry_points`, classified as `unreferenced` or `test-only`.
- **Dependency hygiene**: three independent Metrics (`unused_dependencies`, `unlisted_dependencies`, and `dev_in_production_dependencies`) comparing `package.json`, `Cargo.toml`, `pyproject.toml`, `composer.json`, `pubspec.yaml`, and `go.mod` against imported external packages while excluding standard-library built-ins.
- **Architectural import boundaries**: named zones, built-in `layered` and `hexagonal` presets, `public_files` encapsulation, directional internal `allow`/`deny` rules, external-package `allow_external`/`deny_external` glob rules, and pre-edit `codeint guard <path>` checks before a file even exists on disk ([Import boundaries guide](./docs/guide/import-boundaries.md)).
- **Hazards**: share of source lines matching AST-only Hazard line rules for patterns likely to cause wrong results, slow execution, or a security exposure, reported next to Software entropy with strict non-leakage (path, line, and rule name only, never source text), 32 built-in SQL rules including 5 schema-backed rules, and 10 built-in Shell rules ([Hazards guide](./docs/guide/hazards.md), [SQL guide](./docs/guide/sql.md), [Shell guide](./docs/guide/shell.md), [ADR 0016](./docs/adr/0016-hazards-next-to-software-entropy.md)).

Every file belongs to one **Source set** (`production`, `test`, `generated`, or `vendored`), so fixtures and generated code never distort Production ratios.
**Budgets** cap absolute ratios, `.codeint/ratchet.json` (**Ratchet**, managed with `codeint ratchet update`) locks in whole-repository ceilings with Metric version tracking, and `codeint diff <base>` checks every pull request against its Git merge-base ([ADR 0026](./docs/adr/0026-regression-and-ratchet.md)).
`.codeint/baseline.json` (**Baseline**, managed with `codeint baseline update`) freezes existing Findings with hashed surrounding-line context anchors, so `codeint check` hides them, holds new Findings to every Budget, and shrinks the Baseline as legacy Findings are fixed ([ADR 0030](./docs/adr/0030-finding-baseline.md), [ADR 0032](./docs/adr/0032-context-anchored-finding-matching.md)).

### Codebase navigation, Git history, and ownership

Beyond static snapshots, codeint helps humans and coding agents understand how files connect and evolve:

- **`codeint trace <from> <to>`**: find the shortest runtime import paths between two modules.
- **`codeint impact <paths...>`** / **`--changed <base-ref>`**: compute the transitive reverse-dependency blast radius, separating Production and Test dependents and listing affected Workspace packages.
- **`--package <path>`**: scope `check`, `diff`, `impact`, and `trace` to one Workspace package in a monorepo.
- **`codeint churn`**: rank eroded and duplicated files alongside their Git commit churn independently per Metric.
- **`codeint coupling`**: detect pairs of files that repeatedly change together in the same commits and flag whether they share a direct runtime import.
- **`CODEOWNERS` filtering (`--owner <pattern>`)**: filter `check` and `diff` Findings to a specific team or owner.

### Built for CI and coding agents

- **JSON, JSONC, TOML, and YAML configuration with `.codeint/` state**: configure codeint in `codeint.{json,jsonc,toml,yaml,yml}` or `.codeint/config.{json,jsonc,toml,yaml,yml}` (`codeint init` writes `codeint.jsonc` by default, or pass `--format json|toml|yaml`), set an optional default output `format` (`toon`, `text`, `json`, `sarif`, or `markdown`) for analysis and report commands, lock in monotonic whole-repository ceilings in `.codeint/ratchet.json` via `codeint ratchet update`, and freeze existing Findings in `.codeint/baseline.json` via `codeint baseline update`.
- **Single-scan multi-format reporting**: save `codeint check --format json` or `codeint diff --format json` once, then re-render losslessly into concise monochrome `text`, `toon`, `json`, `sarif`, or `markdown` with `codeint report`.
- **Introspection and diagnostics**: inspect machine-readable CLI/MCP capabilities with `codeint schema`, see effective settings with `default`/`file`/`cli` provenance via `codeint config --resolved`, verify repository health offline with `codeint doctor`, and check an external Wasm Plugin against the Plugin contract with `codeint plugin test`.
- **First-class MCP integration**: run `codeint agent status` and `codeint agent install` to register the local stdio MCP server (`codeint mcp`) for Claude Code, Cursor, and Codex, exposing `check`, `diff`, `trend`, `guard`, `impact`, `trace`, `churn`, and `coupling` with compact TOON output by default and optional `format: "json"`.

### Built for speed and safety

codeint is written in safe Rust (`#![forbid(unsafe_code)]` on every host, core, CLI, and built-in Plugin crate, [ADR 0004](./docs/adr/0004-forbid-unsafe-code.md)) around two clean seams: Language plugins lower source files into a language-neutral Code model, and Metric plugins compute Metrics and Findings over that model.
It reads tracked files and commit history directly from Git objects without modifying your working tree, scanning `laravel/framework` (132k Production lines) in `0.47s` and `flutter/packages` (434k Production lines) in `3.68s` on Apple Silicon.
External WebAssembly Plugins implement the versioned `codeint:plugin@0.5.0` WIT contract with mandatory SHA-256 verification, zero host capabilities, and configurable memory and time caps, using `codeint-plugin-sdk` for safe Rust builds on `wasm32-wasip2` (`no_std`, zero extra tools) or `wasm32-unknown-unknown` (`std` plus `wasm-tools component new`) with zero WASI imports ([ADR 0027](./docs/adr/0027-plugin-sdk-guest-runtime-unsafe.md)).

Read [docs/guide/philosophy-and-architecture.md](./docs/guide/philosophy-and-architecture.md) for the full architecture diagram and design rationale.

## Calibrated Reference ranges

10th to 90th percentile across stratified panels of up to 100 current public repositories per built-in language (77 for SQL under the `200+`-star floor) and 50 current public repositories per calibrated Framework profile, sampled round-robin across three star bands ([ADR 0035](./docs/adr/0035-stratified-100-repository-reference-panels.md)):

| Language | Erosion | Cognitive complexity | Duplication | Verbosity | Import cycles | Docs |
| -------- | ------- | -------------------- | ----------- | --------- | ------------- | ---- |
| TypeScript / JavaScript | 18.7% to 67.1% | 12.1% to 53.2% | 1.8% to 25.2% | 0.0% to 0.2% | 0.0% to 18.2% | [ADR 0035](./docs/adr/0035-stratified-100-repository-reference-panels.md) |
| React (`.jsx` / `.tsx`) | 5.4% to 56.6% | 3.8% to 39.9% | 2.6% to 31.5% | 0.0% to 0.1% | 0.0% to 20.2% | [ADR 0035](./docs/adr/0035-stratified-100-repository-reference-panels.md) |
| Vue 2 (`.vue`) | 2.5% to 47.9% | 3.3% to 40.6% | 2.3% to 26.7% | 0.0% to 0.3% | 0.0% to 12.8% | [ADR 0035](./docs/adr/0035-stratified-100-repository-reference-panels.md) |
| Vue 3 (`.vue`) | 4.2% to 43.8% | 2.2% to 29.7% | 2.1% to 29.7% | 0.0% to 0.2% | 0.0% to 32.5% | [ADR 0035](./docs/adr/0035-stratified-100-repository-reference-panels.md) |
| Rust | 24.5% to 70.4% | 17.6% to 58.5% | 4.7% to 37.7% | 0.0% to 0.5% | 0.0% to 61.1% | [ADR 0035](./docs/adr/0035-stratified-100-repository-reference-panels.md) |
| PHP | 6.4% to 59.1% | 4.4% to 54.2% | 2.5% to 20.9% | 0.0% to 0.4% | 0.0% to 13.3% | [ADR 0035](./docs/adr/0035-stratified-100-repository-reference-panels.md) |
| Dart | 3.9% to 49.7% | 0.0% to 37.7% | 2.7% to 25.6% | 0.0% to 1.1% | 6.1% to 60.0% | [ADR 0035](./docs/adr/0035-stratified-100-repository-reference-panels.md) |
| Python | 23.9% to 71.7% | 24.9% to 63.7% | 1.1% to 22.0% | 0.0% to 1.1% | 0.0% to 44.6% | [ADR 0035](./docs/adr/0035-stratified-100-repository-reference-panels.md) |
| Go | 30.4% to 74.6% | 30.6% to 67.1% | 3.4% to 24.3% | 0.0% to 0.4% | 0.0% to 0.0% | [ADR 0035](./docs/adr/0035-stratified-100-repository-reference-panels.md) |
| SQL | 0.0% to 91.8% | 0.0% to 90.2% | 0.0% to 90.9% | 0.0% to 0.0% | N/A | [ADR 0035](./docs/adr/0035-stratified-100-repository-reference-panels.md) |
| Shell | 36.4% to 93.0% | 21.8% to 82.8% | 0.3% to 71.3% | 0.0% to 3.1% | N/A | [ADR 0035](./docs/adr/0035-stratified-100-repository-reference-panels.md) |

See [docs/guide/languages.md](./docs/guide/languages.md) for per-language rules and sample repository timings, and [docs/evaluation.md](./docs/evaluation.md) for the full 927-repository Evaluation suite report, native-parser recall, and agent-reviewed audit.

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
codeint init --adopt
codeint check
codeint diff main
codeint ratchet update
codeint baseline update
codeint guard src/domain/order.ts
codeint doctor
codeint plugin test plugins/my-plugin.wasm
codeint agent install
```

- `codeint init` writes a starter `codeint.jsonc` with a versioned `$schema` (pass `--format json`, `--format toml`, or `--format yaml` for `codeint.json`, `codeint.toml`, or `codeint.yaml`).
- `codeint init --adopt` onboards an existing codebase in one step: it writes the starter `codeint.jsonc` with `per_file: true`, zero Budgets for each recorded Metric, and commented `new_file_budgets`, writes `.codeint/ratchet.json` and `.codeint/baseline.json` from the current state, and prints a pull-request workflow, without replacing any existing file ([adoption guide](./docs/guide/adopting-on-an-existing-codebase.md)).
- `codeint check` measures the working tree and prints each Metric with its Findings.
- `codeint diff main` compares the working tree against the merge-base of `main` and fails on any Budget, Ratchet, or Regression failure.
- `codeint ratchet update` creates or monotonically tightens `.codeint/ratchet.json` ceilings for whole-repository runs.
- `codeint baseline update` creates `.codeint/baseline.json`, or removes fixed Findings from it, so `codeint check` hides existing Findings and reports only new ones.
- `codeint guard src/domain/order.ts` checks architectural boundary rules for an existing or planned file path.
- `codeint doctor` runs offline diagnostics on Git status, config validity, detected languages, workspaces, and Plugins.
- `codeint plugin test plugins/my-plugin.wasm` runs the Conformance runner against an external Wasm Plugin and prints each violated rule with a fix hint.
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

- [Guide](./docs/guide/README.md): walkthroughs for [Philosophy and Architecture](./docs/guide/philosophy-and-architecture.md), [Getting Started](./docs/guide/getting-started.md), [Configuration](./docs/guide/configuration.md), [Import boundaries](./docs/guide/import-boundaries.md), [Test gap](./docs/guide/test-gap.md), [Hazards](./docs/guide/hazards.md), [Built-in Languages](./docs/guide/languages.md), [CI and GitHub Actions](./docs/guide/ci-and-github-actions.md), [Adopting codeint on an existing codebase](./docs/guide/adopting-on-an-existing-codebase.md), and [Coding-Agent Integration (MCP)](./docs/guide/mcp.md).
- [Reference](./docs/reference.md): tested reference for every command, flag, exit code, report format (`toon`, `json`, `sarif`, `markdown`, `text`), and configuration key.
- [Programmatic API](./docs/programmatic.md): JavaScript/TypeScript SDK and Rust library API.
- [Plugins](./docs/plugins.md): build external WebAssembly Language and Metric plugins with `codeint-plugin-sdk` (`wasm32-wasip2` `no_std` or `wasm32-unknown-unknown` `std`), pin SHA-256 digests, and test them.
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
