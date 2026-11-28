---
status: accepted
---

# Metric version 2 for Test gap

The Owner decided this on 2026-11-29 during the v0.9.0 reliability and SDK contract pass.

## Context

Test gap measures the share of total Production Function mass that tests do not execute, using line execution counts from named LCOV and Cobertura XML Coverage reports ([ADR 0017](./0017-test-gap-from-coverage-reports.md)).
Language plugins already exclude nested function bodies from an enclosing function's Function mass so each line of source code counts toward one function's mass.
In Test gap version 1, however, coverage attribution scanned every listed line across a function's full `[line, end_line]` lexical span without excluding strictly nested child functions.

This mismatch distorted per-function Findings and repository Test gap ratios across all languages:

- An executed outer function containing an untested nested function or closure inherited the inner function's zero-hit lines, penalizing both the inner function and the outer function.
- A parent function whose only executable lines lived inside nested functions was measured on its children's lines instead of being treated as a function with zero listed lines.
- A Shell top-level `<script>` function wrapping named functions inherited every untested line of those named functions.

Per [ADR 0023](./0023-metric-versioning.md), changing a Metric's calculation rules increments its positive integer Metric version.

## Decision

**Bump Test gap to Metric version `2` and attribute each covered line only to the innermost enclosing function span across all languages.**

- For each file present in the merged Coverage reports, consider every function with a valid span (`line <= end_line`).
- A function `outer` excludes any line that falls inside another function `inner` in the same file whose span is strictly nested inside `outer` (`outer.line <= inner.line && inner.end_line <= outer.end_line && (inner.line, inner.end_line) != (outer.line, outer.end_line)`).
- Attribution runs purely in language-neutral coverage application (`crates/metrics/src/coverage/apply.rs`) without changing Language plugin outputs.
- When two functions share identical start and end lines (for example two functions defined on the same line), neither strictly nests inside the other, so both retain those lines because line-based Coverage reports cannot distinguish column ranges.
- A function whose file appears in a Coverage report, but that owns zero listed lines after excluding strictly nested child spans, receives zero listed lines and is omitted from Test gap numerator and denominator.
- Existing handling for omitted files, unlisted lines, invalid spans (`line > end_line`), and additive line merging across multiple LCOV and Cobertura XML Coverage reports remains unchanged.

## Considered options

- **Push line-exclusion logic into each Language plugin.**
  Rejected: every Language plugin would need to duplicate identical span-nesting logic and widen the WIT contract even though `[line, end_line]` spans are already present in the Code model.
  Doing attribution once in `codeint-metrics` keeps all built-in and external Language plugins consistent automatically.
- **Assign tied same-line functions arbitrarily to the first or last function in declaration order.**
  Rejected: line-based Coverage reports carry no column information, so arbitrary tie-breaking would make one function look tested and the other untested based on source order alone.
  Sharing the line across identical-span functions is deterministic and reflects the actual precision of line coverage.
- **Keep Test gap at Metric version 1 and treat the change as a bug fix.**
  Rejected: [ADR 0023](./0023-metric-versioning.md) requires a Metric version bump whenever calculation rules change reported ratios, so historical reports, Ratchets, and Baselines distinguish version 1 and version 2 numbers cleanly.

## Consequences

- Test gap moves from Metric version `1` to Metric version `2` in `Metric::DESCRIPTORS`, and JSON, TOON, and saved reports emit `metric_version: 2` for `test_gap`.
- Existing `.codeint/ratchet.json` and `.codeint/baseline.json` files that record `test_gap` at version `1` report a clear Metric version mismatch until refreshed with `codeint ratchet update` or `codeint baseline update`.
- Commit measurement caches invalidate automatically per [ADR 0023](./0023-metric-versioning.md).
- Test gap has no published Reference ranges, so no Language plugin Reference range updates are required.
- Outer functions, closures, and Shell `<script>` bodies are no longer double-penalized for untested nested functions.
