<h1 align="center">
  <img src="./assets/branding/banner.webp" alt="codeint: see Software entropy grow before it compounds" width="100%">
</h1>

<p align="center">
  <a href="https://github.com/avidianity/codeint/actions/workflows/ci.yml"><img src="https://github.com/avidianity/codeint/actions/workflows/ci.yml/badge.svg?branch=main" alt="CI status"></a>
  <a href="https://github.com/avidianity/codeint/releases/latest"><img src="https://img.shields.io/github/v/release/avidianity/codeint?color=275e5e" alt="GitHub Release"></a>
  <a href="https://www.npmjs.com/package/codeint"><img src="https://img.shields.io/npm/v/codeint?color=275e5e" alt="npm version"></a>
  <a href="./LICENSE"><img src="https://img.shields.io/github/license/avidianity/codeint?color=275e5e" alt="License: MIT"></a>
</p>

codeint measures the Software entropy of a codebase: duplication, needless verbosity, and complexity concentrated in a few large functions.
It is for developers whose code is increasingly written by coding agents, so they see Software entropy grow before it compounds.

codeint reports each Metric on its own, with the Findings behind it, and never blends them into one score.
It gates CI on per-Metric Budgets and on Regressions against a base Git ref.
Languages and Metrics are Plugins: TypeScript/JavaScript, Rust, and PHP ship as built-in Language plugins, and third-party languages and Metrics load as WebAssembly Plugins.

<p align="center">
  <a href="#install">Install</a> ·
  <a href="#use">Use</a> ·
  <a href="#what-to-expect">What to expect</a> ·
  <a href="#github-actions">GitHub Actions</a> ·
  <a href="./docs/reference.md">Reference</a> ·
  <a href="./docs/plugins.md">Plugins</a> ·
  <a href="#development">Development</a>
</p>

[GLOSSARY.md](./GLOSSARY.md) defines every term on this page.

## Install

Install the standalone binary (macOS, Linux, Windows) or run directly with `npx`:

```sh
curl -fsSL https://github.com/avidianity/codeint/releases/latest/download/install.sh | bash
```

```powershell
irm https://github.com/avidianity/codeint/releases/latest/download/install.ps1 | iex
```

```sh
npx codeint check
```

## Use

Run codeint inside a Git repository:

```sh
codeint init
codeint check
codeint diff main
codeint trend
codeint update
```

`codeint init` writes a starter `codeint.json`.
`codeint check` measures the working tree and prints each Metric with its Findings.
`codeint diff main` compares the working tree with the merge-base of `main`, and fails on a Regression.
`codeint trend` prints each Metric for each commit of the main line.
`codeint update` verifies SHA-256 checksums and updates a standalone binary in place from GitHub Releases.

[docs/reference.md](./docs/reference.md) documents every command, option, exit code, report format, and `codeint.json` key, with an example for each.
[docs/plugins.md](./docs/plugins.md) explains how to build, hash, declare, and test Language plugins and Metric plugins against the versioned WIT contract.

## What to expect

codeint measures four independent Production Metrics without blending them into one score:

- **Erosion**: share of Function mass in functions with cyclomatic complexity over 10
- **Duplication**: share of source lines in Clone groups
- **Verbosity**: share of source lines in Flagged lines or Clone groups
- **Import cycles**: share of modules in an Import cycle

### TypeScript and JavaScript Reference ranges

Measured across a panel of 20 open-source TypeScript and JavaScript repositories (10th–90th percentile, [ADR 0008](./docs/adr/0008-typescript-reference-ranges.md)):

| Metric | Typical range (p10–p90) |
| ------ | ----------------------- |
| Erosion | 35.2%–60.0% |
| Duplication | 1.7%–34.6% |
| Verbosity | 1.8%–34.7% |
| Import cycles | 0.0%–30.8% |

### Rust measurements and speed

Sample Production measurements from `codeint check` (release build, Apple Silicon):

| Repository | Files | Prod lines | Erosion | Duplication | Verbosity | Import cycles | Time |
| ---------- | ----- | ---------- | ------- | ----------- | --------- | ------------- | ---- |
| `avidianity/codeint` | 154 | 16,400 | 31.0% | 5.2% | 5.3% | 2.3% | 0.06s |
| `BurntSushi/ripgrep` | 105 | 24,961 | 36.4% | 31.1% | 31.3% | 29.3% | 0.07s |
| `sharkdp/fd` | 25 | 3,423 | 44.6% | 0.0% | 0.1% | 56.5% | 0.02s |
| `dtolnay/serde` | 189 | 19,425 | 45.7% | 40.2% | 40.2% | 0.0% | 0.06s |

### PHP measurements and speed

Sample Production measurements from `codeint check` (release build, Apple Silicon):

| Repository | Files | Prod lines | Erosion | Duplication | Verbosity | Import cycles | Time |
| ---------- | ----- | ---------- | ------- | ----------- | --------- | ------------- | ---- |
| `laravel/laravel` | 29 | 762 | 0.0% | 0.0% | 0.0% | 8.0% | 0.03s |
| `laravel/framework` | 3,208 | 132,791 | 16.9% | 8.1% | 8.2% | 4.4% | 0.47s |

## GitHub Actions

Add `.github/workflows/codeint.yml` to run `codeint diff` on each pull request and upload its SARIF report to GitHub code scanning:

```yaml
name: codeint

on:
  pull_request:

permissions:
  contents: read
  security-events: write

jobs:
  codeint:
    runs-on: ubuntu-24.04
    steps:
      - uses: actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1
        with:
          fetch-depth: 0
      - name: Install codeint
        run: |
          curl -fsSL https://github.com/avidianity/codeint/releases/latest/download/install.sh | bash
          echo "$HOME/.local/bin" >> "$GITHUB_PATH"
      - name: Diff against pull request base
        env:
          BASE_REF: ${{ github.base_ref }}
        run: codeint diff "origin/$BASE_REF" --format sarif --output codeint.sarif
      - name: Upload SARIF
        if: always() && hashFiles('codeint.sarif') != ''
        uses: github/codeql-action/upload-sarif@1bd86f2fdb4b86187fa4059f16f31a3202acc9ea
        with:
          sarif_file: codeint.sarif
```

`fetch-depth: 0` fetches the base branch history so `codeint diff` can find the merge-base.
`if: always() && hashFiles('codeint.sarif') != ''` uploads annotations even when a Budget or a Regression fails.

## Development

You need:

- Rust through rustup; [rust-toolchain.toml](./rust-toolchain.toml) selects the version.
- Node.js 24 and npm, for the development tools in [package.json](./package.json).
- gitleaks and lychee on your `PATH`, for the Git hook.

Set up a checkout:

```sh
npm ci
npx lefthook install
cargo test --workspace --locked
```

Agents and people follow the same rules: start with [.rules](./.rules).
The checks that guard the repository are listed in [docs/checks](./docs/checks/README.md).

## License

[MIT](./LICENSE)
