# How to change the rules of the Rust Language plugin

## Overview

`plugins/languages/rust` (`codeint-lang-rust`, plugin name `rust`) is a built-in Language plugin that lowers `.rs` files into the language-neutral Code model using `syn` and `proc-macro2` under `#![forbid(unsafe_code)]`.

## Source sets and inline `#[cfg(test)]` exclusion

- `src/source_set.rs` classifies a `.rs` file as `Generated` when a leading comment before the first token contains a generated-file marker, `Test` when its path is under `tests/`, `benches/`, or `examples/` or its name is `tests.rs`, `*_test.rs`, or `*_tests.rs`, and `Production` otherwise.
- When lowering a `Production` file, `src/cfg_test.rs` excludes items annotated with `#[cfg(test)]` from functions, source lines, tokens, Flagged lines, and runtime imports; see [ADR 0009](../../../docs/adr/0009-rust-inline-test-exclusion.md).

## Functions, tokens, and Flagged lines

- `src/functions.rs` extracts `fn`, impl method, and default trait method bodies and counts decision points (`if`, `while`, `for`, `loop`, `match` arms minus one plus arm guards, `&&`, `||`, and `?`).
- `src/tokens.rs` emits tokens from `proc_macro2::TokenStream` and marks `use` and `extern crate` declarations as `TokenKind::ImportOrExport`.
- `src/rules.rs` implements six zero-false-positive structural Flagged-line rules: `else-after-return`, `if-else-returns-boolean`, `trailing-return`, `match-only-propagates`, `match-yields-boolean`, and `redundant-closure`.

## Workspaces and intra-crate imports

- `src/workspaces.rs` reads `[workspace]` `members` and `exclude` globs from the root `Cargo.toml` to report Workspace packages.
- `src/imports.rs` and `src/resolution.rs` resolve `crate::` / `self::` / `super::` `use` paths to intra-crate `.rs` files (`foo.rs` or `foo/mod.rs`), classifying `std`/`core`/`alloc`/`proc_macro` and external crates as `External` (bare `mod foo;` tree declarations are not treated as runtime imports so standard parent-child module hierarchies do not create false cycles).

## Reference ranges

- `reference_ranges` returns the 10th to 90th percentile bands from the 20-repository pre-2023 Rust panel recorded in [ADR 0010](../../../docs/adr/0010-rust-reference-ranges.md).
