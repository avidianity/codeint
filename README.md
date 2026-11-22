<h1 align="center">
  <img src="./assets/branding/banner.webp" alt="codeint: see Software entropy grow before it compounds" width="100%">
</h1>

<p align="center">
  <a href="https://github.com/avidianity/codeint/actions/workflows/ci.yml"><img src="https://github.com/avidianity/codeint/actions/workflows/ci.yml/badge.svg?branch=main" alt="CI status"></a>
  <a href="./LICENSE"><img src="https://img.shields.io/github/license/avidianity/codeint?color=275e5e" alt="License: MIT"></a>
</p>

codeint measures the Software entropy of a codebase: duplication, needless verbosity, and complexity concentrated in a few large functions.
It is for developers whose code is increasingly written by coding agents, so they see Software entropy grow before it compounds.

codeint reports each Metric on its own, with the Findings behind it, and never blends them into one score.
It gates CI on per-Metric Budgets and on Regressions against a base Git ref.
Languages and Metrics are Plugins: TypeScript and JavaScript are the first built-in Language plugin, and more languages follow.

<p align="center">
  <a href="#status">Status</a> ·
  <a href="#use">Use</a> ·
  <a href="#github-actions">GitHub Actions</a> ·
  <a href="./docs/reference.md">Reference</a> ·
  <a href="./docs/plugins.md">Plugins</a> ·
  <a href="#development">Development</a>
</p>

[GLOSSARY.md](./GLOSSARY.md) defines every term on this page.

## Status

codeint is in early development and has no release yet.
After the first release, it installs with one command:

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
```

`codeint init` writes a starter `codeint.json`.
`codeint check` measures the working tree and prints each Metric with its Findings.
`codeint diff main` compares the working tree with the merge-base of `main`, and fails on a Regression.
`codeint trend` prints each Metric for each commit of the main line.

[docs/reference.md](./docs/reference.md) documents every command, option, exit code, report format, and `codeint.json` key, with an example for each.
[docs/plugins.md](./docs/plugins.md) explains how to build, hash, declare, and test Language plugins and Metric plugins against the versioned WIT contract.

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
