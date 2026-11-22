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

## Why codeint

Coding agents write locally valid code very quickly.
Each pull request can pass tests and linters while quietly adding copied blocks, redundant branches, deeper functions, and tangled imports.
Over dozens of changes, that structural drag compounds until both humans and agents slow down.

codeint measures **Software entropy** directly from the shape of source code so you see it grow before it compounds.
It ships with built-in Language plugins for TypeScript/JavaScript, Rust, PHP, Dart, and Python, plus sandboxed WebAssembly Plugins for other languages and custom Metrics.

## How it works

### Four independent Metrics, never one blended score

codeint reports four Metrics side by side and never collapses them into one grade or index ([ADR 0003](./docs/adr/0003-no-combined-score.md)).
A single score hides regressions when one Metric improves while another worsens.

- **Erosion**: share of Function mass (`cyclomatic_complexity * sqrt(source_lines)`) in functions with cyclomatic complexity over 10.
- **Duplication**: share of source lines in Clone groups (at least 75 normalized tokens and 14 distinct normalized tokens, catching renamed copy-paste while ignoring import lists and data tables).
- **Verbosity**: share of source lines in zero-false-positive Flagged lines or Clone groups.
- **Import cycles**: share of modules in at least one runtime Import cycle.

### Grounded, ratchet-based enforcement

- **Pre-2023 Reference ranges**: every built-in language is calibrated against 20 public repositories pinned before `2023-01-01` (10th to 90th percentile), so raw numbers have grounded context.
- **Strict Source-set separation**: every file belongs to `production`, `test`, `generated`, or `vendored`, so fixtures and generated code never distort Production ratios.
- **Budgets and Regressions**: Budgets cap absolute entropy in `codeint check`, while `codeint diff <base>` ratchets every pull request against its Git merge-base.
- **Version-free parsing**: built-in Language plugins parse the modern superset of each language family, with no language-version pins needed in `codeint.json`.

### Built for speed and safety

codeint is written in safe Rust (`#![forbid(unsafe_code)]`, [ADR 0004](./docs/adr/0004-forbid-unsafe-code.md)) around two clean seams: Language plugins lower source files into a language-neutral Code model, and Metric plugins compute Metrics and Findings over that model.
It reads tracked files and commit history directly from Git objects without modifying your working tree, scanning `laravel/framework` (132k Production lines) in `0.47s` and `flutter/packages` (434k Production lines) in `3.68s` on Apple Silicon.
Third-party WebAssembly Plugins run with mandatory SHA-256 verification, zero host capabilities, and configurable memory and time caps.

Read [docs/guide/philosophy-and-architecture.md](./docs/guide/philosophy-and-architecture.md) for the full architecture diagram and design rationale.

## Calibrated Reference ranges

10th to 90th percentile across 20 pre-2023 open-source repositories per language ([ADR 0008](./docs/adr/0008-typescript-reference-ranges.md), [ADR 0010](./docs/adr/0010-rust-reference-ranges.md), [ADR 0011](./docs/adr/0011-php-reference-ranges.md), [ADR 0012](./docs/adr/0012-dart-reference-ranges.md), [ADR 0013](./docs/adr/0013-python-reference-ranges.md)):

| Language | Erosion | Duplication | Verbosity | Import cycles |
| -------- | ------- | ----------- | --------- | ------------- |
| TypeScript / JavaScript | 35.2% to 60.0% | 1.7% to 34.6% | 1.8% to 34.7% | 0.0% to 30.8% |
| Rust | 38.0% to 64.2% | 5.0% to 32.5% | 6.0% to 32.9% | 2.1% to 50.6% |
| PHP | 5.5% to 67.7% | 4.1% to 29.0% | 4.2% to 29.2% | 0.0% to 10.7% |
| Dart | 4.7% to 41.9% | 7.3% to 25.6% | 8.0% to 25.9% | 7.3% to 52.4% |
| Python | 14.4% to 62.4% | 1.2% to 20.1% | 1.4% to 20.4% | 0.0% to 32.1% |

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
codeint trend
```

- `codeint init` writes a starter `codeint.json` with a versioned `$schema`.
- `codeint check` measures the working tree and prints each Metric with its Findings.
- `codeint diff main` compares the working tree against the merge-base of `main` and fails on a Regression.
- `codeint trend` prints each Metric across the latest main-line commits.

Or call codeint programmatically from JavaScript/TypeScript (`npm install codeint`) or Rust (`codeint-cli`):

```ts
import { check, diff } from "codeint";

const report = await check();
const prDiff = await diff({ base: "main" });
```

## Documentation

- [Guide](./docs/guide/README.md): modular walkthroughs for [Philosophy and Architecture](./docs/guide/philosophy-and-architecture.md), [Getting Started](./docs/guide/getting-started.md), [Configuration (`codeint.json`)](./docs/guide/configuration.md), [Built-in Languages](./docs/guide/languages.md), and [CI and GitHub Actions](./docs/guide/ci-and-github-actions.md).
- [Reference](./docs/reference.md): tested reference for every command, flag, exit code, report format (`toon`, `json`, `sarif`, `markdown`), and `codeint.json` key.
- [Programmatic API](./docs/programmatic.md): JavaScript/TypeScript SDK and Rust library API.
- [Plugins](./docs/plugins.md): build, hash, declare, and test external WebAssembly Language and Metric plugins.
- [Glossary](./GLOSSARY.md): canonical definitions of every domain term on this page.
- [llms.txt](./llms.txt): compact documentation index for coding agents and LLM tools.

## Development

You need Rust through rustup ([rust-toolchain.toml](./rust-toolchain.toml)), Node.js 24 and npm ([package.json](./package.json)), and `gitleaks` plus `lychee` on your `PATH`.

```sh
npm ci
npx lefthook install
cargo test --workspace --locked
```

Agents and people follow the same rules: start with [.rules](./.rules).
The repository checks are listed in [docs/checks/README.md](./docs/checks/README.md).

## License

[MIT](./LICENSE)
