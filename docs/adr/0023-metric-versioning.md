---
status: accepted
---

# Explicit integer Metric versions

The Owner decided this on 2026-11-25 while introducing the `0.4.0` Plugin contract and preparing to decouple Verbosity from Duplication.

## Context

Every built-in Metric and external Metric plugin implements specific calculation rules.
When those rules change, such as decoupling Verbosity from Duplication clone lines, historical numbers no longer mean the same thing.
Without an explicit version on each Metric, three things fail silently:

- Historical JSON, TOON, and saved reports cannot tell a reader which calculation rules produced a reported ratio.
- Commit-measurement caches used by `codeint diff` and `codeint trend` can serve stale cached measurements from an older formula unless someone bumps an unrelated global cache counter by hand.
- Published Reference ranges can drift behind a formula change without CI catching the mismatch or informing users of third-party Language plugins.

## Decision

**Every built-in Metric and every external Metric plugin declares a positive integer Metric version starting at `1`, and increments that integer whenever its calculation rules change.**

- Each built-in Metric stores its `version: u32` in the single `Metric::DESCRIPTORS` table in `codeint-model`.
- Each external Metric plugin exports `version() -> u32` in `wit/metric.wit` (`codeint:plugin@0.4.0`), and the Plugin host rejects `0` at load time.
- Every JSON, TOON, and saved check or diff report emits `metric_version` on each Metric object (including Dependency hygiene and external Metric plugins), and `codeint trend` emits a `metric_versions` map for the reported Metrics.
  `schema/codeint-report.schema.json` requires a positive integer `metric_version` on every Metric object.
- Commit-measurement cache keys for `codeint diff` and `codeint trend` include the sorted map of every built-in Metric version and every loaded external Metric plugin's version.
  Bumping any Metric version automatically invalidates matching commit cache entries and recomputes them without manual cache clearing.
- Every Reference range record in `wit/language.wit` carries `metric-version: u32`.
  CI enforces exact lockstep matching for built-in Language plugins, while external Language plugins hide mismatched Reference ranges at runtime with an explicit note instead of failing the run.

## Considered options

- **Rely on codeint's package version alone.**
  Rejected: codeint releases change far more often than Metric formulas, so package versions cannot distinguish a formula change from an unrelated release, and external Metric plugins version independently.
- **Use SemVer strings for Metric versions.**
  Rejected: a Metric formula has no backwards-compatible minor or patch semantics; any change that alters ratios requires re-baselining caches, Ratchets, and Reference ranges, so a positive integer is simpler and unambiguous.
- **Bump a single global cache version manually whenever any Metric changes.**
  Rejected: manual global bumps are easy to forget, do not cover external Metric plugins, and do not record which Metric changed in reports or Reference ranges.

## Consequences

- Historical JSON, TOON, and saved reports carry the exact `metric_version` that produced each Metric value.
- Saved reports written before `metric_version` existed still deserialize cleanly in `codeint report`, defaulting missing `metric_version` fields to `1`.
- Changing a built-in Metric's version in `Metric::DESCRIPTORS` immediately invalidates commit caches for `codeint diff` and `codeint trend` and fails CI Reference range lockstep checks until affected Reference ranges are updated.
- Changing an external Metric plugin's `version()` export automatically invalidates commit caches for that repository without manual intervention.
- Downstream features such as `.codeint/ratchet.json` can store and compare `metric_version` directly against `Metric::built_in_versions()` and `Metric::versions_with_plugins()`.
