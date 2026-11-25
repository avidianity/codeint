---
status: accepted
---

# Decouple Verbosity from Duplication and bump Verbosity to Metric version 2

The Owner decided this on 2026-11-25 for Ticket #485 of Spec #450.
This record supersedes the v1 combined Verbosity union recorded in [ADR 0008](./0008-typescript-reference-ranges.md).

## Context

In Verbosity Metric version `1`, `verbose_lines` counted the union of distinct Flagged lines and exact Clone group lines from Duplication.
ADR 0008 kept that combined union on 2026-11-21 while noting that Verbosity tracked Duplication within `0.003` on every repository in the TypeScript and JavaScript panel.

Experience across all seven built-in languages and framework panels showed three problems with the union:

- Duplicated non-flagged code inflated both Duplication and Verbosity by the same lines, violating the core principle of independent Metrics ([ADR 0003](./0003-no-combined-score.md)).
- A pull request that added a duplicated block triggered both a Duplication Regression and a Verbosity Regression even when it introduced zero Flagged lines.
- Real changes in Flagged lines were drowned out in the Verbosity ratio by much larger duplicated blocks, making Verbosity Budgets and Reference ranges redundant with Duplication.

## Decision

**Verbosity counts distinct Flagged lines only, is completely independent of Duplication, and bumps its Metric version from `1` to `2`.**

- `Verbosity::measure` reads only the Flagged lines of the `CodeModel` and takes no `CloneLines` input.
- `verbose_lines` and `flagged_lines` in Verbosity totals both equal the count of distinct `(path, line)` Flagged lines in the Source set; `clone_lines` is removed from the Verbosity report section (while saved v1 reports containing `clone_lines` still deserialize cleanly in `codeint report`).
- Markdown reports render Verbosity's count as Flagged lines of source lines instead of splitting into Flagged lines and clone lines.
- `Metric::Verbosity` increments its `version` in `Metric::DESCRIPTORS` from `1` to `2`, automatically invalidating cached commit measurements for `codeint diff` and `codeint trend` ([ADR 0023](./0023-metric-versioning.md)).
- Built-in Language plugins declare Verbosity Reference ranges at `metric_version: 2` in lockstep with CI while full panel recalibration for Verbosity v2 runs in Spec #452.

## Considered options

- **Keep the v1 union and add a separate Flagged-lines-only Metric.**
  Rejected: keeping a Duplication-dominated Verbosity alongside a Flagged-lines Metric duplicates concepts and still double-counts clone lines in regressions.
- **Keep `clone_lines: 0` on the Verbosity report section for schema compatibility.**
  Rejected: emitting a permanently zero `clone_lines` field under `verbosity` misleads readers and keeps Duplication terminology inside Verbosity; omitting `clone_lines` from v2 output while ignoring unknown fields on deserialization preserves `codeint report` compatibility for saved v1 reports.
- **Keep Verbosity at Metric version `1` after removing clone lines.**
  Rejected: ADR 0023 requires incrementing a Metric's version whenever its calculation rules change so historical reports, commit caches, and Reference ranges never mix v1 and v2 values silently.

## Consequences

- Duplicated non-flagged lines raise Duplication only and leave Verbosity at `0` when no Flagged lines exist.
- Adding a Flagged line inside a duplicated block raises Verbosity by that Flagged line and Duplication by the Clone group lines independently.
- Every JSON, TOON, saved check/diff report, and `codeint trend` report records Verbosity at `metric_version: 2`, and commit caches from v1 invalidate automatically.
- Verbosity Reference ranges across built-in Language plugins target `metric_version: 2` for lockstep consistency and will be recalibrated across the language panels under Spec #452.
