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
| `init` | Write a starter `codeint.json`; see [`codeint init`](#codeint-init) |
| `update` | Update a standalone codeint binary in place from GitHub Releases; see [`codeint update`](#codeint-update) |
| `mcp` | Run a local stdio Model Context Protocol (MCP) server exposing `check`, `diff`, `trend`, `guard`, `impact`, `trace`, `churn`, and `coupling` tools; see [`codeint mcp`](#codeint-mcp) |
| `guard` | Show the architectural boundary zone, allowed and denied target zones, `public_files` constraints, and allowed importers for a file path; see [`codeint guard`](#codeint-guard) |
| `schema` | Print a machine-readable capability manifest of CLI commands, flags, built-in Metrics, output formats, MCP tools, and exit codes; see [`codeint schema`](#codeint-schema) |
| `config` | Inspect the effective active configuration and where each setting came from; see [`codeint config`](#codeint-config) |
| `report` | Re-render a saved `check` or `diff` JSON report into another format without re-running analysis; see [`codeint report`](#codeint-report) |
| `doctor` | Run read-only diagnostics on `codeint.json`, Git repository status, detected languages, Workspace packages, and external Wasm Plugins; see [`codeint doctor`](#codeint-doctor) |
| `agent` | Inspect and configure codeint MCP integration for local coding agents; see [`codeint agent`](#codeint-agent) |
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
codeint 0.5.0
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
| `-h, --help` | Print the help of `codeint init` |

`codeint init` writes a starter `codeint.json` at the root of the Git repository that contains the current folder:

```sh exit=0
codeint init
```

```text stdout
Wrote /home/me/shop/codeint.json
```

The file links to the JSON schema of the running codeint version, so your editor completes and checks it.
It sets every key to its default, and lists under `languages` each top-level folder or file that a Language plugin measures:

```json file=codeint.json
{
  "$schema": "https://raw.githubusercontent.com/avidianity/codeint/v0.5.0/schema/codeint.schema.json",
  "source_sets": {
    "production": [],
    "test": [],
    "generated": [],
    "vendored": []
  },
  "budgets": {
    "production": {},
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
  ]
}
```

When no file belongs to a Language plugin, the file leaves `languages` out, so every Language plugin measures the whole repository.
`codeint init` never replaces an existing `codeint.json`.
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
      reference_range:
        low: 0.352
        high: 0.6
        source: "codeint panel of 20 TypeScript and JavaScript repositories, ADR 0008, codeint 0.1.0"
      eroded_function_mass: 67.8823
      total_function_mass: 85.0032
      findings[1]{id,path,line,cyclomatic_complexity,source_lines,function_mass}:
        f0740d06c0975d68,src/route.ts,1,16,18,67.8823
    duplication:
      ratio: 0
      reference_range:
        low: 0.017
        high: 0.346
        source: "codeint panel of 20 TypeScript and JavaScript repositories, ADR 0008, codeint 0.1.0"
      clone_lines: 0
      clone_groups: []
    verbosity:
      ratio: 0.0256
      reference_range:
        low: 0.018
        high: 0.347
        source: "codeint panel of 20 TypeScript and JavaScript repositories, ADR 0008, codeint 0.1.0"
      verbose_lines: 1
      flagged_lines: 1
      clone_lines: 0
      findings[1]{id,path,line,rule}:
        2a57800b29941394,src/sign.ts,4,else-after-return
    import_cycles:
      ratio: 0
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
skipped[1]{path,reason}:
  src/broken.ts,"cannot parse: Unexpected token at line 1, column 23"
declaration_only[1]: src/types.d.ts
```

codeint measures the files that Git does not ignore.
It does not follow symbolic links, does not measure Git submodules, and skips files that are not valid UTF-8.
It lists each file it cannot parse under `skipped`, and each file that holds only type declarations under `declaration_only`.

These commands measure the same repository with other options:

```sh exit=0
codeint check src
codeint check --all
codeint check --owner @team
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
      new[1]{id,path,line,cyclomatic_complexity,source_lines,function_mass}:
        f0740d06c0975d68,src/route.ts,1,16,18,67.8823
      resolved[1]{id,path,line,cyclomatic_complexity,source_lines,function_mass}:
        1b3d7d358ce8632e,src/parse.ts,1,12,14,44.8999
      unchanged: 0
    duplication:
      base_ratio: 0
      head_ratio: 0
      new: []
      resolved: []
      unchanged: 0
    verbosity:
      base_ratio: 0.0476
      head_ratio: 0.0256
      new: []
      resolved: []
      unchanged: 1
    import_cycles:
      base_ratio: 0
      head_ratio: 0
      new: []
      resolved: []
      unchanged: 0
regressions[1]{source_set,metric,rule,base,head,new_findings,ratio_rise_limit}:
  production,erosion,numerator_rise_with_new_findings,44.8999,67.8823,1,0.01
```

```text stderr
codeint: the production erosion numerator eroded_function_mass rose from 44.8999 to 67.8823 with 1 new Finding
```

codeint reads the merge-base straight from Git objects, so it never checks out a commit and leaves the working tree and the index as they are.
The working tree side includes uncommitted and untracked changes, and leaves out the files that Git ignores, as `codeint check` does.
Commits added to the base branch after your branch started are not part of the comparison, because the merge-base comes before them.

Both sides use the `codeint.json` of the working tree, so the comparison shows changes in the code, not in the configuration.
Both sides follow the same file rules as `codeint check`.

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
      new: []
      resolved[1]{id,path,line,cyclomatic_complexity,source_lines,function_mass}:
        1b3d7d358ce8632e,src/parse.ts,1,12,14,44.8999
      unchanged: 0
    duplication:
      base_ratio: 0
      head_ratio: 0
      new: []
      resolved: []
      unchanged: 0
    verbosity:
      base_ratio: 0.0476
      head_ratio: 0.0476
      new: []
      resolved: []
      unchanged: 1
    import_cycles:
      base_ratio: 0
      head_ratio: 0
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

```sh exit=0
codeint diff main --owner @team
```

#### Regressions

A Regression is a Metric that got worse from the merge-base to the head side by more than its limit.
`codeint diff` checks each Metric of each Source set against three default rules:

- Any new Import cycle is a Regression.
- A ratio rise over 0.01 is a Regression.
  codeint compares the ratios as the report shows them, rounded to four decimal places, so a rise of exactly 0.01 passes.
- A numerator rise that adds new Findings is a Regression.
  The numerator is `eroded_function_mass`, `clone_lines`, `verbose_lines`, or `modules_in_import_cycles`.
  A Finding that only moves, so the numerator stays the same, is not a Regression.

[`regression_limits`](#regression-limits) in `codeint.json` changes these limits.
`codeint diff` also applies the [Budgets](#budgets) of `codeint.json` to the head side, as `codeint check` does.

When a Budget or a Regression fails, codeint still writes the whole report, lists each failure under `over_budget` or `regressions`, names it on standard error, and exits with code 1.
Each Regression names its rule and its base and head values: the ratios for `ratio_rise` and `new_findings`, and the numerators for `numerator_rise_with_new_findings`.
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
source_sets:
  production[3]{commit,committed_on,files,erosion,duplication,verbosity,import_cycles}:
    2864291867e0e3a3ef91cd050c1893d5ae9eb809,2026-11-02,1,0,0,0.1429,0
    2ef203655bd71559d3e54d1ab60a2390fb90674c,2026-11-09,2,0.8946,0,0.0476,0
    e3bc223e22791aa541f8104438ce1606515231ce,2026-11-16,3,0,0,0.0476,0
```

The main line is the first-parent history of HEAD: a merge commit is a row, and the commits of the branch it merged are not.
Without `--since`, codeint measures the latest 20 commits of the main line.
`--since {YYYY-MM-DD}` measures every main-line commit committed on or after the start of that day in UTC, with no limit on the count:

```sh exit=0
codeint trend --since 2026-11-05
```

```text stdout
commits: 2
source_sets:
  production[2]{commit,committed_on,files,erosion,duplication,verbosity,import_cycles}:
    2ef203655bd71559d3e54d1ab60a2390fb90674c,2026-11-09,2,0.8946,0,0.0476,0
    e3bc223e22791aa541f8104438ce1606515231ce,2026-11-16,3,0,0,0.0476,0
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
codeint update --check --version v0.5.0
```

```text stdout
codeint 0.5.0 is already up to date (v0.5.0)
```

### `codeint mcp`

| Option | Meaning |
| ------ | ------- |
| `-h, --help` | Print the help of `codeint mcp` |

`codeint mcp` runs a local Model Context Protocol (MCP) server over standard input and standard output with no open network port.
It exposes eight read-only tools (`check`, `diff`, `trend`, `guard`, `impact`, `trace`, `churn`, and `coupling`) so coding agents can measure Software entropy, inspect architectural boundaries, reverse-dependency blast radius, and import paths, rank high-churn eroded or duplicated files, detect co-change coupling, and fix Regressions before opening a pull request.
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

`codeint guard <path>` evaluates the architectural boundary rules in `codeint.json` for an existing or planned file path without requiring the file to exist on disk.

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

`codeint doctor` runs strictly read-only, offline diagnostics on Git repository status, `codeint.json` validity, `$schema` version compatibility, detected Language plugins, Workspace packages, and external Wasm Plugin paths and SHA-256 digests.
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
| `0` | The run succeeded, and no Budget, Regression, or blocking doctor error failed |
| `1` | The run succeeded, and at least one Budget, Regression, or blocking doctor error failed; codeint still writes the whole report and names each failure on standard error |
| `2` | An operational error stopped the run, such as an unknown option, a Git ref that names no commit, or an invalid `codeint.json`; codeint names the error on standard error |

## `codeint.json`

codeint reads `codeint.json` at the root of the Git repository.
Without the file, codeint uses the default of every key.
[`codeint init`](#codeint-init) writes a starter file, and [schema/codeint.schema.json](../schema/codeint.schema.json) is its JSON schema.

In the key paths below, `[]` stands for each item of a list, and a name in braces stands for each of its values:

| Placeholder | Values |
| ----------- | ------ |
| `{source_set}` | `production`, `test`, `generated`, `vendored` |
| `{metric}` | `erosion`, `duplication`, `verbosity`, `import_cycles` |

| Key | Type | Meaning |
| --- | ---- | ------- |
| `$schema` | string | The address of the JSON schema, for editors; when it points to a newer official codeint release tag than the running binary, codeint exits with code 2 |
| `source_sets` | object | Globs that put files into a Source set; see [Source sets](#source-sets) |
| `source_sets.{source_set}` | list of strings | Globs of repository-relative paths that belong to this Source set; empty by default |
| `budgets` | object | The Budgets of each Source set; see [Budgets](#budgets) |
| `budgets.{source_set}` | object | The Budgets of this Source set; empty by default |
| `budgets.{source_set}.{metric}` | number from 0 to 1 | The highest ratio of this Metric in this Source set |
| `budgets.{source_set}.boundaries` | number from 0 to 1 | The highest Boundaries ratio in this Source set |
| `budgets.{source_set}.orphan_files` | number from 0 to 1 | The highest Orphan files ratio in this Source set |
| `regression_limits` | object | How much each Metric may get worse in `codeint diff`; see [Regression limits](#regression-limits) |
| `regression_limits.{metric}` | object | The Regression limit of this Metric in every Source set |
| `regression_limits.{metric}.ratio_rise` | number from 0 to 1 | The highest ratio rise that passes |
| `regression_limits.{metric}.new_findings` | string | When new Findings are a Regression: `any_new_finding`, `numerator_rise_with_new_findings`, or `never` |
| `regression_limits.boundaries` | object | The Regression limit of Boundaries in every Source set |
| `regression_limits.boundaries.ratio_rise` | number from 0 to 1 | The highest Boundaries ratio rise that passes |
| `regression_limits.boundaries.new_findings` | string | When new Boundaries Findings are a Regression: `any_new_finding` (default), `numerator_rise_with_new_findings`, or `never` |
| `regression_limits.orphan_files` | object | The Regression limit of Orphan files in every Source set |
| `regression_limits.orphan_files.ratio_rise` | number from 0 to 1 | The highest Orphan files ratio rise that passes |
| `regression_limits.orphan_files.new_findings` | string | When new Orphan files Findings are a Regression: `any_new_finding` (default), `numerator_rise_with_new_findings`, or `never` |
| `regression_limits.{source_set}` | object | The Regression limits of this Source set only |
| `regression_limits.{source_set}.{metric}` | object | The Regression limit of this Metric in this Source set |
| `regression_limits.{source_set}.{metric}.ratio_rise` | number from 0 to 1 | The highest ratio rise that passes |
| `regression_limits.{source_set}.{metric}.new_findings` | string | When new Findings are a Regression: `any_new_finding`, `numerator_rise_with_new_findings`, or `never` |
| `regression_limits.{source_set}.boundaries` | object | The Regression limit of Boundaries in this Source set |
| `regression_limits.{source_set}.boundaries.ratio_rise` | number from 0 to 1 | The highest Boundaries ratio rise that passes |
| `regression_limits.{source_set}.boundaries.new_findings` | string | When new Boundaries Findings are a Regression: `any_new_finding` (default), `numerator_rise_with_new_findings`, or `never` |
| `regression_limits.{source_set}.orphan_files` | object | The Regression limit of Orphan files in this Source set |
| `regression_limits.{source_set}.orphan_files.ratio_rise` | number from 0 to 1 | The highest Orphan files ratio rise that passes |
| `regression_limits.{source_set}.orphan_files.new_findings` | string | When new Orphan files Findings are a Regression: `any_new_finding` (default), `numerator_rise_with_new_findings`, or `never` |
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
| `boundaries` | object | Architectural import boundaries defining named zones, optional `public_files` entry points, directional `allow` or `deny` rules, and optional `strict_zones` mode |
| `boundaries.preset` | string | Optional built-in architecture preset (`layered` or `hexagonal`) providing default zones and rules that can be overridden or extended |
| `boundaries.zones` | object | Map of zone name to zone definition (`patterns` globs and optional `public_files` globs) |
| `boundaries.rules` | list of objects | Directional import rules between architectural zones |
| `boundaries.rules[].from` | string | Source zone name, or `*` to match any zone without a specific rule |
| `boundaries.rules[].allow` | list of strings | Target zone names (or `*`) that the source zone is allowed to import; mutually exclusive with `deny` |
| `boundaries.rules[].deny` | list of strings | Target zone names (or `*`) that the source zone is forbidden to import; mutually exclusive with `allow` |
| `boundaries.rules[].reason` | string | Optional human-readable explanation shown in boundary Findings |
| `boundaries.strict_zones` | boolean | When `true`, report files in evaluated Source sets that do not match any configured zone; `false` by default |
| `boundaries.source_sets` | list of strings | Source sets evaluated by boundary rules; defaults to `["production"]` when omitted or empty |
| `duplication` | object | Settings for Duplication detection, including opt-in shingle-based near-clone detection; see [Duplication](#duplication) |
| `duplication.near_clones` | boolean | When `true`, detect near-duplicate code blocks using shingle similarity in addition to exact Clone groups; `false` by default |
| `duplication.min_similarity` | number in (0, 1] | Minimum Jaccard shingle similarity required for two blocks to match as a near-clone; `0.85` by default |
| `entry_points` | list of strings | Globs of repository-relative paths treated as additional Production entry points for Orphan files detection; empty by default |

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
codeint: codeint.json has an unknown key budget; the keys allowed there are $schema, source_sets, budgets, regression_limits, languages, plugins, plugin_limits, boundaries, duplication, entry_points
Remove the key, or correct its spelling to one of the allowed keys.
```

### Source sets

Each file belongs to one Source set: `production`, `test`, `generated`, or `vendored`.
The Language plugin and the core rules settle the Source set of each file.

- `typescript-javascript` puts a file in `test` when its name ends in `.test.{extension}` or `.spec.{extension}`, such as `src/greet.test.ts`, or when any folder in its path is named `__tests__`, `test`, or `tests`, such as `test/greet.ts` or `packages/app/tests/setup.ts`.
  A file named `test.ts` or `tests.ts` outside such a folder stays in `production`.
- `rust` puts a file in `test` when any folder in its path is named `tests` or `benches`, when its name ends in `_test.rs` or `_tests.rs`, or when its name is `tests.rs`.
  Inline `#[cfg(test)]` modules inside a production file are skipped from `production` metrics.
- `php` puts a file in `test` when any folder in its path is named `tests` or `test`, or when its name ends in `Test.php`.
- `dart` puts a file in `test` when any folder in its path is named `test`, `tests`, or `integration_test`, or when its name ends in `_test.dart`, and puts files ending in `.g.dart`, `.freezed.dart`, `.gr.dart`, `.mocks.dart`, `.pb.dart`, `.pbenum.dart`, `.pbjson.dart`, or `.pbserver.dart` into `generated`.
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

### Regression limits

A key of `regression_limits` named after a Metric sets its limit in every Source set.
A key named after a Source set holds limits for that Source set only:

```json codeint.json
{
  "regression_limits": {
    "erosion": { "ratio_rise": 0.02 },
    "import_cycles": { "new_findings": "numerator_rise_with_new_findings" },
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
- `new_findings` says when new Findings are a Regression.
  `any_new_finding` fails on any new Finding, the default for Import cycles.
  `numerator_rise_with_new_findings` fails only when the numerator rises too, the default for the other Metrics.
  `never` lets new Findings pass; together with `ratio_rise: 1`, the Metric never makes a Regression there, which suits the `generated` and `vendored` Source sets.

codeint settles each setting on its own: the Source set limit wins, then the Metric limit, then the default.
So the example above gives production Erosion a ratio rise limit of 0.02 and fails it on any new Finding.

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
codeint ships four built-in Language plugins:

| Plugin name | Extensions | Workspace manifest |
| ----------- | ---------- | ------------------ |
| `typescript-javascript` | `.ts`, `.tsx`, `.mts`, `.cts`, `.js`, `.jsx`, `.mjs`, `.cjs` | `package.json` / `pnpm-workspace.yaml` |
| `rust` | `.rs` | `Cargo.toml` |
| `php` | `.php` | `composer.json` |
| `dart` | `.dart` | `pubspec.yaml` |

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
    { "plugin": "python", "include": ["services"], "exclude": [] }
  ]
}
```

### Plugins

Each entry of `plugins` declares an external WebAssembly component Language plugin or Metric plugin loaded from either a local `path` or an HTTPS `url`, with a required 64-character hexadecimal SHA-256 digest.
See [docs/plugins.md](./plugins.md) to build, hash, test, and version external Plugins.
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

`boundaries` enforces directional import rules and `public_files` encapsulation over resolved runtime imports across every supported Language plugin.
When `boundaries` is omitted, boundary evaluation is completely inactive.
Set `boundaries.preset` to `layered` (`presentation`, `application`, `domain`, `infrastructure`) or `hexagonal` (`domain`, `ports`, `application`, `adapters`) to start from built-in zone globs and directional rules; entries in `boundaries.zones` override matching preset zones or add new ones, and rules in `boundaries.rules` replace preset rules for matching `from` zones.
Each zone in `boundaries.zones` declares repository-relative `patterns` globs and optional `public_files` globs naming the only files external zones may import.
When a file matches patterns in multiple zones, the most specific pattern (most literal characters, then longest pattern, then zone name alphabetically) wins.
Each rule in `boundaries.rules` sets `from` (a zone name or `*`), either `allow` or `deny` (mutually exclusive), and an optional `reason`.
By default, `boundaries` evaluates the `production` Source set only; set `source_sets` to evaluate additional Source sets, and set `strict_zones` to `true` to report files matching no zone.
In `codeint diff`, each boundary Finding carries a stable content-based identifier so pre-existing Findings are classified as unchanged while any newly introduced boundary Finding fails by default (`new_findings: "any_new_finding"`).

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
        "reason": "App layer stays independent of infrastructure"
      },
      {
        "from": "infra",
        "deny": ["app"],
        "reason": "Infrastructure must not import application code"
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

### Duplication

By default, Duplication reports exact normalized Clone groups so calibrated Reference ranges stay unchanged.
Set `duplication.near_clones` to `true` to also detect near-duplicate blocks that differ by small insertions, deletions, or edits using shingle Jaccard similarity at or above `duplication.min_similarity` (`0.85` by default, in `(0, 1]`).
Near-clone mode honors the same `import-or-export`, minimum token, and minimum distinct token rules as exact Clone groups, clusters overlapping exact and near-clone fragments across three or more locations into Clone families, and keeps Verbosity coupled only to exact Clone groups.

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

Orphan files detection traverses resolved runtime imports from the union of manifest-declared entry points (such as `package.json`, `Cargo.toml`, `pyproject.toml`, `composer.json`, and `pubspec.yaml`) and configured `entry_points` globs in `codeint.json`.
Production files unreachable from any Production entry point are reported as `unreferenced`, or as `test-only` when still imported by Test files.
Generated and Vendored Source sets are excluded, and when no entry points exist for a language or isolated Workspace package, Orphan files reporting stays silently inactive.
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

## Result cache

codeint caches the measurement of each commit it reads from Git objects, so a second `codeint diff` against the same base reads it back instead of measuring the commit again.
That covers the merge-base, the head ref of `--head`, and each commit of `codeint trend`.
An entry belongs to one commit and to the settings that shape a measurement: the codeint version, each Language plugin and its version, and the effective `codeint.json`.
Any change to `codeint.json` other than its `$schema` address measures the commit again.
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
            "cyclomatic_complexity": 16,
            "source_lines": 18,
            "function_mass": 67.8823
          }
        ]
      },
      "duplication": {
        "ratio": 0.0,
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
        "reference_range": {
          "low": 0.018,
          "high": 0.347,
          "source": "codeint panel of 20 TypeScript and JavaScript repositories, ADR 0008, codeint 0.1.0"
        },
        "verbose_lines": 1,
        "flagged_lines": 1,
        "clone_lines": 0,
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
| `source_sets` | object | One section per Source set, under the keys `production`, `test`, `generated`, and `vendored` |
| `languages` | object, optional | One section per Language plugin, under its name, when files of two or more Language plugins were measured; each section has `files`, `source_lines`, and `source_sets` for that plugin's files |
| `skipped` | array, optional | Files that codeint could not measure, each with its `path` and the `reason` |
| `declaration_only` | array of strings, optional | Paths of files that hold only type declarations and add no source lines |
| `over_budget` | array, optional | Metrics over their Budget, each with its `source_set`, `metric`, the `ratio` as the report shows it, and the `budget` |

A Source set section has `files` and `source_lines` for that Source set, and one object per Metric: `erosion`, `duplication`, `verbosity`, and `import_cycles`.
Each Metric object has these fields:

- `ratio`: the Metric value, a number from 0 to 1, rounded to four decimal places.
- `reference_range`: either an object with `low`, `high`, and `source`, or a string that says why no Reference range is shown.
  The top-level Source sets show a Reference range only when one Language plugin measured every file; a section under `languages` always shows its own plugin's ranges.
- The counts behind the ratio:
  `eroded_function_mass` and `total_function_mass` for Erosion, `clone_lines` for Duplication, `verbose_lines` for Verbosity, and `modules_in_import_cycles` and `modules` for Import cycles.
  Duplication and Verbosity divide by the `source_lines` of the Source set.
  Verbosity also splits `verbose_lines` into `flagged_lines` and `clone_lines`; a line that is both counts in each.
- The Findings, worst first:
  `clone_groups` for Duplication, and `findings` for every other Metric.
- `near_clones`, optional on `duplication`: present with `min_similarity` when near-clone detection is enabled in `codeint.json`; Duplication's `reference_range` then notes that Reference ranges are calibrated for exact Clone groups.
- `clone_families`, optional on `duplication`: unified Clone families when overlapping or transitively related Clone groups span three or more locations, ordered worst-first by `duplicated_lines`, with `clone_families_left_out` when truncated without `--all`, and `unlisted_clone_family_group_ids` when listed `clone_groups` belong to unlisted Clone families.
- `findings_left_out`, optional: how many Findings the list leaves out; `--all` lists every Finding.

`import_cycles` also has `unresolved_imports`: the runtime imports that the Language plugin could not resolve, so no Import cycle passes through them.
It has `count`, the list `imports`, and the optional `imports_left_out`.
Each import has `importer`, the path of the importing file, `specifier`, the text of the import, and `reason`.

Each Finding is an object with an `id`, followed by fields that depend on its kind:

- An eroded function has `path`, `line`, `cyclomatic_complexity`, `source_lines`, and `function_mass`.
- A Clone group has `tokens`, optional `similarity` (present on near-clone Findings as a ratio in `(0, 1]`), and `locations`; each location has `path`, `start_line`, and `end_line`.
- A Clone family has `duplicated_lines` (unique source lines across the family without double-counting overlaps), `clone_group_ids`, and canonical `locations` ordered by `path` then `start_line`.
- A Flagged line has `path`, `line`, and `rule`.
- An Import cycle has `modules`, the paths of the modules in the cycle.

The `id` of a Finding is a string of 16 lowercase hexadecimal digits.
It comes from the Metric, the file or files of the Finding, and their content, never from a line number, so it stays the same when a Finding moves within its file.
It is the same in every run and on every operating system, and CRLF line endings do not change it.
Two Findings with the same content in one file get different ids, numbered in the order they appear.
The content behind the `id` of each kind:

- An eroded function: every token from its first line to its last line, so a change to the function's code gives it a new `id`; whitespace and comments do not count.
- A Clone group: the token sequence that its copies share, with every name and every literal value treated alike, and the path of each copy.
- A Clone family: the sorted normalized token sequences of its constituent Clone groups and the canonical path of each member location.
- A Flagged line: the rule and the tokens of the line.
- An Import cycle: the paths of its modules.

Every `path` is relative to the root of the Git repository and uses `/` as the separator.
Line numbers start at 1.

An optional field is left out when it has no content: a Source set without files, an empty `skipped`, `declaration_only`, or `over_budget` list, and `findings_left_out` when no Finding is left out.

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
          "version": "0.5.0",
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
                "text": "Verbosity is the share of source lines that are Flagged lines, clone lines, or both. A Flagged line matches a verbose-code pattern of the source language, and the result names the rule it matches."
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
            "text": "Eroded function: cyclomatic complexity 16, over 10; 18 source lines; Function mass 67.8823."
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
| `duplication` | A Clone group or Clone family | The first fragment or canonical location, from its start line to its end line; the other fragments or locations are related locations |
| `verbosity` | A Flagged line | The line; the message names the Flagged-line rule |
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
Then it lists the Findings of each Source set, worst first, and last the files that codeint did not measure.
It lists at most 20 Findings per Metric, so the comment stays short; `--all` lists every Finding.

```sh exit=0
codeint check --format markdown
```

```markdown stdout
# codeint report

codeint measured 4 files with 39 source lines.

| Source set | Metric | Value | Behind the value | Reference range |
| ---------- | ------ | ----- | ---------------- | --------------- |
| Production | Erosion | 0.7986 | 67.8823 of 85.0032 Function mass in eroded functions | 0.352 to 0.6, source: codeint panel of 20 TypeScript and JavaScript repositories, ADR 0008, codeint 0.1.0 |
| Production | Duplication | 0 | 0 clone lines of 39 source lines | 0.017 to 0.346, source: codeint panel of 20 TypeScript and JavaScript repositories, ADR 0008, codeint 0.1.0 |
| Production | Verbosity | 0.0256 | 1 verbose line of 39 source lines: 1 Flagged line, 0 clone lines | 0.018 to 0.347, source: codeint panel of 20 TypeScript and JavaScript repositories, ADR 0008, codeint 0.1.0 |
| Production | Import cycles | 0 | 0 of 4 modules in an Import cycle | 0 to 0.308, source: codeint panel of 20 TypeScript and JavaScript repositories, ADR 0008, codeint 0.1.0 |

## Production Findings

### Erosion

| Function | Cyclomatic complexity | Source lines | Function mass |
| -------- | --------------------- | ------------ | ------------- |
| `src/route.ts:1` | 16 | 18 | 67.8823 |

### Verbosity

| Flagged line | Rule |
| ------------ | ---- |
| `src/sign.ts:4` | else-after-return |

## Files not measured

- `src/broken.ts`: cannot parse: Unexpected token at line 1, column 23
- `src/types.d.ts`: declaration only
```
