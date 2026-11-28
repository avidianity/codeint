# Test gap

**Test gap** (`test_gap`) measures the share of total Production **Function mass** that your tests do not execute.
It reads line execution counts from **Coverage reports** that your own test run writes, in LCOV or Cobertura XML.
codeint never runs your tests and never guesses where a Coverage report lives ([ADR 0017](../adr/0017-test-gap-from-coverage-reports.md)).

## What Test gap measures

For every Production function, codeint computes an **untested share**:

- Assign each listed line in a file to the innermost enclosing function span, excluding lines that fall inside a strictly nested child function or closure ([ADR 0039](../adr/0039-metric-v2-test-gap.md)).
- Count the function's owned lines that a Coverage report lists with a hit count of `0`.
- Divide by all of the function's owned lines that a Coverage report lists.
- Lines that no Coverage report lists count on neither side.

Test gap is the sum over Production functions of `Function mass * untested share`, divided by the total Function mass of those functions.
Because **Function mass** is `cyclomatic_complexity * sqrt(source_lines)`, an untested complex function raises Test gap far more than an untested trivial helper.

### Why Test gap instead of CRAP

CRAP (Change Risk Anti-Patterns) squares cyclomatic complexity and multiplies it by untested share cubed.
In codeint, **Erosion** and **Function mass** already weigh complexity, so CRAP would move together with Erosion instead of reporting an independent signal.
Test gap moves only when test execution moves ([ADR 0017](../adr/0017-test-gap-from-coverage-reports.md)).

### Scope, absence, and edge cases

- **Production only**: Test gap covers the Production **Source set** only.
  Test, Generated, and Vendored files are never judged by whether tests execute them.
- **Absent without a Coverage report**: When you name no Coverage report, Test gap is absent (never `0` or `1`), and the report prints one line stating why.
  In a multi-language repository, any language covered by no named report also shows Test gap as absent with a one-line reason, so a missing report never reads as "nothing is tested".
- **Files outside every report**: A Production function whose file appears in no named report, in a language that at least one named report covers, counts as fully untested (`untested share = 1`).
  Code that your test runner never loaded cannot hide behind a zero Test gap.
- **Nested functions and closures**: A line inside a strictly nested function or closure belongs only to that inner function, matching Function mass ownership across all languages (including Shell `<script>` top-level bodies wrapping named functions).
  An untested inner function never penalizes an executed outer function.
- **Functions sharing identical start and end lines**: When two functions occupy the exact same start and end lines (such as two inline functions on one line), neither strictly nests inside the other and both retain those lines.
  Line-based Coverage reports carry no column information, so codeint cannot distinguish execution between two functions on the same line.
- **Functions with no listed lines**: A function whose file appears in a Coverage report, but that owns no listed line (such as non-executable declarations or a wrapper whose only executable lines lie inside nested functions), is left out of Test gap on both numerator and denominator.
- **Multiple reports add up**: When you name several Coverage reports, a line counts as executed when any named report executed it.
  You can combine unit and end-to-end reports, or mix LCOV and Cobertura XML in the same run.
- **Line hits only**: Branch records, function records, method records, and summary rates in Coverage reports are ignored.
- **No Reference range**: Computing a calibrated **Reference range** would require running the test suite of every panel repository, so no Language plugin publishes a Reference range for Test gap.

## Reading Test gap reports and Findings

`codeint check` reports Test gap for the repository Production total, for each covered language, and for each **Workspace package**.
Each Test gap **Finding** holds:

- the repository-relative file path
- the function name, when it has one
- the function's first line
- its **Function mass**
- its **untested share**

Findings are ordered by `Function mass * untested share`, largest first.
The top of the list is where writing one test removes the most untested Function mass.
A fully tested function produces no Finding.

## Supported formats, path matching, and safety

codeint reads two Coverage report formats and detects the format from file content, so the file name does not matter:

- **Supported**: LCOV and Cobertura XML.
- **Not supported**: Clover, JaCoCo, Go `coverprofile`, and tool-specific JSON formats.
  Naming an unsupported file stops the run with exit code `2` and lists both supported formats.

codeint matches each path in a Coverage report to a repository file automatically:

- For Cobertura XML, codeint tries each listed `<source>` root as a prefix first.
- For both formats, codeint matches each path by the longest matching ending against repository files, handling absolute paths, repository-relative paths, and package-relative paths.
- codeint never follows a path outside the repository.
- Any report path that matches no repository file, or matches several files equally well, is named in one unmatched-path note in the report so a wrong path cannot silently change Test gap.
- A report line beyond the end of its matched file stops the run with exit code `2`, naming the report, the file, and the line, so a stale report cannot produce a wrong ratio.

Coverage reports are treated as untrusted input with strict size and XML safety bounds; see [docs/reference.md#test-gap](../reference.md#test-gap).

## Generating LCOV Coverage reports

> [!NOTE]
> Several test runners (including Vitest and `c8`) omit files that tests never imported unless you configure an include option.
> codeint still counts functions in those omitted files as fully untested once another file of that language appears in a report.
> Setting the runner's include option keeps the runner's own summary aligned with codeint.

### Bun

Bun writes LCOV as its only coverage file format, and requires `--coverage-reporter=lcov` because text output is the default:

```sh
bun test --coverage --coverage-reporter=lcov
```

This writes `coverage/lcov.info`.

### Node test runner

Node's built-in test runner writes LCOV as its only coverage file format:

```sh
node --test --experimental-test-coverage --test-reporter=lcov --test-reporter-destination=lcov.info
```

Add `--test-reporter=spec --test-reporter-destination=stdout` to keep terminal test output alongside `lcov.info`.

### Vitest

Vitest does not write LCOV by default; enable the `lcov` reporter:

```sh
npx vitest run --coverage --coverage.reporter=lcov
```

This writes `coverage/lcov.info`.
In `vitest.config.ts`, set `test.coverage.include` (for example `["src/**/*.ts"]`, and `all: true` on older Vitest releases) so untouched source files are included in Vitest's own report as well.

### Jest

Pass `--coverageReporters=lcov` to write `coverage/lcov.info`:

```sh
npx jest --coverage --coverageReporters=lcov
```

Configure `collectCoverageFrom` in your Jest configuration to include source files that tests never imported.

### c8

Pass `--reporter=lcov` alongside `--all` and `--include` so unloaded files are tracked:

```sh
npx c8 --all --include "src/**/*.js" --reporter=lcov node --test
```

This writes `coverage/lcov.info`.

### Rust (`cargo-llvm-cov`)

Write an LCOV file with `--lcov` and `--output-path`:

```sh
cargo llvm-cov --lcov --output-path lcov.info
```

Add `--workspace` for a multi-crate workspace.

### Dart and Flutter

Both Flutter and Dart write LCOV as their coverage file format.

In a Flutter project, run:

```sh
flutter test --coverage
```

This writes `coverage/lcov.info`.

In a pure Dart project, collect raw coverage and format it to LCOV with `package:coverage`:

```sh
dart test --coverage=coverage
dart pub global run coverage:format_coverage --lcov --in=coverage --out=coverage/lcov.info --packages=.dart_tool/package_config.json --report-on=lib
```

### Python (`coverage.py`)

Write LCOV with `coverage lcov`:

```sh
coverage run -m pytest
coverage lcov -o coverage.lcov
```

You can also write Cobertura XML with `coverage xml` (below).

## Generating Cobertura XML Coverage reports

### PHPUnit

PHPUnit 13.4 writes Cobertura XML and cannot write LCOV.
With PCOV or Xdebug (`xdebug.mode=coverage`) enabled, run:

```sh
vendor/bin/phpunit --coverage-cobertura coverage.xml
```

### Pest

Pest 5 writes Cobertura XML and cannot write LCOV.
With PCOV or Xdebug enabled, run:

```sh
vendor/bin/pest --coverage --coverage-cobertura=coverage.xml
```

### Python (`coverage.py`)

Write Cobertura XML with `coverage xml`:

```sh
coverage run -m pytest
coverage xml -o coverage.xml
```

## Configuring codeint

List your Coverage reports in the configuration file under `coverage_reports`, using paths relative to the repository root.
You can mix LCOV and Cobertura XML files in the same list, and set a Production **Budget** under `budgets.production.test_gap`:

```jsonc
{
  "coverage_reports": [
    "web/coverage/lcov.info",
    "api/coverage.xml",
  ],
  "budgets": {
    "production": {
      "test_gap": 0.25,
    },
  },
}
```

To pass Coverage reports from a CI step, use the repeatable `--coverage-report` option on `codeint check`:

```sh
codeint check --coverage-report web/coverage/lcov.info --coverage-report api/coverage.xml
```

When `--coverage-report` is present on the command line, it replaces `coverage_reports` from the configuration file for that run.

## Budgets, `codeint diff`, and `codeint trend`

- **Budget**: `budgets.production.test_gap` accepts a number from `0` to `1`.
  A Test gap ratio under or equal to its Budget passes.
  A ratio over its Budget still writes the full report, names the failure on standard error, and exits with code `1`.
- **Missing Coverage report with a Budget**: If `budgets.production.test_gap` is set and Test gap is absent, codeint stops the run with exit code `2` and reports that the Budget needs a Coverage report.
  A CI job that lost its Coverage report never passes silently.
- **Invalid Budget or Regression keys**: Setting `test_gap` under `budgets.test`, `budgets.generated`, or `budgets.vendored`, or anywhere under `regression_limits`, stops the run with exit code `2` and names the key.
- **`codeint diff` and `codeint trend`**: Neither command computes Test gap, because a Coverage report exists only for the code under measurement and not for the merge-base or past commits.
  When a Coverage report is named, both commands print one line stating that Test gap is left out.

See [docs/reference.md#test-gap](../reference.md#test-gap) for every CLI flag, schema key, security bound, and report field.
