# codeint

codeint measures the Software entropy of a codebase: duplication, needless verbosity, and complexity concentrated in a few large functions.
It is for developers whose code is increasingly written by coding agents, so they see Software entropy grow before it compounds.

codeint reports each Metric on its own, with the Findings behind it, and never blends them into one score.
It gates CI on per-Metric Budgets and on Regressions against a base Git ref.
Languages and Metrics are Plugins: TypeScript and JavaScript are the first built-in Language plugin, and more languages follow.

[GLOSSARY.md](./GLOSSARY.md) defines every term on this page.

## Status

codeint is in early development and has no release yet.
After the first release, it installs with one command:

```sh
curl -fsSL https://github.com/avidianity/codeint/releases/latest/download/install.sh | bash
```

```powershell
irm https://github.com/avidianity/codeint/releases/latest/download/install.ps1 | iex
```

```sh
npx codeint check
```

## Start a configuration

`codeint init` writes a starter `codeint.json` at the root of the Git repository that contains the current folder:

```sh
codeint init
```

```text
Wrote /home/me/shop/codeint.json
```

The file links to the JSON schema of the running codeint version, so your editor completes and checks it.
It sets every key to its default, and lists under `languages` each top-level folder or file that a Language plugin measures:

```json
{
  "$schema": "https://raw.githubusercontent.com/avidianity/codeint/v0.1.0/schema/codeint.schema.json",
  "source_sets": { "production": [], "test": [], "generated": [], "vendored": [] },
  "budgets": { "production": {}, "test": {}, "generated": {}, "vendored": {} },
  "regression_limits": {},
  "languages": [
    { "plugin": "typescript-javascript", "include": ["packages", "scripts"], "exclude": [] }
  ]
}
```

When no file belongs to a Language plugin, the file leaves `languages` out, so every Language plugin measures the whole repository.
`codeint init` never replaces an existing `codeint.json`; it exits with code 2 instead.

## Budgets

A Budget is the highest ratio a Metric may reach in one Source set.
Set Budgets in `codeint.json` at the root of the repository, one object per Source set:

```json
{
  "budgets": {
    "production": { "erosion": 0.2, "duplication": 0.05, "verbosity": 0.1, "import_cycles": 0 },
    "test": { "duplication": 0.2 }
  }
}
```

Each Budget is a number from 0 to 1.
A Budget applies only to its own Source set, and a Metric without a Budget never fails.
A Budget applies to the total of every language, never to the section of one Language plugin under `languages`.
codeint compares each Budget with the ratio as the report shows it, rounded to four decimal places, so a ratio equal to its Budget passes.

When a Metric is over its Budget, codeint still writes the whole report, lists the Metric under `over_budget`, names it on standard error, and exits with code 1:

```text
codeint: the production erosion ratio 0.4321 is over its Budget 0.2
```

## Compare with a base branch

`codeint diff {base-ref}` compares the working tree with the merge-base of the base ref and HEAD:

```sh
codeint diff main
codeint diff origin/main --format markdown
```

codeint reads the merge-base straight from Git objects, so it never checks out a commit and leaves the working tree and the index as they are.
The working tree side includes uncommitted and untracked changes, and leaves out the files that Git ignores, as `codeint check` does.
Commits added to the base branch after your branch started are not part of the comparison, because the merge-base comes before them.

Both sides use the `codeint.json` of the working tree, so the comparison shows changes in the code, not in the configuration.
Both sides follow the same file rules as `codeint check`: symbolic links are not followed, Git submodules are not measured, and files that are not valid UTF-8 are skipped.

The report classifies each Finding by its `id`: new when only the working tree has it, resolved when only the merge-base has it, and unchanged when both have it.
For each Metric of each Source set, it shows the ratio on both sides, the new and the resolved Findings, worst first, and the number of unchanged Findings:

```text
base:
  ref: main
  merge_base: 651980e53c24d8e14611d5b8cc84c11f0bbb3dca
head: working tree
source_sets:
  production:
    erosion:
      base_ratio: 0
      head_ratio: 0.9182
      new[1]{id,path,line,cyclomatic_complexity,source_lines,function_mass}:
        fb04850b9b041f9e,src/parse.ts,1,12,14,44.8999
      resolved: []
      unchanged: 0
```

It lists at most 20 new and 20 resolved Findings per Metric, and counts the rest in `new_left_out` and `resolved_left_out`; `--all` lists every one.
`--format json` and `--format markdown` print the same comparison.
`--format sarif` lists only the new Findings, so code scanning annotates what the change added.
A base ref that names no commit stops the run with exit code 2 and names the ref.

### Compare two refs

`--head {ref}` measures a second ref instead of the working tree, so you can compare two commits or two branches:

```sh
codeint diff main --head feature
```

codeint reads the head ref from Git objects too, so neither the working tree nor HEAD takes part.
The merge-base is the merge-base of the base ref and the head ref.
Both sides still use the `codeint.json` of the working tree, never the one in a commit.
So the classification matches running `codeint check` on a checkout of each side, when each checkout has that same `codeint.json`.
The report names the head ref and its commit:

```text
head:
  ref: feature
  commit: 8369109f265f4391e4419e8b7c29d33a75ebc59b
```

A head ref that names no commit, or that shares no history with the base ref, stops the run with exit code 2.

### Regressions

A Regression is a Metric that got worse from the merge-base to the head side by more than its limit.
`codeint diff` checks each Metric of each Source set against three default rules:

- Any new Import cycle is a Regression.
- A ratio rise over 0.01 is a Regression.
  codeint compares the ratios as the report shows them, rounded to four decimal places, so a rise of exactly 0.01 passes.
- A numerator rise that adds new Findings is a Regression.
  The numerator is `eroded_function_mass`, `clone_lines`, `verbose_lines`, or `modules_in_import_cycles`.
  A Finding that only moves, so the numerator stays the same, is not a Regression.

`codeint diff` also applies the Budgets of `codeint.json` to the head side, the working tree or the `--head` ref, as `codeint check` does.

When a Budget or a Regression fails, codeint still writes the whole report, lists each failure under `over_budget` or `regressions`, names it on standard error, and exits with code 1.
Each Regression names its rule and its base and head values: the ratios for `ratio_rise` and `new_findings`, and the numerators for `numerator_rise_with_new_findings`.
`ratio_rise_limit` is the ratio rise limit of the Metric in that Source set:

```text
regressions[1]{source_set,metric,rule,base,head,new_findings,ratio_rise_limit}:
  production,import_cycles,ratio_rise,0.01,0.0204,0,0.01
```

```text
codeint: the production import_cycles ratio rose from 0.01 to 0.0204, more than its Regression limit 0.01
```

With no failure, the report has no `over_budget` or `regressions` key, and codeint exits with code 0.

#### Regression limits

Set Regression limits in `codeint.json` under `regression_limits`.
A key named after a Metric sets its limit in every Source set.
A key named after a Source set holds limits for that Source set only:

```json
{
  "regression_limits": {
    "erosion": { "ratio_rise": 0.02 },
    "import_cycles": { "new_findings": "numerator_rise_with_new_findings" },
    "generated": { "duplication": { "ratio_rise": 1 } }
  }
}
```

Each limit has two settings, and you may set either one or both:

- `ratio_rise` is the highest ratio rise that passes, a number from 0 to 1; the default is 0.01.
- `new_findings` says when new Findings are a Regression.
  `any_new_finding` fails on any new Finding, the default for Import cycles.
  `numerator_rise_with_new_findings` fails only when the numerator rises too, the default for the other Metrics.

codeint settles each setting on its own: the Source set limit wins, then the Metric limit, then the default.
So `{ "erosion": { "ratio_rise": 0.02 }, "production": { "erosion": { "new_findings": "any_new_finding" } } }` gives production Erosion a ratio rise limit of 0.02 and fails it on any new Finding.

An invalid limit stops the run with exit code 2 and names the key:

```text
codeint: codeint.json: regression_limits.production.erosion.ratio_rise must be between 0 and 1, not 1.5
```

## Report formats

`codeint check` prints the report in TOON by default.
Pass `--format` to choose another format:

```sh
codeint check --format toon
codeint check --format json
codeint check --format sarif
codeint check --format markdown
```

`--output {file}` writes the report in the chosen format to a file instead of standard output.

### JSON report

`--format json` prints one JSON object with the same fields and values as the TOON report.
Tools can build on this shape: the compatibility rule below says how it may change.

```json
{
  "files": 1,
  "source_lines": 7,
  "source_sets": {
    "production": {
      "files": 1,
      "source_lines": 7,
      "erosion": {
        "ratio": 0.0,
        "reference_range": "no published Reference range for this language",
        "eroded_function_mass": 0.0,
        "total_function_mass": 5.2915,
        "findings": []
      },
      "duplication": {
        "ratio": 0.0,
        "reference_range": "no published Reference range for this language",
        "clone_lines": 0,
        "clone_groups": []
      },
      "verbosity": {
        "ratio": 0.1429,
        "reference_range": "no published Reference range for this language",
        "verbose_lines": 1,
        "flagged_lines": 1,
        "clone_lines": 0,
        "findings": [{ "id": "2a57800b29941394", "path": "src/sign.ts", "line": 4, "rule": "else-after-return" }]
      },
      "import_cycles": {
        "ratio": 0.0,
        "reference_range": "no published Reference range for this language",
        "modules_in_import_cycles": 0,
        "modules": 1,
        "findings": [],
        "unresolved_imports": { "count": 0, "imports": [] }
      }
    }
  },
  "skipped": [{ "path": "src/broken.ts", "reason": "cannot parse: Unexpected token at line 1, column 14" }],
  "declaration_only": ["src/types.d.ts"]
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
- `findings_left_out`, optional: how many Findings the list leaves out; `--all` lists every Finding.

`import_cycles` also has `unresolved_imports`: the runtime imports that the Language plugin could not resolve, so no Import cycle passes through them.
It has `count`, the list `imports`, and the optional `imports_left_out`.
Each import has `importer`, the path of the importing file, `specifier`, the text of the import, and `reason`.

Each Finding is an object with an `id`, followed by fields that depend on its kind:

- An eroded function has `path`, `line`, `cyclomatic_complexity`, `source_lines`, and `function_mass`.
- A Clone group has `tokens` and `locations`; each location has `path`, `start_line`, and `end_line`.
- A Flagged line has `path`, `line`, and `rule`.
- An Import cycle has `modules`, the paths of the modules in the cycle.

The `id` of a Finding is a string of 16 lowercase hexadecimal digits.
It comes from the Metric, the file or files of the Finding, and their content, never from a line number, so it stays the same when a Finding moves within its file.
It is the same in every run and on every operating system, and CRLF line endings do not change it.
Two Findings with the same content in one file get different ids, numbered in the order they appear.
The content behind the `id` of each kind:

- An eroded function: every token from its first line to its last line, so a change to the function's code gives it a new `id`; whitespace and comments do not count.
- A Clone group: the token sequence that its copies share, with every name and every literal value treated alike, and the path of each copy.
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

The tool lists one rule per Metric, and each result names its Metric in `ruleId`:

| `ruleId` | Finding | Location |
| -------- | ------- | -------- |
| `erosion` | An eroded function | The line where the function starts |
| `duplication` | A Clone group | The first fragment, from its start line to its end line; the other fragments are related locations |
| `verbosity` | A Flagged line | The line; the message names the Flagged-line rule |
| `import_cycles` | An Import cycle | Line 1 of the first module; the other modules are related locations |

Each result has the level `note`, and its `properties.sourceSet` names its Source set: `production`, `test`, `generated`, or `vendored`.

```json
{
  "ruleId": "verbosity",
  "ruleIndex": 2,
  "level": "note",
  "message": { "text": "Flagged line: rule else-after-return." },
  "locations": [
    {
      "physicalLocation": {
        "artifactLocation": { "uri": "src/sign.ts", "uriBaseId": "SRCROOT" },
        "region": { "startLine": 4 }
      }
    }
  ],
  "properties": { "sourceSet": "production" }
}
```

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

```markdown
# codeint report

codeint measured 1 file with 7 source lines.

| Source set | Metric | Value | Behind the value | Reference range |
| ---------- | ------ | ----- | ---------------- | --------------- |
| Production | Erosion | 0 | 0 of 5.2915 Function mass in eroded functions | no published Reference range for this language |
| Production | Duplication | 0 | 0 clone lines of 7 source lines | no published Reference range for this language |
| Production | Verbosity | 0.1429 | 1 verbose line of 7 source lines: 1 Flagged line, 0 clone lines | no published Reference range for this language |
| Production | Import cycles | 0 | 0 of 1 module in an Import cycle | no published Reference range for this language |

## Production Findings

### Verbosity

| Flagged line | Rule |
| ------------ | ---- |
| `src/sign.ts:4` | else-after-return |

## Files not measured

- `src/broken.ts`: cannot parse: Unexpected token at line 1, column 14
- `src/types.d.ts`: declaration only
```

## Development

You need:

- Rust through rustup; [rust-toolchain.toml](./rust-toolchain.toml) selects the version.
- Node.js 24 and npm, for the development tools in [package.json](./package.json).
- gitleaks and lychee on your `PATH`, for the Git hook.

Set up a checkout:

```sh
npm ci
npx lefthook install
cargo test --workspace --locked
```

Agents and people follow the same rules: start with [.rules](./.rules).
The checks that guard the repository are listed in [docs/checks](./docs/checks/README.md).

## License

[MIT](./LICENSE)
