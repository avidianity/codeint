---
status: accepted
---

# Direct numerator-rise Regression rules and `.codeint/` Ratchet ceilings

The Owner decided this on 2026-11-25 for Spec #450 (Tickets #487 and #491).

## Context

In earlier 0.x releases, `codeint diff` failed on `numerator_rise_with_new_findings` for core Metrics (Erosion, Cognitive complexity, Duplication, Verbosity, and external Metrics) while allowing a `0.01` ratio-rise buffer.
That rule left two gaps that let Software entropy grow silently across pull requests:

- An existing Finding could worsen in place (for example, an eroded function gaining more branches or lines without changing its identifier), raising the numerator while creating zero new Finding identifiers, and slipping past CI until the ratio crossed `0.01`.
- Small ratio increases below `0.01` across consecutive pull requests compounded on the default branch because no machine-managed repository ceiling tracked the best ratios reached over time.

Separately, storing machine-updated state alongside hand-edited configuration at the repository root clutters root folders and mixes human-authored policy with automated baseline state.

## Decision

**Adopt direct numerator rise (`numerator_rise`) as the default Regression rule for core Metrics, enforce `any_new_finding` by default on structural Metrics and Hazards, introduce the `.codeint/` repository state folder with four-path configuration discovery, and add an opt-in `.codeint/ratchet.json` Ratchet managed by `codeint ratchet update`.**

### Regression rules

- Replace `numerator_rise_with_new_findings` with direct numerator rise (`numerator_rise`): a pull request fails whenever `head_numerator > base_numerator` (rounded to 4 decimal places), even when no new Finding identifier appears.
- Keep the `0.01` `ratio_rise` buffer when the numerator does not rise, so deleting clean code or moving an unchanged Finding across files passes.
- Default Import cycles, Boundaries, Orphan files, Hazards, and Dependency hygiene to `any_new_finding` (plus numerator rise), so a new Finding cannot hide behind removing an unrelated Finding elsewhere.
- Accept `numerator_rise` as canonical in `regression_limits` and continue accepting `numerator_rise_with_new_findings` as a deprecated 0.x alias.

### `.codeint/` repository state folder and four-path configuration discovery

- Recognize four configuration file locations at the repository root: `codeint.json`, `codeint.jsonc`, `.codeint/config.json`, and `.codeint/config.jsonc`.
- Fail fast with exit code `2` and list every conflicting file whenever more than one of those four files exists.
- Keep `codeint init` writing `codeint.jsonc` at the repository root and refusing to overwrite any existing configuration file in any of the four locations.

### `.codeint/ratchet.json` Ratchet

- Store repository-wide Ratchet ceilings in `.codeint/ratchet.json` by default (configurable via `ratchet.path`, restricted to repository-relative paths without `..` or escaping symlinks).
- Record each measured Metric's ceiling ratio (rounded to 4 decimal places) alongside its positive integer Metric version ([ADR 0023](./0023-metric-versioning.md)) across per-Source-set Metrics, Dependency hygiene, Production Test gap when `coverage_reports` are configured, and loaded external Metric plugins.
- Enforce the Ratchet automatically in whole-repository `codeint check` and `codeint diff` runs whenever the Ratchet file exists:
  - Fail with exit code `1` and report `over_ratchet` in JSON, TOON, Markdown, and SARIF when any measured Metric rises above its recorded ceiling.
  - Fail loudly with exit code `2` naming every stale Metric, its recorded versus current Metric version, and the exact re-baseline command `codeint ratchet update` when any measured Metric's Metric version does not match the Ratchet file.
  - Skip Ratchet enforcement with an explicit `ratchet_note` on `--package` scoped runs because Ratchet ceilings apply to the whole repository.
- Provide `codeint ratchet update` to manage the Ratchet file atomically:
  - Create the Ratchet file (and `.codeint/` parent folder if missing) on first run.
  - Monotonically lower ceilings whenever a Metric improves (`current < stored`), keep equal ceilings unchanged, and automatically refresh Metrics whose Metric version changed.
  - Refuse with exit code `2` without modifying the file when any unchanged-version Metric regressed (`current > stored`), unless `--reset` is explicitly passed.

## Considered options

- **Keep `numerator_rise_with_new_findings` and rely only on hand-tuned Budgets.**
  Rejected: existing Findings could grow worse in place without tripping CI, and hand-tuned Budgets stay loose after teams clean up code unless someone remembers to tighten them by hand.
- **Embed Ratchet ceilings inside `codeint.json` / `codeint.jsonc`.**
  Rejected: mixing machine-rewritten ceilings into a hand-edited JSONC file loses comments, causes merge conflicts, and blurs human policy with automated baseline state.
- **Silently skip Ratchet entries whose Metric version changed.**
  Rejected: silently disabling CI ceilings after an upgrade hides regressions; failing loudly with the exact `codeint ratchet update` command keeps enforcement explicit and auditable.
- **Allow `codeint ratchet update` to loosen ceilings automatically without a flag.**
  Rejected: an unconditional update makes it too easy to paper over a regression in a pull request; requiring `--reset` makes intentional loosening explicit in command history and code review.

## Consequences

- Worsening an existing eroded function, complex function, Clone group, or Flagged line now fails `codeint diff` immediately under `numerator_rise`, while moving unchanged code or deleting clean code within the `0.01` buffer still passes.
- Teams can keep all codeint files inside `.codeint/` (`config.jsonc` and `ratchet.json`) or keep `codeint.jsonc` at the root with `.codeint/ratchet.json` for state.
- Repositories that opt into `.codeint/ratchet.json` get automatic whole-repository ceiling protection in both `codeint check` and `codeint diff`, with monotonic tightening via `codeint ratchet update` and loud version-mismatch diagnostics after formula upgrades.
- `--package` runs continue to work cleanly in monorepos while surfacing a clear `ratchet_note` that whole-repository Ratchet enforcement was skipped.
