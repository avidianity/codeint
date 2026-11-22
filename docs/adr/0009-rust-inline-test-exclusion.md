# 0009. Exclude inline `#[cfg(test)]` blocks from Rust Production measurements

Date: 2026-11-22
Status: Accepted

## Context

`GLOSSARY.md` states that every source file belongs to exactly one Source set: Production, Test, Generated, or Vendored.
In TypeScript and JavaScript, tests live in separate files, so file-level classification cleanly separates Production code from Test code.
In Rust, integration tests live under `tests/`, benchmarks under `benches/`, and examples under `examples/`, but many repositories place unit tests inline inside Production `.rs` files under `#[cfg(test)] mod tests { ... }` blocks.

If a Rust Production file with an inline `#[cfg(test)]` block were measured as-is, its unit tests would inflate Production Erosion, Verbosity, Duplication, and Import cycles.
Conversely, splitting one file across two Source sets would violate the glossary invariant that every source file belongs to exactly one Source set and would complicate file-level reporting.

## Decision

The Rust Language plugin classifies every `.rs` file into one Source set by path and generated-file header:

- `Generated` when a header comment before the first token of code contains a generated-file marker.
- `Test` when the path is under `tests/`, `benches/`, or `examples/`, or the file name is `tests.rs`, `*_test.rs`, or `*_tests.rs`.
- `Production` for all other `.rs` files (with `Vendored` and build-output folders settled by the core).

When lowering a `Production` `.rs` file, the Rust Language plugin excludes items annotated with `#[cfg(test)]` (such as `#[cfg(test)] mod tests { ... }` and `#[cfg(test)] use ...`) from functions, source lines, tokens, Flagged lines, and runtime imports.

## Consequences

- Every `.rs` file still belongs to exactly one Source set, preserving the glossary invariant and the Code model contract.
- Inline unit tests in Production Rust files never distort Production Metrics or create false Production Import cycles.
- Dedicated test files (such as `tests/*.rs` and sibling `tests.rs` files) are still classified as `Test` and measured in the `Test` Source set.
