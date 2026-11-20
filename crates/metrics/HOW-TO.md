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
for (package, masses) in erosion.by_package(&model.workspace) {
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
