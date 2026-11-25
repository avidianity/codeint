# Stack instructions

This page applies before you add, remove, or upgrade a dependency, and when you create a crate, a module, or a test.
codeint is a Rust command-line tool that analyzes code in any language that has a Language plugin.
TypeScript and JavaScript are the first built-in Language plugin.
It ships through npm and GitHub Releases.

Keep the core language-neutral.
A rule that differs between languages, such as decision points, import resolution, workspace detection, Source set rules, or Flagged-line rules, lives in a Language plugin, never in the core or in a Metric plugin.

## Mandatory: toolchain

- The Rust toolchain is pinned in [rust-toolchain.toml](../../rust-toolchain.toml).
  Change it only in its own pull request.
- Every crate uses Rust edition 2024.
- Workspace lints live in the `[workspace.lints]` table of [Cargo.toml](../../Cargo.toml).
  Each crate opts in with `[lints] workspace = true`, except `crates/plugin-sdk`, which repeats every workspace lint except `unsafe_code`; see [ADR 0027](../adr/0027-plugin-sdk-guest-runtime-unsafe.md).
- Run `cargo fmt --all` before you commit.
  The hook rejects unformatted code.

## Mandatory: workspace layout

| Path | Holds |
| ---- | ----- |
| `crates/{name}/` | One product crate per folder; the folder name is the package name without the `codeint-` prefix |
| `plugins/languages/{name}/` | One built-in Language plugin crate per folder (package name `codeint-lang-{name}`) |
| `crates/config/` | The only crate that reads environment variables, process arguments, or files of configuration |
| `crates/plugin-host/` | The WebAssembly Plugin host and its capability grants; changes are Tier 3 |
| `crates/plugin-sdk/` | The Rust guest SDK for external Plugins; it depends on no other workspace crate |
| `wit/` | The published WIT interface of Plugins |
| `npm/` | The npm wrapper package and the platform packages |
| `xtask/` | The Project's own checks, run as `cargo xtask {check}` |

Each crate root declares `#![forbid(unsafe_code)]`; see [ADR 0004](../adr/0004-forbid-unsafe-code.md).
The only exception is `crates/plugin-sdk`: its crate root holds no `unsafe`, its guest runtime module `src/runtime.rs` holds all of it, and every other module declares `#![forbid(unsafe_code)]`; see [ADR 0027](../adr/0027-plugin-sdk-guest-runtime-unsafe.md).

## Mandatory: dependencies

- Pin every direct dependency to an exact version: `oxc_parser = "=0.159.0"`.
  [ADR 0006](../adr/0006-exact-dependency-pins.md) records why.
- Declare shared dependencies once in `[workspace.dependencies]`, and use `{ workspace = true }` in each crate.
- Commit `Cargo.lock`.
  Every Cargo command in CI uses `--locked`.
- Find the newest version with `cargo info {crate}` or the crates.io API, then read the docs of that exact version on docs.rs.
- Dev tools for checks live in the root [package.json](../../package.json), also with exact versions, and `package-lock.json` is committed.

## Mandatory: tests

- Put unit tests in a sibling file `tests.rs`, declared with `#[cfg(test)] mod tests;` in the module under test.
  Never write an inline `mod tests { ... }` block; see [ADR 0005](../adr/0005-unit-tests-in-sibling-files.md).
- Put integration tests in the crate's `tests/` folder.
- Put test input files under the crate's `tests/fixtures/` folder.
  Write each fixture yourself; never copy one from another repository.
- `cargo test --workspace --locked` runs every test.
  `cargo xtask affected-tests` runs the tests of the crates that the branch changed and of every crate that depends on them.

## Mandatory: errors and output

- A library crate returns typed errors; only the command-line crate turns them into an exit code and a message.
- Exit codes: `0` when every check passes, `1` when a Budget or a Regression fails, `2` for an operational error.

## Default: lints

The workspace denies every warning that the compiler and Clippy give.
A lint that does not fit the Project changes for the whole workspace in `Cargo.toml`, through an ADR.
Never add `#[allow(...)]` or `#[expect(...)]`; the check [no-suppressions](../checks/no-suppressions.md) rejects it.
