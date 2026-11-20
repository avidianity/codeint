---
status: accepted
---

# Every crate forbids unsafe code

The Owner decided this on 2026-11-21 during Bootstrap, to settle a conflict between the Factory Kit and Rust convention.

## Context

The Factory Kit check no-comments rejects new code comments.
Rust convention requires a `// SAFETY:` comment on every `unsafe` block, and Clippy can enforce it.
The planned dependencies, wasmtime, oxc, and gix, expose safe APIs, so codeint itself needs no `unsafe`.

## Decision

**Every crate root declares `#![forbid(unsafe_code)]`.**
No `unsafe` block and no `SAFETY` comment exist in codeint.
Doc comments in the published WIT interface stay allowed, because they document a published API.

## Considered options

- **Allow `unsafe` with a `SAFETY` comment exception in no-comments.**
  Rejected: it opens an exception in the comment rule for code that nothing needs.

## Consequences

- The compiler rejects any `unsafe` block, so the no-comments rule has no Rust exception.
- A future need for `unsafe` requires a new ADR that supersedes this one.
