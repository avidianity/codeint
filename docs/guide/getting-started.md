# Getting Started

This guide walks through installing codeint, initializing `codeint.json`, and running your first measurements.

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

## Run inside a Git repository

1. Create a starter `codeint.json` pinned to your installed version's JSON schema:

   ```sh
   codeint init
   ```

2. Measure the current working tree:

   ```sh
   codeint check
   ```

3. Compare your branch against `main` and fail on any Regression:

   ```sh
   codeint diff main
   ```

4. Inspect how each Metric evolved over the latest 20 main-line commits:

   ```sh
   codeint trend
   ```

5. Upgrade a standalone binary in place (and keep `codeint.json`'s `$schema` tag in sync):

   ```sh
   codeint update
   ```

## Exit codes

- `0`: succeeded with no Budget or Regression failures.
- `1`: succeeded, and at least one Budget or Regression failed.
- `2`: operational error (invalid `codeint.json`, missing folder, unknown option, or not inside a Git repository).

See [docs/reference.md](../reference.md) for every command flag and output format (`toon`, `json`, `sarif`, `markdown`).
