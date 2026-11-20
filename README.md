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

## Report formats

`codeint check` prints the report in TOON by default.
Pass `--format` to choose another format:

```sh
codeint check --format toon
codeint check --format json
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
        "findings": [{ "path": "src/sign.ts", "line": 4, "rule": "else-after-return" }]
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
| `skipped` | array, optional | Files that codeint could not measure, each with its `path` and the `reason` |
| `declaration_only` | array of strings, optional | Paths of files that hold only type declarations and add no source lines |

A Source set section has `files` and `source_lines` for that Source set, and one object per Metric: `erosion`, `duplication`, `verbosity`, and `import_cycles`.
Each Metric object has these fields:

- `ratio`: the Metric value, a number from 0 to 1, rounded to four decimal places.
- `reference_range`: either an object with `low`, `high`, and `source`, or a string that says why no Reference range is shown.
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

Each Finding is an object:

- An eroded function has `path`, `line`, `cyclomatic_complexity`, `source_lines`, and `function_mass`.
- A Clone group has `tokens` and `locations`; each location has `path`, `start_line`, and `end_line`.
- A Flagged line has `path`, `line`, and `rule`.
- An Import cycle has `modules`, the paths of the modules in the cycle.

Every `path` is relative to the root of the Git repository and uses `/` as the separator.
Line numbers start at 1.

An optional field is left out when it has no content: a Source set without files, an empty `skipped` or `declaration_only` list, and `findings_left_out` when no Finding is left out.

#### Compatibility rule

Later versions of codeint change the JSON report only in ways that keep existing readers working:

- They only add fields, and never rename or remove a field.
- A field keeps its type and its meaning, including its unit and its scale.
- A field that is always present stays always present.
- They may add a Source set key, a Metric object, or a kind of Finding.

Readers ignore fields they do not know, and treat a missing optional field as empty.

### Markdown report

`--format markdown` prints a report to paste into a pull request comment.
It starts with one table: each Metric's value per Source set, the counts behind the value, and the Reference range.
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
