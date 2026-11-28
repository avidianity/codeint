# Programmatic API

codeint can be called directly from JavaScript, TypeScript, and Rust without parsing terminal text.
The JavaScript/TypeScript package (`codeint`) exposes `check`, `diff`, `trend`, and `init`, while the Rust library (`codeint-cli`) also exposes `impact`, `trace`, `guard`, `schema`, `resolved_config`, `doctor`, `agent_status`, `agent_install`, `churn`, and `coupling`.
`codeint update` remains CLI-only.

## Failure semantics

- `check` and `diff` return a structured result on both exit `0` (passed) and exit `1` (Budget or Regression failed).
- Operational errors (exit `2`, such as an invalid configuration file, a missing folder, or a directory outside Git) throw `CodeintError` in JavaScript/TypeScript or return `Err(CodeintError)` in Rust.

## JavaScript and TypeScript (`codeint`)

Install the `codeint` npm package, which ships dual ESM and CommonJS modules, TypeScript declarations, and the native platform binary with zero runtime dependencies:

```sh
npm install --save-dev codeint
```

```ts
import { CodeintError, check, diff, init, resolveBinary, trend } from "codeint";

const checked = await check({ cwd: process.cwd(), all: false });
if (!checked.passed) {
  console.error("Budget failures:", checked.failures);
  console.log("Production erosion:", checked.report.source_sets.production?.erosion.ratio);
}

const diffed = await diff("origin/main", { cwd: process.cwd() });
if (!diffed.passed) {
  console.error("Regressions:", diffed.failures);
}

const history = await trend({ cwd: process.cwd(), since: "2026-01-01" });
console.log("Commits measured:", history.commits);

const { configPath } = await init({ cwd: process.cwd() });
console.log("Starter config:", configPath);

console.log("Bundled native binary:", resolveBinary());
```

CommonJS callers can import the same functions with `require("codeint")`.
Pass `binaryPath` in any options object to override the native executable path.
Catch `CodeintError` to inspect `error.exitCode` and `error.stderr` on operational errors.

## Rust (`codeint-cli`)

Within the workspace or from a path dependency on `crates/cli`, import the typed library functions directly:

```rust
use std::path::Path;
use codeint_cli::{
    CheckOptions, DiffOptions, ReportFormat, TrendOptions, check, diff, guard, init, trend,
};

let folder = Path::new(".");
let checked = check(folder, &CheckOptions::default())?;
if !checked.passed {
    for failure in &checked.failures {
        eprintln!("{failure}");
    }
}
let json = checked.report.render(ReportFormat::Json)?;

let diffed = diff(folder, "origin/main", &DiffOptions::default())?;
let history = trend(folder, &TrendOptions::default())?;
let boundary = guard(folder, Path::new("src/domain/order.ts"))?;
let created = init(folder)?;
```

Operational failures return `Err(CodeintError)`, which exposes `message()` and `remedy()`.
