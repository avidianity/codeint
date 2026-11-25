# codeint reference

This page documents every command, option, exit code, report format, and `codeint.json` key of codeint.
[GLOSSARY.md](../GLOSSARY.md) defines every term on this page.
For calling codeint from JavaScript, TypeScript, or Rust, see [docs/programmatic.md](./programmatic.md).

## Sample repository

Every example on this page runs in the same sample repository at `/home/me/shop`.
Branch `main` has two commits:

1. `Add sign`, on 2026-11-02, adds `src/sign.ts` with a Flagged line, `src/types.d.ts` with type declarations only, and `src/broken.ts`, which does not parse.
2. `Add parse`, on 2026-11-09, adds `src/parse.ts` with an eroded function.

`origin/main` points to the same commit as `main`.
Branch `feature` starts at `main` and is checked out.
Its commit `Simplify parse`, on 2026-11-16, makes `src/parse.ts` simple and adds `src/total.ts`.
The working tree adds `src/route.ts`, an eroded function that is not committed.

## Commands

### `codeint`

| Command | Meaning |
| ------- | ------- |
| `check` | Measure the working tree and print the report; see [`codeint check`](#codeint-check) |
| `diff` | Compare the working tree, or a head Git ref, with the merge-base of a base Git ref; see [`codeint diff`](#codeint-diff) |
| `trend` | Measure each commit of the main line and print each Metric over time; see [`codeint trend`](#codeint-trend) |
| `impact` | Compute the transitive reverse-dependency blast radius of one or more repository files; see [`codeint impact`](#codeint-impact) |
| `trace` | Find the shortest runtime import path between two files in the repository; see [`codeint trace`](#codeint-trace) |
| `init` | Write a starter `codeint.jsonc`, and with `--adopt` the initial Ratchet and Baseline; see [`codeint init`](#codeint-init) |
| `update` | Update a standalone codeint binary in place from GitHub Releases; see [`codeint update`](#codeint-update) |
| `mcp` | Run a local stdio Model Context Protocol (MCP) server exposing `check`, `diff`, `trend`, `guard`, `impact`, `trace`, `churn`, and `coupling` tools; see [`codeint mcp`](#codeint-mcp) |
| `guard` | Show the architectural boundary zone, allowed and denied target zones, `public_files` constraints, allowed importers, and external-package rules for a file path; see [`codeint guard`](#codeint-guard) |
| `schema` | Print a machine-readable capability manifest of CLI commands, flags, built-in Metrics, output formats, MCP tools, and exit codes; see [`codeint schema`](#codeint-schema) |
| `config` | Inspect the effective active configuration and where each setting came from; see [`codeint config`](#codeint-config) |
| `report` | Re-render a saved `check` or `diff` JSON report into another format without re-running analysis; see [`codeint report`](#codeint-report) |
| `doctor` | Run read-only diagnostics on `codeint.json`, Git repository status, detected languages, Workspace packages, and external Wasm Plugins; see [`codeint doctor`](#codeint-doctor) |
| `agent` | Inspect and configure codeint MCP integration for local coding agents; see [`codeint agent`](#codeint-agent) |
| `ratchet` | Manage the repository Ratchet ceilings in `.codeint/ratchet.json`; see [`codeint ratchet`](#codeint-ratchet) |
| `baseline` | Manage the repository Baseline of existing Findings in `.codeint/baseline.json`; see [`codeint baseline`](#codeint-baseline) |
| `churn` | Rank eroded and duplicated files alongside their Git commit churn independently per Metric; see [`codeint churn`](#codeint-churn) |
| `coupling` | Detect pairs of files that repeatedly change together in the same commits and flag direct runtime imports; see [`codeint coupling`](#codeint-coupling) |
| `help` | Print the help of codeint, or of the command that follows it |

| Option | Meaning |
| ------ | ------- |
| `-h, --help` | Print the help of codeint; every command also takes `-h, --help` and prints its own help |
| `-V, --version` | Print the version of codeint |

```sh exit=0
codeint --version
```

```text stdout
codeint 0.6.0
```

```sh exit=0
codeint --help
codeint -h
codeint -V
codeint help
codeint help check
```

### `codeint init`

| Option | Meaning |
| ------ | ------- |
| `--adopt` | Also enable per-file Regression checks, write a zero Budget for each Production and Test Metric that the Baseline records, suggest new-file Budgets, write `.codeint/ratchet.json` and `.codeint/baseline.json` from the current state, and print pull-request CI instructions |
| `-h, --help` | Print the help of `codeint init` |

`codeint init` writes a starter `codeint.jsonc` at the root of the Git repository that contains the current folder:

```sh exit=0
codeint init
```

```text stdout
Wrote /home/me/shop/codeint.jsonc
```

The file links to the JSON schema of the running codeint version, so your editor completes and checks it.
It sets every key to its default, and lists under `languages` each top-level folder or file that a Language plugin measures:

```json file=codeint.jsonc
{
  "$schema": "https://raw.githubusercontent.com/avidianity/codeint/v0.6.0/schema/codeint.schema.json",
  "source_sets": {
    "production": [],
    "test": [],
    "generated": [],
    "vendored": []
  },
  "budgets": {
    "production": {
      // "test_gap": 0.2
    },
    "test": {},
    "generated": {},
    "vendored": {}
  },
  "regression_limits": {},
  "languages": [
    {
      "plugin": "typescript-javascript",
      "include": [
        "src"
      ],
      "exclude": []
    }
  ],
  "coverage_reports": []
}
```

When no file belongs to a Language plugin, the file leaves `languages` out, so every Language plugin measures the whole repository.
When `sql` files are detected, `codeint init` adds a commented `sql.dialects` hint to the starter file.
`codeint init` also adds a commented-out Import boundary starter block only when high-confidence signals exist:
when built-in Language plugins discover two or more Workspace packages, it suggests commented-out `boundaries.zones` entries for each Workspace package;
in a single-package repository whose Production source files match at least two zones of the `layered` or `hexagonal` preset, it suggests the matching commented-out `boundaries.preset`;
in a flat or custom layout without those signals, it emits no boundary suggestions.
Because the suggested block stays commented out, `codeint check` and `codeint config` pass cleanly immediately after `codeint init`.
`codeint init --adopt` onboards an existing codebase in one step ([Adopting codeint on an existing codebase](./guide/adopting-on-an-existing-codebase.md)).
It writes the same starter `codeint.jsonc` with three differences: `regression_limits.per_file` is `true`, `budgets.production` and `budgets.test` hold a zero Budget for each Metric that the Baseline records, and a commented-out `new_file_budgets` block follows the last key.
It then measures the whole repository once, writes `.codeint/ratchet.json` as `codeint ratchet update` would on a first run, writes `.codeint/baseline.json` as `codeint baseline update` would on a first run, adds the zero Budgets to `codeint.jsonc`, and prints the next steps, including a pull-request workflow.
The zero Budgets cover the Production Metrics that the run measures without extra inputs (Erosion, Cognitive complexity, Duplication, Verbosity, Import cycles, Boundaries, Orphan files, and Hazards), the same Metrics except Orphan files under `budgets.test`, and the three Dependency hygiene Metrics under `budgets`.
A Metric that the run does not measure gets no Budget; Test gap gets none, because codeint never guesses Coverage reports, and its commented `test_gap` line stays.
Because the Baseline hides every existing Finding, `codeint check` compares each zero Budget with the non-baselined ratio, which is `0` on day one, so `codeint check` passes immediately afterward.
A new Finding of a recorded Metric makes the non-baselined ratio positive, so `codeint check` fails with exit code 1 and `the non-baselined production erosion ratio 0.4714 is over its Budget 0`.
That holds even when the same change tidies a legacy Finding, because the new Finding itself fails the Budget.
The Ratchet separately caps each whole-repository ratio, and it alone does not catch such a swap.
`codeint init --adopt` never replaces a file: it stops with exit code 2, before it writes anything, when a configuration file, `.codeint/ratchet.json`, or `.codeint/baseline.json` already exists.
When measuring or writing the state files fails, it removes the files it wrote and exits with code 2; when a removal fails too, the error names every path left behind.
Run `codeint ratchet update` or `codeint baseline update` instead to refresh an existing state file.
When a configuration file already exists, the message points to `codeint ratchet update` and `codeint baseline update`, which create the state files for it.

```sh exit=0
codeint init --adopt
```

```text stdout
Wrote /home/me/shop/codeint.jsonc
Wrote /home/me/shop/.codeint/ratchet.json (5 Metric ceilings)
Wrote /home/me/shop/.codeint/baseline.json (2 existing Findings)

Next steps:
1. Commit codeint.jsonc and the .codeint folder.
2. Run codeint check locally and in your coding agent loop; the Baseline hides the existing Findings, and the zero Budgets in codeint.jsonc (production, test, and dependency Metrics) fail on any new Finding of a Metric the Baseline records; the Ratchet separately caps each whole-repository ratio.
3. Save this as .github/workflows/codeint.yml to gate every pull request:

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
          version: v0.6.0

4. After you fix legacy code, run codeint ratchet update and codeint baseline update, and commit the result.
5. To hold every new file to a strict bar, uncomment new_file_budgets in codeint.jsonc.

Guide: https://github.com/avidianity/codeint/blob/main/docs/guide/adopting-on-an-existing-codebase.md
```

`codeint init` never replaces an existing `codeint.jsonc` or `codeint.json`.
With this `codeint.json` in place:

```json codeint.json
{}
```

`codeint init` stops with exit code 2:

```sh exit=2
codeint init
```

```text stderr
codeint: /home/me/shop/codeint.json already exists, and codeint init never replaces it
Edit the existing codeint.json, or delete it and run codeint init again.
```

### `codeint check`

| Argument | Meaning |
| -------- | ------- |
| `[PATH]` | A folder inside the Git repository to measure; the current folder by default |

| Option | Meaning |
| ------ | ------- |
| `--package <PATH>` | Scope measurement and output to this Workspace package |
| `--output <FILE>` | Write the report to this file instead of standard output |
| `--all` | List every Finding instead of the 20 worst per Metric |
| `--format <FORMAT>` | The format of the report: `toon`, the default, `json`, `sarif`, or `markdown`; see [Report formats](#report-formats) |
| `--owner <OWNER>` | Include only Findings whose `CODEOWNERS` owner matches this pattern |
| `--coverage-report <PATH>` | Path to an LCOV or Cobertura XML Coverage report; may repeat; when present, replaces `coverage_reports` from `codeint.json`; see [Test gap](#test-gap) |
| `--ignore-baseline` | Show every Finding and evaluate Budgets on every Finding, as if `.codeint/baseline.json` did not exist; see [`codeint baseline`](#codeint-baseline) |
| `-h, --help` | Print the help of `codeint check` |

`codeint check` measures the working tree, including uncommitted and untracked files, and prints the report:

```sh exit=0
codeint check
```

```text stdout
files: 4
source_lines: 39
source_sets:
  production:
    files: 4
    source_lines: 39
    erosion:
      ratio: 0.7986
      metric_version: 1
      reference_range:
        low: 0.352
        high: 0.6
        source: "codeint panel of 20 TypeScript and JavaScript repositories, ADR 0008, codeint 0.1.0"
      eroded_function_mass: 67.8823
      total_function_mass: 85.0032
      findings[1]{id,path,line,end_line,name,cyclomatic_complexity,cognitive_complexity,source_lines,function_mass}:
        f0740d06c0975d68,src/route.ts,1,18,route,16,15,18,67.8823
    cognitive_complexity:
      ratio: 0
      metric_version: 1
      reference_range: no published Reference range for this language
      complex_cognitive_complexity: 0
      total_cognitive_complexity: 20
      findings: []
    duplication:
      ratio: 0
      metric_version: 1
      reference_range:
        low: 0.017
        high: 0.346
        source: "codeint panel of 20 TypeScript and JavaScript repositories, ADR 0008, codeint 0.1.0"
      clone_lines: 0
      clone_groups: []
    verbosity:
      ratio: 0.0256
      metric_version: 2
      reference_range:
        low: 0.018
        high: 0.347
        source: "codeint panel of 20 TypeScript and JavaScript repositories, ADR 0008, codeint 0.1.0"
      verbose_lines: 1
      flagged_lines: 1
      findings[1]{id,path,line,rule}:
        2a57800b29941394,src/sign.ts,4,else-after-return
    import_cycles:
      ratio: 0
      metric_version: 1
      reference_range:
        low: 0
        high: 0.308
        source: "codeint panel of 20 TypeScript and JavaScript repositories, ADR 0008, codeint 0.1.0"
      modules_in_import_cycles: 0
      modules: 4
      findings: []
      unresolved_imports:
        count: 0
        imports: []
test_gap_note: Test gap is not measured because no Coverage report was named.
skipped[1]{path,reason}:
  src/broken.ts,"cannot parse: Unexpected token at line 1, column 23"
declaration_only[1]: src/types.d.ts
```

codeint measures the files that Git does not ignore.
It does not follow symbolic links, does not measure Git submodules, and skips files that are not valid UTF-8.
It lists each file it cannot parse under `skipped`, and each file that holds only type declarations under `declaration_only`.
When `--coverage-report` or `coverage_reports` names one or more Coverage reports, `codeint check` reports Test gap (`test_gap`) for the Production Source set, per language, and per Workspace package, with Findings ordered heaviest untested Function mass first, and prints one note naming any unmatched report paths.
When no Coverage report is named, or when no report covers a language, Test gap is absent (never `0` or `1`) and the report states why in one line; see [Test gap](#test-gap).

These commands measure the same repository with other options:

```sh exit=0
codeint check src
codeint check --all
codeint check --owner @team
codeint check --coverage-report coverage/lcov.info
codeint check --ignore-baseline
codeint check --format markdown --output report.md
```

A `PATH` that does not exist stops the run with exit code 2:

```sh exit=2
codeint check missing
```

```text stderr
codeint: missing does not exist
Pass a folder that exists, or leave the path out to measure the current folder.
```

Passing `--package <PATH>` in a repository without that Workspace package stops the run with exit code 2:

```sh exit=2
codeint check --package packages/missing
```

### `codeint diff`

| Argument | Meaning |
| -------- | ------- |
| `<BASE_REF>` | The base Git ref, such as `main` or `origin/main`; codeint measures its merge-base with the head |

| Option | Meaning |
| ------ | ------- |
| `--head <REF>` | Measure this Git ref from Git objects instead of the working tree; the merge-base is then taken with this ref instead of HEAD |
| `--package <PATH>` | Scope measurement and output to this Workspace package |
| `--output <FILE>` | Write the report to this file instead of standard output |
| `--all` | List every Finding instead of the 20 worst per Metric |
| `--format <FORMAT>` | The format of the report: `toon`, the default, `json`, `sarif`, or `markdown` |
| `--owner <OWNER>` | Include only Findings whose `CODEOWNERS` owner matches this pattern |
| `-h, --help` | Print the help of `codeint diff` |

`codeint diff {base-ref}` compares the working tree with the merge-base of the base ref and HEAD.
In the sample repository, the working tree adds the eroded function of `src/route.ts`, so the comparison finds a [Regression](#regressions) and exits with code 1:

```sh exit=1
codeint diff main
```

```text stdout
base:
  ref: main
  merge_base: 2ef203655bd71559d3e54d1ab60a2390fb90674c
head: working tree
source_sets:
  production:
    erosion:
      base_ratio: 0.8946
      head_ratio: 0.7986
      metric_version: 1
      new[1]{id,path,line,end_line,name,cyclomatic_complexity,cognitive_complexity,source_lines,function_mass}:
        f0740d06c0975d68,src/route.ts,1,18,route,16,15,18,67.8823
      resolved[1]{id,path,line,end_line,name,cyclomatic_complexity,cognitive_complexity,source_lines,function_mass}:
        1b3d7d358ce8632e,src/parse.ts,1,14,parse,12,11,14,44.8999
      unchanged: 0
    cognitive_complexity:
      base_ratio: 0
      head_ratio: 0
      metric_version: 1
      new: []
      resolved: []
      unchanged: 0
    duplication:
      base_ratio: 0
      head_ratio: 0
      metric_version: 1
      new: []
      resolved: []
      unchanged: 0
    verbosity:
      base_ratio: 0.0476
      head_ratio: 0.0256
      metric_version: 2
      new: []
      resolved: []
      unchanged: 1
    import_cycles:
      base_ratio: 0
      head_ratio: 0
      metric_version: 1
      new: []
      resolved: []
      unchanged: 0
regressions[1]{source_set,metric,rule,base,head,new_findings,ratio_rise_limit}:
  production,erosion,numerator_rise,44.8999,67.8823,1,0.01
```

```text stderr
codeint: the production erosion numerator eroded_function_mass rose from 44.8999 to 67.8823
```

codeint reads the merge-base straight from Git objects, so it never checks out a commit and leaves the working tree and the index as they are.
The working tree side includes uncommitted and untracked changes, and leaves out the files that Git ignores, as `codeint check` does.
Commits added to the base branch after your branch started are not part of the comparison, because the merge-base comes before them.

Both sides use the `codeint.json` of the working tree, so the comparison shows changes in the code, not in the configuration.
Both sides follow the same file rules as `codeint check`.
`codeint diff` computes Test gap for neither side; when a Coverage report is named, it prints one line stating that Test gap is left out because a Coverage report exists only for the code under measurement.

The report classifies each Finding by its `id`: new when only the head side has it, resolved when only the merge-base has it, and unchanged when both have it.
For each Metric of each Source set, it shows the ratio on both sides, the new and the resolved Findings, worst first, and the number of unchanged Findings.
It lists at most 20 new and 20 resolved Findings per Metric, and counts the rest in `new_left_out` and `resolved_left_out`; `--all` lists every one.
`--format json` and `--format markdown` print the same comparison.
`--format sarif` lists only the new Findings, so code scanning annotates what the change added.

```sh exit=1
codeint diff origin/main --all
codeint diff main --format json
codeint diff main --format markdown
codeint diff main --format sarif --output diff.sarif
```

A base ref that names no commit stops the run with exit code 2 and names the ref:

```sh exit=2
codeint diff no-such-branch
```

```text stderr
codeint: the Git ref "no-such-branch" does not name a commit in this repository: couldn't parse revision, input="no-such-branch": Reference "no-such-branch" could not be found: NotFound
Pass a branch, tag, or commit that exists in this repository; fetch it first when it lives on a remote: git fetch origin main
```

Passing `--package <PATH>` for a Workspace package that does not exist stops the run with exit code 2:

```sh exit=2
codeint diff main --package packages/missing
```

#### Compare two refs

`--head {ref}` measures a second ref instead of the working tree, so you can compare two commits or two branches.
The commit of `feature` only makes `src/parse.ts` simpler, so this comparison passes:

```sh exit=0
codeint diff main --head feature
```

```text stdout
base:
  ref: main
  merge_base: 2ef203655bd71559d3e54d1ab60a2390fb90674c
head:
  ref: feature
  commit: e3bc223e22791aa541f8104438ce1606515231ce
source_sets:
  production:
    erosion:
      base_ratio: 0.8946
      head_ratio: 0
      metric_version: 1
      new: []
      resolved[1]{id,path,line,end_line,name,cyclomatic_complexity,cognitive_complexity,source_lines,function_mass}:
        1b3d7d358ce8632e,src/parse.ts,1,14,parse,12,11,14,44.8999
      unchanged: 0
    cognitive_complexity:
      base_ratio: 0
      head_ratio: 0
      metric_version: 1
      new: []
      resolved: []
      unchanged: 0
    duplication:
      base_ratio: 0
      head_ratio: 0
      metric_version: 1
      new: []
      resolved: []
      unchanged: 0
    verbosity:
      base_ratio: 0.0476
      head_ratio: 0.0476
      metric_version: 2
      new: []
      resolved: []
      unchanged: 1
    import_cycles:
      base_ratio: 0
      head_ratio: 0
      metric_version: 1
      new: []
      resolved: []
      unchanged: 0
```

codeint reads the head ref from Git objects too, so neither the working tree nor HEAD takes part.
The merge-base is the merge-base of the base ref and the head ref.
Both sides still use the `codeint.json` of the working tree, never the one in a commit.
So the classification matches running `codeint check` on a checkout of each side, when each checkout has that same `codeint.json`.

A head ref that names no commit, or that shares no history with the base ref, stops the run with exit code 2:

```sh exit=2
codeint diff main --head no-such-branch
```

```text stderr
codeint: the Git ref "no-such-branch" does not name a commit in this repository: couldn't parse revision, input="no-such-branch": Reference "no-such-branch" could not be found: NotFound
Pass a branch, tag, or commit that exists in this repository; fetch it first when it lives on a remote: git fetch origin main
```

When a repository has a `CODEOWNERS` file (`.github/CODEOWNERS`, `CODEOWNERS`, or `docs/CODEOWNERS`), codeint attaches matched owners to Findings and `--owner <OWNER>` filters Findings to those matching the pattern:

```sh exit=1
codeint diff main --owner @team
```

#### Regressions

A Regression is a Metric that got worse from the merge-base to the head side by more than its limit.
`codeint diff` checks each Metric of each Source set against three default rules:

- Any new Finding on Import cycles, Boundaries, Orphan files, Hazards, or Dependency hygiene is a Regression (`any_new_finding`, plus numerator rise).
- A ratio rise over 0.01 is a Regression.
  codeint compares the ratios as the report shows them, rounded to four decimal places, so a rise of exactly 0.01 passes when the numerator does not rise.
- Any numerator rise (rounded to four decimal places) is a Regression (`numerator_rise`), even when no new Finding identifier appeared.
  The numerator is `eroded_function_mass`, `complex_cognitive_complexity`, `clone_lines`, `verbose_lines`, `modules_in_import_cycles`, `boundary_findings`, `orphan_files`, `hazard_lines`, `unused`, `unlisted`, or `dev_in_production`.
  A Finding that only moves to another file, so the numerator stays the same, is not a Regression under `numerator_rise`.
- When `regression_limits.per_file` is `true` (`false` by default), `codeint diff` also compares file-level Metric numerators and Findings for every modified file.
  If any individual modified file worsens, `codeint diff` reports a per-file Regression naming that file, Metric, base value, and head value even when another file shrank enough to keep the repository total flat or lower.

[`regression_limits`](#regression-limits) in `codeint.json` changes these limits.
`codeint diff` also applies the [Budgets](#budgets) of `codeint.json` to the head side, as `codeint check` does.

When a Budget or a Regression fails, codeint still writes the whole report, lists each failure under `over_budget` or `regressions`, names it on standard error, and exits with code 1.
Each Regression names its rule and its base and head values: the ratios for `ratio_rise` and `new_findings`, the repository numerators for `numerator_rise`, and the file-level numerators alongside `file` for `per_file_numerator_rise` and `per_file_new_findings`.
In SARIF output, per-file Regressions also appear under `runs[0].properties.perFileRegressions`.
`ratio_rise_limit` is the ratio rise limit of the Metric in that Source set.
The output of `codeint diff main` [above](#codeint-diff) shows a Regression.
With no failure, the report has no `over_budget` or `regressions` key, and codeint exits with code 0.

### `codeint trend`

| Option | Meaning |
| ------ | ------- |
| `--since <YYYY-MM-DD>` | Measure every main-line commit committed on or after this day, in UTC, instead of the latest 20 |
| `--output <FILE>` | Write the report to this file instead of standard output |
| `--format <FORMAT>` | The format of the report: `toon`, the default, `json`, or `markdown` |
| `-h, --help` | Print the help of `codeint trend` |

`codeint trend` measures each commit of the main line of the current branch and prints each Metric over time, oldest commit first:

```sh exit=0
codeint trend
```

```text stdout
commits: 3
metric_versions:
  cognitive_complexity: 1
  duplication: 1
  erosion: 1
  import_cycles: 1
  verbosity: 2
source_sets:
  production[3]{commit,committed_on,files,erosion,cognitive_complexity,duplication,verbosity,import_cycles}:
    2864291867e0e3a3ef91cd050c1893d5ae9eb809,2026-11-02,1,0,0,0,0.1429,0
    2ef203655bd71559d3e54d1ab60a2390fb90674c,2026-11-09,2,0.8946,0,0,0.0476,0
    e3bc223e22791aa541f8104438ce1606515231ce,2026-11-16,3,0,0,0,0.0476,0
```

The main line is the first-parent history of HEAD: a merge commit is a row, and the commits of the branch it merged are not.
Without `--since`, codeint measures the latest 20 commits of the main line.
`--since {YYYY-MM-DD}` measures every main-line commit committed on or after the start of that day in UTC, with no limit on the count:

```sh exit=0
codeint trend --since 2026-11-05
```

```text stdout
commits: 2
metric_versions:
  cognitive_complexity: 1
  duplication: 1
  erosion: 1
  import_cycles: 1
  verbosity: 2
source_sets:
  production[2]{commit,committed_on,files,erosion,cognitive_complexity,duplication,verbosity,import_cycles}:
    2ef203655bd71559d3e54d1ab60a2390fb90674c,2026-11-09,2,0.8946,0,0,0.0476,0
    e3bc223e22791aa541f8104438ce1606515231ce,2026-11-16,3,0,0,0,0.0476,0
```

codeint compares the committer date, the date that `git log --since` uses, and stops at the first main-line commit committed before that day.
In a shallow clone, the trend stops at the oldest commit the clone has.

The report has one table per Source set, with one row per commit.
Each row shows the commit, the day it was committed in UTC, the number of measured files, and the ratio of each Metric.
A Source set appears only when at least one commit of the trend has a file in it.
`--format json` and `--format markdown` print the same rows:

```sh exit=0
codeint trend --format markdown
codeint trend --format json --output trend.json
```

A trend lists no Findings, so it has no SARIF format; run `codeint diff` for the Findings of a change.
A trend applies no Budget and no Regression limit, and exits with code 0 when it measures every commit.
`codeint trend` computes Test gap for no commit; when a Coverage report is named, it prints one line stating that Test gap is left out because a Coverage report exists only for the code under measurement.
A day that is not a real day stops the run with exit code 2:

```sh exit=2
codeint trend --since 2026-02-30
```

```text stderr
error: invalid value '2026-02-30' for '--since <YYYY-MM-DD>': "2026-02-30" is not a real day written as YYYY-MM-DD, such as 2026-01-31

For more information, try '--help'.
```

codeint reads each commit from Git objects, so it never checks out a commit and never touches the working tree; uncommitted changes are not part of a trend.
Every commit uses the `codeint.json` of the working tree, as `codeint diff` does.
Each commit goes through the [result cache](#result-cache), so a repeated trend measures only the commits that are new since the last run.

### `codeint impact`

| Argument | Meaning |
| -------- | ------- |
| `[PATH]...` | Repository file paths to analyze |

| Option | Meaning |
| ------ | ------- |
| `--changed <BASE_REF>` | Analyze files changed since the merge-base of this Git ref and HEAD |
| `--depth <N>` | Maximum reverse-dependency hop distance to traverse |
| `--package <PATH>` | Scope reported dependents to this Workspace package |
| `--output <FILE>` | Write the report to this file instead of standard output |
| `--format <FORMAT>` | The format of the report: `toon`, the default, `json`, or `text` |
| `-h, --help` | Print the help of `codeint impact` |

`codeint impact` computes the transitive reverse-dependency blast radius of one or more repository files, distinguishing direct dependents (distance 1) from transitive dependents (distance 2 or more), separating Production dependents from Test dependents, and listing affected Workspace packages:

```sh exit=0
codeint impact src/sign.ts
codeint impact --changed main
codeint impact src/sign.ts --depth 1 --format text
codeint impact src/sign.ts --format toon
codeint impact src/sign.ts --format json --output impact.json
```

Passing `--package <PATH>` for a Workspace package that does not exist stops the run with exit code 2:

```sh exit=2
codeint impact src/sign.ts --package packages/missing
```

### `codeint trace`

| Argument | Meaning |
| -------- | ------- |
| `<FROM>` | Repository-relative path of the starting module |
| `<TO>` | Repository-relative path of the target module |

| Option | Meaning |
| ------ | ------- |
| `--all` | List all shortest runtime import paths up to `--max-paths` |
| `--max-paths <COUNT>` | Maximum number of shortest paths to list when `--all` is set |
| `--package <PATH>` | Scope the query to this Workspace package |
| `--output <FILE>` | Write the report to this file instead of standard output |
| `--format <FORMAT>` | The format of the report: `toon`, the default, `json`, or `text` |
| `-h, --help` | Print the help of `codeint trace` |

`codeint trace` finds the shortest runtime import path connecting two measured files in the working tree.
When `<FROM>` or `<TO>` is a Production file that its Language plugin marks as a program start, the report names `"program-start mark"` as the entry-point reason in `from_entry_point` or `to_entry_point` (and `Entry point: <path> (program-start mark)` in `text` format).
When a step on a traced path comes from one import that leads to several files, the report lists that import and every file it leads to in `multi_file_imports` (and `Import <specifier> in <importer> leads to: <targets>` in `text` format).
When no runtime import path connects them, it reports `connected: false` and exits with code 0:

```sh exit=0
codeint trace src/sign.ts src/parse.ts
```

```text stdout
from: src/sign.ts
to: src/parse.ts
connected: false
shortest_distance: null
paths: []
truncated: false
```

These commands exercise `--all`, `--max-paths`, `--output`, and each output format:

```sh exit=0
codeint trace src/sign.ts src/parse.ts --format toon
codeint trace src/sign.ts src/parse.ts --format text
codeint trace src/sign.ts src/parse.ts --all --max-paths 5 --format json --output trace.json
```

Passing `--package <PATH>` for a Workspace package that does not exist stops the run with exit code 2:

```sh exit=2
codeint trace src/sign.ts src/parse.ts --package packages/missing
```

### `codeint update`

| Option | Meaning |
| ------ | ------- |
| `--check` | Check whether a newer release is available without modifying the binary |
| `--version <TAG>` | Install this release tag, such as `v0.1.0`, instead of the latest release |
| `-h, --help` | Print the help of `codeint update` |

`codeint update` downloads the matching platform archive and `SHA256SUMS` from GitHub Releases, verifies the SHA-256 digest before touching the binary, and atomically replaces a standalone `codeint` executable in place.
When run inside a repository whose `codeint.json` uses the codeint `$schema` URL, `codeint update` also updates that `$schema` tag to match the installed release.
If the running binary lives inside `node_modules`, `codeint update` refuses to overwrite it and exits `2` with the package-manager update command instead.

```sh exit=0
codeint update --check --version v0.6.0
```

```text stdout
codeint 0.6.0 is already up to date (v0.6.0)
```

### `codeint mcp`

| Option | Meaning |
| ------ | ------- |
| `-h, --help` | Print the help of `codeint mcp` |

`codeint mcp` runs a local Model Context Protocol (MCP) server over standard input and standard output with no open network port.
It exposes eight read-only tools (`check`, `diff`, `trend`, `guard`, `impact`, `trace`, `churn`, and `coupling`) so coding agents can measure Software entropy and Hazards, inspect architectural boundaries, reverse-dependency blast radius, and import paths, rank high-churn eroded or duplicated files, detect co-change coupling, and fix Regressions before opening a pull request.
The `check` MCP tool takes `path`, `package`, `coverage_reports`, `all`, and `ignore_baseline` (boolean, `false` by default; the same meaning as `codeint check --ignore-baseline`).
Its stale-Baseline error names `ignore_baseline: true` as the way to show every Finding.
It uses `coverage_reports` from `codeint.json`, returning Test gap (or its absent form and reason), Test gap Findings ordered heaviest untested Function mass first, any Test gap Budget failure, and the missing-report error when `budgets.production.test_gap` is set with Test gap absent.
When a measured language defines Hazard line rules (such as SQL), `check`, `diff`, and `trend` also return `hazards`, Hazard line Findings (path, line, and rule name only), `sql_schemas`, and `rejected_statements`.
When standard input closes, the server exits with code 0:

```sh exit=0
codeint mcp
```

### `codeint guard`

| Argument | Meaning |
| -------- | ------- |
| `<PATH>` | Path to an existing or planned file inside the Git repository |

| Option | Meaning |
| ------ | ------- |
| `--format <FORMAT>` | The format of the output: `text`, the default, `toon`, or `json` |
| `-h, --help` | Print the help of `codeint guard` |

`codeint guard <path>` evaluates the architectural boundary rules (both internal zone rules and `allow_external`/`deny_external` external-package rules) in `codeint.json` or `codeint.jsonc` for an existing or planned file path without requiring the file to exist on disk.

```sh exit=0
codeint guard src/route.ts
codeint guard src/route.ts --format text
codeint guard src/route.ts --format toon
codeint guard src/route.ts --format json
```

### `codeint schema`

| Option | Meaning |
| ------ | ------- |
| `--output <FILE>` | Write the schema manifest to this file instead of standard output |
| `--format <FORMAT>` | The format of the schema manifest: `json`, the default, or `toon` |
| `-h, --help` | Print the help of `codeint schema` |

`codeint schema` prints a deterministic, machine-readable manifest describing the running binary version, every CLI command and flag, built-in Metrics, Source sets, built-in Language plugins, output formats, MCP tools, and exit codes.
It is derived directly from live CLI and MCP definitions, works outside a Git repository, and stays 100% local and offline.

```sh exit=0
codeint schema
codeint schema --format json
codeint schema --format toon
codeint schema --output schema.json
```

### `codeint config`

| Argument | Meaning |
| -------- | ------- |
| `[PATH]` | A folder inside the Git repository to inspect; the current folder by default |

| Option | Meaning |
| ------ | ------- |
| `--resolved` | Print the effective active configuration and per-field provenance |
| `--format <FORMAT>` | The format of the output: `text`, the default, `toon`, or `json` |
| `--budget <SPEC>` | Override a Budget ratio as `SOURCE_SET.METRIC=RATIO` |
| `--source-set <SPEC>` | Override a Source set glob as `SOURCE_SET=GLOB` |
| `--plugin-memory-mb <MB>` | Override `plugin_limits.memory_mb` (1 to 4096) |
| `--plugin-timeout-ms <MS>` | Override `plugin_limits.timeout_ms` (1 to 300000) |
| `-h, --help` | Print the help of `codeint config` |

`codeint config --resolved` prints every effective active setting merged from built-in defaults, `codeint.json`, and CLI overrides alongside its provenance (`default`, `file`, or `cli`).

```sh exit=0
codeint config --resolved
codeint config --resolved src --format text
codeint config --resolved --format toon
codeint config --resolved --format json --budget production.erosion=0.25 --source-set test=spec/** --plugin-memory-mb 128 --plugin-timeout-ms 10000
```

### `codeint report`

| Argument | Meaning |
| -------- | ------- |
| `<INPUT>` | Path to a saved `check` or `diff` JSON report, or `-` to read from standard input |

| Option | Meaning |
| ------ | ------- |
| `--output <FILE>` | Write the report to this file instead of standard output |
| `--format <FORMAT>` | The format of the report: `toon`, the default, `json`, `sarif`, `markdown`, or `text` |
| `-h, --help` | Print the help of `codeint report` |

`codeint report` re-renders a saved `codeint check --format json` or `codeint diff --format json` payload into another format without re-running analysis.
Pass `-` as `<INPUT>` to read the JSON payload from standard input.
`--format text` prints the same compact text report as `--format toon`.

```sh exit=0
codeint report report.json --format toon
codeint report report.json --format json
codeint report report.json --format sarif --output report.sarif
codeint report report.json --format markdown
codeint report report.json --format text
```

An input file that does not exist, malformed JSON, or an unsupported report schema version stops the run with exit code 2:

```sh exit=2
codeint report missing.json --format sarif
```

### `codeint doctor`

| Argument | Meaning |
| -------- | ------- |
| `[PATH]` | A folder to inspect; the current folder by default |

| Option | Meaning |
| ------ | ------- |
| `--format <FORMAT>` | The format of the output: `text`, the default, `json`, or `toon` |
| `-h, --help` | Print the help of `codeint doctor` |

`codeint doctor` runs strictly read-only, offline diagnostics on Git repository status, `codeint.json` validity, `$schema` version compatibility, detected Language plugins, Workspace packages, and external Wasm Plugin paths, SHA-256 digests, and interface compatibility.
It separates passing checks, warnings, and blocking errors, exiting with code 0 when no blocking error is found and code 1 when at least one blocking error is found.

```sh exit=0
codeint doctor
codeint doctor src
codeint doctor --format text
codeint doctor --format json
codeint doctor --format toon
```

### `codeint agent`

| Command | Meaning |
| ------- | ------- |
| `status` | Report detected local coding agents and whether `codeint mcp` is registered for each |
| `install` | Register `codeint mcp` in supported local coding agent configuration files |
| `help` | Print the help of `codeint agent`, or of the subcommand that follows it |

| Option | Meaning |
| ------ | ------- |
| `-h, --help` | Print the help of `codeint agent` |

`codeint agent` manages local `codeint mcp` registration for Claude Code (`claude-code`, `.claude.json`), Cursor (`cursor`, `.cursor/mcp.json`), and Codex (`codex`, `.codex/config.toml`).
Both `status` and `install` accept `--client <claude-code|cursor|codex>`, `--format <text|toon|json>`, and `--config-root <DIR>` (or the `CODEINT_AGENT_HOME` environment variable) to override the home folder.
`codeint agent install` also accepts `--dry-run` to preview the exact resulting configuration without modifying any file on disk.
When writing a configuration file, `codeint agent install` preserves unrelated keys and existing MCP servers, writes atomically via a temporary file rename, and leaves already-registered configurations untouched on repeated runs.

```sh exit=0
CODEINT_AGENT_HOME=.agent-home codeint agent status
CODEINT_AGENT_HOME=.agent-home codeint agent status --format json
CODEINT_AGENT_HOME=.agent-home codeint agent status --format toon
CODEINT_AGENT_HOME=.agent-home codeint agent install --dry-run --client claude-code
codeint agent help
```

### `codeint ratchet`

| Command | Meaning |
| ------- | ------- |
| `update` | Create or monotonically tighten `.codeint/ratchet.json` from the current working tree |
| `help` | Print the help of `codeint ratchet`, or of the subcommand that follows it |

| Option | Meaning |
| ------ | ------- |
| `-h, --help` | Print the help of `codeint ratchet` |

`codeint ratchet update` measures the whole repository and writes each measured Metric's ceiling ratio (rounded to 4 decimal places) alongside its positive integer Metric version to `.codeint/ratchet.json` (or the configured `ratchet.path`).
It creates the Ratchet file on the first run, lowers ceilings whenever a Metric improves, keeps equal ceilings unchanged, and automatically refreshes entries whose Metric version changed.
When any unchanged-version Metric has risen above its stored ceiling, `codeint ratchet update` stops with exit code 2 without modifying the file unless you pass `--reset`.
When the existing Ratchet file is not valid JSON or fails validation, `codeint ratchet update --reset` replaces it with a regenerated file and says so; without `--reset` it stops with exit code 2.
`codeint ratchet update` accepts an optional `[PATH]` folder inside the repository, `--reset`, `--coverage-report <PATH>` (repeatable), and `--format <text|toon|json>` (`text` by default).

```sh exit=0
codeint ratchet update
codeint ratchet update --format json
codeint ratchet update --reset --format toon
codeint ratchet help
```

### `codeint baseline`

| Command | Meaning |
| ------- | ------- |
| `update` | Create `.codeint/baseline.json`, or remove fixed Findings from it, from the current working tree |
| `help` | Print the help of `codeint baseline`, or of the subcommand that follows it |

| Option | Meaning |
| ------ | ------- |
| `-h, --help` | Print the help of `codeint baseline` |

A **Baseline** is a committed list of existing Findings in `.codeint/baseline.json` that `codeint check` hides and removes permanently as those Findings are fixed ([ADR 0030](./adr/0030-finding-baseline.md)).
It stores the Finding `id` values of each measured Metric per Source set, and of Dependency hygiene, alongside each Metric's positive integer Metric version.
A repository without `.codeint/baseline.json` reports exactly as before.

`codeint baseline update` measures the whole repository and writes `.codeint/baseline.json`:

- On the first run, it records every current Finding of every measured Metric.
- When a run measures a Metric that has no entry in the Baseline yet, such as Test gap on the first run with a Coverage report, it records that Metric's current Findings, as the first run would.
- For a Metric that already has an entry, it removes each Finding that is fixed and never adds a new Finding; it names the count of new Findings it left out.
- When a Metric's Metric version changed, it replaces that Metric's Findings with the current Findings and records the new Metric version.
- With `--reset`, it replaces every measured Metric's Findings with the current Findings, so new Findings join the Baseline.
- It keeps the entries of Metrics that this run does not measure, such as Test gap without a Coverage report.

When the existing Baseline file is not valid JSON or fails validation, `codeint baseline update --reset` replaces it with a regenerated file and says so; without `--reset` it stops with exit code 2.
The file is committed, so the old content is recoverable from Git.
`codeint baseline update` accepts an optional `[PATH]` folder inside the repository, `--reset`, `--coverage-report <PATH>` (repeatable), and `--format <text|toon|json>` (`text` by default).

```sh exit=0
codeint baseline update
codeint baseline update --format json
codeint baseline update --reset --format toon
codeint baseline help
```

When `.codeint/baseline.json` exists, `codeint check` (including `--package` runs and the MCP `check` tool):

- leaves out every Finding whose `id` the Baseline lists for that Metric and Source set, and counts them in `baselined_findings` on the Metric and in the top-level `baseline` object (`path` and `hidden_findings`);
- keeps every Metric ratio as measured, so Reference ranges and the Ratchet still see the whole repository;
- compares each Budget with the non-baselined ratio: the measured ratio scaled by the share of the Metric's Finding weight that the Baseline does not hide, and marks such Budget failures with `non_baselined`;
- stops with exit code 2 and names `codeint baseline update` when a Metric version in the Baseline differs from the current Metric version.

`codeint check --ignore-baseline` ignores `.codeint/baseline.json`: it shows every Finding, evaluates Budgets on every Finding, and skips the Metric version check.
`codeint diff` does not read the Baseline.

### `codeint churn`

| Option | Meaning |
| ------ | ------- |
| `--since <WINDOW>` | Lookback window in days (such as `90d`) or a UTC date (`YYYY-MM-DD`); defaults to `90d` |
| `--commits <COUNT>` | Walk at most this many main-line commits from HEAD |
| `--output <FILE>` | Write the report to this file instead of standard output |
| `--all` | List every Finding instead of the 20 worst per Metric |
| `--format <FORMAT>` | The format of the report: `toon`, the default, `text`, `json`, or `markdown` |
| `-h, --help` | Print the help of `codeint churn` |

`codeint churn` walks main-line Git history natively via `gix` to count per-file commit touches over the configured window, then ranks eroded and duplicated files independently per Metric without any blended score.
In a shallow clone, `codeint churn` stops at the shallow boundary and includes a structured warning in the report.

```sh exit=0
codeint churn
codeint churn --since 90d --format text
codeint churn --commits 10 --all --format toon
codeint churn --format json --output churn.json
codeint churn --format markdown
```

### `codeint coupling`

| Option | Meaning |
| ------ | ------- |
| `--since <WINDOW>` | Lookback window in days (such as `90d`) or a UTC date (`YYYY-MM-DD`); defaults to `90d` |
| `--commits <COUNT>` | Walk at most this many main-line commits from HEAD |
| `--output <FILE>` | Write the report to this file instead of standard output |
| `--all` | List every Finding instead of the 20 worst per Metric |
| `--format <FORMAT>` | The format of the report: `toon`, the default, `text`, `json`, or `markdown` |
| `--max-commit-files <COUNT>` | Exclude commits touching more than this many files from co-change pair accumulation; defaults to `50` |
| `--min-co-changes <COUNT>` | Report only pairs that changed together in at least this many commits; defaults to `3` |
| `--min-ratio <RATIO>` | Report only pairs whose coupling ratio is at least this value between 0 and 1; defaults to `0.3` |
| `--include-tests` | Include Test Source set files alongside Production files |
| `-h, --help` | Print the help of `codeint coupling` |

`codeint coupling` walks main-line Git history natively via `gix` to find pairs of files that repeatedly change together in the same commits, excludes bulk commits above `--max-commit-files`, and flags whether each coupled pair has a direct runtime import edge.
By default it measures Production files only; pass `--include-tests` to include Test files as well.

```sh exit=0
codeint coupling
codeint coupling --since 90d --max-commit-files 50 --min-co-changes 3 --min-ratio 0.3 --format text
codeint coupling --commits 10 --include-tests --all --format toon
codeint coupling --format json --output coupling.json
codeint coupling --format markdown
```

## Exit codes

| Exit code | Meaning |
| --------- | ------- |
| `0` | The run succeeded, and no Budget, Ratchet, Regression, or blocking doctor error failed |
| `1` | The run succeeded, and at least one Budget, Ratchet, Regression, or blocking doctor error failed; codeint still writes the whole report and names each failure on standard error |
| `2` | An operational error stopped the run, such as an unknown option, a Git ref that names no commit, a Ratchet or Baseline Metric version mismatch, or an invalid `codeint.json`; codeint names the error on standard error |

## `codeint.json`

codeint reads `codeint.json`, `codeint.jsonc`, `.codeint/config.json`, or `.codeint/config.jsonc` at the root of the Git repository.
All four files accept JSONC syntax: `//` line comments, `/* ... */` block comments, and trailing commas before `}` or `]`.
Keep only one of those four files in the repository; having more than one stops the run with exit code 2 and names every conflicting file.
Without any configuration file, codeint uses the default of every key.
[`codeint init`](#codeint-init) writes a starter `codeint.jsonc` (and never replaces an existing configuration file at any of the four paths), and [schema/codeint.schema.json](../schema/codeint.schema.json) is its JSON schema.

In the key paths below, `[]` stands for each item of a list, and a name in braces stands for each of its values:

| Placeholder | Values |
| ----------- | ------ |
| `{source_set}` | `production`, `test`, `generated`, `vendored` |
| `{metric}` | `erosion`, `cognitive_complexity`, `duplication`, `verbosity`, `import_cycles` |
| `{dependency_metric}` | `unused_dependencies`, `unlisted_dependencies`, `dev_in_production_dependencies` |

| Key | Type | Meaning |
| --- | ---- | ------- |
| `$schema` | string | The address of the JSON schema, for editors; when it points to a newer official codeint release tag than the running binary, codeint exits with code 2 |
| `source_sets` | object | Globs that put files into a Source set; see [Source sets](#source-sets) |
| `source_sets.{source_set}` | list of strings | Globs of repository-relative paths that belong to this Source set; empty by default |
| `budgets` | object | The Budgets of each Source set and dependency hygiene Metric; see [Budgets](#budgets) |
| `budgets.{source_set}` | object | The Budgets of this Source set; empty by default |
| `budgets.{source_set}.{metric}` | number from 0 to 1 | The highest ratio of this Metric in this Source set |
| `budgets.{source_set}.boundaries` | number from 0 to 1 | The highest Boundaries ratio in this Source set |
| `budgets.production.orphan_files` | number from 0 to 1 | The highest Orphan files ratio in the Production Source set; setting `orphan_files` under any other Source set stops the run with exit code 2 |
| `budgets.{source_set}.hazards` | number from 0 to 1 | The highest Hazards ratio in this Source set; passes when Hazards is absent; see [Hazards](#hazards) |
| `budgets.production.test_gap` | number from 0 to 1 | The highest Test gap ratio in the Production Source set; setting `test_gap` under any other Source set stops the run with exit code 2 |
| `budgets.{dependency_metric}` | number from 0 to 1 | The highest ratio of this Dependency hygiene Metric across workspace packages |
| `new_file_budgets` | object | The Budgets enforced across newly added files in `codeint diff`, per Source set; see [New-file Budgets](#new-file-budgets) |
| `new_file_budgets.{source_set}` | object | The new-file Budgets of this Source set; empty by default |
| `new_file_budgets.{source_set}.{metric}` | number from 0 to 1 | The highest ratio of this Metric across newly added files in this Source set |
| `new_file_budgets.{source_set}.boundaries` | number from 0 to 1 | The highest Boundaries ratio across newly added files in this Source set |
| `new_file_budgets.production.orphan_files` | number from 0 to 1 | The highest Orphan files ratio across newly added Production files; setting `orphan_files` under any other Source set stops the run with exit code 2 |
| `new_file_budgets.{source_set}.hazards` | number from 0 to 1 | The highest Hazards ratio across newly added files in this Source set; passes when Hazards is absent; see [Hazards](#hazards) |
| `new_file_budgets.production.test_gap` | number from 0 to 1 | The highest Test gap ratio across newly added Production files; setting `test_gap` under any other Source set stops the run with exit code 2 |
| `regression_limits` | object | How much each Metric may get worse in `codeint diff`; see [Regression limits](#regression-limits) |
| `regression_limits.per_file` | boolean | When `true`, `codeint diff` compares file-level Metric numerators and Findings for every modified file and fails if any modified file worsens; `false` by default |
| `regression_limits.{metric}` | object | The Regression limit of this Metric in every Source set |
| `regression_limits.{metric}.ratio_rise` | number from 0 to 1 | The highest ratio rise that passes |
| `regression_limits.{metric}.new_findings` | string | When new Findings or a numerator rise are a Regression: `any_new_finding`, `numerator_rise` (or deprecated alias `numerator_rise_with_new_findings`), or `never` |
| `regression_limits.boundaries` | object | The Regression limit of Boundaries in every Source set |
| `regression_limits.boundaries.ratio_rise` | number from 0 to 1 | The highest Boundaries ratio rise that passes |
| `regression_limits.boundaries.new_findings` | string | When new Boundaries Findings or a numerator rise are a Regression: `any_new_finding` (default), `numerator_rise`, or `never` |
| `regression_limits.orphan_files` | object | The Regression limit of Orphan files in the Production Source set |
| `regression_limits.orphan_files.ratio_rise` | number from 0 to 1 | The highest Orphan files ratio rise that passes |
| `regression_limits.orphan_files.new_findings` | string | When new Orphan files Findings or a numerator rise are a Regression: `any_new_finding` (default), `numerator_rise`, or `never` |
| `regression_limits.hazards` | object | The Regression limit of Hazards in every Source set; see [Hazards](#hazards) |
| `regression_limits.hazards.ratio_rise` | number from 0 to 1 | The highest Hazards ratio rise that passes; `0.01` by default |
| `regression_limits.hazards.new_findings` | string | When new Hazards Findings or a numerator rise are a Regression: `any_new_finding` (default), `numerator_rise`, or `never` |
| `regression_limits.{dependency_metric}` | object | The Regression limit of this Dependency hygiene Metric |
| `regression_limits.{dependency_metric}.ratio_rise` | number from 0 to 1 | The highest ratio rise that passes |
| `regression_limits.{dependency_metric}.new_findings` | string | When new Dependency hygiene Findings or a numerator rise are a Regression: `any_new_finding` (default), `numerator_rise`, or `never` |
| `regression_limits.{source_set}` | object | The Regression limits of this Source set only |
| `regression_limits.{source_set}.{metric}` | object | The Regression limit of this Metric in this Source set |
| `regression_limits.{source_set}.{metric}.ratio_rise` | number from 0 to 1 | The highest ratio rise that passes |
| `regression_limits.{source_set}.{metric}.new_findings` | string | When new Findings or a numerator rise are a Regression: `any_new_finding`, `numerator_rise` (or deprecated alias `numerator_rise_with_new_findings`), or `never` |
| `regression_limits.{source_set}.boundaries` | object | The Regression limit of Boundaries in this Source set |
| `regression_limits.{source_set}.boundaries.ratio_rise` | number from 0 to 1 | The highest Boundaries ratio rise that passes |
| `regression_limits.{source_set}.boundaries.new_findings` | string | When new Boundaries Findings or a numerator rise are a Regression: `any_new_finding` (default), `numerator_rise`, or `never` |
| `regression_limits.production.orphan_files` | object | The Regression limit of Orphan files in the Production Source set; setting `orphan_files` under any other Source set stops the run with exit code 2 |
| `regression_limits.production.orphan_files.ratio_rise` | number from 0 to 1 | The highest Orphan files ratio rise that passes |
| `regression_limits.production.orphan_files.new_findings` | string | When new Orphan files Findings or a numerator rise are a Regression: `any_new_finding` (default), `numerator_rise`, or `never` |
| `regression_limits.{source_set}.hazards` | object | The Regression limit of Hazards in this Source set; see [Hazards](#hazards) |
| `regression_limits.{source_set}.hazards.ratio_rise` | number from 0 to 1 | The highest Hazards ratio rise that passes in this Source set |
| `regression_limits.{source_set}.hazards.new_findings` | string | When new Hazards Findings or a numerator rise are a Regression in this Source set: `any_new_finding` (default), `numerator_rise`, or `never` |
| `languages` | list of objects | Which Language plugin measures which folders; see [Languages](#languages) |
| `languages[].plugin` | string | The name of the Language plugin, such as `typescript-javascript` |
| `languages[].include` | list of strings | Globs of repository-relative folders or files that the Language plugin measures; a folder covers every file below it |
| `languages[].exclude` | list of strings | Globs inside the included folders that the Language plugin leaves out; empty by default |
| `plugins` | list of objects | External WebAssembly component Language and Metric plugins to load from local paths or HTTPS URLs; see [Plugins](#plugins) |
| `plugins[].name` | string | The name of the external Plugin |
| `plugins[].kind` | string | Whether the external Plugin is a Language plugin or a Metric plugin: `language` (the default) or `metric` |
| `plugins[].path` | string | Path to the WebAssembly component file, relative to the repository root or absolute; set either `path` or `url`, not both |
| `plugins[].url` | string | HTTPS URL of the WebAssembly component file; set either `path` or `url`, not both |
| `plugins[].sha256` | string | The required 64-character hexadecimal SHA-256 digest of the plugin file |
| `plugin_limits` | object | Memory cap and time limit enforced on each WebAssembly Plugin; see [Plugins](#plugins) |
| `plugin_limits.memory_mb` | integer from 1 to 4096 | Maximum linear memory in MiB that a WebAssembly Plugin may allocate; `64` by default |
| `plugin_limits.timeout_ms` | integer from 1 to 300000 | Maximum wall-clock time in milliseconds for each WebAssembly Plugin call; `5000` by default |
| `boundaries` | object | Architectural import boundaries defining named zones, optional `public_files` entry points, directional internal `allow` or `deny` rules, external-package `allow_external` or `deny_external` glob rules, and optional `strict_zones` mode |
| `boundaries.preset` | string | Optional built-in architecture preset (`layered` or `hexagonal`) providing default zones and internal rules that can be overridden or extended |
| `boundaries.zones` | object | Map of zone name to zone definition (`patterns` globs and optional `public_files` globs) |
| `boundaries.rules` | list of objects | Directional import rules between architectural zones and for external packages |
| `boundaries.rules[].from` | string | Source zone name, or `*` to match any zone without a specific rule |
| `boundaries.rules[].allow` | list of strings | Target zone names (or `*`) that the source zone is allowed to import; mutually exclusive with `deny` |
| `boundaries.rules[].deny` | list of strings | Target zone names (or `*`) that the source zone is forbidden to import; mutually exclusive with `allow` |
| `boundaries.rules[].allow_external` | list of strings | External package name globs that the source zone is allowed to import (use `[]` to forbid all third-party packages while still permitting standard-library built-ins); mutually exclusive with `deny_external` |
| `boundaries.rules[].deny_external` | list of strings | External package name globs that the source zone is forbidden to import; mutually exclusive with `allow_external` |
| `boundaries.rules[].reason` | string | Optional human-readable explanation shown in boundary Findings |
| `boundaries.strict_zones` | boolean | When `true`, report files in evaluated Source sets that do not match any configured zone; `false` by default |
| `boundaries.source_sets` | list of strings | Source sets evaluated by boundary rules; defaults to `["production"]` when omitted or empty |
| `dependencies` | object | Dependency hygiene settings; see [Dependencies](#dependencies) |
| `dependencies.ignore` | list of strings | Canonical external package names excluded from unused, unlisted, and dev-in-production dependency Findings; empty by default |
| `duplication` | object | Settings for Duplication detection, including opt-in shingle-based near-clone detection; see [Duplication](#duplication) |
| `duplication.near_clones` | boolean | When `true`, detect near-duplicate code blocks using shingle similarity in addition to exact Clone groups; `false` by default |
| `duplication.min_similarity` | number in (0, 1] | Minimum Jaccard shingle similarity required for two blocks to match as a near-clone; `0.85` by default |
| `entry_points` | list of strings | Globs of repository-relative paths treated as additional Production entry points for Orphan files detection; empty by default |
| `coverage_reports` | list of strings | Paths relative to the repository root of LCOV or Cobertura XML Coverage reports used to measure Test gap; empty by default; see [Test gap](#test-gap) |
| `sql` | object | SQL settings, including Dialect mappings for folders or files; see [SQL](#sql) |
| `sql.dialects` | list of objects | Folder or file patterns mapped to a SQL Dialect; files covered by no entry use generic SQL |
| `sql.dialects[].dialect` | string | SQL Dialect for matching files: `postgresql`, `mysql` (covers MariaDB), `sqlite`, or `duckdb` |
| `sql.dialects[].include` | list of strings | Globs of repository-relative folders or files that use this SQL Dialect; a folder covers every file below it |
| `hazards` | object | Hazards Metric settings; see [Hazards](#hazards) |
| `hazards.disabled_rules` | list of strings | Names of Hazard line rules turned off across the repository; empty by default |
| `framework_profiles` | boolean, list of strings, or object | Global Framework profile selection (`false` to disable all, `true` for auto-detection, a list of profile names or `{"only": [...]}` to allow-list, or `{"exclude": [...]}` to exclude named profiles); auto-detected by default; see [Framework profiles](#framework-profiles) |
| `packages` | object | Per-Workspace-package configuration overrides keyed by repository-relative package root (or `"."` for the repository root), such as `packages.<path>.framework_profiles`; empty by default; see [Framework profiles](#framework-profiles) |
| `ratchet` | object | Ratchet settings; see [Ratchet](#ratchet) |
| `ratchet.path` | string | Path relative to the repository root of the Ratchet file; `.codeint/ratchet.json` by default |

A key that the schema does not have, or a value of the wrong type, stops the run with exit code 2 and names the key.
With this `codeint.json`:

```json codeint.json
{ "budget": { "production": { "erosion": 0.2 } } }
```

`codeint check` names the unknown key:

```sh exit=2
codeint check
```

```text stderr
codeint: codeint.json has an unknown key budget; the keys allowed there are $schema, source_sets, budgets, new_file_budgets, regression_limits, languages, plugins, plugin_limits, boundaries, dependencies, duplication, entry_points, coverage_reports, sql, hazards, framework_profiles, packages, ratchet
Remove the key, or correct its spelling to one of the allowed keys.
```

### Source sets

Each file belongs to one Source set: `production`, `test`, `generated`, or `vendored`.
Source sets settle in strict priority order from lowest to highest:

1. Per-file lowering by the Language plugin.
2. Core Vendored (`vendor/`, `third_party/`) and standard build-folder (`dist/`, `build/`, `out/`, `coverage/`, minified lines) rules.
3. Active Framework profile `generated_paths`, which reclassify matching non-Vendored files to `generated` and never override `vendored`.
4. Explicit `source_sets` globs in `codeint.json`, which always win.

- `typescript-javascript` puts a file in `test` when its name ends in `.test.{extension}` or `.spec.{extension}`, such as `src/greet.test.ts`, or when any folder in its path is named `__tests__`, `test`, or `tests`, such as `test/greet.ts` or `packages/app/tests/setup.ts`.
  A file named `test.ts` or `tests.ts` outside such a folder stays in `production`.
- `rust` puts a file in `test` when any folder in its path is named `tests` or `benches`, when its name ends in `_test.rs` or `_tests.rs`, or when its name is `tests.rs`.
  Inline `#[cfg(test)]` modules inside a production file are skipped from `production` metrics.
- `php` puts a file in `test` when any folder in its path is named `tests` or `test`, or when its name ends in `Test.php`.
- `dart` puts a file in `test` when any folder in its path is named `test`, `tests`, or `integration_test`, or when its name ends in `_test.dart`, and puts files ending in `.g.dart`, `.freezed.dart`, `.gr.dart`, `.mocks.dart`, `.pb.dart`, `.pbenum.dart`, `.pbjson.dart`, or `.pbserver.dart` into `generated`.
- `python` puts a file in `test` when any folder in its path is named `test` or `tests`, when its name starts with `test_`, ends with `_test.py`, or is `conftest.py`, and puts files ending in `_pb2.py` or `_pb2_grpc.py` into `generated`.
- `go` puts a file in `test` when its name ends in `_test.go` or when any folder in its path is named `testdata`, puts a file in `generated` when a line comment before any code matches `// Code generated ... DO NOT EDIT.`, and puts files under `vendor/` in `vendored`.
  `go` evaluates no build conditions, so platform-specific files are measured like any other file.
- `sql` puts a file in `generated` when its first 20 lines contain a database dump header (`-- PostgreSQL database dump`, `-- Dumped by pg_dump`, `-- MySQL dump` followed by a space, `-- MariaDB database dump`, or `-- MariaDB dump` followed by a space) or `-- Code generated` with `DO NOT EDIT.` (case-insensitive), when any line starts with a migration tool marker (`--> statement-breakpoint`, `-- atlas:txmode`, or `-- atlas:nolint`), or when its path lies under `prisma/migrations/` or `drizzle/`; puts a file in `test` when any folder in its path is named `test`, `tests`, `fixtures`, or `testdata`, or when its file name ends with `.test.sql` or starts with `test_` (and ends with `.sql`); and puts every other `.sql` file (including hand-written migrations) in `production`.
- `shell` puts a file in `test` when any folder in its path is named `test`, `tests`, or `spec`, or when its name starts with `test_` or ends with `_test` or `.test` before the `.sh`, `.bash`, or `.zsh` extension; puts Autotools helper scripts such as `ltmain.sh` and `configure.sh`, and files whose header comment contains a generated-file marker (such as `Code generated`, `DO NOT EDIT`, `do not modify`, or `this file is generated`), in `generated`; and puts files under `vendor/` or `third_party/` in `vendored`.
- Core rules put files under `vendor/` or `third_party/` into `vendored`, and files with generated-file markers (such as `@generated` or `GENERATED CODE`) into `generated`.

Globs under `source_sets` win over every rule.
Globs match the repository-relative path with forward slashes: `*` matches within one folder, and `**` matches across folders.
When globs of several Source sets match one file, the first of `vendored`, `generated`, `test`, and `production` wins.

```json codeint.json
{
  "source_sets": {
    "test": ["src/route.ts"],
    "generated": ["src/**/*.generated.ts"],
    "vendored": ["vendor/**"],
    "production": []
  }
}
```

```sh exit=0
codeint check --format json
```

### Budgets

A Budget is the highest ratio a Metric may reach in one Source set.
Each Budget is a number from 0 to 1.
A Budget applies only to its own Source set, and a Metric without a Budget never fails.
A Budget applies to the total of every language, never to the section of one Language plugin under `languages`.
For Import cycles, Orphan files, Import boundaries, and Dependency hygiene, repository totals and Budgets count only languages with imports.
When no measured language has imports, those repository totals are absent and Budgets on them do not fail.
For Hazards, repository totals and Budgets count only languages whose Language plugin defines Hazard line rules; when Hazards is absent, a Hazards Budget passes.
`budgets.production.orphan_files` applies to the Production Source set only; setting `orphan_files` under `budgets.test`, `budgets.generated`, or `budgets.vendored` stops the run with exit code 2 and names the key.
`budgets.production.test_gap` applies to the Production Source set only; setting `test_gap` under `budgets.test`, `budgets.generated`, or `budgets.vendored` stops the run with exit code 2 and names the key.
When `budgets.production.test_gap` is set and Test gap is absent (no Coverage report applies), codeint stops the run with exit code 2 and reports that the Budget needs a Coverage report; with no Test gap Budget, an absent Test gap changes no exit code.
codeint compares each Budget with the ratio as the report shows it, rounded to four decimal places, so a ratio equal to its Budget passes.

```json codeint.json
{
  "budgets": {
    "production": { "erosion": 0.2, "duplication": 0.05, "verbosity": 0.1, "import_cycles": 0 },
    "test": { "duplication": 0.2 }
  }
}
```

When a Metric is over its Budget, codeint still writes the whole report, lists the Metric under `over_budget`, names it on standard error, and exits with code 1:

```sh exit=1
codeint check
```

```text stderr
codeint: the production erosion ratio 0.7986 is over its Budget 0.2
```

### New-file Budgets

`new_file_budgets` sets stricter per-Source-set Budgets that `codeint diff` enforces solely across files present in the head snapshot and absent at the merge-base.
When a change adds no new files, `new_file_budgets` stays silent and adds no failures.
Repository-wide Dependency hygiene keys are not allowed under `new_file_budgets`, and `orphan_files` and `test_gap` are accepted under `new_file_budgets.production` only; setting them under another Source set stops the run with exit code 2.
When a Metric across the newly added files exceeds its new-file Budget, `codeint diff` writes the full report, lists each failure under `over_new_file_budget` (and under `runs[0].properties.newFileBudgetFailures` in SARIF), names it on standard error, and exits with code 1:

```json codeint.json
{
  "new_file_budgets": {
    "production": {
      "erosion": 0,
      "boundaries": 0,
      "orphan_files": 0,
      "hazards": 0,
      "test_gap": 0.2
    }
  }
}
```

```sh exit=1
codeint diff main
```

### Regression limits

A key of `regression_limits` named after a Metric sets its limit in every Source set.
A key named after a Source set holds limits for that Source set only:

```json codeint.json
{
  "regression_limits": {
    "erosion": { "ratio_rise": 0.02 },
    "import_cycles": { "new_findings": "numerator_rise" },
    "generated": { "duplication": { "ratio_rise": 1 } },
    "production": { "erosion": { "new_findings": "any_new_finding" } }
  }
}
```

```sh exit=1
codeint diff main
```

Each limit has two settings, and you may set either one or both:

- `ratio_rise` is the highest ratio rise that passes, a number from 0 to 1; the default is 0.01.
- `new_findings` says when new Findings or a numerator rise are a Regression.
  `any_new_finding` fails on any new Finding or numerator rise, the default for Import cycles, Boundaries, Orphan files, Hazards, and Dependency hygiene.
  `numerator_rise` fails whenever the numerator rises (rounded to four decimal places), the default for Erosion, Cognitive complexity, Duplication, Verbosity, and external Metrics; `numerator_rise_with_new_findings` is accepted as a deprecated 0.x alias for `numerator_rise`.
  `never` lets new Findings and numerator rises pass; together with `ratio_rise: 1`, the Metric never makes a Regression there, which suits the `generated` and `vendored` Source sets.

codeint settles each setting on its own: the Source set limit wins, then the Metric limit, then the default.
So the example above gives production Erosion a ratio rise limit of 0.02 and fails it on any new Finding.
Set `regression_limits.per_file` to `true` (`false` by default) to prevent a change from worsening one modified file while shrinking another file:

```json codeint.json
{
  "regression_limits": {
    "per_file": true
  }
}
```

```sh exit=1
codeint diff main
```

```text stderr
codeint: the production erosion numerator eroded_function_mass rose from 44.8999 to 67.8823
codeint: file src/route.ts regressed in production erosion: eroded_function_mass rose from 0 to 67.8823
```

When `per_file` is `true`, each modified file is checked against its own base file-level numerator (and new Findings when `new_findings` is `any_new_finding`), while Metrics configured with `new_findings: "never"` skip per-file checks.
Unmodified files and modified files that improve or stay equal pass.
Setting `orphan_files` under `regression_limits.test`, `regression_limits.generated`, or `regression_limits.vendored` stops the run with exit code 2 and names the key, because Orphan files applies to the Production Source set only (use `regression_limits.orphan_files` or `regression_limits.production.orphan_files`).
Setting `test_gap` under `regression_limits` (at the top level or under any Source set) stops the run with exit code 2 and names the key, because `codeint diff` leaves Test gap out.

An invalid limit stops the run with exit code 2 and names the key:

```json codeint.json
{ "regression_limits": { "production": { "erosion": { "ratio_rise": 1.5 } } } }
```

```sh exit=2
codeint diff main
```

```text stderr
codeint: codeint.json: regression_limits.production.erosion.ratio_rise must be between 0 and 1, not 1.5
Set the value to a ratio from 0 to 1, as the report shows ratios.
```

### Languages

Each entry of `languages` names a Language plugin, the globs of the folders or files it measures, and the globs it leaves out.
Several entries may name one Language plugin; it measures what any of them includes.
Without `languages`, every Language plugin measures the whole repository; with it, a Language plugin measures only what its entries include.
When two Language plugins claim the same file extension for the same folder, codeint stops with exit code 2 until `languages` assigns separate folders to each plugin.
codeint ships eight built-in Language plugins:

| Plugin name | Extensions | Workspace manifest |
| ----------- | ---------- | ------------------ |
| `typescript-javascript` | `.ts`, `.tsx`, `.mts`, `.cts`, `.js`, `.jsx`, `.mjs`, `.cjs`, `.vue`, `.svelte` | `package.json` / `pnpm-workspace.yaml` |
| `rust` | `.rs` | `Cargo.toml` |
| `php` | `.php` | `composer.json` |
| `dart` | `.dart` | `pubspec.yaml` |
| `python` | `.py` | `pyproject.toml` |
| `go` | `.go` | `go.mod` |
| `sql` | `.sql` | none (no imports; belongs to enclosing Workspace package) |
| `shell` | `.sh`, `.bash`, `.zsh` | none (no imports; belongs to enclosing Workspace package) |

For Go (`go`), each folder with a `go.mod` file below the repository root is one Workspace package (`go.work` is not read), an import of a repository Go package leads to every non-test `.go` file in that folder, standard library and cgo (`"C"`) imports count as built in, every named function and method (`ReceiverType.MethodName`) with a body is one function, function literals inside a function count toward the enclosing function one nesting level deeper (package-level function literals are not separate functions), and decision points are `if`, `for`, `case` except `default`, `&&`, and `||`.
Go Verbosity applies five Flagged line rules (never flagging idiomatic `if err != nil` checks):
`else-after-return` (an `else` after a branch chain that always returns, silent when an `if` short variable is used in `else`), `if-else-returns-boolean` (an `if`/`else` or `if` followed by `return` returning `true` and `false`), `conditional-yields-boolean` (an `if`/`else` assigning `true` and `false` to the same variable), `trailing-bare-return` (a bare `return` as the last statement of a function with no results), and `redundant-break` (an unlabeled `break` as the last statement of a `switch` or `select` `case`).
Calibrated Go Reference ranges come from a panel of 20 pre-2023 public Go repositories ([ADR 0021](./adr/0021-go-reference-ranges.md), source `codeint panel of 20 Go repositories, ADR 0021, codeint 0.6.0`): Erosion `37.9% to 69.2%`, Duplication `4.4% to 18.1%`, Verbosity `4.5% to 18.3%`, and Import cycles `0.0% to 0.0%`.
See [docs/guide/go.md](./guide/go.md) for the full Go guide.

For SQL (`sql`), codeint reads every `.sql` file in two layers with `sqlparser` ([ADR 0018](./adr/0018-sql-through-sqlparser-in-two-layers.md)), supports generic SQL by default plus `postgresql`, `mysql` (covering MariaDB), `sqlite`, and `duckdb` Dialects, defines 2 Flagged line rules and 32 Hazard line rules (27 statement rules and 5 schema-backed rules), builds schemas automatically from repository SQL files, and leaves import Metrics absent because SQL has no imports.
Calibrated SQL Reference ranges come from a panel of 20 pre-2023 public SQL repositories ([ADR 0022](./adr/0022-sql-reference-ranges.md), source `codeint panel of 20 SQL repositories, ADR 0022, codeint 0.6.0`): Erosion `9.4% to 97.6%`, Duplication `4.2% to 92.1%`, and Verbosity `4.2% to 92.1%` (Import cycles is absent because SQL has no imports).
See [SQL](#sql) below and [docs/guide/sql.md](./guide/sql.md) for details.

For Shell (`shell`), codeint reads every `.sh`, `.bash`, and `.zsh` file with a hand-written lexer, detects POSIX `sh`, Bash, or Zsh from the first-line shebang and then from the extension, measures each named function plus one `<script>` function for top-level commands, defines 4 Flagged line rules and 10 Hazard line rules, and leaves import Metrics absent because shell scripts have no imports.
Calibrated Shell Reference ranges come from a stratified panel of 50 current public Shell repositories ([ADR 0031](./adr/0031-shell-reference-ranges.md), source `codeint panel of 50 Shell repositories, ADR 0031, codeint 0.6.0`): Erosion `41.6% to 92.7%`, Duplication `0.8% to 75.1%`, and Verbosity `0.0% to 2.5%` (Import cycles is absent because shell scripts have no imports).
See [Shell](#shell) below and [docs/guide/shell.md](./guide/shell.md) for details.

```json codeint.json
{
  "languages": [
    { "plugin": "typescript-javascript", "include": ["src"], "exclude": ["src/broken.ts"] }
  ]
}
```

```sh exit=0
codeint check
```

In a multi-language repository, list each built-in Language plugin with its own folders:

```json illustration
{
  "languages": [
    { "plugin": "typescript-javascript", "include": ["web"], "exclude": ["web/dist"] },
    { "plugin": "rust", "include": ["crates"], "exclude": [] },
    { "plugin": "php", "include": ["app", "src"], "exclude": [] },
    { "plugin": "dart", "include": ["lib"], "exclude": [] },
    { "plugin": "python", "include": ["services"], "exclude": [] },
    { "plugin": "go", "include": ["cmd", "internal"], "exclude": [] },
    { "plugin": "sql", "include": ["db", "analytics"], "exclude": [] },
    { "plugin": "shell", "include": ["scripts"], "exclude": [] }
  ]
}
```

### Plugins

Each entry of `plugins` declares an external WebAssembly component Language plugin or Metric plugin loaded from either a local `path` or an HTTPS `url`, with a required 64-character hexadecimal SHA-256 digest.
External Plugins implement `codeint:plugin@0.4.0` (`codeint:plugin/language@0.4.0` or `codeint:plugin/metric@0.4.0`).
A Plugin built for another interface version (such as `0.2.0`) stops the run with exit code 2, naming the Plugin, both interface versions, and how to rebuild it.
See [docs/plugins.md](./plugins.md) to build, hash, test, version, and upgrade external Plugins.
Set `kind` to `"language"` (the default) for a Language plugin, or `"metric"` for a Metric plugin.
Set either `path` or `url` on each entry, never both.
External Metric plugins appear alongside the built-in Metrics in `codeint check`, `codeint diff`, and `codeint trend`, and support `budgets` and `regression_limits` under their Metric key.
Relative paths resolve from the root of the repository.
Remote plugins require an `https://` URL (plain `http://` and other schemes are refused), are downloaded once into the `plugins` subfolder of the [result cache](#result-cache) keyed by `<sha256>.wasm`, and are reused from the cache on later runs without touching the network.
codeint verifies the SHA-256 digest before caching, compiling, or instantiating the component, and grants the component no host capabilities (no file system, network, environment, or clock access).
A missing `sha256`, an invalid digest, a missing file, a non-HTTPS URL, or a hash mismatch stops the run with exit code 2:

```json codeint.json
{
  "plugins": [
    {
      "name": "sample",
      "kind": "language",
      "path": "plugins/sample.wasm",
      "sha256": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    },
    {
      "name": "remote-metric",
      "kind": "metric",
      "url": "https://example.com/plugins/remote-metric.wasm",
      "sha256": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    }
  ]
}
```

```sh exit=2
codeint check
```

Every WebAssembly Plugin runs under a memory cap (`64` MiB by default) and a per-call time limit (`5000` ms by default).
When a Language plugin hits a limit on a file, codeint skips that file and lists it under `skipped`, and finishes the run.
When a Metric plugin hits a limit, codeint stops the run with exit code 2 and names the Plugin.
Use `plugin_limits` to raise or adjust these limits:

```json codeint.json
{
  "plugin_limits": {
    "memory_mb": 128,
    "timeout_ms": 10000
  }
}
```

```sh exit=0
codeint check
```

### Boundaries

`boundaries` enforces directional internal zone rules, `public_files` encapsulation, and optional external-package rules (`allow_external` and `deny_external`) over runtime imports across every supported Language plugin.
When `boundaries` is omitted, boundary evaluation is completely inactive.
Set `boundaries.preset` to `layered` (`presentation`, `application`, `domain`, `infrastructure`) or `hexagonal` (`domain`, `ports`, `application`, `adapters`) to start from built-in zone globs and directional internal rules; built-in presets never include guessed third-party package rules.
Entries in `boundaries.zones` override matching preset zones or add new ones; rules in `boundaries.rules` with `allow` or `deny` replace preset rules for matching `from` zones, while rules setting only `allow_external` or `deny_external` keep the preset's internal rules intact.
Each zone in `boundaries.zones` declares repository-relative `patterns` globs and optional `public_files` globs naming the only files external zones may import.
When a file matches patterns in multiple zones, the most specific pattern (most literal characters, then longest pattern, then zone name alphabetically) wins.
Each rule in `boundaries.rules` sets `from` (a zone name or `*`), at least one of internal (`allow` or `deny`, mutually exclusive) or external (`allow_external` or `deny_external`, mutually exclusive), and an optional `reason`.
Internal and external directives are independent and may be combined on the same `from` zone; mixing `allow` and `deny` or mixing `allow_external` and `deny_external` for the same `from` zone stops the run with exit code `2`.
`allow_external` and `deny_external` match third-party package names with globs (such as `prisma` or `@aws-sdk/*`) while always permitting standard-library built-ins, so `allow_external: []` allows pure standard-library code.
When no rule sets `allow_external` or `deny_external`, `resolved_imports` counts resolved internal imports only; when at least one external-package rule exists, checked external imports are included in `resolved_imports`.
By default, `boundaries` evaluates the `production` Source set only; set `source_sets` to evaluate additional Source sets, and set `strict_zones` to `true` to report files matching no zone.
In `codeint diff`, each boundary Finding carries a stable content-based identifier so pre-existing Findings are classified as unchanged while any newly introduced boundary Finding fails by default (`new_findings: "any_new_finding"`).
See [docs/guide/import-boundaries.md](./guide/import-boundaries.md) for walkthroughs and examples.

```json codeint.json
{
  "budgets": {
    "production": {
      "boundaries": 0
    }
  },
  "regression_limits": {
    "boundaries": {
      "ratio_rise": 0.01,
      "new_findings": "any_new_finding"
    },
    "production": {
      "boundaries": {
        "ratio_rise": 0,
        "new_findings": "any_new_finding"
      }
    }
  },
  "boundaries": {
    "preset": "layered",
    "zones": {
      "app": {
        "patterns": ["src/**"],
        "public_files": ["src/**"]
      },
      "infra": {
        "patterns": ["infra/**"]
      }
    },
    "rules": [
      {
        "from": "app",
        "allow": ["app"],
        "allow_external": [],
        "reason": "App layer stays independent of infrastructure and third-party packages"
      },
      {
        "from": "infra",
        "deny": ["app"],
        "deny_external": ["prisma", "@aws-sdk/*"],
        "reason": "Infrastructure must not import application code or forbidden SDKs"
      }
    ],
    "strict_zones": false,
    "source_sets": ["production"]
  }
}
```

```sh exit=0
codeint check
```

### Dependencies

codeint compares declared manifest dependencies against imported external packages for every Workspace package that has a dependency manifest (`package.json`, `Cargo.toml`, `pyproject.toml`, `composer.json`, `pubspec.yaml`, or `go.mod`), reporting three independent Metrics:

- `unused_dependencies`: declared runtime and dev dependencies not imported by any Production, Test, or Generated file in the owning package.
  Optional declarations (such as `peerDependencies`, `optionalDependencies`, ambient `@types/*` packages, Cargo `optional = true`, Python `optional-dependencies`, Composer `suggest`, and Go `require ... // indirect`) satisfy imports without being flagged as unused.
  In Go, a module imported only by `_test.go` files stays a runtime dependency with no Finding, and a local `replace` pointing at a repository folder resolves imports to that folder's Go files.
  In JavaScript and TypeScript projects, `devDependencies` invoked in command position by `package.json` `scripts` (including workspace member scripts for root `devDependencies`) are credited as used while keeping their `dev` classification for `declared` counts and `dev_in_production_dependencies`.
- `unlisted_dependencies`: external packages imported by Production or Test files in a package that has a dependency manifest but does not declare them.
  Standard library and runtime built-ins are never flagged.
  In JavaScript and TypeScript workspaces, a workspace member may import dependencies hoisted in the root `package.json`, crediting the root as used while still enforcing `dev_in_production_dependencies` for root dev dependencies imported by runtime Production files.
- `dev_in_production_dependencies`: dependencies declared only as dev dependencies (including Go modules named only by `tool` directives without a plain `require` line) that are imported by runtime Production files.
  Dev dependencies imported by Test files or non-runtime tooling paths (tooling dot-directories, `scripts/`, `examples/`, `example/`, `tool/`, `tools/`, `benchmarks/`, `bench/`, `benches/`, and `docs/` at the repository root or a package root, root-level or package-root config/build files such as `*.config.*`, rc dotfiles, `conf.py`, `setup.py`, `noxfile.py`, and `fabfile.py`, and `conftest.py`) are treated as used and are not flagged.

List canonical external package names under `dependencies.ignore` to exclude them from all three dependency Metrics.
Set independent Budgets under `budgets.unused_dependencies`, `budgets.unlisted_dependencies`, and `budgets.dev_in_production_dependencies`, and adjust `codeint diff` Regression limits under `regression_limits` (all three default to `new_findings: "any_new_finding"`):

```json codeint.json
{
  "dependencies": {
    "ignore": ["tailwindcss"]
  },
  "budgets": {
    "unused_dependencies": 0,
    "unlisted_dependencies": 0,
    "dev_in_production_dependencies": 0
  },
  "regression_limits": {
    "unused_dependencies": { "ratio_rise": 0.01, "new_findings": "any_new_finding" },
    "unlisted_dependencies": { "ratio_rise": 0, "new_findings": "any_new_finding" },
    "dev_in_production_dependencies": { "ratio_rise": 0, "new_findings": "any_new_finding" }
  }
}
```

```sh exit=0
codeint check
```

### Duplication

By default, Duplication reports exact normalized Clone groups so calibrated Reference ranges stay unchanged.
Set `duplication.near_clones` to `true` to also detect near-duplicate blocks that differ by small insertions, deletions, or edits using shingle Jaccard similarity at or above `duplication.min_similarity` (`0.85` by default, in `(0, 1]`).
Near-clone mode honors the same `import-or-export`, minimum token, and minimum distinct token rules as exact Clone groups, clusters overlapping exact and near-clone fragments across three or more locations into Clone families, and leaves Verbosity unaffected because Verbosity counts Flagged lines only ([ADR 0024](./adr/0024-verbosity-decoupling.md)).

```json codeint.json
{
  "duplication": {
    "near_clones": true,
    "min_similarity": 0.85
  }
}
```

```sh exit=0
codeint check
```

### Orphan files

Orphan files detection traverses resolved runtime imports from the union of manifest-declared entry points (such as `package.json`, `Cargo.toml`, `pyproject.toml`, `composer.json`, `pubspec.yaml`, and Go public packages outside `internal/` relative to `go.mod`), Production files marked as program starts by a Language plugin (such as Go files declaring `package main`), and configured `entry_points` globs in `codeint.json`.
Marked files in the Test, Generated, or Vendored Source sets stay out of entry points.
Production files unreachable from any Production entry point are reported as `unreferenced`, or as `test-only` when still imported by Test files.
Generated and Vendored Source sets and non-runtime tooling paths (tooling dot-directories, `scripts/`, `examples/`, `example/`, `tool/`, `tools/`, `benchmarks/`, `bench/`, `benches/`, and `docs/` at the repository root or a package root, root-level or package-root config/build files such as `*.config.*`, rc dotfiles, `conf.py`, `setup.py`, `noxfile.py`, and `fabfile.py`, and `conftest.py`) are excluded from both entry-point roots and evaluated Production files.
When no runtime entry points exist for a language or isolated Workspace package, Orphan files reporting stays silently inactive.
In `codeint diff`, each Orphan files Finding carries a stable `id` so pre-existing orphans remain unchanged while newly introduced orphans fail by default (`new_findings: "any_new_finding"`).

```json codeint.json
{
  "entry_points": ["src/*.ts"],
  "budgets": {
    "production": {
      "orphan_files": 0
    }
  },
  "regression_limits": {
    "orphan_files": {
      "ratio_rise": 0.01,
      "new_findings": "any_new_finding"
    },
    "production": {
      "orphan_files": {
        "ratio_rise": 0,
        "new_findings": "any_new_finding"
      }
    }
  }
}
```

```sh exit=0
codeint check
```

### Test gap

**Test gap** (`test_gap`, a reserved built-in Metric key) is the share of total Production Function mass that tests do not execute.
codeint never runs tests and never guesses where a Coverage report is.
Name one or more Coverage reports in `coverage_reports` (paths relative to the repository root) or with the repeatable `--coverage-report <PATH>` option on `codeint check`.
When `--coverage-report` is present on the command line, `coverage_reports` from `codeint.json` is not used for that run.
See [docs/guide/test-gap.md](./guide/test-gap.md) for copy-paste setup commands for each supported test tool.

Test gap computes from the Code model alone after codeint fills per-function coverage facts from the named reports:

- For each Production function, `untested_share` is its listed lines with hit count `0` divided by all its listed lines.
  Lines that no report lists count on neither side.
- Test gap is the sum over Production functions of `Function mass * untested_share`, divided by the total Function mass of those functions.
- A function whose file appears in no named report, in a language that at least one named report covers, counts as fully untested (`untested_share = 1`).
- A function whose file appears in a report, but of which the report lists no line, is left out of Test gap on both sides.
- When several Coverage reports (or several Cobertura `<class>` entries for one file) list the same line, the line counts as executed when any of them has a hit count above `0`.
- Test gap covers the Production Source set only (repository total, each covered language, and each Workspace package) and never appears for Test, Generated, or Vendored files.
- Test gap has no Reference range, and no Language plugin publishes one.
- With no Coverage report named, or for a language covered by no named report, Test gap is absent (never `0` or `1`) and the report states why in one line.

codeint detects the format of each named file from its content, so the file name does not matter, and one run may mix LCOV and Cobertura XML reports:

- **LCOV**: reads `SF`, `DA`, and `end_of_record`; ignores `TN`, `FN`, `FNDA`, `FNF`, `FNH`, `BRDA`, `BRF`, `BRH`, `LF`, and `LH`; stops with exit code 2 on any other record, naming the file and line.
- **Cobertura XML**: reads XML documents whose root element is `<coverage>`, reading `<source>` roots, `<class filename="...">`, and `<line number="..." hits="...">`, while ignoring branch facts, rates, and `<methods>`.
- Any file that does not exist, cannot be read, contains invalid UTF-8, has broken XML, or is neither LCOV nor Cobertura XML stops the run with exit code 2 and names the file (listing both LCOV and Cobertura XML when the format is not recognized).

Coverage reports are untrusted input and enforce these bounds:

- Maximum report size: `64` MiB (`67,108,864` bytes).
- Maximum Cobertura XML element nesting depth: `64`.
- Cobertura XML rejects `DOCTYPE` declarations, `ENTITY` declarations, custom or external entities (only predefined XML entities and character references are allowed), and processing instructions other than the XML declaration, so a report can never read other files or make network requests.
- Report paths and Cobertura `<source>` roots never resolve to files outside the repository.

Path matching and stale-report checks apply identically to LCOV and Cobertura XML:

- For Cobertura XML, each `<source>` root is tried as a prefix of each `filename` before matching.
- codeint matches each report path to a repository file by the longest matching ending, handling absolute paths, repository-relative paths, and package-relative paths.
- Every report path that matches no repository file, or matches several files equally well, is named in one unmatched-path note in the report; with no unmatched path, the note is omitted.
- A report line beyond the end of its matched file stops the run with exit code 2, naming the report, the file, and the line; a line equal to the file's line count is accepted.

```json codeint.json
{
  "coverage_reports": ["coverage/lcov.info", "api/coverage.xml"],
  "budgets": {
    "production": {
      "test_gap": 0.25
    }
  }
}
```

Pass reports on the command line to override `coverage_reports` for a single run:

```text illustration
codeint check --coverage-report coverage/lcov.info --coverage-report api/coverage.xml
```

### SQL

The built-in `sql` Language plugin measures `.sql` files in two layers with `sqlparser` ([ADR 0018](./adr/0018-sql-through-sqlparser-in-two-layers.md)): a tokenizer layer for source lines, functions, decision points, Cognitive complexity, and Duplication, and a per-statement parser layer for Flagged line rules, Hazard line rules, and SQL schemas.
See [docs/guide/sql.md](./guide/sql.md) for a walkthrough and monorepo examples.

- **Statements and names**: each top-level statement is one function spanning its first and last line.
  A `CREATE` statement for a `TABLE`, `VIEW`, `INDEX`, `FUNCTION`, `PROCEDURE`, `TRIGGER`, `TYPE`, or `MACRO` takes the object's name, keeping schema prefixes; other statements have no name.
- **Decision points and Cognitive complexity**: each `WHEN` arm of a `CASE`, each `AND` and `OR` in a condition (excluding `BETWEEN ... AND`, transaction `AND CHAIN`, and `CREATE OR REPLACE`), each `JOIN` (including `STRAIGHT_JOIN`), each subquery (`(SELECT ...)` or `(WITH ...)`), and each set operation (`UNION`, `INTERSECT`, or `EXCEPT`, excluding wildcard `EXCEPT`) adds `1`; window functions and `GROUP BY` add `0`.
  Inside a stored routine (function, procedure, or trigger, including PostgreSQL dollar-quoted SQL/PLpgSQL bodies, MySQL `DELIMITER`-delimited routines, and SQLite triggers), each `IF`, `ELSIF`/`ELSEIF`, loop (`WHILE`, `FOR`, `REPEAT`, `LOOP`), and error handler (`EXCEPTION`, `HANDLER`) adds `1` and deepens block nesting.
  Cognitive complexity uses subquery depth plus routine block depth.
- **Dialects (`sql.dialects`)**: map repository-relative folder or file globs to `postgresql`, `mysql` (covers MariaDB), `sqlite`, or `duckdb`.
  Files covered by no entry use generic SQL (`generic`); ClickHouse, SQL Server, Oracle, BigQuery, and Snowflake stay generic SQL.
  An unknown Dialect name, an empty `include` list, or one file matched by entries with different Dialects stops the run with exit code 2.
  PostgreSQL `psql` backslash commands and DuckDB CLI dot-commands count as source lines and are not parsed as statements.
- **Templated SQL and rejected statements**: files containing `{{`, `{%`, `%}`, `<%`, or `%>` outside SQL strings and comments are skipped under `skipped` with reason `templated SQL`.
  A single statement that the parser rejects still counts in every token-layer Metric, receives no Flagged line or Hazard line rules, and is listed under `rejected_statements` with `path`, `line`, and `dialect`.
- **SQL Schema (`sql_schemas`)**: built automatically in repository-relative path order from `CREATE TABLE` (excluding `CREATE TEMP` / `CREATE TEMPORARY TABLE`), `CREATE INDEX`, `ALTER TABLE`, `DROP TABLE`, and `DROP INDEX` across Production, Test, and Generated SQL (never Vendored, and skipping rollback down-migration files named `down.sql` or ending in `.down.sql`), scoped per Workspace package and Dialect (plus root per Dialect; generic SQL is its own Dialect).
  Reports list each scope's table count under `sql_schemas`.
  The 5 schema-backed Hazard line rules stay silent when a table is unknown or an unqualified column is ambiguous.
- **Flagged line rules (Verbosity)**:
  - `or-chain-collapses-to-in`: three or more `OR` equality comparisons of the same column against constant literals.
  - `having-without-aggregate`: a `HAVING` condition that contains no aggregate function.
- **Reference ranges**: calibrated across 20 pre-2023 public SQL repositories ([ADR 0022](./adr/0022-sql-reference-ranges.md), source `codeint panel of 20 SQL repositories, ADR 0022, codeint 0.6.0`): Erosion `9.4% to 97.6%`, Duplication `4.2% to 92.1%`, and Verbosity `4.2% to 92.1%` (Import cycles is absent).

All 32 built-in SQL Hazard line rules (27 statement rules and 5 schema-backed rules):

| Rule | Kind | Matches |
| ---- | ---- | ------- |
| `select-star` | statement | `SELECT *` or `SELECT t.*` (outside direct `EXISTS` subqueries) |
| `leading-wildcard-like` | statement | `LIKE` or `ILIKE` string literal starting with `%` or `_` |
| `large-offset` | statement | `OFFSET` constant above `1,000` |
| `tables-without-join-condition` | statement | Non-lateral `CROSS JOIN`, a join without a constraint, or multiple non-lateral `FROM` tables with no linking condition |
| `distinct-with-order-by` | statement | `SELECT DISTINCT` paired with `ORDER BY` |
| `subquery-in-select-list` | statement | A subquery inside a `SELECT` projection expression |
| `function-on-filter-column` | statement | A function call (or `TRIM`/`SUBSTRING`/`EXTRACT`/`CEIL`/`FLOOR`/`POSITION`/`OVERLAY`) applied to a column in a filter (excluding aggregates in `HAVING`) |
| `not-in-subquery` | statement | `NOT IN` with a subquery |
| `union-without-all` | statement | `UNION` without `ALL` |
| `select-without-filter-or-limit` | statement | `SELECT` from a table with no `WHERE`/`PREWHERE` and no `LIMIT`/`FETCH`/`TOP` (outside `CREATE VIEW`) |
| `count-without-filter` | statement | `COUNT(*)` from a table with no `WHERE`/`PREWHERE` and no `FILTER` clause (outside `CREATE VIEW`) |
| `order-by-random` | statement | `ORDER BY` calling `RANDOM()`, `RAND()`, `NEWID()`, or `GEN_RANDOM_UUID()` |
| `distinct-over-join` | statement | `DISTINCT` over two or more joined tables (excluding `DISTINCT ON`) |
| `table-read-twice` | statement | The same non-CTE table read more than once in one statement |
| `subquery-refers-to-outer-table` | statement | A correlated subquery referencing a table or alias from an outer scope |
| `long-in-list` | statement | An `IN` list with more than `100` items |
| `deep-subquery-nesting` | statement | Subqueries nested three levels or more |
| `join-without-aliases` | statement | A join of two or more tables where at least one table lacks an alias |
| `column-position` | statement | An integer column position in `ORDER BY` or `GROUP BY` |
| `update-without-filter` | statement | `UPDATE` with no `WHERE` clause and no filtering `INNER JOIN` |
| `delete-without-filter` | statement | `DELETE` with no `WHERE` clause and no filtering `INNER JOIN` |
| `truncate` | statement | `TRUNCATE` statement |
| `drop` | statement | `DROP` statement for any object |
| `grant-or-revoke` | statement | `GRANT` or `REVOKE` statement |
| `always-true-condition` | statement | A tautological filter condition such as `OR 1 = 1`, `TRUE`, or `NOT FALSE` (excluding `LEFT [OUTER] JOIN ... ON TRUE` and `INSERT ... SELECT ... WHERE TRUE ON CONFLICT`) |
| `sql-built-at-run-time` | statement | Dynamic SQL execution such as `EXECUTE IMMEDIATE`, `EXECUTE '...'`, or `sp_executesql` |
| `plain-text-secret` | statement | A non-empty string literal in a password/secret clause, or written to a column whose name contains `password`, `passwd`, `pwd`, `secret`, `token`, `api_key`, `apikey`, `private_key`, or `access_key` |
| `filter-on-unindexed-column` | schema | A filter condition on a known table's column not covered by its primary key, index prefix, or a leading indexed column in the same `AND` chain |
| `order-by-unindexed-column` | schema | An `ORDER BY` on a known table's column not covered by its primary key or composite index prefix (excluding aggregates and `CASE` in grouped queries) |
| `join-on-unindexed-column` | schema | A join condition (`ON` or `USING`) on a known table's column not covered by its primary key, index prefix, or a leading indexed column in the same `AND` chain |
| `unknown-column` | schema | A reference or assignment to a column not present on a known table in the built schema |
| `text-compared-to-number` | schema | A text-typed column in a known table compared to a bare numeric literal |

```json codeint.json
{
  "sql": {
    "dialects": [
      {
        "dialect": "postgresql",
        "include": ["db/postgres"]
      },
      {
        "dialect": "duckdb",
        "include": ["analytics"]
      }
    ]
  }
}
```

```sh exit=0
codeint check
```

### Shell

The built-in `shell` Language plugin measures `.sh`, `.bash`, and `.zsh` files with a hand-written lexer and no external parser dependency.
codeint dispatches files by extension, so extensionless scripts are not measured.
See [docs/guide/shell.md](./guide/shell.md) for a walkthrough.

- **Flavors**: a first-line shebang naming `sh`, `dash`, or `ash` selects POSIX `sh`, one naming `bash` selects Bash, and one naming `zsh` selects Zsh, directly or through `env`.
  Without a recognized shebang, `.bash` selects Bash, `.zsh` selects Zsh, and `.sh` selects POSIX `sh`.
- **Functions and names**: each `name() { ... }` and `function name { ... }` definition is one function with its own name.
  Commands outside named functions form one function named `<script>` when any exist.
- **Decision points and Cognitive complexity**: `if`, `elif`, `for`, `while`, `until`, `select`, each `case` branch except a lone `*)` wildcard, `&&`, and `||` each add `1`; Cognitive complexity adds nesting depth for nested control flow.
- **Source lines**: comments and blank lines are not source lines; single-quoted, double-quoted, and ANSI-C `$'...'` strings, nested `$(...)` substitutions, heredocs (`<<EOF`, `<<-EOF`, `<<'EOF'`), and line continuations are read as code.
  An unterminated quote, substitution, or heredoc makes the file unparsable.
- **No imports**: Import cycles, Import boundaries, Orphan files, and Dependency hygiene are absent for shell scripts.
- **Flagged line rules (Verbosity)**:
  - `useless-cat`: `cat file | command` with exactly one plain file operand.
  - `legacy-backticks`: command substitution written with backticks instead of `$(...)`.
  - `redundant-exit-status-check`: a `[ ... ]`, `[[ ... ]]`, or `test` comparison of `$?` with `0`.
  - `else-after-return`: an `else` after an `if` branch that always ends in `return` or `exit`.
- **Reference ranges**: Erosion `41.6% to 92.7%`, Duplication `0.8% to 75.1%`, and Verbosity `0.0% to 2.5%` from a stratified panel of 50 current public Shell repositories ([ADR 0031](./adr/0031-shell-reference-ranges.md)).

All 10 built-in Shell Hazard line rules:

| Rule | Matches |
| ---- | ------- |
| `missing-strict-mode` | A script with top-level commands but no `set -e` and `set -u` (POSIX `sh`), or no `set -e`, `set -u`, and `set -o pipefail` (Bash and Zsh), from `set`, Zsh `setopt`, or shebang flags |
| `unquoted-variable` | An unquoted `$VAR` or `${VAR}` expansion in a command name, argument, or redirection target |
| `eval-with-variable` | `eval` with an argument that expands a variable |
| `pipe-download-to-shell` | `curl` or `wget` piped into a shell interpreter, or a shell reading `<(curl ...)` or `<(wget ...)` |
| `unchecked-cd` | `cd` outside a condition and not followed by `&&` or `\|\|`, in a script without `set -e` |
| `unguarded-rm-rf` | Recursive forced `rm` on a path that starts with `$VAR/` or `${VAR}/` without a `${VAR:?}` guard |
| `hardcoded-secret` | A non-placeholder literal assigned to a name containing `password`, `passwd`, `secret`, `token`, `api_key`, `apikey`, `private_key`, `access_key`, or a similar word, or a literal with a known secret shape such as an AWS access key, a `ghp_` token, or a private key block |
| `read-without-r` | `read` without `-r` |
| `posix-sh-bashism` | Bash-only syntax in a POSIX `sh` script: `[[`, `declare`, `function`, `mapfile`, `readarray`, `select`, `shopt`, `source`, `typeset`, `((`, array assignment, `<<<`, `\|&`, `&>`, `&>>`, or process substitution |
| `insecure-tls-download` | `curl -k`, `curl --insecure`, or `wget --no-check-certificate` |

### Hazards

**Hazards** (`hazards`, a reserved built-in Metric key) is the share of source lines that are Hazard lines (`hazard_lines / source_lines`), sitting next to Software entropy ([ADR 0016](./adr/0016-hazards-next-to-software-entropy.md)).
See [docs/guide/hazards.md](./guide/hazards.md) for the full guide.

- **Absent when no rules exist**: for languages whose Language plugin defines no Hazard line rules (`typescript-javascript`, `rust`, `php`, `dart`, `python`, and `go`), Hazards is absent (never `0`).
  Repository totals count only files of languages with Hazard line rules on both numerator and denominator.
- **Equal weight and AST-only**: every Hazard line carries equal weight with no severity levels, and every rule matches parsed statements rather than raw text.
  A line matching several rules counts as one Hazard line and produces one Finding per rule.
- **Strict non-leakage**: a Hazard line Finding holds only `id`, `path`, `line`, `rule`, and optional `owners`; no report format, log line, or error message prints the matched source text.
- **Disabling rules (`hazards.disabled_rules`)**: list rule names to turn off across the repository.
  A turned-off rule adds no Hazard line and no Finding on either side of `codeint diff`; an unknown rule name stops the run with exit code 2 and names it.
- **Budgets and Regression limits**: `budgets.{source_set}.hazards` accepts a number from `0` to `1` and passes when Hazards is absent.
  `regression_limits.hazards` and `regression_limits.{source_set}.hazards` accept `ratio_rise` (default `0.01`) and `new_findings` (default `any_new_finding`, plus numerator rise where the numerator is `hazard_lines`).
- **No Reference range**: Hazards has no published Reference range.

```json codeint.json
{
  "hazards": {
    "disabled_rules": ["select-star"]
  },
  "budgets": {
    "production": {
      "hazards": 0.05
    }
  },
  "regression_limits": {
    "hazards": {
      "ratio_rise": 0.01,
      "new_findings": "any_new_finding"
    },
    "production": {
      "hazards": {
        "ratio_rise": 0,
        "new_findings": "any_new_finding"
      }
    }
  }
}
```

```sh exit=0
codeint check
```

### Framework profiles

A **Framework profile** is a package-level rule set inside a Language plugin that adds framework detection, convention entry points, framework-generated Source set paths, fallback import hints, and optional Reference ranges while keeping per-file lowering framework-blind ([ADR 0025](./adr/0025-framework-profiles.md)).
`typescript-javascript` ships built-in Framework profiles for `react`, `vue2`, `vue3`, `svelte`, `nextjs`, `nuxt`, `sveltekit`, `angular`, and `nestjs`, `php` ships `laravel`, `symfony`, `cakephp`, and `codeigniter`, `python` ships `django`, `fastapi`, and `flask`, `dart` ships `flutter`, and external WebAssembly Language plugins may expose their own Framework profiles through `codeint:plugin@0.4.0`.
Active Framework profile names appear under `framework_profiles` per language and per Workspace package in TOON, JSON, Markdown, and text reports (omitted when none are active).

- **Automatic detection (default)**: each Workspace package resolves Framework profiles strictly from its own local manifest and files; a monorepo root manifest never activates a profile on a child package without a local signal, and `peerDependencies`-only UI libraries do not activate app-only conventions (`svelte`, `sveltekit`, and `nuxt` allow `devDependencies`).
- **Global override (`framework_profiles`)**: set `false` to disable all profiles, `true` to use automatic detection, a list of profile names (or `{"only": [...]}`) to activate only those profiles, or `{"exclude": [...]}` to exclude named profiles.
- **Per-package override (`packages.<path>.framework_profiles`)**: override `framework_profiles` for one Workspace package keyed by its repository-relative root (or `"."` for the repository root).
- **Fail-fast validation**: every configured profile name is validated against the union of `known_framework_profiles()` across all loaded Language plugins; an unknown name stops the run with exit code 2.
- **Disabling import hints**: disabling a profile also disables its fallback import hints during import resolution.

```json codeint.json
{
  "framework_profiles": { "exclude": ["svelte"] },
  "packages": {
    ".": { "framework_profiles": false }
  }
}
```

```sh exit=0
codeint check
```

### Ratchet

A **Ratchet** file (`.codeint/ratchet.json` by default, or `ratchet.path` in configuration) stores each measured Metric's ceiling ratio (rounded to 4 decimal places) alongside its positive integer Metric version across per-Source-set Metrics, Dependency hygiene, Production Test gap when `coverage_reports` are configured, and loaded external Metric plugins ([ADR 0026](./adr/0026-regression-and-ratchet.md)).
When the Ratchet file exists, whole-repository `codeint check` and `codeint diff` enforce it automatically, failing with exit code 1 (`over_ratchet`) if any measured Metric rises above its recorded ceiling and failing with exit code 2 on any Metric version mismatch while naming `codeint ratchet update`.
Single-package runs (`--package`) skip Ratchet enforcement and emit a `ratchet_note` explaining why.
Paths escaping the repository (absolute paths, `..` traversal, or escaping symlinks) stop the run with exit code 2.

```json codeint.json
{
  "ratchet": {
    "path": ".codeint/ratchet.json"
  }
}
```

```sh exit=0
codeint ratchet update
```

## Result cache

codeint caches the measurement of each commit it reads from Git objects, so a second `codeint diff` against the same base reads it back instead of measuring the commit again.
That covers the merge-base, the head ref of `--head`, and each commit of `codeint trend`.
An entry belongs to one commit and to the settings that shape a measurement: the codeint version, the Plugin interface version, every Metric version (all built-in Metrics and every loaded external Metric plugin), each Language plugin and its version, the effective `codeint.json`, and (when Coverage reports are named) the list and content of those Coverage reports.
Any change to `codeint.json` other than its `$schema` address, any bump to a Metric version, or any change to the named Coverage reports measures again instead of serving a stale cached result.
The working tree is never cached, because it has no commit.

The cache lives in the `codeint` folder of your user cache folder:

| Platform | Folder |
| -------- | ------ |
| Linux and macOS | `$XDG_CACHE_HOME/codeint`, or `~/.cache/codeint` when `XDG_CACHE_HOME` is unset, empty, or relative |
| Windows | `%LOCALAPPDATA%\codeint` |

Set `CODEINT_CACHE_DIR` to use another folder; a relative path starts at the current folder:

```sh exit=0
CODEINT_CACHE_DIR=.cache/codeint codeint trend
```

Without any of these variables, codeint runs without a cache.

Deleting the folder is always safe.
codeint ignores an entry it cannot read or parse and writes it again.
When it cannot write an entry, it prints a warning to standard error and the run still succeeds:

```text illustration
codeint: cannot write the cache entry /home/me/.cache/codeint: File exists (os error 17); the next run measures commit b4e728dff30ac8b8e4967849530b9322716748c4 again
```

## Report formats

Every command that prints a report takes `--format` and `--output {file}`.
`--output` writes the report in the chosen format to a file instead of standard output.
`codeint check` and `codeint diff` print TOON by default, and also print JSON, SARIF, and Markdown.
`codeint trend` prints TOON by default, and also prints JSON and Markdown.

### TOON report

`--format toon` prints [TOON](https://github.com/toon-format/toon), a compact text form of the JSON report with the same fields and values.
It is the default, so these commands print the same reports as they print without `--format`:

```sh exit=0
codeint check --format toon
codeint diff main --head feature --format toon
codeint trend --format toon
```

The output of `codeint check` [above](#codeint-check) is a TOON report.

### JSON report

`--format json` prints one JSON object with the same fields and values as the TOON report.
Tools can build on this shape: the [compatibility rule](#compatibility-rule) says how it may change.

```sh exit=0
codeint check --format json
```

```json stdout
{
  "files": 4,
  "source_lines": 39,
  "source_sets": {
    "production": {
      "files": 4,
      "source_lines": 39,
      "erosion": {
        "ratio": 0.7986,
        "metric_version": 1,
        "reference_range": {
          "low": 0.352,
          "high": 0.6,
          "source": "codeint panel of 20 TypeScript and JavaScript repositories, ADR 0008, codeint 0.1.0"
        },
        "eroded_function_mass": 67.8823,
        "total_function_mass": 85.0032,
        "findings": [
          {
            "id": "f0740d06c0975d68",
            "path": "src/route.ts",
            "line": 1,
            "end_line": 18,
            "name": "route",
            "cyclomatic_complexity": 16,
            "cognitive_complexity": 15,
            "source_lines": 18,
            "function_mass": 67.8823
          }
        ]
      },
      "cognitive_complexity": {
        "ratio": 0.0,
        "metric_version": 1,
        "reference_range": "no published Reference range for this language",
        "complex_cognitive_complexity": 0,
        "total_cognitive_complexity": 20,
        "findings": []
      },
      "duplication": {
        "ratio": 0.0,
        "metric_version": 1,
        "reference_range": {
          "low": 0.017,
          "high": 0.346,
          "source": "codeint panel of 20 TypeScript and JavaScript repositories, ADR 0008, codeint 0.1.0"
        },
        "clone_lines": 0,
        "clone_groups": []
      },
      "verbosity": {
        "ratio": 0.0256,
        "metric_version": 2,
        "reference_range": {
          "low": 0.018,
          "high": 0.347,
          "source": "codeint panel of 20 TypeScript and JavaScript repositories, ADR 0008, codeint 0.1.0"
        },
        "verbose_lines": 1,
        "flagged_lines": 1,
        "findings": [
          {
            "id": "2a57800b29941394",
            "path": "src/sign.ts",
            "line": 4,
            "rule": "else-after-return"
          }
        ]
      },
      "import_cycles": {
        "ratio": 0.0,
        "metric_version": 1,
        "reference_range": {
          "low": 0.0,
          "high": 0.308,
          "source": "codeint panel of 20 TypeScript and JavaScript repositories, ADR 0008, codeint 0.1.0"
        },
        "modules_in_import_cycles": 0,
        "modules": 4,
        "findings": [],
        "unresolved_imports": {
          "count": 0,
          "imports": []
        }
      }
    }
  },
  "test_gap_note": "Test gap is not measured because no Coverage report was named.",
  "skipped": [
    {
      "path": "src/broken.ts",
      "reason": "cannot parse: Unexpected token at line 1, column 23"
    }
  ],
  "declaration_only": [
    "src/types.d.ts"
  ]
}
```

The top-level object has these fields:

| Field | Type | Meaning |
| ----- | ---- | ------- |
| `files` | integer | Number of measured source files, in every Source set |
| `source_lines` | integer | Source lines of the measured files; blank and comment-only lines do not count |
| `import_metrics_note` | string, optional | One line explaining why import Metrics are absent when the measured language has no imports |
| `test_gap_note` | string, optional | One line explaining why Test gap is absent when no Coverage report covers the measured code |
| `coverage_unmatched_note` | string, optional | One note naming every Coverage report path that matched no repository file or matched several files equally well |
| `source_sets` | object | One section per Source set, under the keys `production`, `test`, `generated`, and `vendored` |
| `dependencies` | object, optional | Dependency hygiene Metrics (`unused_dependencies`, `unlisted_dependencies`, and `dev_in_production_dependencies`) when manifests are present for a language with imports |
| `languages` | object, optional | One section per Language plugin, under its name, when files of two or more Language plugins were measured; each section has `files`, `source_lines`, `source_sets`, and optional `import_metrics_note`, `test_gap_note`, and `dependencies` for that plugin's files |
| `skipped` | array, optional | Files that codeint could not measure, each with its `path` and the `reason` (such as `templated SQL`) |
| `rejected_statements` | array, optional | Statements that a Language plugin's parser could not parse (still counted in token Metrics, no statement rules applied), each with `path`, `line`, and `dialect` |
| `sql_schemas` | array, optional | Table counts of each SQL schema built per Workspace package (optional `package`, omitted for root) and `dialect`, with `tables` count |
| `declaration_only` | array of strings, optional | Paths of files that hold only type declarations and add no source lines |
| `over_budget` | array, optional | Metrics over their Budget, each with its `source_set`, `metric`, the `ratio` as the report shows it, and the `budget` |

A Source set section has `files` and `source_lines` for that Source set, and one object per measured Metric: `erosion`, `cognitive_complexity`, `duplication`, `verbosity`, (in `production` when covered by a Coverage report) `test_gap`, (when at least one measured language defines Hazard line rules) `hazards`, and (for languages with imports) `import_cycles`, `boundaries`, and `orphan_files`.
Each Metric object has these fields:

- `ratio`: the Metric value, a number from 0 to 1, rounded to four decimal places.
- `metric_version`: positive integer Metric version identifying the calculation rules of this Metric (`codeint diff` includes `metric_version` on each Metric change object, and `codeint trend` includes a top-level `metric_versions` map).
- `reference_range`: either an object with `low`, `high`, and `source`, or a string that says why no Reference range is shown.
  The top-level Source sets show a Reference range only when one Language plugin measured every file; a section under `languages` always shows its own plugin's ranges.
  `test_gap` and `hazards` have no published Reference range.
- The counts behind the ratio:
  `eroded_function_mass` and `total_function_mass` for Erosion, `complex_cognitive_complexity` and `total_cognitive_complexity` for Cognitive complexity, `untested_function_mass` and `total_function_mass` for Test gap, `clone_lines` for Duplication, `verbose_lines` for Verbosity, `hazard_lines` and `source_lines` for Hazards, and `modules_in_import_cycles` and `modules` for Import cycles.
  Duplication and Verbosity divide by the `source_lines` of the Source set; Hazards divides by the `source_lines` of languages with Hazard line rules in that Source set.
  Verbosity reports both `verbose_lines` and `flagged_lines`, equal to the distinct Flagged lines in that Source set (independent of Duplication, [ADR 0024](./adr/0024-verbosity-decoupling.md)).
- The Findings, worst first:
  `clone_groups` for Duplication, and `findings` for every other Metric.
- `near_clones`, optional on `duplication`: present with `min_similarity` when near-clone detection is enabled in `codeint.json`; Duplication's `reference_range` then notes that Reference ranges are calibrated for exact Clone groups.
- `clone_families`, optional on `duplication`: unified Clone families when overlapping or transitively related Clone groups span three or more locations, ordered worst-first by `duplicated_lines`, with `clone_families_left_out` when truncated without `--all`, and `unlisted_clone_family_group_ids` when listed `clone_groups` belong to unlisted Clone families.
- `findings_left_out`, optional: how many Findings the list leaves out; `--all` lists every Finding.
- `baselined_findings`, optional: how many Findings of the Metric the [Baseline](#codeint-baseline) hides.

`import_cycles` also has `unresolved_imports`: the runtime imports that the Language plugin could not resolve, so no Import cycle passes through them.
It has `count`, the list `imports`, and the optional `imports_left_out`.
Each import has `importer`, the path of the importing file, `specifier`, the text of the import, and `reason`.

For a language with no imports, TOON and JSON reports omit `import_cycles`, `orphan_files`, `boundaries`, and `dependencies` (`unused_dependencies`, `unlisted_dependencies`, and `dev_in_production_dependencies`) from that language's section and its Workspace package entries.
Those six Metrics are absent rather than `0`.
The section includes `import_metrics_note` as a single line stating that the language has no imports.
In a multi-language repository, the repository totals for `import_cycles`, `orphan_files`, `boundaries`, and `dependencies` (including Workspace package entries and Budget checks) count only files and manifests of languages with imports on both sides of each ratio.
When every measured language has no imports, those repository totals are absent, `import_metrics_note` is included at the top level, and Budgets on those Metrics do not fail.

Each Finding is an object with an `id`, followed by fields that depend on its kind:

- An eroded function has `path`, `line`, `end_line`, optional `name` (omitted for anonymous closures), `cyclomatic_complexity`, `cognitive_complexity`, `source_lines`, and `function_mass`.
- A complex function (Cognitive complexity) has `path`, `line`, `end_line`, optional `name` (omitted for anonymous closures), `cognitive_complexity`, `cyclomatic_complexity`, and `source_lines`.
- A Test gap Finding has `path`, `line`, `end_line`, optional `name` (omitted for anonymous closures), `function_mass`, `untested_share`, and optional `owners`; Findings are ordered by `function_mass * untested_share`, largest first, and fully tested functions produce no Finding.
- A Clone group has `tokens`, optional `similarity` (present on near-clone Findings as a ratio in `(0, 1]`), and `locations`; each location has `path`, `start_line`, and `end_line`.
- A Clone family has `duplicated_lines` (unique source lines across the family without double-counting overlaps), `clone_group_ids`, and canonical `locations` ordered by `path` then `start_line`.
- A Flagged line has `path`, `line`, and `rule`.
- A Hazard line has `path`, `line`, `rule`, and optional `owners`, and never includes source text.
- An Import cycle has `modules`, the paths of the modules in the cycle.
- An Import boundary Finding has `kind` (`zone_rule`, `external_package`, `public_files`, or `unzoned_file`), `importer`, `message`, optional `target` (internal target path), optional `external_package` (canonical third-party package name for `external_package` Findings), optional `specifier`, optional `source_zone`, optional `target_zone`, optional `reason`, and optional `owners`.

The `id` of a Finding is a string of 16 lowercase hexadecimal digits.
It comes from the Metric, the file or files of the Finding, and their content, never from a line number, so it stays the same when a Finding moves within its file.
It is the same in every run and on every operating system, and CRLF line endings do not change it.
Two Findings with the same content in one file get different ids, numbered in the order they appear.
The content behind the `id` of each kind:

- An eroded, complex, or untested (Test gap) function: every token from its first line to its last line, so a change to the function's code gives it a new `id`; whitespace and comments do not count.
- A Clone group: the token sequence that its copies share, with every name and every literal value treated alike, and the path of each copy.
- A Clone family: the sorted normalized token sequences of its constituent Clone groups and the canonical path of each member location.
- A Flagged line or Hazard line: the rule and the tokens of the line.
- An Import cycle: the paths of its modules.

Every `path` is relative to the root of the Git repository and uses `/` as the separator.
Line numbers start at 1.

An optional field is left out when it has no content: a Source set without files, an empty `skipped`, `rejected_statements`, `sql_schemas`, `declaration_only`, or `over_budget` list, `hazards` when no measured language defines Hazard line rules, import Metrics for a language with no imports, `findings_left_out` when no Finding is left out, `baselined_findings` when the Baseline hides no Finding of the Metric, and `baseline` when `.codeint/baseline.json` is absent or `--ignore-baseline` is passed.

#### Compatibility rule

Later versions of codeint change the JSON report only in ways that keep existing readers working:

- They only add fields, and never rename or remove a field.
- A field keeps its type and its meaning, including its unit and its scale.
- A field that is always present stays always present.
- They may add a Source set key, a Metric object, or a kind of Finding.

Readers ignore fields they do not know, and treat a missing optional field as empty.

### SARIF report

`--format sarif` prints a [SARIF 2.1.0](https://docs.oasis-open.org/sarif/sarif/v2.1.0/errata01/os/sarif-v2.1.0-errata01-os-complete.html) log, so code scanning tools such as GitHub code scanning show each Finding as an annotation.
The log has one run, and the run has one result per Finding of the report.
It takes the Findings from the total, never from the sections under `languages`, so a Finding appears once.
For a language with no imports, the log holds no Import cycle, Import boundary, Orphan file, or Dependency hygiene Finding for files of that language.
`--all` and `--output` work as they do for every other format: without `--all`, the log holds the 20 worst Findings per Metric and Source set.

```sh exit=0
codeint check --format sarif
```

```json stdout
{
  "$schema": "https://docs.oasis-open.org/sarif/sarif/v2.1.0/errata01/os/schemas/sarif-schema-2.1.0.json",
  "version": "2.1.0",
  "runs": [
    {
      "tool": {
        "driver": {
          "name": "codeint",
          "version": "0.6.0",
          "informationUri": "https://github.com/avidianity/codeint",
          "rules": [
            {
              "id": "erosion",
              "name": "Erosion",
              "shortDescription": {
                "text": "A function with cyclomatic complexity over 10"
              },
              "fullDescription": {
                "text": "Erosion is the share of total Function mass held by functions whose cyclomatic complexity is greater than 10. Function mass is cyclomatic complexity times the square root of source lines."
              },
              "help": {
                "text": "Split the function into smaller functions, each with a cyclomatic complexity of 10 or less."
              }
            },
            {
              "id": "duplication",
              "name": "Duplication",
              "shortDescription": {
                "text": "A Clone group: code fragments with the same token sequence"
              },
              "fullDescription": {
                "text": "Duplication is the share of source lines that belong to at least one Clone group. A Clone group is two or more code fragments with the same token sequence once identifiers and literals are normalized."
              },
              "help": {
                "text": "Extract the fragments of the Clone group into one shared function or module."
              }
            },
            {
              "id": "verbosity",
              "name": "Verbosity",
              "shortDescription": {
                "text": "A Flagged line: code that can be deleted or collapsed"
              },
              "fullDescription": {
                "text": "Verbosity is the share of source lines that are Flagged lines. A Flagged line matches a verbose-code pattern of the source language, and the result names the rule it matches."
              },
              "help": {
                "text": "Delete or collapse the line as its rule describes; the behavior stays the same."
              }
            },
            {
              "id": "import_cycles",
              "name": "Import cycles",
              "shortDescription": {
                "text": "An Import cycle among runtime imports"
              },
              "fullDescription": {
                "text": "Import cycles is the share of modules that belong to at least one Import cycle. An Import cycle is a group of modules in which every module can reach every other module through runtime imports."
              },
              "help": {
                "text": "Break the Import cycle, for example by moving the code that the modules share into a module that imports none of them."
              }
            },
            {
              "id": "cognitive_complexity",
              "name": "Cognitive complexity",
              "shortDescription": {
                "text": "A function with cognitive complexity over 15"
              },
              "fullDescription": {
                "text": "Cognitive complexity is the share of total cognitive complexity held by functions whose cognitive complexity is greater than 15. Cognitive complexity weights control-flow structures by nesting depth."
              },
              "help": {
                "text": "Flatten nested control flow or split the function into smaller functions, each with a cognitive complexity of 15 or less."
              }
            }
          ]
        }
      },
      "originalUriBaseIds": {
        "SRCROOT": {
          "description": {
            "text": "The root of the Git repository"
          }
        }
      },
      "results": [
        {
          "ruleId": "erosion",
          "ruleIndex": 0,
          "level": "note",
          "message": {
            "text": "Eroded function route: cyclomatic complexity 16, over 10; 18 source lines; Function mass 67.8823."
          },
          "locations": [
            {
              "physicalLocation": {
                "artifactLocation": {
                  "uri": "src/route.ts",
                  "uriBaseId": "SRCROOT"
                },
                "region": {
                  "startLine": 1
                }
              }
            }
          ],
          "partialFingerprints": {
            "codeintFindingId": "f0740d06c0975d68"
          },
          "properties": {
            "sourceSet": "production"
          }
        },
        {
          "ruleId": "verbosity",
          "ruleIndex": 2,
          "level": "note",
          "message": {
            "text": "Flagged line: rule else-after-return."
          },
          "locations": [
            {
              "physicalLocation": {
                "artifactLocation": {
                  "uri": "src/sign.ts",
                  "uriBaseId": "SRCROOT"
                },
                "region": {
                  "startLine": 4
                }
              }
            }
          ],
          "partialFingerprints": {
            "codeintFindingId": "2a57800b29941394"
          },
          "properties": {
            "sourceSet": "production"
          }
        }
      ]
    }
  ]
}
```

The tool lists one rule per Metric, and each result names its Metric in `ruleId`:

| `ruleId` | Finding | Location |
| -------- | ------- | -------- |
| `erosion` | An eroded function | The line where the function starts |
| `cognitive_complexity` | A function with cognitive complexity over 15 | The line where the function starts |
| `test_gap` | An untested or partly tested Production function | The line where the function starts; the message names the Function mass and the untested share (omitted when Test gap is absent) |
| `duplication` | A Clone group or Clone family | The first fragment or canonical location, from its start line to its end line; the other fragments or locations are related locations |
| `verbosity` | A Flagged line | The line; the message names the Flagged-line rule |
| `<hazard-rule>` | A Hazard line | The line; one SARIF rule per matched Hazard line rule name; the message is `Hazard line: rule <rule>.` and never prints source text |
| `import_cycles` | An Import cycle | Line 1 of the first module; the other modules are related locations |

Each result has the level `note`, and its `properties.sourceSet` names its Source set: `production`, `test`, `generated`, or `vendored`.

Each `uri` is the path that the other formats print, relative to the root of the Git repository, with `/` as the separator.
A character that a URI cannot hold, such as a space, is percent-encoded: `src/my sign.ts` becomes `src/my%20sign.ts`.
`uriBaseId` is `SRCROOT`, the root of the Git repository; the log does not print its absolute path, so the log is the same on every machine.

### Markdown report

`--format markdown` prints a report to paste into a pull request comment.
It starts with one table: each Metric's value per Source set, the counts behind the value, and the Reference range.
Next come each Metric over its Budget and a table of the ratios of each workspace package, when there are any.
When files of two or more Language plugins were measured, a section per Language plugin follows, with that plugin's Metric table and workspace package table.
For a language with no imports, the report omits the six import Metrics and prints the one-line reason.
For Test gap, the report shows Production totals (repository, per language, and per Workspace package), lists Test gap Findings heaviest untested Function mass first under `Production Findings`, or prints the one-line absent reason when no Coverage report applies.
When SQL files or Hazard line rules are measured, the report also includes `Hazards` rows and Findings tables (showing `Path`, `Line`, and `Rule`, never source text), `## SQL schemas` with per-scope table counts, and `## Rejected statements` before `## Files not measured`.
Then it lists the Findings of each Source set, worst first, and last the files that codeint did not measure.
It lists at most 20 Findings per Metric, so the comment stays short; `--all` lists every Finding.

```sh exit=0
codeint check --format markdown
```

```markdown stdout
# codeint report

codeint measured 4 files with 39 source lines.

Test gap is not measured because no Coverage report was named.

| Source set | Metric | Value | Behind the value | Reference range |
| ---------- | ------ | ----- | ---------------- | --------------- |
| Production | Erosion | 0.7986 | 67.8823 of 85.0032 Function mass in eroded functions | 0.352 to 0.6, source: codeint panel of 20 TypeScript and JavaScript repositories, ADR 0008, codeint 0.1.0 |
| Production | Cognitive complexity | 0 | 0 of 20 cognitive complexity in functions over 15 | no published Reference range for this language |
| Production | Duplication | 0 | 0 clone lines of 39 source lines | 0.017 to 0.346, source: codeint panel of 20 TypeScript and JavaScript repositories, ADR 0008, codeint 0.1.0 |
| Production | Verbosity | 0.0256 | 1 Flagged line of 39 source lines | 0.018 to 0.347, source: codeint panel of 20 TypeScript and JavaScript repositories, ADR 0008, codeint 0.1.0 |
| Production | Import cycles | 0 | 0 of 4 modules in an Import cycle | 0 to 0.308, source: codeint panel of 20 TypeScript and JavaScript repositories, ADR 0008, codeint 0.1.0 |

## Production Findings

### Erosion

| Function | Cyclomatic complexity | Source lines | Function mass |
| -------- | --------------------- | ------------ | ------------- |
| `route` (`src/route.ts:1`) | 16 | 18 | 67.8823 |

### Verbosity

| Flagged line | Rule |
| ------------ | ---- |
| `src/sign.ts:4` | else-after-return |

## Files not measured

- `src/broken.ts`: cannot parse: Unexpected token at line 1, column 23
- `src/types.d.ts`: declaration only
```
