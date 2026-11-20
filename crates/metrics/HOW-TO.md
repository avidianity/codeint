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
- `cycles` lists the Findings, each with its modules sorted by path, ordered by their sorted module lists.
