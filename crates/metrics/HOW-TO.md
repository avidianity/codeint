# How to compute the Metrics

## Measure Erosion

```rust
use codeint_metrics::Erosion;

let erosion = Erosion::measure(&model);
println!("{} of {}", erosion.eroded_function_mass(), erosion.total_function_mass());
println!("ratio {}", erosion.ratio());
for function in erosion.eroded_functions() {
    println!("{}:{} has Function mass {}", function.path, function.line, function.function_mass);
}
for (package, masses) in erosion.by_package(&model) {
    println!("{package}: {} of {}, ratio {}", masses.eroded, masses.total, masses.ratio());
}
```

- `Erosion::measure` reads only the `CodeModel`; it never knows the source language.
- The cyclomatic complexity of a function is 1 plus its decision points.
- Function mass is the cyclomatic complexity times the square root of the function's source lines.
- A function is eroded when its cyclomatic complexity is greater than 10.
- `ratio` is the eroded Function mass divided by the total Function mass, and 0 when the total is 0.
- `eroded_functions` lists the Findings, the largest Function mass first, then by path and line.
- `by_package` gives the `FunctionMasses` of every package of the workspace, the root package included, even a package with no functions.
  Each package's masses are sums over its files, so the package masses add up to the totals.
  Compute a total from summed masses, never from an average of package ratios.

## Identify a Finding

```rust
use codeint_metrics::Erosion;

let erosion = Erosion::measure(&model);
for function in erosion.eroded_functions() {
    println!("{} is {}:{}", function.id, function.path, function.line);
}
```

- Every Finding has a `FindingId`: `ErodedFunction::id`, `CloneGroup::id`, `FlaggedLineFinding::id`, and `ImportCycle::id()`.
  Its `Display` form is 16 lowercase hexadecimal digits, and the report shows it as `id`.
- The id is a hash of the Metric key, the paths of the Finding, and its content, never a line number:
  the tokens from the first to the last line of an eroded function; the normalized token sequence and the fragment paths of a Clone group; the rule and the tokens of a Flagged line; the module paths of an Import cycle.
  Token content is the `text_hash` of each token, so whitespace, comments, and line endings never change an id.
- Findings with the same Metric, paths, and content get an occurrence number in source order, so equal Findings in one file stay distinct, and a comparison of two runs counts them right.
- A Metric computes ids with the crate's `FindingIds`, created once per measurement with its `Metric`, and calls `next_in_source_order` for its Findings in path and line order.
- The hash is `codeint_model::StableHasher`, which writes every number as little-endian bytes, so ids are the same on every platform.

## Report a Metric per Source set

- A Metric measures one `CodeModel` and never reads the Source set of a file itself.
- The report crate splits the Code model with `CodeModel::into_source_sets` and measures every Metric once per Source set.
- To add a Metric to the report, add its section as a field of `SourceSetSection` in `crates/report/src/lib.rs`, built from the `CodeModel` of that Source set.
  The report then shows the Metric for each Source set that holds a file.
- To show the Metric per workspace package too, give it a `by_package(&model)` method and add its numerator to `PackageSection`.
  Every Source set's `CodeModel` keeps the whole workspace, so each Source set lists every package.

## Find Clone groups

```rust
use codeint_metrics::{Duplication, MinimumCloneTokens, MinimumDistinctTokens};

let duplication = Duplication::measure(
    &model,
    MinimumCloneTokens::DEFAULT,
    MinimumDistinctTokens::DEFAULT,
);
println!("{} of {} source lines are clone lines", duplication.clone_lines.count(), duplication.source_lines);
let ratio = duplication.ratio();
```

- `Duplication::measure` reads only the token stream of each file in the `CodeModel`.
- A Clone group holds two or more fragments with the same normalized token sequence, at least the minimum number of tokens long.
  Normalizing treats every identifier as one token and every literal as another, so copies that differ only in names and literal values form one Clone group; other tokens must have the same text hash.
  This finds type-1 and type-2 clones in one pass.
  Each fragment is as long as the tokens that all copies share, and copies of one fragment never overlap in one file.
- A fragment never holds a token of kind `ImportOrExport`.
  Such a token splits the token stream, so no fragment spans an import or export declaration.
  The Language plugin decides which tokens have that kind; Duplication never reads token text to decide it.
- A Clone group needs at least the minimum number of distinct normalized tokens in its fragment.
  Count them after normalizing: all identifiers count as one token, all literals as one, and every other token by its text hash.
  So `{ a: 'x', b: 'y' }` uses six distinct normalized tokens: `{`, identifier, `:`, literal, `,`, and `}`.
  All fragments of a Clone group have the same normalized sequence, so the count is the same for each.
- A clone line is a line on which a token of a fragment starts; `clone_lines` holds each such line once, by path and line.
- `clone_lines.count()` is the number of clone lines, and `clone_lines.contains(path, line)` tells whether a line is one.
- `ratio` is `clone_lines.count() / source_lines`, and 0 when the model has no source lines.
- Clone groups come largest first, then by their first fragment.
- `clone_families` clusters overlapping or transitively related exact Clone groups and near-clone groups whose merged canonical fragments span three or more locations into one `CloneFamily` per connected component.
  Each `CloneFamily` holds its stable `FindingId` (hashed from the `clone_family` key, canonical fragment paths, and sorted normalized group digests), unique `duplicated_lines` without double-counting overlaps, constituent `clone_group_ids`, and canonical `fragments` ordered by path then start line.
  Clone families are ordered worst-first by `duplicated_lines`, then by location count, then by their canonical fragments.
- `Duplication::measure_with_near_clones` adds opt-in shingle/Jaccard near-clone detection using MinHash/LSH candidate pruning when passed `Some(NearCloneConfig)`.
  Near-clone `CloneGroup`s set `similarity: Some(ratio)` and contribute to `clone_lines` (and `ratio()`), while `exact_clone_lines` holds only exact Clone group lines so `Verbosity` stays coupled only to exact Clone groups.
- `MinimumCloneTokens::DEFAULT` is 75 tokens, and `MinimumDistinctTokens::DEFAULT` is 14 distinct normalized tokens, as [ADR 0007](../../docs/adr/0007-default-minimum-clone-size.md) decides.
- `by_package(&model)` gives a `LineShare` for every package of `model.workspace`: its clone lines and its source lines.
  A Clone group that spans two packages adds each copy's lines to the package that holds it.

## Measure Verbosity

```rust
use codeint_metrics::{Duplication, MinimumCloneTokens, MinimumDistinctTokens, Verbosity};

let duplication = Duplication::measure(
    &model,
    MinimumCloneTokens::DEFAULT,
    MinimumDistinctTokens::DEFAULT,
);
let verbosity = Verbosity::measure(&model, &duplication.clone_lines);
println!("{} of {}", verbosity.verbose_lines(), verbosity.source_lines());
println!("{} flagged, {} clone lines", verbosity.flagged_line_count(), verbosity.clone_line_count());
println!("ratio {}", verbosity.ratio());
for finding in verbosity.flagged_lines() {
    println!("{}:{} matches {}", finding.path, finding.line, finding.rule);
}
```

- `Verbosity::measure` reads the Flagged lines of the `CodeModel` and the clone lines of the same `CodeModel`; the Flagged-line rules live in the Language plugin, and the Metric only counts lines.
- Pass the clone lines of the `Duplication` already measured on the same `CodeModel`, so that clone detection runs once per Source set.
- `verbose_lines` counts each line that is a Flagged line, a clone line, or both once, even when two rules match it.
- `flagged_line_count` counts the distinct Flagged lines, and `clone_line_count` the clone lines; a line that is both adds to each.
- `ratio` is the verbose lines divided by the source lines, and 0 when there are no source lines.
- `flagged_lines` lists the Findings, one per line and rule, by path, line, and rule.
- `by_package(&model)` gives a `LineShare` for every package of `model.workspace`: its verbose lines and its source lines.

## Split a Metric by workspace package

```rust
let erosion_by_package = erosion.by_package(&model);
for (package, share) in duplication.by_package(&model) {
    println!("{package}: {} of {} lines, ratio {}", share.lines, share.source_lines, share.ratio());
}
```

- Each Metric measures the whole model once, then attributes each file's numerator and denominator to the package that holds the file.
  Measuring each package on its own would lose a Clone group or an Import cycle that spans two packages.
- Every map holds every package, the root package included, so the package numerators and denominators add up to the totals.
- Compute a total from summed numerators and denominators, never from an average of package ratios.

## Measure Import cycles

```rust
use codeint_metrics::ImportCycles;

let import_cycles = ImportCycles::measure(&model);
println!("{} of {}", import_cycles.modules_in_import_cycles(), import_cycles.modules());
println!("ratio {}", import_cycles.ratio());
for cycle in import_cycles.cycles() {
    println!("{:?}", cycle.modules());
}
```

- `ImportCycles::measure` reads only the `CodeModel`; it never knows the source language.
- Every file of `CodeModel::files` is a module.
- An edge is a `RuntimeImport` whose resolution is `Resolved` to another module; external, unresolved, and non-module targets add no edge.
- An Import cycle is a strongly connected component of more than one module, or one module that imports itself.
- `ratio` is the modules in at least one Import cycle divided by all modules, and 0 when there are no modules.
- `cycles` lists the Findings, largest cycle first, then by their sorted module lists, each with its modules sorted by path.
- `by_package(&model)` gives a `ModuleShare` for every package of `model.workspace`: its modules in an Import cycle and all its modules.
  An Import cycle that spans two packages counts each module in the package that holds it.

## List unresolved imports

```rust
use codeint_metrics::ImportCycles;

let import_cycles = ImportCycles::measure(&model);
for import in import_cycles.unresolved_imports() {
    println!("{} imports {}: {}", import.importer, import.specifier, import.reason);
}
```

- `unresolved_imports` lists each `RuntimeImport` whose resolution is `Unresolved`, with the reason the Language plugin gave.
  External imports, such as bare imports of packages and built-in modules, are never unresolved.
- The list is sorted by importer path, and keeps source order within one importer.
- An unresolved import adds no edge and never changes the ratio; it only shows how complete the import graph is.
- The report shows the count and the first unresolved imports under `import_cycles.unresolved_imports` of each Source set, with the same limit and `--all` option as Findings.

## Measure Dependency hygiene

```rust
use std::collections::BTreeSet;
use codeint_metrics::DependencyHygiene;

let hygiene = DependencyHygiene::measure(&model, &BTreeSet::new());
println!("unused ratio {}", hygiene.unused_ratio());
println!("unlisted ratio {}", hygiene.unlisted_ratio());
println!("dev-in-production ratio {}", hygiene.dev_in_production_ratio());
```

- `DependencyHygiene::measure` reads one language's `CodeModel` across all Source sets before splitting by Source set, and never knows the source language.
- It matches each file to the deepest package root in `model.dependency_manifests` (falling back to the root package `""` when present).
- When a model has no dependency manifests, `measure` returns an empty result with no Findings.
- `unused` lists declared `Runtime` and `Dev` dependencies not imported by any Production, Test, or Generated file in their package; `Optional` declarations satisfy imports without being flagged as unused.
- `unlisted` lists external packages (`ImportResolution::External`) imported by Production or Test files in a package that has a manifest but does not declare them; built-ins, resolved internal imports, and unresolved imports are never flagged.
- `dev_in_production` lists dependencies declared only as `Dev` that are imported by Production files; `Dev` dependencies imported by Test files are counted as used and not flagged.
- Vendored files are excluded, and canonical package names in `ignored` are excluded from all three Metrics.
- Use `DependencyHygiene::merge` to combine independent per-language measurements into a repository total.
