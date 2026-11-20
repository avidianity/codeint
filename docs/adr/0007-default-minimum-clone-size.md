---
status: accepted
---

# The default minimum clone size is 75 tokens, with a structural filter

The Owner decided this on 2026-11-21, after Ticket #33 of Spec #10 measured candidate minimums the same day.
`MinimumCloneTokens::DEFAULT` holds the 75 tokens.

Amendment, 2026-11-21: the Owner also approved the structural filter that this record measured, and Ticket #76 builds it.
The filter is part of what a Clone group is.
`MinimumDistinctTokens::DEFAULT` holds its threshold of 14 distinct normalized tokens.

## Context

Duplication finds Clone groups of at least a minimum number of tokens.
It normalizes every identifier to one token and every literal to another, so it finds type-1 and type-2 clones in one pass.
A small minimum finds more Clone groups, but more of them are false positives.
A large minimum misses real clones of moderate size.
An earlier run on the private workspace at 50 tokens gave about 35% Duplication with type-2 matching, against 8% with type-1 matching only.

### Corpus

- A private Bun workspace of about 1.09 million TypeScript lines.
  It is not named here, and no code from it appears in this record.
- `excalidraw/excalidraw` at commit `2c3dcc06ec807e81ab9437eceb1a8f7a3c590743` (2026-11-19): a React drawing application, heavy in JSX, SVG icons, and literal data tables.
- `nestjs/nest` at commit `e0600bb04c3aedc3f643cc1dd2d5e515e12cbc2a` (2026-11-20): a server framework, heavy in classes, decorators, and per-transport adapters, with many small example projects under `sample/`.

### Method

- Build codeint at commit `e481397` of the Integration branch `feat/10-codeint-v1`, with `cargo build --release --locked`.
- Change the minimum per run through a local-only knob that was never committed.
- Run `codeint check --all` on each repository at 30, 50, 75, 100, and 150 tokens.
- Read Clone groups, clone lines, and Duplication of the Production Source set; the Test Source set is listed for reference only.
- For each repository and candidate, sample 20 Production Clone groups at random with a fixed seed (33).
- Judge each sampled Clone group as a true positive (code worth extracting into one shared place) or a false positive.
- Measure wall time and peak resident memory with `/usr/bin/time -l` on an Apple M5 with 10 cores and 32 GB of memory.
  Each figure is the best of at least three runs.

That is 300 judged Clone groups for the five candidates, and 180 more for the structural filter below.
One person judged every sample, so the false-positive rates are estimates with wide error bars: 20 samples give about ±20 percentage points.

## Measurements

### Counts in the Production Source set

| Minimum | Private: groups | Private: Duplication | excalidraw: groups | excalidraw: Duplication | nest: groups | nest: Duplication |
| ------- | --------------- | -------------------- | ------------------ | ----------------------- | ------------ | ----------------- |
| 30 | 37,815 | 49.8% | 4,907 | 33.2% | 2,939 | 42.3% |
| 50 | 9,623 | 23.3% | 1,418 | 17.5% | 1,023 | 26.2% |
| 75 | 2,826 | 11.0% | 557 | 10.2% | 396 | 16.4% |
| 100 | 1,057 | 6.5% | 290 | 7.2% | 210 | 11.7% |
| 150 | 331 | 2.9% | 104 | 4.3% | 85 | 7.2% |

Production holds 419,374 source lines in the private workspace, 130,935 in excalidraw, and 74,230 in nest.

Duplication of the Test Source set is higher at every candidate.
At 75 tokens it is 20.7% in the private workspace, 37.0% in excalidraw, and 34.0% in nest.

### Type-1 matching only, for comparison

| Minimum | Private | excalidraw | nest |
| ------- | ------- | ---------- | ---- |
| 50 | 1.6% | 4.3% | 13.9% |
| 75 | 0.9% | 1.8% | 9.1% |
| 100 | 0.8% | 0.9% | 7.0% |

### False-positive rate of sampled Production Clone groups

| Minimum | Private | excalidraw | nest | Mean |
| ------- | ------- | ---------- | ---- | ---- |
| 30 | 95% | 90% | 75% | 87% |
| 50 | 80% | 80% | 65% | 75% |
| 75 | 80% | 70% | 45% | 65% |
| 100 | 65% | 75% | 50% | 63% |
| 150 | 50% | 95% | 40% | 62% |

The rate falls as the minimum grows in logic-heavy code, such as nest.
It does not fall in data-heavy code: excalidraw is at 95% at 150 tokens, because long SVG icon lists and literal tables form the longest Clone groups.

The false positives fall into these kinds, counted over all 300 samples:

- Literal data: object and array tables, environment-variable mappings, chart and column definitions, font range tables, and test fixtures (71).
- Import lists: runs of import declarations that share only their shape (46).
- Type declarations: interfaces, type unions, schema field lists, database column lists, and method overloads (23).
- JSX markup: icon paths and lists of the same component with different props (23).
- Same-shape sequences: delegating methods, route declarations, and getters that differ only in names (21).
- Independent example projects: the nest `sample/` projects, which cannot share code by design (15).
- Fragments too small to extract, such as one query idiom or one argument list (10).
- Generated code: Emscripten bindings committed as source (2).

True positives were mostly repeated logic: the same branch for horizontal and vertical gaps, the same interceptor for two HTTP platforms, the same request helper in two SDK packages, and the same test setup steps in one test suite.

### Estimated true-positive Clone groups

The Clone group count times one minus the false-positive rate estimates how many useful Clone groups a run finds.

| Minimum | Private | excalidraw | nest |
| ------- | ------- | ---------- | ---- |
| 50 | about 1,900 | about 280 | about 360 |
| 75 | about 570 | about 170 | about 220 |
| 100 | about 370 | about 70 | about 105 |

At 75 tokens, a run finds about 1.5 to 2.5 times as many true positives as at 100 tokens, at the same false-positive rate.
At 50 tokens, it finds more true positives, but three in four reported Clone groups are false positives.

### Runtime and peak memory

| Minimum | Private | excalidraw | nest |
| ------- | ------- | ---------- | ---- |
| 30 | 3.5 s, 1.9 GB | 0.46 s, 212 MB | 0.65 s, 339 MB |
| 50 | 1.8 s, 552 MB | 0.24 s, 114 MB | 0.38 s, 130 MB |
| 75 | 0.99 s, 349 MB | 0.23 s, 81 MB | 0.28 s, 107 MB |
| 100 | 0.94 s, 329 MB | 0.20 s, 70 MB | 0.26 s, 97 MB |
| 150 | 0.94 s, 322 MB | 0.18 s, 65 MB | 0.24 s, 82 MB |

These are times of the whole `codeint check`, not of Duplication alone.
Below 75 tokens, memory grows quickly, because many more token windows share a hash and each Clone group keeps all its fragments.

## Why type-2 matching gave about 35% on the private workspace

At 50 tokens, the whole private workspace has 32,771 Clone groups and 33.9% Duplication with type-2 matching, against 8.1% with type-1 matching only.

Normalization makes every identifier one token and every literal another.
So any run of declarations that shares only its shape matches every other such run.
Examples are import lists, object literals of the form "name, colon, value", schema and column declarations, configuration mappings, and JSX attribute lists.
The private workspace is rich in exactly these: schema-first contracts, typed configuration, and metric dashboards built from column tables.

Two facts show that a larger minimum does not fix this:

- These runs are often hundreds of tokens long, so they pass any minimum; at 150 tokens, half of the sampled private Clone groups are still false positives.
- At 50 tokens, 60% of the private Clone groups use fewer than 14 different normalized tokens; at 100 tokens, 36% still do.

## A structural filter fixes most of it

A local experiment tested two filters together:

- Leave import declarations and export-from declarations out of the token stream.
- Drop a Clone group when its fragment uses fewer than 14 different normalized tokens; real logic mixes keywords, operators, and punctuation, and a list of the same shape does not.

The threshold of 14 came from the 300 judged samples: true positives used 19 different normalized tokens at the median, and false positives used 9.

| Minimum | Private: Duplication | Private: false positives | excalidraw: Duplication | excalidraw: false positives | nest: Duplication | nest: false positives |
| ------- | -------------------- | ------------------------ | ----------------------- | --------------------------- | ----------------- | --------------------- |
| 50 | 7.7% | 55% | 7.4% | 30% | 17.3% | 55% |
| 75 | 4.2% | 25% | 4.6% | 25% | 11.8% | 45% |
| 100 | 2.5% | 30% | 3.1% | 35% | 9.0% | 30% |

The false-positive rates come from 20 new samples per cell, judged the same way.
Most remaining nest false positives are the independent example projects; without them, nest is at 21% at 75 tokens.
Over the whole private workspace at 50 tokens, the filter lowers Duplication from 33.9% to 17.1%; most of the rest is in the Test Source set.
The remaining false positives are SVG icons, generated code, fragments that span the end of one declaration and the start of the next, and same-shape test assertions.

The experiment scanned spans naively, so its runtime is not representative.

### The filter as built

Ticket #76 builds the filter in two places:

- The Language plugin gives every token of an import or export declaration the token kind `import-or-export`.
  In TypeScript that is an `import` declaration, an `import x = require()` declaration, an `export * from` declaration, an `export { a } from` declaration, and an `export { a }` declaration, the same set the experiment left out.
  A declaration that declares code, such as `export const`, keeps its usual token kinds.
- Duplication never puts a token of that kind in a fragment.
  The experiment removed these tokens and joined the code around them; the built filter splits the token stream there instead, so no fragment spans an import or export declaration.
- Duplication drops a Clone group whose fragment uses fewer than 14 distinct normalized tokens.
  The count treats all identifiers as one token, all literals as one, and every other token by its text hash.

On the private workspace, the build of Ticket #76 at 75 tokens gives these figures, best of three runs, on the same machine:

| Source set | Clone groups | clone lines | Duplication |
| ---------- | ------------ | ----------- | ----------- |
| Production | 799 | 17,473 | 4.2% |
| Test | 4,074 | 74,226 | 14.9% |

The whole `codeint check --all` takes 0.93 s and 369 MB of peak resident memory.

## Decision

**The default minimum clone size is 75 tokens.**

- At 75 tokens, the false-positive rate is about the same as at 100 and 150 tokens, and it finds about twice as many true positives as at 100 tokens.
- At 50 tokens and below, three in four or more reported Clone groups are false positives, and memory grows by half or more on a large workspace.
- At 150 tokens, data-heavy code still reports mostly false positives, and moderate real clones disappear.
- With the structural filter above, 75 tokens is the best candidate in all three repositories: 25% to 45% false positives.

**A Clone group leaves out import and export declarations and needs at least 14 distinct normalized tokens.**

- Import lists and same-shaped data were the two largest kinds of false positive, and a larger minimum does not remove them.
- The threshold of 14 lies between the median of true positives (19) and of false positives (9) in the judged samples.
- Which declarations count as imports and exports is a rule of the Language plugin, so the core never reads token text to decide it.

## Considered options

- **Keep 50 tokens.**
  Rejected: 75% false positives on average, and 23% Production Duplication on the private workspace, most of it from shape-only matches.
- **100 tokens.**
  Rejected: the same false-positive rate as 75 tokens, with half the true positives.
  PMD CPD has no default for its `--minimum-tokens` option, and its documented examples use 100 (pmd.github.io/pmd/pmd_userdocs_cpd.html, checked 2026-11-21).
- **150 tokens.**
  Rejected: it misses most real clones of 10 to 20 lines, and it still reports mostly false positives in data-heavy code.
- **Type-1 matching only.**
  Rejected for this decision: it hides clones that differ only in names, which are the clones agents produce when they copy and adapt code.
  It is a separate question from the minimum.

## Consequences

- A default of 75 tokens alone lowers private Production Duplication from 23% to 11%, but about two in three reported Clone groups are still false positives.
- The structural filter lowers private Production Duplication from 11% to 4.2%, and the false-positive rate at 75 tokens from about 65% to 25% to 45%.
- The Clone group term in `GLOSSARY.md` names the filter.
- The Plugin interface gains the token kind `import-or-export`, so every Language plugin must mark these tokens.
- Scoping can remove the rest: example projects, committed generated code, and icon modules belong in a Source set or an exclusion, not in the detector.
- No Reference range for Duplication is shown for TypeScript; this record does not set one.
