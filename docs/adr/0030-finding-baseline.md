---
status: accepted
---

# Finding Baseline in `.codeint/baseline.json`

## Context

Teams with an existing codebase want strict Budgets, such as `0` Import boundaries or `0` Hazards, without a large refactor first.
`codeint check` has no Git diff context, so on a developer's machine or inside a coding-agent loop it lists every legacy Finding and fails every strict Budget.
The Ratchet ([ADR 0026](./0026-regression-and-ratchet.md)) caps whole-repository ratios, but it neither hides legacy Findings nor lets a strict Budget pass on legacy code.
Spec #470 asks for a committed Baseline of existing Finding identifiers that `codeint check` hides, that shrinks permanently as legacy Findings are fixed, and that fails loudly on a Metric version change.

## Decision

**Store a Baseline of Finding identifiers per Metric in `.codeint/baseline.json`, hide those Findings in `codeint check`, and compare each Budget with a non-baselined ratio.**

### File

- The Baseline lives at the fixed path `.codeint/baseline.json`, separate from `.codeint/ratchet.json`.
- It holds `schema_version` `1`, then one entry per measured Metric under `source_sets.{source_set}.{metric}` and `dependencies.{metric}`.
- Each entry holds the positive integer Metric version ([ADR 0023](./0023-metric-versioning.md)) and the sorted Finding `id` values.
- Reading and writing share the containment and atomic-write rules of the Ratchet file: no path escapes the repository, symlinks included.

### `codeint check`

- When the file exists, `codeint check` leaves out each Finding whose `id` the Baseline lists for that Metric and Source set.
- Each Metric reports `baselined_findings`, and the report reports `baseline` with `path` and `hidden_findings`.
- Metric ratios stay as measured, so Reference ranges, the Ratchet, and trend comparisons see the whole repository.
- Each Budget compares with the non-baselined ratio: the measured ratio times the share of the Metric's Finding weight that the Baseline does not hide.
- The Finding weight follows the Metric's numerator: Function mass for Erosion, cognitive complexity for Cognitive complexity, untested Function mass for Test gap, distinct lines for Duplication, Verbosity, and Hazards, distinct modules for Import cycles, and one per Finding for the other Metrics, including external Metric plugins.
- A Metric version that differs from the Baseline stops the run with exit code 2 and names `codeint baseline update`.
- `--ignore-baseline` ignores the file completely.
- `codeint diff` does not read the Baseline, because it already compares Findings with the merge-base.

### `codeint baseline update`

- The first run records every current Finding of every measured Metric.
- Later runs remove fixed Findings of a Metric that already has an entry and never add a new Finding to it; the command succeeds and names how many new Findings it left out.
- A measured Metric that has no entry yet, such as Test gap on the first run with a Coverage report, records its current Findings exactly as the first run would, so its legacy Findings never show as new.
- A Metric whose Metric version changed takes the current Findings and the new Metric version.
- `--reset` replaces every measured Metric's Findings with the current Findings.
- `--reset` also regenerates a Baseline file that is not valid JSON or fails validation, and the command says it replaced an invalid file; the file is committed, so Git keeps the old content. Without `--reset`, an invalid file stops the run with exit code 2.
- A Baseline file lists only the dependency Metric keys that the Ratchet allows; any other key is invalid.
- Entries of Metrics that a run does not measure, such as Test gap without a Coverage report, stay unchanged.

## Considered options

- **Recompute every Metric over non-baselined Findings only.**
  Rejected: each Metric computes its numerator differently, and Duplication and Import cycles share lines and modules across Findings.
  Recomputing would duplicate every Metric's rules in the report layer.
  The weight share reuses each Finding's own size and is exact for additive numerators.
- **Fail a Budget only when the Metric has any non-baselined Finding.**
  Rejected: one tiny new Finding would fail a loose Budget that the legacy ratio already exceeds, so the Budget would stop measuring size.
- **Refuse `codeint baseline update` when new Findings exist, like the Ratchet refuses a loosened ceiling.**
  Rejected: the run would also stop fixed Findings from leaving the Baseline, and new Findings already fail `codeint check`.
- **Drop entries of unmeasured Metrics, like the Ratchet.**
  Rejected: an update without a Coverage report would drop every Test gap entry, and the next run with a report would show all legacy Test gap Findings.
- **A configurable path such as `baseline.path`.**
  Rejected for now: Spec #470 names `.codeint/baseline.json`, and no user asked for another path.

## Consequences

- Repositories without `.codeint/baseline.json` report exactly as before.
- A coding agent that runs `codeint check` sees only Findings it introduced, while Budgets keep their meaning for new code.
- A Finding whose code changes gets a new `id`, so editing a legacy function makes it a new Finding; this is intended, because changed code meets the current bar.
- On `--package` runs, a Clone group that crosses the package boundary can get a different `id` than in the whole-repository Baseline and then shows as new.
- A Metric version bump needs one `codeint baseline update` per repository.
