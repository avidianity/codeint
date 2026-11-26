# Adopting codeint on an existing codebase

A mature codebase already holds hundreds of Findings.
You do not need to fix them before you adopt codeint.
Freeze today's state on day one, then stop new decay locally and in pull requests, and let the legacy debt shrink as you touch old code.

This page calls that workflow stop the bleeding.
It uses four pieces:

| Piece | File or key | What it does |
| ----- | ----------- | ------------ |
| Ratchet | `.codeint/ratchet.json` | Caps each whole-repository Metric ratio at its current value |
| Baseline | `.codeint/baseline.json` | Hides existing Findings from `codeint check`, so only new Findings count |
| Zero Budgets | `budgets.production` and `budgets.test` in `codeint.jsonc` | Fail `codeint check` on any new Finding, because the Baseline hides the legacy Findings from them |
| New-file Budgets | `new_file_budgets` in `codeint.jsonc` | Holds every file that a pull request adds to a strict bar |
| Per-file check | `regression_limits.per_file` in `codeint.jsonc` | Fails a pull request when any modified file gets worse |

## Adopt in one command

Run this at the root of your Git repository, or in any folder inside it:

```sh
codeint init --adopt
```

The command does five things:

1. It writes a starter `codeint.jsonc` with `"regression_limits": { "per_file": true }` and a commented-out `new_file_budgets` block.
2. It measures the whole repository once.
3. It writes `.codeint/ratchet.json` and `.codeint/baseline.json` from that measurement.
4. It adds a zero Budget under `budgets.production` and `budgets.test` for each Metric that the Baseline records, such as `"erosion": 0`, and a zero Budget for each Dependency hygiene Metric.
   It adds no Test gap Budget, because Test gap needs a Coverage report that you name.
5. It prints the next steps, including a pull-request workflow for GitHub Actions.

It never replaces a file.
When `codeint.jsonc`, `codeint.json`, `.codeint/config.jsonc`, `.codeint/config.json`, `.codeint/ratchet.json`, or `.codeint/baseline.json` already exists, it stops with exit code 2 and writes nothing.
In that case, edit the existing file, or use the commands in the sections below.
When only the configuration file exists, the message names `codeint ratchet update` and `codeint baseline update`, which create the Ratchet and Baseline for it.

Commit `codeint.jsonc` and the `.codeint` folder.
Then run `codeint check`:

```sh
codeint check
```

The run passes.
The Baseline hides every existing Finding, so each zero Budget sees a non-baselined ratio of `0`, and the Ratchet holds each ratio at its current value.
The report counts the hidden Findings, so the legacy debt stays visible.

Add a new Finding, such as a new eroded function, and `codeint check` fails with exit code 1:

```text
codeint: the non-baselined production erosion ratio 0.4714 is over its Budget 0
```

The zero Budget for that Metric catches the new Finding, because the Baseline does not hide it.
The same happens when one change adds a new eroded file and tidies a legacy one: the tidied file leaves the Ratchet ceiling slack, but the new Finding still fails the zero Budget.
The Ratchet alone would miss that swap; it caps whole-repository ratios, not individual Findings.
A Metric that the run did not measure has no Budget, and Test gap has none until you add one and name a Coverage report.
That failure is the point: a coding agent or a developer sees it before the code reaches review.

## Gate pull requests

Save the workflow that `codeint init --adopt` prints as `.github/workflows/codeint.yml`:

```yaml
name: codeint

on:
  pull_request:

permissions:
  contents: read
  pull-requests: write

jobs:
  codeint:
    runs-on: ubuntu-24.04
    steps:
      - uses: actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1
        with:
          fetch-depth: 0
      - uses: avidianity/codeint@main
        with:
          version: v0.7.1
```

`fetch-depth: 0` lets `codeint diff` find the Git merge-base.
`version` pins CI to the codeint release that wrote your Ratchet and Baseline, so CI and your machine use the same Metric versions; change it when you upgrade codeint and re-run `codeint ratchet update` and `codeint baseline update`.
See [CI and GitHub Actions](./ci-and-github-actions.md) for the action inputs, SARIF upload, and the single-scan workflow.

## The Ratchet: `.codeint/ratchet.json`

The Ratchet stores one ceiling per Metric and Source set, with the Metric version that produced it.
`codeint check` and `codeint diff` fail when a Metric rises above its ceiling.

Lower the ceilings after you improve the code:

```sh
codeint ratchet update
```

The command only tightens ceilings.
It refuses to loosen a ceiling for an unchanged Metric version, unless you pass `--reset`.
When a codeint upgrade changes a Metric version, `codeint check` fails and names the command that re-records that Metric.
See [`codeint ratchet`](../reference.md#codeint-ratchet).

## The Baseline: `.codeint/baseline.json`

The Baseline stores each existing Finding's identifier and hashed surrounding-line context anchors (never raw source text), with the Metric version that produced it.
`codeint check` hides those Findings and both `codeint check` and `codeint diff` evaluate Budgets on the Findings that remain, pairing duplicate Findings by surrounding-line context so inserting an identical Finding above legacy code reports the new line.
This lets you set a strict Budget, such as `"erosion": 0`, on day one without failing on legacy code; `codeint init --adopt` writes those zero Budgets for you.

Remove fixed Findings after you clean up:

```sh
codeint baseline update
```

The command never adds a new Finding to the Baseline.
It names the count of new Findings that it left out.
Pass `--reset` only when you accept new Findings as legacy, or when the file is invalid.

To plan a cleanup sprint, show every Finding:

```sh
codeint check --ignore-baseline
```

See [`codeint baseline`](../reference.md#codeint-baseline).

## New-file Budgets: `new_file_budgets`

Legacy files stay as they are, but new files can meet a strict bar.
Uncomment the block that `codeint init --adopt` wrote, and add a comma after the key above it:

```jsonc
"new_file_budgets": {
  "production": {
    "erosion": 0,
    "cognitive_complexity": 0,
    "hazards": 0,
    "boundaries": 0
  }
}
```

`codeint diff <base>` measures only the files that exist in head and not at the merge-base.
It fails with exit code 1 when those files exceed a new-file Budget.
A pull request that adds no file passes this check without noise.
See [New-file Budgets](../reference.md#new-file-budgets).

## Per-file checks: `per_file`

By default, `codeint diff` compares whole-repository numerators.
A pull request can delete one large legacy function in one file and add a complex function in another file, and still pass.

`codeint init --adopt` turns on `regression_limits.per_file`.
With it, `codeint diff` also checks each modified file on its own, and names the file, the Metric, and the base and head values when one gets worse.
The default stays `false` for repositories that have not opted in, so moving an unchanged function between files still passes there.
See [Regression limits](../reference.md#regression-limits).

## Daily workflow

1. Write code.
   Run `codeint check` in your editor loop or coding agent loop; the zero Budgets (production, test, and dependency Metrics) fail it on any new Finding of a recorded Metric.
2. Open a pull request.
   The workflow runs `codeint diff` and blocks Regressions, new-file Budget failures, and per-file Regressions.
3. When you fix legacy code, run `codeint ratchet update` and `codeint baseline update`, and commit both files in the same pull request.

Both files only shrink over time, so the legacy debt trends to zero.

## Related pages

- [Getting Started](./getting-started.md)
- [Configuration](./configuration.md)
- [CI and GitHub Actions](./ci-and-github-actions.md)
- [`codeint init`](../reference.md#codeint-init)
- [ADR 0026: Direct numerator-rise Regression rules and `.codeint/` Ratchet ceilings](../adr/0026-regression-and-ratchet.md)
- [ADR 0030: Finding Baseline in `.codeint/baseline.json`](../adr/0030-finding-baseline.md)
- [ADR 0032: Context-anchored Finding matching and Baseline schema version 2](../adr/0032-context-anchored-finding-matching.md)
