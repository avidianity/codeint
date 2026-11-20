# codeint

codeint measures the Software entropy of a codebase, in any language that has a Language plugin: duplication, needless verbosity, and complexity concentrated in a few large functions.
It is for developers whose code is increasingly written by coding agents, so they see Software entropy grow before it compounds.
TypeScript and JavaScript are the first built-in Language plugin.

## Language

### Measurement

**Software entropy**:
The maintainability cost carried in the shape of code: duplication, needless verbosity, and complexity concentrated in a few large functions.
_Avoid_: slop, sloppiness, structural debt, tech debt, code quality

**Metric**:
One named measurement of Software entropy, reported on its own and never blended with other Metrics into a combined score.
_Avoid_: score, index, grade, rating

**Erosion**:
The share of total Function mass held by functions whose cyclomatic complexity is greater than 10.
_Avoid_: complexity score, hotspot ratio

**Function mass**:
The weight of one function, equal to its cyclomatic complexity times the square root of its source lines.
_Avoid_: function weight, function size

**Verbosity**:
The share of source lines that are Flagged lines, clone lines, or both.
_Avoid_: bloat, wordiness

**Flagged line**:
A source line that matches a verbose-code pattern of the source language.
_Avoid_: lint hit, violation, smell

**Duplication**:
The set of Clone groups in a codebase and the source lines they cover.
_Avoid_: copy-paste, redundancy

**Clone group**:
Two or more code fragments with the same token sequence once identifiers and literals are normalized.
_Avoid_: duplicate, copy

**Import cycle**:
A group of modules in which every module can reach every other module through imports.
_Avoid_: circular dependency, dependency loop

**Finding**:
One located piece of evidence behind a Metric value, such as an eroded function, a Clone group, a Flagged line, or an Import cycle.
_Avoid_: issue, violation, warning, hotspot

**Reference range**:
The published typical value of a Metric for a class of codebases in one language family, shown next to a measured value for context.
_Avoid_: benchmark, target, threshold

**Source set**:
One of four groups a source file belongs to: Production, Test, Generated, or Vendored.
_Avoid_: file kind, file category

**Budget**:
The highest value a Metric may reach in one Source set before codeint reports a failure.
_Avoid_: limit, threshold, quota

**Regression**:
An increase in a Metric between a base Git ref and the code under measurement that exceeds the allowed limit.
_Avoid_: degradation, drift

### Extension

**Plugin**:
A unit that extends codeint with either support for one source language or one Metric.
_Avoid_: extension, add-on, provider

**Code model**:
The language-neutral description of a codebase that Language plugins produce and Metric plugins read.
_Avoid_: AST, IR, inventory, syntax tree

**Language plugin**:
A Plugin that owns every rule specific to one language family and lowers its source files into the Code model.
_Avoid_: parser plugin, language adapter, language provider

**Metric plugin**:
A Plugin that computes one Metric from the Code model alone, without knowing the source language.
_Avoid_: analyzer, detector, rule

## Relationships

- A **Language plugin** supports exactly one language family.
- A **Metric plugin** works on every language that has a **Language plugin**.
- A **Metric plugin** computes exactly one **Metric**.
- **Verbosity** counts the lines of every **Clone group** that **Duplication** finds.
- A **Metric** may have one or more **Reference ranges**, each for one language family.
- Everything that differs between languages belongs to a **Language plugin**: decision points, imports, workspaces, **Source set** rules, and **Flagged line** rules.
- A **Metric** value is backed by zero or more **Findings**.
- A **Budget** applies to one **Metric** in one **Source set**.
- Every source file belongs to exactly one **Source set**.

## Flagged ambiguities

- "plugin" and "extension" were both used for the same idea.
  Resolved: **Plugin** is the term; "extension" is avoided.
- "score" was used for both a single Metric value and a blended index.
  Resolved: codeint reports **Metrics** only; it has no blended score.
