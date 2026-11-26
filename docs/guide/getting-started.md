# Getting Started

This guide walks through installing codeint, initializing `codeint.jsonc`, and running your first measurements, diagnostics, and codebase queries.

## Install

Install the standalone binary on macOS or Linux:

```sh
curl -fsSL https://github.com/avidianity/codeint/releases/latest/download/install.sh | bash
```

Install the standalone binary on Windows (PowerShell):

```powershell
irm https://github.com/avidianity/codeint/releases/latest/download/install.ps1 | iex
```

Or run directly through npm without a global install:

```sh
npx codeint check
```

## First measurements inside a Git repository

1. Create a starter configuration file pinned to your installed version's JSON schema (writes `codeint.jsonc` by default, or pass `--format json|toml|yaml`; never overwrites an existing config across `codeint.{json,jsonc,toml,yaml,yml}` or `.codeint/config.{json,jsonc,toml,yaml,yml}`).
   When codeint finds two or more Workspace packages, or a single-package repository matching at least two zones of the `layered` or `hexagonal` preset, it includes a commented-out `boundaries` starter suggestion that stays inactive until you uncomment it:

   ```sh
   codeint init
   ```

   On an existing codebase with many Findings, run `codeint init --adopt` instead.
   It also enables `per_file`, writes zero Budgets for each Metric it records in the Baseline, suggests `new_file_budgets`, writes `.codeint/ratchet.json` and `.codeint/baseline.json`, and prints a pull-request workflow; see [Adopting codeint on an existing codebase](./adopting-on-an-existing-codebase.md).

2. Verify your repository setup, config validity, detected languages, Workspace packages, and external Plugins offline:

   ```sh
   codeint doctor
   ```

3. Check an external Wasm Plugin against the Plugin contract before you declare it in `codeint.jsonc`; use `--kind metric` for a Metric plugin:

   ```sh
   codeint plugin test plugins/my-plugin.wasm
   ```

4. Inspect the effective active configuration and where each setting comes from (`default`, `file`, or `cli`):

   ```sh
   codeint config --resolved
   ```

5. Measure the current working tree (pass `--format text` for a concise monochrome terminal summary, or set `"format": "text"` in `codeint.jsonc` as the repository default):

   ```sh
   codeint check
   codeint check --format text
   ```

6. Compare your branch against `main` and fail on any Budget, Ratchet, or Regression failure:

   ```sh
   codeint diff main
   ```

7. Lock in current whole-repository ratios in `.codeint/ratchet.json` and monotonically tighten them as code improves (use `--reset` only when intentionally loosening unchanged-version ceilings):

   ```sh
   codeint ratchet update
   ```

   On an existing codebase, also freeze today's Findings in `.codeint/baseline.json`, so `codeint check` hides them and fails only on new Findings (`codeint check --ignore-baseline` shows every Finding):

   ```sh
   codeint baseline update
   ```

8. Save a JSON report once and re-render it losslessly into concise `text`, Markdown, SARIF, or TOON without re-scanning:

   ```sh
   codeint diff main --format json --output diff.json
   codeint report diff.json --format text
   codeint report diff.json --format markdown
   codeint report diff.json --format sarif --output codeint.sarif
   ```

9. Inspect how each Metric evolved over the latest 20 main-line commits:

   ```sh
   codeint trend
   ```

## Navigation, boundaries, and Git history queries

Use codeint before editing or refactoring to understand architectural rules and blast radius:

- Check architectural boundary zones and allowed imports for an existing or planned file:

  ```sh
  codeint guard src/domain/order.ts
  ```

- Find the shortest runtime import path between two modules:

  ```sh
  codeint trace src/routes/checkout.ts src/db/pool.ts
  ```

- Compute the transitive reverse-dependency blast radius of files or everything changed since `main`:

  ```sh
  codeint impact src/domain/order.ts
  codeint impact --changed main
  ```

- Rank eroded and duplicated files alongside their Git commit churn independently per Metric:

  ```sh
  codeint churn --since 90d
  ```

- Find pairs of files that repeatedly change together in the same commits:

  ```sh
  codeint coupling --since 90d
  ```

- Upgrade a standalone binary in place (and keep your configuration file's `$schema` tag in sync):

  ```sh
  codeint update
  ```

## Exit codes

- `0`: succeeded with no Budget, Ratchet, Regression, blocking `codeint doctor` error, or failed `codeint plugin test` check.
- `1`: succeeded, and at least one Budget, Ratchet, Regression, blocking `codeint doctor` error, or `codeint plugin test` check failed.
- `2`: operational error (invalid configuration file, more than one configuration file across `codeint.{json,jsonc,toml,yaml,yml}` and `.codeint/config.{json,jsonc,toml,yaml,yml}`, Ratchet or Baseline Metric version mismatch, missing folder, unknown option, or not inside a Git repository).

See [docs/reference.md](../reference.md) for every command flag and output format (`toon`, `json`, `sarif`, `markdown`, `text`).
