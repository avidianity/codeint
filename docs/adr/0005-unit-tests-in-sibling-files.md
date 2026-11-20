---
status: accepted
---

# Rust unit tests live in sibling test files

The Owner decided this on 2026-11-21 during Bootstrap, to settle a conflict between the Factory Kit and Rust convention.

## Context

Rust convention puts unit tests in a `#[cfg(test)] mod tests { ... }` block inside the source file.
The Factory Kit check behavior-has-test decides from changed paths whether a pull request that changes behavior also changes a test.
Inline test blocks make a test change look like a source change, so the check cannot decide.

## Decision

**Unit tests live in a sibling `tests.rs` file, declared with `#[cfg(test)] mod tests;`, and integration tests live in each crate's `tests/` folder.**
The behavior-has-test check treats paths matching `**/tests.rs` and `**/tests/**` as test files.

## Considered options

- **Inline `mod tests` blocks.**
  Rejected: the check cannot tell a test change from a source change by path.

## Consequences

- Unit tests keep access to private items, as with inline blocks.
- The check stays a simple path rule.
- An inline `mod tests { ... }` block is a convention violation in this Project.
