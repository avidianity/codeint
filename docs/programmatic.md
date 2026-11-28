# Programmatic API

codeint can be called directly from JavaScript, TypeScript, and Rust without parsing terminal text.
Both the JavaScript/TypeScript package (`codeint`) and the Rust library (`codeint-cli`) expose all 18 programmatic operations: `check`, `diff`, `trend`, `impact`, `trace`, `init`, `guard`, `schema`, `config` (`resolved_config` in Rust), `report` (`rerender_report` in Rust), `doctor`, `agentStatus` (`agent_status`), `agentInstall` (`agent_install`), `churn`, `coupling`, `ratchetUpdate` (`ratchet_update`), `baselineUpdate` (`baseline_update`), and `pluginTest` (`plugin_test`).
`codeint update` remains CLI-only.

## Failure semantics

- `check`, `diff`, `doctor`, and `pluginTest` (`plugin_test` in Rust) return a structured gated outcome on both exit `0` (passed) and exit `1` (Budget, Ratchet, Regression, `doctor` blocking error, or `plugin test` failure).
- Operational errors (exit `2`, such as an invalid configuration file, a missing folder, or a directory outside Git) throw `CodeintError` in JavaScript/TypeScript or return `Err(CodeintError)` in Rust.

## JavaScript and TypeScript (`codeint`)

Install the `codeint` npm package, which ships dual ESM and CommonJS modules, TypeScript declarations, and the native platform binary with zero runtime dependencies:

```sh
npm install --save-dev codeint
```

The package exports all 18 operations (`check`, `diff`, `trend`, `impact`, `trace`, `init`, `guard`, `schema`, `config`, `report`, `doctor`, `agentStatus`, `agentInstall`, `churn`, `coupling`, `ratchetUpdate`, `baselineUpdate`, and `pluginTest`), plus `CodeintError` and `resolveBinary`.
Gated operations (`check`, `diff`, `doctor`, and `pluginTest`) resolve to `{ passed, exitCode, failures, report }` on both exit `0` and exit `1`.

```ts
import {
  CodeintError,
  check,
  diff,
  doctor,
  guard,
  init,
  pluginTest,
  resolveBinary,
  trend,
} from "codeint";

const checked = await check({ cwd: process.cwd(), all: false });
if (!checked.passed) {
  console.error("Budget failures:", checked.failures);
  console.log("Production erosion:", checked.report.source_sets.production?.erosion.ratio);
}

const diffed = await diff("origin/main", { cwd: process.cwd() });
if (!diffed.passed) {
  console.error("Regressions:", diffed.failures);
}

const diagnosed = await doctor({ cwd: process.cwd() });
if (!diagnosed.passed) {
  console.error("Doctor failures:", diagnosed.failures);
}

const tested = await pluginTest("dist/plugin.wasm", { cwd: process.cwd(), kind: "language" });
if (!tested.passed) {
  console.error("Plugin failures:", tested.failures);
}

const history = await trend({ cwd: process.cwd(), since: "2026-01-01" });
console.log("Commits measured:", history.commits);

const boundary = await guard("src/domain/order.ts", { cwd: process.cwd() });
console.log("Boundary exists:", boundary.exists);

const { configPath } = await init({ cwd: process.cwd() });
console.log("Starter config:", configPath);

console.log("Bundled native binary:", resolveBinary());
```

CommonJS callers can import the same functions with `require("codeint")`.
Pass `binaryPath` in any options object to override the native executable path.
Catch `CodeintError` to inspect `error.exitCode`, `error.stderr`, and `error.cause` on operational errors.

## Rust (`codeint-cli`)

Within the workspace or from a path dependency on `crates/cli`, import the typed library functions directly:

```rust
use std::path::Path;
use codeint_cli::{
    CheckOptions, DiffOptions, InitOptions, ReportFormat, TrendOptions, check, diff, guard, init,
    trend,
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
let created = init(folder, &InitOptions::default())?;
```

Operational failures return `Err(CodeintError)`, which exposes `message()` and `remedy()`.
