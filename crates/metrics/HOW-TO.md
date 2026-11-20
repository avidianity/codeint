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
```

- `Erosion::measure` reads only the `CodeModel`; it never knows the source language.
- The cyclomatic complexity of a function is 1 plus its decision points.
- Function mass is the cyclomatic complexity times the square root of the function's source lines.
- A function is eroded when its cyclomatic complexity is greater than 10.
- `ratio` is the eroded Function mass divided by the total Function mass, and 0 when the total is 0.
- `eroded_functions` lists the Findings, the largest Function mass first, then by path and line.

## Report a Metric per Source set

- A Metric measures one `CodeModel` and never reads the Source set of a file itself.
- The report crate splits the Code model with `CodeModel::into_source_sets` and measures every Metric once per Source set.
- To add a Metric to the report, add its section as a field of `SourceSetSection` in `crates/report/src/lib.rs`, built from the `CodeModel` of that Source set.
  The report then shows the Metric for each Source set that holds a file.
