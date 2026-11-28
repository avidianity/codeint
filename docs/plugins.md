# Plugin author guide

This guide explains how to build, hash, declare, and test a Language plugin and a Metric plugin for codeint using the official Rust SDK (`codeint-plugin-sdk`).
[GLOSSARY.md](../GLOSSARY.md) defines every term on this page.
[ADR 0002](./adr/0002-plugins-as-wasm-components.md) records why Plugins are WebAssembly components behind a WIT interface.
[ADR 0027](./adr/0027-plugin-sdk-guest-runtime-unsafe.md) records why `codeint-plugin-sdk` isolates all guest runtime `unsafe` shims in one module so Plugin authors write 100% safe Rust.

## Versioned WIT contract

codeint defines its Plugin contract in `package codeint:plugin@0.4.0`.
Two WIT files hold the contract:

- [wit/language.wit](../wit/language.wit) defines `codeint:plugin/language@0.4.0` and the `language-plugin` world.
- [wit/metric.wit](../wit/metric.wit) defines `codeint:plugin/metric@0.4.0` and the `metric-plugin` world.

The `metric` interface reuses `file-model` and `resolution` from `language`.
A Language plugin exports `language` and lowers source files into the Code model.
A Metric plugin exports `metric` and measures the Code model of one Source set without knowing the source language.

### Zero-capability sandbox

Both worlds import nothing:

```wit
world language-plugin {
    export language;
}

world metric-plugin {
    export metric;
}
```

codeint instantiates every external Plugin with an empty `wasmtime::component::Linker`.
A Plugin receives no filesystem, network, environment variable, random, or clock imports.
Any component that imports a WASI host interface such as `wasi:filesystem/*`, `wasi:sockets/*`, `wasi:clocks/*`, or `wasi:random/*` fails to instantiate with exit code 2.
Any language or toolchain that produces a WebAssembly component with zero host imports works with codeint.

## Choose a Rust build path

`codeint-plugin-sdk` supports two zero-WASI Rust build paths with the exact same Plugin code.

| Build path | Target | Extra CLI tools | Standard library | When to choose |
| ---------- | ------ | --------------- | ---------------- | -------------- |
| Default (`no_std`) | `wasm32-wasip2` | None (`wasm-component-ld` ships with `rustc`) | `core` + `alloc` (`prelude::*`) | Default for new Plugins; one `cargo build` command |
| `std` escape hatch | `wasm32-unknown-unknown` | `wasm-tools` (`component new`) | Full `std` (OS calls stubbed or error) | When you need `std`-only crates such as `regex` |

Both paths emit zero WASI imports and reuse linear memory cleanly under codeint's 64 MiB default per-Plugin cap.

### Path 1: Zero-tool `wasm32-wasip2` (`no_std`, recommended)

Install the Rust toolchain listed in [rust-toolchain.toml](../rust-toolchain.toml) and add the `wasm32-wasip2` target:

```sh
rustup target add wasm32-wasip2
```

`codeint-plugin-sdk` automatically supplies:

- A freeing global allocator (`dlmalloc::GlobalDlmalloc`) so each fresh per-call `Store` instantiated from pre-compiled `InstancePre` stays within codeint's 64 MiB default cap.
- A `#[panic_handler]` that traps cleanly via `wasm32::unreachable`.
- The `cabi_realloc` export and C memory intrinsics (`memcmp`, `memcpy`, `memmove`, `memset`) that `wasm32-wasip2` `no_std` targets omit.
- Bundled `codeint:plugin@0.4.0` WIT bindings behind safe `LanguagePlugin` and `MetricPlugin` traits, plus `export_language_plugin!` and `export_metric_plugin!` macros.

Your Plugin crate needs zero `unsafe` code, zero `wit-bindgen` setup, and only one `#![cfg_attr(..., no_std)]` line at the top of `src/lib.rs`, so it can keep `#![forbid(unsafe_code)]`.
Because that attribute activates only on `wasm32-wasip2`, native `cargo test` runs on your host machine with full `std`.

Build a zero-WASI component with one command:

```sh
cargo build --target wasm32-wasip2 --release
```

Disable the default allocator or panic handler with `default-features = false` if you supply your own (`features = ["default-allocator"]` and `features = ["panic-handler"]` are on by default).

### Path 2: `wasm32-unknown-unknown` with `std` and `wasm-tools`

Choose this path when a parser or helper crate requires Rust's `std` (for example `regex`).
On `wasm32-unknown-unknown`, Rust's `std` imports zero WASI host functions and supplies `dlmalloc`, a panic handler, and memory intrinsics directly.

Install the `wasm32-unknown-unknown` target and `wasm-tools`:

```sh
rustup target add wasm32-unknown-unknown
cargo install wasm-tools
```

Enable the `std` feature on `codeint-plugin-sdk` in `Cargo.toml`:

```toml
[lib]
crate-type = ["cdylib"]

[dependencies]
codeint-plugin-sdk = { path = "path/to/codeint/crates/plugin-sdk", default-features = false, features = ["std"] }
```

Build the core WebAssembly module and wrap it into a zero-import component with `wasm-tools component new`:

```sh
cargo build --target wasm32-unknown-unknown --release
wasm-tools component new \
  target/wasm32-unknown-unknown/release/my_plugin.wasm \
  -o my_plugin.wasm
```

Do not pass `--adapt` to `wasm-tools component new`, because codeint's empty `Linker` rejects WASI adapters.
Pin the SHA-256 digest of the component that `wasm-tools component new` writes, not of the core module in `target/wasm32-unknown-unknown/`, as [Compute the SHA-256 hash](#compute-the-sha-256-hash) shows.
Enabling the SDK `std` feature on `wasm32-wasip2` fails at compile time.
That is the only compile-time stop.
A plugin crate that uses `std` on `wasm32-wasip2` without that feature, for example with `default-features = false` and no `no_std` attribute, still compiles and produces a component with `wasi:*` imports.
codeint rejects any such component when it loads it, with an error such as `component imports instance wasi:io/poll@0.2.12, but a matching implementation was not found in the linker`.
Check a build with `wasm-tools component wit my_plugin.wasm` and expect no `import wasi:` lines.

### Safe Rust and `no_std` tips

- Prefer `BTreeMap` and `BTreeSet` from `codeint_plugin_sdk::prelude::*` over `HashMap` or `HashSet`.
  Crates backed by `getrandom` can pull in `wasi:random` imports, which codeint's empty `Linker` rejects; ordered maps also keep Finding order deterministic.
- `serde_json` works in `no_std` with `default-features = false, features = ["alloc"]`.
- Declare `#![forbid(unsafe_code)]` at your Plugin crate root.
  The `export_language_plugin!` and `export_metric_plugin!` expansions compile under it in both build paths, as both sample Plugins in this repository show.

## Build a Language plugin

[crates/plugin-host/tests/fixtures/sample-plugin/](../crates/plugin-host/tests/fixtures/sample-plugin/src/lib.rs) is a complete Language plugin built on `codeint-plugin-sdk` for a toy `.mini` language.

### Crate setup

Configure the crate as a `cdylib` and depend on `codeint-plugin-sdk`:

```toml
[lib]
crate-type = ["cdylib"]

[dependencies]
codeint-plugin-sdk = { path = "../../../../../crates/plugin-sdk" }

[profile.release]
opt-level = "s"
lto = true
panic = "abort"
```

Implement `LanguagePlugin` and export it with `export_language_plugin!`:

```rust
#![forbid(unsafe_code)]
#![cfg_attr(all(target_arch = "wasm32", target_os = "wasi", target_env = "p2"), no_std)]

use codeint_plugin_sdk::fnv1a_64;
use codeint_plugin_sdk::language::{FileModel, LanguagePlugin, LowerError, SourceSet};
use codeint_plugin_sdk::prelude::*;

struct SampleMiniPlugin;

impl LanguagePlugin for SampleMiniPlugin {
    fn extensions() -> Vec<String> {
        vec!["mini".to_string()]
    }

    fn lower(path: String, text: String) -> Result<FileModel, LowerError> {
        if !path.ends_with(".mini") {
            return Err(LowerError::UnsupportedFile(format!(
                "{path} does not end with .mini"
            )));
        }
        let _hash = fnv1a_64(&path);
        Ok(FileModel {
            source_lines: text.lines().filter(|line| !line.trim().is_empty()).count() as u32,
            ..FileModel::empty(SourceSet::Production)
        })
    }
}

codeint_plugin_sdk::export_language_plugin!(SampleMiniPlugin);
```

### Trait methods and SDK defaults

`LanguagePlugin` maps one-to-one onto all thirteen functions of `codeint:plugin/language@0.4.0`.
Only `extensions` and `lower` are required; the other eleven methods have safe defaults so a minimal Language plugin fits in a few dozen lines.

| Function | SDK default | Responsibility |
| -------- | ----------- | -------------- |
| `extensions() -> Vec<String>` | Required | Return file extensions without a leading dot, such as `["mini"]`. |
| `lower(path, text) -> Result<FileModel, LowerError>` | Required | Lower one file into its `source_set`, `source_lines`, `functions`, `tokens`, `flagged_lines`, `hazard_lines`, `runtime_imports`, and `program_start` mark. Return `DeclarationOnly`, `Unparsable`, `UnsupportedFile`, or `TooLarge` on error. |
| `has_imports() -> bool` | `false` | State whether this language has runtime imports, manifests, and entry points; when `false`, codeint never calls the five import/manifest methods below. |
| `manifest_paths(repository_paths) -> Vec<String>` | `Vec::new()` | Select manifest paths from `repository_paths` that the host reads once on the Plugin's behalf. |
| `workspace_packages(manifests) -> Result<Vec<String>, WorkspaceError>` | `Ok(Vec::new())` | Return relative Workspace package roots from the manifest contents, or an empty list for a single-package repository. Return `WorkspaceError` with the `path` of the one invalid manifest; the host then skips that manifest with a warning and calls again without it. |
| `resolve_imports(repository_paths, manifests, requests, active_profiles) -> Vec<Resolution>` | `Vec::new()` | Return one `Resolved(Vec<String>)` (non-empty paths from `repository_paths`), `External(String)`, `Builtin`, or `Unresolved(String)` entry per request in request order. |
| `declared_dependencies(manifests) -> Vec<DeclaredDep>` | `Vec::new()` | Extract manifest-declared external dependencies (`package_root`, normalized `name`, and `kind` of `Runtime`, `Dev`, or `Optional`). |
| `entry_points(repository_paths, manifests) -> Vec<String>` | `Vec::new()` | Discover standard entry-point source files from `repository_paths` and `manifests`, returning only paths from `repository_paths`. |
| `reference_ranges() -> Vec<ReferenceRange>` | `Vec::new()` | Return published Reference ranges for this language, at most one per Metric, each tagged with its calibrated positive integer `metric_version`. |
| `known_framework_profiles() -> Vec<String>` | `Vec::new()` | Return the stable names of the Framework profiles this Language plugin knows. |
| `resolve_framework_profiles(package_root, files, manifests, selection) -> Result<FrameworkProfileResult, String>` | `Ok(FrameworkProfileResult::empty())` | Resolve active Framework profiles, convention entry points, generated paths, framework Reference ranges, and optional Reference range note for one Workspace package. |
| `has_hazard_rules() -> bool` | `false` | State whether this Language plugin defines Hazard line rules; when `false`, the Hazards Metric is absent for this language rather than `0`. |
| `hazard_rules() -> Vec<String>` | `Vec::new()` | Return the stable names of the Hazard line rules this Language plugin may emit; empty when `has_hazard_rules` returns `false`. |

Follow these invariants in `lower`:

- Return `LowerError::UnsupportedFile` for any path whose extension the Plugin does not claim.
  `codeint plugin test` accepts `UnsupportedFile` or `Unparsable` for an unclaimed extension, and fails any other result: an `Ok`, a `DeclarationOnly`, `TooLarge`, or a hit memory or time limit.
  The Plugin host reports a trap as `Unparsable`, so a trap on an unclaimed extension passes `lower` too.

- Keep `functions` sorted by start `line` (`line >= 1` and `line <= end_line`), with `name` set to `None` for anonymous lambdas or closures.
- Always set `coverage` to `None` on every `Function` that `lower` returns.
  codeint fills in `coverage` from Coverage reports after Language plugins run and before Metrics run.
- Keep `tokens` sorted by 1-based `line`, computing `text_hash` with `codeint_plugin_sdk::fnv1a_64`.
- Produce identical `source_lines`, `functions`, and `hazard_lines` for LF and CRLF line endings.
- Do not count nested functions' decision points, cognitive complexity, or source lines in their enclosing function.
- Report every Hazard line in `hazard_lines` with a 1-based `line` inside the file and a `rule` name listed by `hazard_rules`, and never include source text.
  Leave `hazard_lines` empty when `has_hazard_rules` returns `false`.
- Set `program_start` to `false` unless the source text marks the file as a program start, such as a Go `package main` file.
  codeint counts every marked Production file as an entry point for Orphan files and names the mark in `codeint trace`; marked files in other Source sets stay out of entry points.

### Invalid manifests

Return `WorkspaceError::Manifest` from `workspace_packages` when one manifest is invalid, and set `path` to that manifest's exact path from the `manifests` argument.
The host does not stop on this error.
It removes the named manifest, records one warning with the path and your `message`, and calls `workspace_packages` again with the remaining manifests.
It repeats this for each further invalid manifest, then passes only the remaining manifests to `resolve_imports`, `declared_dependencies`, and `entry_points`.
Files under a skipped manifest count toward the surrounding Workspace package.
Report one invalid manifest per call, because the host removes only the one `path` names.

The error stays fatal, with exit code 2, in three cases: the plugin traps, the `path` names a manifest that the host did not pass, and the error is not a manifest error, such as an invalid Workspace package root.

### Compile the sample Language plugin

Run this command from the repository root to build the sample Language plugin from scratch:

```sh
cargo build \
  --manifest-path crates/plugin-host/tests/fixtures/sample-plugin/Cargo.toml \
  --target wasm32-wasip2 \
  --release \
  --locked
```

Cargo writes the WebAssembly component to `crates/plugin-host/tests/fixtures/sample-plugin/target/wasm32-wasip2/release/sample_mini_plugin.wasm`.
Pass `--target-dir target/wasm-plugins` to place build artifacts under the root `target/` folder instead.

Build the same sample through the `std` path with:

```sh
cargo build \
  --manifest-path crates/plugin-host/tests/fixtures/sample-plugin/Cargo.toml \
  --target wasm32-unknown-unknown \
  --features codeint-plugin-sdk/std \
  --release \
  --locked
wasm-tools component new \
  crates/plugin-host/tests/fixtures/sample-plugin/target/wasm32-unknown-unknown/release/sample_mini_plugin.wasm \
  -o sample_mini_plugin.wasm
```

codeint's test suite builds both samples through both paths and checks that every component imports only `codeint:plugin/*` types and that both paths produce identical results.

## Build a Metric plugin

[crates/plugin-host/tests/fixtures/sample-metric-plugin/](../crates/plugin-host/tests/fixtures/sample-metric-plugin/src/lib.rs) is a complete Metric plugin built on `codeint-plugin-sdk` that measures `long_functions`, the share of functions with more than 10 source lines.

### Crate setup

Use the same `cdylib` and `codeint-plugin-sdk` dependency, implement `MetricPlugin`, and export it with `export_metric_plugin!`:

```rust
#![forbid(unsafe_code)]
#![cfg_attr(all(target_arch = "wasm32", target_os = "wasi", target_env = "p2"), no_std)]

use codeint_plugin_sdk::metric::{CodeModel, Measurement, MetricPlugin};
use codeint_plugin_sdk::prelude::*;

struct SampleLongFunctionsPlugin;

impl MetricPlugin for SampleLongFunctionsPlugin {
    fn key() -> String {
        "long_functions".to_string()
    }

    fn name() -> String {
        "Long functions".to_string()
    }

    fn description() -> String {
        "Share of functions with more than 10 source lines".to_string()
    }

    fn measure(model: CodeModel) -> Measurement {
        let total = model.files.len() as f64;
        Measurement::from_counts(0.0, total, Vec::new())
    }
}

codeint_plugin_sdk::export_metric_plugin!(SampleLongFunctionsPlugin);
```

### Trait methods and host validation

`MetricPlugin` maps one-to-one onto all five functions of `codeint:plugin/metric@0.4.0`.
`version()` defaults to `1`; override it whenever your Metric's calculation rules change.

| Function | SDK default | Responsibility |
| -------- | ----------- | -------------- |
| `key() -> String` | Required | Return the Metric key used in reports, Budgets, and Regression limits, such as `long_functions`. |
| `name() -> String` | Required | Return the display name shown in human-readable reports, such as `Long functions`. |
| `description() -> String` | Required | Return a short description used in SARIF rule metadata. |
| `version() -> u32` | `1` | Return the positive integer version (`>= 1`) of this Metric's measurement semantics. |
| `measure(model) -> Measurement` | Required | Compute `ratio`, `numerator`, `denominator`, and `findings` for one Source set's Code model. |

The Plugin host enforces these rules at load and measurement time:

- `key` must be non-empty, start with a lowercase ASCII letter, and contain only lowercase ASCII letters, digits, `_`, or `-`.
- `key` must not collide with built-in Metric keys (`erosion`, `cognitive_complexity`, `duplication`, `verbosity`, `import_cycles`, `boundaries`, `orphan_files`, `unused_dependencies`, `unlisted_dependencies`, `dev_in_production_dependencies`, `test_gap`, `hazards`), reserved report or configuration keys (`files`, `source_lines`, `packages`, `commit`, `committed_on`, `production`, `test`, `generated`, `vendored`, `dependencies`), or another external Metric plugin's key.
- `version` must be a positive integer (`>= 1`); returning `0` stops the run at load time with exit code 2.
- If a Language plugin returns a Reference range whose `metric_version` does not match the current Metric's version (including `0`), codeint hides that stale Reference range from report comparison and shows a note explaining the version mismatch instead of failing the run.
- `ratio` must be finite and between `0.0` and `1.0`.
- `numerator` and `denominator` must be finite and `>= 0.0`.
- Each Finding must have a non-empty `id`, a 1-based `line` (`>= 1`), and a valid repository-relative `path` using `/`.
- Compute each Finding `id` from file path and content (never from line numbers) using `codeint_plugin_sdk::Fnv1a64` so `codeint diff` tracks moved Findings as unchanged.
- Return the same `findings` for the same Code model on every run: `codeint plugin test` fails a Metric plugin whose `measure` is non-deterministic.
  A Finding `id` that differs between runs of unchanged code breaks the Baseline and `codeint diff`, which match Findings by `id` across runs ([ADR 0030](adr/0030-finding-baseline.md)).
- Order `findings` worst first.

### Coverage facts in the Code model

Each `function` in `file-model` carries `coverage: option<function-coverage>`, where `function-coverage` holds two counts from Coverage reports:

- `listed-lines`: the count of the function's lines listed by a Coverage report.
- `executed-lines`: the count of those listed lines executed by tests.

Rules for `function.coverage`:

- A Language plugin always returns `none`.
- codeint fills in `coverage` after Language plugins run and before any Metric runs.
- `none` means no Coverage report covers the function.
- `some({ listed-lines, executed-lines: 0 })` means a Coverage report covers the function and tests executed 0 of its listed lines.
  A Metric plugin can distinguish absent coverage (`none`) from 0 executed lines (`some` with `executed-lines == 0`).

`code-model` also carries `languages: list<language-facts>`, sorted by `language` name, where each `language-facts` record holds:

- `language`: the Language plugin name, such as `typescript-javascript` or `rust`.
- `has-coverage`: whether at least one Coverage report covers at least one file of that language.
- `has-hazard-rules`: whether that Language plugin defines Hazard line rules.
- `has-imports`: whether that language has runtime imports, manifests, and entry points.

Rules for `code-model.languages` and `has-coverage`:

- codeint attaches one `language-facts` entry per language in the Code model and preserves `languages` on every Source set.
- `has-coverage` is `false` until codeint fills coverage facts from Coverage reports after Language plugins run and before Metrics run.
- A Metric plugin can check `has-coverage` to stay absent for a language that no Coverage report covers, rather than reporting 0.

### Hazard line rules and per-file Hazard lines

A Language plugin exports `has-hazard-rules() -> bool` and `hazard-rules() -> list<string>`, and reports per-file matches in `file-model.hazard-lines: list<hazard-line>`.
Each `hazard-line` record holds two fields and never holds source text:

- `line`: the 1-based line number inside the file.
- `rule`: the stable identifier of the Hazard line rule the line matched, listed in `hazard-rules`.

`code-model.languages` exposes each language's `has-hazard-rules` value to Metric plugins, and each `source-file.model` exposes `hazard-lines`.

Rules for `has-hazard-rules`, `hazard-rules`, `hazard-lines`, and `language-facts.has-hazard-rules`:

- Return `false` from `has-hazard-rules`, an empty list from `hazard-rules`, and an empty `hazard-lines` list on every file when the Language plugin defines no Hazard line rules.
- Return `true` from `has-hazard-rules` and a non-empty list of stable, non-empty rule names from `hazard-rules` when the Language plugin defines Hazard line rules.
- A line that matches two Hazard line rules appears once per rule in `hazard-lines`.
- Every `hazard-line.line` must be between `1` and the line count of the file, and every `hazard-line.rule` must appear in the Language plugin's `hazard-rules` list.
- The built-in `sql` Language plugin returns `true` and lists 32 Hazard line rules; the other six built-in Language plugins return `false`, an empty `hazard-rules` list, and an empty `hazard-lines` list on every file.
- When `has-hazard-rules` is `false`, the Hazards Metric is absent for that language rather than `0`.
- A Metric plugin reads `has-hazard-rules` on each `language-facts` entry in `code-model.languages` and `hazard-lines` on each `source-file.model`.

### Languages without imports

A Language plugin exports `has-imports() -> bool` to state whether its language has runtime imports, manifests, and entry points.
`code-model.languages` exposes each language's `has-imports` value to Metric plugins on `language-facts`.

Rules for `has-imports` and `language-facts.has-imports`:

- Return `true` when the language has runtime imports, manifests, or entry points.
  The six built-in Language plugins with imports (`typescript-javascript`, `rust`, `php`, `dart`, `python`, and `go`) return `true`; the built-in `sql` Language plugin returns `false`.
- Return `false` when the language has no imports, such as SQL or a configuration language.
- When `has-imports` returns `false`, codeint never calls `manifest-paths`, `workspace-packages`, `resolve-imports`, `declared-dependencies`, or `entry-points` on that Language plugin.
- When `has-imports` returns `false`, TOON, JSON, Markdown, and SARIF reports omit Import cycles, Orphan files, Import boundaries, and Dependency hygiene for that language, and TOON, JSON, and Markdown reports include a one-line `import_metrics_note`.
- A Metric plugin reads `has-imports` on each `language-facts` entry in `code-model.languages` to tell whether a language has imports rather than treating empty `runtime-imports` as zero import issues.

### Multi-file resolved imports

A runtime import can resolve to several repository files, such as an import of a whole folder or package directory:

```wit
variant resolution {
    resolved(list<string>),
    external(string),
    builtin,
    unresolved(string),
}
```

Rules for `resolution.resolved`:

- Every entry in the list must be a path from `repository-paths`, relative to the repository root, with forward slashes.
- The list must contain at least one file.
  Returning `resolved([])` stops the run with exit code 2 and an error naming the Plugin.
- Each file in the list becomes one directed import link from the importing file for Import cycles, Orphan files, Import boundaries, `codeint trace`, and Metric plugins.
- The built-in `go` Language plugin returns every non-test `.go` file of the imported Go package; the other five built-in Language plugins with imports return a single-element list per resolved import, and `sql` returns `has-imports` as `false`.

### Program-start mark

Each `file-model` carries `program-start: bool`, which lets a Language plugin mark a file as a program start when only its source text shows it (such as a Go file declaring `package main`).

Rules for `file-model.program-start`:

- `program-start` is `false` unless the Language plugin sets it to `true` in `lower`.
- The built-in `go` Language plugin sets `program-start` to `true` on files declaring `package main`; the other six built-in Language plugins always set `program-start` to `false`.
- A Metric plugin reads `program-start` on each `source-file.model` passed to `measure`.

### Compile the sample Metric plugin

Run this command from the repository root to build the sample Metric plugin from scratch:

```sh
cargo build \
  --manifest-path crates/plugin-host/tests/fixtures/sample-metric-plugin/Cargo.toml \
  --target wasm32-wasip2 \
  --release \
  --locked
```

Cargo writes the WebAssembly component to `crates/plugin-host/tests/fixtures/sample-metric-plugin/target/wasm32-wasip2/release/sample_long_functions_plugin.wasm`.

## Compute the SHA-256 hash

Every external Plugin entry in `codeint.jsonc` or `codeint.json` requires the 64-character hexadecimal SHA-256 digest of the `.wasm` file.
codeint verifies the digest in constant time before compiling or instantiating the component.
A missing digest, malformed digest, or hash mismatch stops the run with exit code 2.

Compute the digest for each built `.wasm` file:

On Linux:

```sh
sha256sum path/to/plugin.wasm
```

On macOS:

```sh
shasum -a 256 path/to/plugin.wasm
```

On Windows PowerShell:

```powershell
Get-FileHash -Algorithm SHA256 path\to\plugin.wasm
```

Copy the 64-character hexadecimal digest into `plugins[].sha256` in `codeint.jsonc` or `codeint.json`.
Recompute the digest whenever you rebuild the `.wasm` file.

## Declare Plugins in `codeint.jsonc` or `codeint.json`

Declare external Language plugins and Metric plugins under the top-level `plugins` array in `codeint.jsonc` or `codeint.json`.
See [docs/reference.md](./reference.md#plugins) for the full configuration schema.

```json
{
  "plugins": [
    {
      "name": "sample-mini",
      "kind": "language",
      "path": "plugins/sample_mini_plugin.wasm",
      "sha256": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
    },
    {
      "name": "long-functions",
      "kind": "metric",
      "path": "plugins/sample_long_functions_plugin.wasm",
      "sha256": "fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210"
    }
  ],
  "languages": [
    { "plugin": "typescript-javascript", "include": ["web"] },
    { "plugin": "sample-mini", "include": ["scripts"] }
  ],
  "budgets": {
    "production": {
      "long_functions": 0.25
    }
  },
  "regression_limits": {
    "long_functions": {
      "ratio_rise": 0.02,
      "new_findings": "numerator_rise"
    }
  }
}
```

Key rules:

- `name` must be unique across Plugins and must not reuse a built-in Plugin name such as `typescript-javascript`.
- `kind` is `"language"` (the default when omitted) or `"metric"`.
- `path` resolves relative to the repository root, or uses an absolute path as given.
- Use `languages` to scope Language plugins when two plugins claim the same file extension in the same repository.
  When two Language plugins claim the same extension in overlapping folders, codeint stops with exit code 2.
- Use an external Metric plugin's `key` (such as `long_functions`) in `budgets` and `regression_limits` the same way you use built-in Metric keys.

## Test Plugins

### Run native unit tests on the host

Because `#![cfg_attr(all(target_arch = "wasm32", target_os = "wasi", target_env = "p2"), no_std)]` disables `std` only on `wasm32-wasip2`, run unit tests for your `LanguagePlugin` or `MetricPlugin` directly on your native machine with full `std`:

```sh
cargo test
```

### Run the Seam 2 conformance suite

Every Language plugin (built-in or external) must pass the shared Seam 2 conformance suite in [crates/plugin-host/tests/conformance.rs](../crates/plugin-host/tests/conformance.rs).
The suite verifies:

- Non-empty file extensions without leading dots.
  An extension that contains `/` or whitespace never matches a repository file name, so `codeint plugin test` fails it.
  Duplicate extensions are allowed; codeint skips an extension that is already listed.
- Non-empty Framework profile names, because an empty name can never be selected in config.
- `UnsupportedFile`, empty file, `DeclarationOnly`, and `Unparsable` handling in `lower`.
- Production and Test Source set classification.
- LF and CRLF equivalence for `source-lines` and `functions`.
- Ordered `functions` and `tokens`, valid 1-based line spans, absent `coverage` (`none`) on every function, and non-empty Flagged line rules.
- When `has-imports` returns `true`:
  - Non-empty `runtime-imports` on the production fixture.
  - `manifest-paths` returning only paths from `repository-paths`.
  - `workspace-packages` parsing valid manifests, never returning an error for no manifests or a valid manifest, and returning `WorkspaceError::Manifest` on invalid manifests.
  - `resolve-imports` returning one `Resolved` (non-empty list of paths from `repository-paths`), `External`, `Builtin`, or `Unresolved` result per request in request order.
  - `declared-dependencies` returning manifest-declared dependencies and `entry-points` returning only paths from `repository-paths`.
- When `has-imports` returns `false`, empty `runtime-imports` and no calls to `manifest-paths`, `workspace-packages`, `resolve-imports`, `declared-dependencies`, or `entry-points`.
- `reference-ranges` returning valid ranges (at most one per Metric, bounds within `0..=1`), each at the current version of its built-in Metric.
  A range with `metric_version` 0 or a version that differs from the current Metric version fails the check, because `codeint check` hides it; the hint names the Metric, the range's version, and the current version.
  Ranges for keys that are not built-in Metrics are not version-checked, because `codeint check` never looks them up.
- `has-hazard-rules`, `hazard-rules`, and `hazard-lines` staying consistent (empty `hazard-rules` and empty `hazard-lines` when `false`, non-empty list of non-empty rule names when `true`, every `hazard-line.line` within `1..=line_count`, and every `hazard-line.rule` listed in `hazard-rules`).
- Safe concurrent `lower` calls across threads.

Run the conformance suite and the host sandbox tests:

```sh
cargo test --locked -p codeint-plugin-host --test conformance
cargo test --locked -p codeint-plugin-host --test load
```

### Check a Plugin with the Conformance runner

`codeint plugin test <WASM>` runs the built-in Conformance runner against a built `.wasm` file.
It needs no repository and no `codeint.json`, and it exits with code 0 when every check passes, code 1 when a check fails, and code 2 when the file cannot be read.
Pass `--kind metric` for a Metric plugin; the default kind is `language`.

```sh
codeint plugin test target/wasm32-wasip2/release/my_plugin.wasm
codeint plugin test target/wasm32-wasip2/release/my_metric.wasm --kind metric --format json
```

Each failed check prints its rule and a fix hint.
The runner reports these rules:

- Language plugin: a valid WebAssembly component with zero WASI imports, an exact `codeint:plugin@0.4.0` match, valid extensions, Hazard rules and Framework profiles that are consistent and resolve under every selection (disabled, auto, exclude, and only, each with every known profile, with each profile alone, and, for three or more profiles, with the first two) for the root package and a member package, sent the real package files and only the manifests `manifest-paths` chose, each with the content of the probe file it names, for the packages of the workspace that `workspace-packages` reports and for a fixed member package, as `codeint check` sends them, with entry points and generated paths that are repository files (skipped when the plugin lists no Framework profiles), a deterministic and stateless `lower` with 1-based line bounds, sorted functions and tokens, and listed Hazard rules, valid, current-version answers from `reference-ranges`, and, when `has-imports` is true, valid answers from `manifest-paths`, `workspace-packages`, `resolve-imports`, `declared-dependencies`, and `entry-points` (skipped when `has-imports` is false), and an `end-to-end` run of the real `codeint check` measurement pipeline on a probe repository that holds the probe files and the manifests `manifest-paths` chose, with no Framework profile configured and once with `framework_profiles` set to each known profile.
- Metric plugin: a valid WebAssembly component with zero WASI imports, an exact `codeint:plugin@0.4.0` match, a valid Metric key that collides with no built-in Metric or reserved report key, a Metric version of 1 or higher, a non-empty name and description, a deterministic and stateless `measure` that returns a ratio from 0 to 1, finite non-negative numbers, and Findings with non-empty ids, 1-based lines, and repository-relative paths, and an `end-to-end` run of the real `codeint check` pipeline with the Metric plugin configured on a small TypeScript probe repository.
- Both kinds: peak memory and call time within `--plugin-memory-mb` and `--plugin-timeout-ms`.

The `end-to-end` check writes the probe repository to a temporary folder, runs `git init` there through the Git library, writes a `codeint.json` that declares only your Plugin, and calls `codeint check` in the same process.
Any error that makes `codeint check` exit with code 2 fails the check with the error text.
The folder is removed on every path, including a failure.
The built-in Language plugins stay loaded, as in `codeint check`; a Language plugin is the only one that measures the probe files, because the `codeint.json` scopes `languages` to it, so a claimed extension that a built-in plugin also claims never raises an extension conflict.
The probe repository leaves out manifests that `manifest-paths` did not choose, because a built-in Language plugin reads its own manifests and could report a member package that hides an empty `workspace-packages` answer of your Plugin.

The built-in probe sources cannot trigger rules that fire only on specific text.
Pass `--source <FILE>` (repeatable) to lower your own sample files too, such as a file that holds the text of each Hazard rule.
See [docs/reference.md](./reference.md#codeint-plugin) for every option and output format.

### Test end to end with the CLI

Copy your built `.wasm` file into a Git repository, compute its SHA-256 digest, declare it in `codeint.jsonc` or `codeint.json`, and run `codeint check` and `codeint diff`:

```sh
codeint check --format json
codeint check --format markdown
codeint check --format sarif
codeint diff main --format json
```

Verify that:

- External Language plugins measure files matching their extensions and report `skipped` and `declaration_only` files.
- External Metric plugins appear alongside built-in Metrics in TOON, JSON, Markdown, and SARIF reports.
- `budgets` fail with exit code 1 when an external Metric exceeds its Budget.
- `codeint diff` classifies new, resolved, and unchanged Findings using your Finding `id` values and enforces `regression_limits`.

Run the CLI plugin integration tests in this repository to see end-to-end examples:

```sh
cargo test --locked -p codeint-cli --test plugins
```

## Interface versioning

The WIT package declaration carries a Semantic Versioning number:

```wit
package codeint:plugin@0.4.0;
```

This version identifies the exported interfaces `codeint:plugin/language@0.4.0` and `codeint:plugin/metric@0.4.0` in the compiled WebAssembly component.

### Compatibility rules

- **Pre-1.0 (`0.x.y`)**: While the major version is `0`, a minor version bump (`0.3.0` to `0.4.0`) may introduce breaking changes.
  A patch version bump (`0.4.0` to `0.4.1`) contains only non-breaking fixes.
- **Stable (`1.x.y` and later)**:
  - Additive, backward-compatible changes (such as adding a new optional interface or new world) bump the minor version (`1.0.0` to `1.1.0`).
  - Breaking changes (removing or renaming a function, changing record fields, or changing variant or enum cases) bump the major version (`1.0.0` to `2.0.0`).

### Host compatibility

`wasmtime::component::bindgen!` binds the codeint host to the exact WIT interface version compiled into the binary.
At load time, wasmtime checks that the component's exported interface name and version match what the host expects.

- A Plugin built against an incompatible WIT version (such as `codeint:plugin@0.3.0` or `codeint:plugin@0.2.0`) fails at load time with exit code 2, naming the plugin, its found interface version, the required `0.4.0` interface version, and how to rebuild it.
- Rebuild the Plugin against the updated `codeint-plugin-sdk` or `wit/` files and update its `sha256` in `codeint.jsonc` or `codeint.json` when upgrading to a breaking WIT version.
- Never silently mutate an existing released WIT version in place; every breaking change gets a new version number.

### Moving a Plugin from 0.3.0 to 0.4.0

codeint loads only `codeint:plugin@0.4.0`.
A Plugin built against `codeint:plugin@0.3.0` stops the run with exit code 2.

Move an existing external Plugin to `0.4.0`:

1. Depend on `codeint-plugin-sdk` (or update your copy of [wit/language.wit](../wit/language.wit) and [wit/metric.wit](../wit/metric.wit) to `package codeint:plugin@0.4.0;`).
2. For Language plugins: add `metric_version` to every `ReferenceRange`, accept `active_profiles: Vec<String>` in `resolve_imports`, and implement `known_framework_profiles` and `resolve_framework_profiles` if your Plugin defines Framework profiles (`codeint-plugin-sdk` defaults both to empty).
3. For Metric plugins: ensure `version() -> u32` returns a positive integer (`>= 1`; `codeint-plugin-sdk` defaults to `1`).
4. Rebuild the WebAssembly component for `wasm32-wasip2` (or `wasm32-unknown-unknown` plus `wasm-tools component new`) so it exports `codeint:plugin/language@0.4.0` or `codeint:plugin/metric@0.4.0` with zero WASI imports.
5. Compute the SHA-256 digest of the rebuilt `.wasm` file and update `plugins[].sha256` in `codeint.jsonc` or `codeint.json`.
6. Run `codeint plugin test` and `codeint check` (and the Seam 2 conformance suite for Language plugins) to confirm the rebuilt Plugin loads.
