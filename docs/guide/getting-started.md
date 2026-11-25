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

1. Create a starter `codeint.jsonc` pinned to your installed version's JSON schema (never overwriting an existing `codeint.json` or `codeint.jsonc`).
   When codeint finds two or more Workspace packages, or a single-package repository matching at least two zones of the `layered` or `hexagonal` preset, it includes a commented-out `boundaries` starter suggestion that stays inactive until you uncomment it:

   ```sh
   codeint init
   ```

2. Verify your repository setup, config validity, detected languages, Workspace packages, and external Plugins offline:

   ```sh
   codeint doctor
   ```

3. Inspect the effective active configuration and where each setting comes from (`default`, `file`, or `cli`):

   ```sh
   codeint config --resolved
   ```

4. Measure the current working tree:

   ```sh
   codeint check
   ```

5. Compare your branch against `main` and fail on any Budget failure or Regression:

   ```sh
   codeint diff main
   ```

6. Save a JSON report once and re-render it losslessly into Markdown, SARIF, or TOON without re-scanning:

   ```sh
   codeint diff main --format json --output diff.json
   codeint report diff.json --format markdown
   codeint report diff.json --format sarif --output codeint.sarif
   ```

7. Inspect how each Metric evolved over the latest 20 main-line commits:

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

- `0`: succeeded with no Budget, Regression, or blocking `codeint doctor` error.
- `1`: succeeded, and at least one Budget, Regression, or blocking `codeint doctor` error failed.
- `2`: operational error (invalid `codeint.jsonc` / `codeint.json`, both config files present at once, missing folder, unknown option, or not inside a Git repository).

See [docs/reference.md](../reference.md) for every command flag and output format (`toon`, `json`, `sarif`, `markdown`, `text`).
