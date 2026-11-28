# codeint

codeint measures the Software entropy of a codebase (Erosion, Cognitive complexity, Test gap, Duplication, Verbosity, Import cycles, Import boundaries, Orphan files, and Dependency hygiene) and reports Hazards next to Software entropy, in any language that has a Language plugin.
It is for developers whose code is increasingly written by coding agents, so they see Software entropy grow before it compounds.
Built-in Language plugins cover TypeScript/JavaScript/Vue/Svelte, Rust, PHP, Dart, Python, Go, SQL, and Shell.

## Language

### Measurement

**Software entropy**:
The maintainability cost carried in the shape of code: duplication, needless verbosity, complexity concentrated in a few large functions, and weight that tests do not reach.
_Avoid_: structural debt, tech debt, code quality

**Metric**:
One named measurement of a codebase, reported on its own and never blended with other Metrics into a combined score.
_Avoid_: score, index, grade, rating

**Metric version**:
A positive integer that identifies the calculation rules of one Metric and increases whenever those rules change.
_Avoid_: formula revision, schema version

**Erosion**:
The share of total Function mass held by functions whose cyclomatic complexity is greater than 10.
_Avoid_: complexity score, hotspot ratio

**Function mass**:
The weight of one function, equal to its cyclomatic complexity times the square root of its source lines.
_Avoid_: function weight, function size

**Cognitive complexity**:
The share of total nesting-weighted cognitive complexity held by functions whose cognitive complexity is greater than 15.
_Avoid_: readability score, nesting score

**Test gap**:
The share of total Function mass that tests do not execute.
Each function adds its Function mass times the share of it that the Coverage report shows as not executed.
_Avoid_: CRAP, change risk, coverage score

**Coverage report**:
A file, written by the user's own test run, that records which source lines the tests executed.
_Avoid_: coverage data, test report

**Verbosity**:
The share of source lines that are Flagged lines.
_Avoid_: bloat, wordiness

**Flagged line**:
A source line that matches a verbose-code pattern of the source language.
_Avoid_: lint hit, violation, smell

**Hazards**:
The share of source lines that are Hazard lines.
_Avoid_: lint score, defect rate

**Hazard line**:
A source line that matches a pattern of the source language that is likely to cause wrong results, slow execution, or a security exposure.
_Avoid_: violation, vulnerability, anti-pattern

**Duplication**:
The share of source lines that belong to at least one Clone group.
_Avoid_: copy-paste, redundancy

**Clone group**:
Two or more code fragments with the same token sequence once identifiers and literals are normalized.
The fragments leave out import and export declarations, and use enough different tokens to be logic rather than a list of the same shape.
_Avoid_: duplicate, copy

**Clone family**:
Three or more overlapping exact or near-clone fragments clustered together across locations in a codebase.
_Avoid_: duplicate cluster, copy group

**Evaluation suite**:
The repeatable accuracy, performance, memory, and Reference range calibration harness built from owned synthetic planted-defect suites and a pinned manifest of public repositories measured only in ephemeral scratch folders outside codeint.
_Avoid_: benchmark pack, test corpus, validation set

**Import cycle**:
A group of modules in which every module can reach every other module through runtime imports.
The Import cycles Metric is the share of modules that belong to at least one Import cycle.
_Avoid_: circular dependency, dependency loop

**Import boundary**:
A directional rule or public-file constraint governing which architectural zones may import modules from another zone or which third-party external packages a zone may import.
_Avoid_: layer rule, module fence

**Orphan file**:
A Production source file unreachable through runtime imports from any entry point, classified as unreferenced or test-only.
_Avoid_: dead file, unused file

**Dependency hygiene**:
The set of three independent Metrics comparing package manifests against imported external packages: unused dependencies, unlisted dependencies, and dev dependencies imported in Production.
_Avoid_: package audit, dependency audit

**Unresolved import**:
A runtime import that the Language plugin could not match to a repository file or classify as external.
_Avoid_: broken import, missing import

**Workspace package**:
A package declared by a repository workspace or by a module manifest such as `go.mod`, measured on its own alongside the repository total.
_Avoid_: subpackage, member package, workspace member

**Finding**:
One located piece of evidence behind a Metric value, such as an eroded function, an untested function, a Clone group, a Flagged line, a Hazard line, or an Import cycle.
_Avoid_: issue, violation, warning, hotspot

**Reference range**:
The published typical value of a Metric for a class of codebases in one language family, shown next to a measured value for context.
_Avoid_: benchmark, target, threshold

**Source set**:
One of four groups a source file belongs to: Production, Test, Generated, or Vendored.
_Avoid_: file kind, file category

**Tooling file**:
A Production source file that builds, tests, lints, or configures the repository rather than shipping as application or library runtime code, classified either by its Language plugin (`is-tooling-file`) or by a universal repository tooling folder (`.github/`, `scripts/`, `docs/`, `examples/`, `tools/`, `benches/`).
_Avoid_: build script, config file, helper script, non-runtime file

**Budget**:
The highest value a Metric may reach in one Source set before codeint reports a failure.
_Avoid_: limit, threshold, quota

**Ratchet**:
A repository file (`.codeint/ratchet.json` by default) that records each Metric's ceiling ratio alongside its Metric version so whole-repository checks fail if Software entropy or Hazards rise above the recorded ceiling.
_Avoid_: lockfile, snapshot, baseline file

**Baseline**:
A committed list of existing Findings in `.codeint/baseline.json` that `codeint check` hides and removes permanently as those Findings are fixed.
_Avoid_: ignore list, suppression file, debt file

**Regression**:
An increase in a Metric, from the merge-base of a base Git ref to the code under measurement, that exceeds the allowed limit.
_Avoid_: degradation, drift

### Extension

**Plugin**:
A unit that extends codeint with either support for one source language or one Metric.
_Avoid_: extension, add-on, provider

**Code model**:
The language-neutral description of a codebase that Language plugins produce, codeint completes with Coverage report facts, and Metric plugins read.
_Avoid_: AST, IR, inventory, syntax tree

**Language plugin**:
A Plugin that owns every rule specific to one language family and lowers its source files into the Code model.
_Avoid_: parser plugin, language adapter, language provider

**Conformance runner**:
The built-in `codeint plugin test` command that loads one Wasm **Plugin** through the Plugin host and checks it against the Plugin contract, printing each violated rule with a fix hint.
_Avoid_: plugin linter, plugin validator, plugin test suite

**Dialect**:
One variant of a language that its Language plugin reads with different rules, such as PostgreSQL or SQLite within SQL.
_Avoid_: flavor, variant, engine

**Framework profile**:
A package-level rule set inside a **Language plugin** that adds framework detection, convention entry points, framework-generated **Source set** paths, fallback import hints, and optional **Reference ranges** while keeping per-file reading framework-blind.
_Avoid_: framework plugin, preset, adapter

**Metric plugin**:
A Plugin that computes one Metric from the Code model alone, without knowing the source language.
_Avoid_: analyzer, detector, rule

## Relationships

- A **Language plugin** supports exactly one language family.
- A language family may have several **Dialects**; one **Language plugin** reads them all.
- A **Metric plugin** works on every language that has a **Language plugin**.
- A **Metric plugin** computes exactly one **Metric**.
- Every **Metric** except **Hazards** measures **Software entropy**.
- **Verbosity** counts **Flagged lines** only and is independent of **Duplication**.
- **Test gap** reads test execution from a **Coverage report**; codeint never runs tests.
- **Test gap** covers the Production **Source set** only.
- A **Language plugin** may report **Hazard lines**; **Hazards** counts them without knowing the language.
- A **Metric** may have one or more **Reference ranges**, each for one language family.
- Everything that differs between languages belongs to a **Language plugin**: decision points, imports, workspaces, **Source set** rules, **Flagged line** rules, and **Hazard line** rules.
- A **Metric** value is backed by zero or more **Findings**.
- Every **Metric** carries a positive integer **Metric version** that increases whenever its calculation rules change.
- A **Budget** applies to one **Metric** in one **Source set**.
- A **Ratchet** stores one ceiling and **Metric version** per measured **Metric** and applies to whole-repository runs only.
- A **Baseline** stores the **Finding** identifiers and **Metric version** of each measured **Metric**; a **Budget** applies to the **Findings** that the **Baseline** does not hide.
- Every source file belongs to exactly one **Source set**.
- A **Tooling file** stays in the Production **Source set** for all Production **Metrics** and **Hazards** except **Orphan files** and **Dependency hygiene** dev-in-production checks.
  Listing a **Tooling file** under `entry_points` overrides the **Orphan files** exclusion (not `dev_in_production_dependencies`).

## Flagged ambiguities

- "plugin" and "extension" were both used for the same idea.
  Resolved: **Plugin** is the term; "extension" is avoided.
- "score" was used for both a single Metric value and a blended index.
  Resolved: codeint reports **Metrics** only; it has no blended score.
