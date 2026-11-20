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

## Find Clone groups

```rust
use codeint_metrics::{Duplication, MinimumCloneTokens};

let duplication = Duplication::measure(&model, MinimumCloneTokens::PROVISIONAL_DEFAULT);
println!("{} of {} source lines are clone lines", duplication.clone_lines, duplication.source_lines);
let ratio = duplication.ratio();
```

- `Duplication::measure` reads only the token stream of each file in the `CodeModel`.
- A Clone group holds two or more fragments with the same token sequence, at least the minimum number of tokens long.
  Each fragment is as long as the tokens that all copies share, and copies of one fragment never overlap in one file.
- A clone line is a line on which a token of a fragment starts; `clone_lines` counts each such line once.
- `ratio` is `clone_lines / source_lines`, and 0 when the model has no source lines.
- Clone groups come largest first, then by their first fragment.
- `MinimumCloneTokens::PROVISIONAL_DEFAULT` is 50 tokens until the calibration Ticket #33 fixes the default.
- `by_package(&model)` gives a `LineShare` for every package of `model.workspace`: its clone lines and its source lines.
  A Clone group that spans two packages adds each copy's lines to the package that holds it.

## Measure Verbosity

```rust
use codeint_metrics::Verbosity;

let verbosity = Verbosity::measure(&model);
println!("{} of {}", verbosity.verbose_lines(), verbosity.source_lines());
println!("ratio {}", verbosity.ratio());
for finding in verbosity.flagged_lines() {
    println!("{}:{} matches {}", finding.path, finding.line, finding.rule);
}
```

- `Verbosity::measure` reads only the `CodeModel`; the Flagged-line rules live in the Language plugin, and the Metric only counts lines.
- `verbose_lines` counts each flagged source line once, even when two rules match it.
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
