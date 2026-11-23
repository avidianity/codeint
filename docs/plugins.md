# Plugin author guide

This guide explains how to build, hash, declare, and test a Language plugin and a Metric plugin for codeint.
[GLOSSARY.md](../GLOSSARY.md) defines every term on this page.
[ADR 0002](./adr/0002-plugins-as-wasm-components.md) records why Plugins are WebAssembly components behind a WIT interface.

## Versioned WIT contract

codeint defines its Plugin contract in `package codeint:plugin@0.2.0`.
Two WIT files hold the contract:

- [wit/language.wit](../wit/language.wit) defines `codeint:plugin/language@0.2.0` and the `language-plugin` world.
- [wit/metric.wit](../wit/metric.wit) defines `codeint:plugin/metric@0.2.0` and the `metric-plugin` world.

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
A Plugin receives no filesystem, network, environment variable, or clock imports.
Any component that imports a WASI host interface such as `wasi:filesystem/*`, `wasi:sockets/*`, or `wasi:clocks/*` fails to instantiate with exit code 2.

Because standard Rust `wasm32-wasip2` builds pull in WASI imports, the sample plugins in this repository use `#![no_std]` with `extern crate alloc`, a small bump allocator, and an exported `cabi_realloc`.
Any language or toolchain that produces a WebAssembly component with zero host imports works with codeint.

## Prerequisites

Install the Rust toolchain listed in [rust-toolchain.toml](../rust-toolchain.toml) and add the `wasm32-wasip2` target:

```sh
rustup target add wasm32-wasip2
```

## Build a Language plugin

[crates/plugin-host/tests/fixtures/sample-plugin/](../crates/plugin-host/tests/fixtures/sample-plugin/src/lib.rs) is a complete Language plugin for a toy `.mini` language.

### Crate setup

Configure the crate as a `cdylib` and depend on `wit-bindgen` without default features:

```toml
[lib]
crate-type = ["cdylib"]

[dependencies]
wit-bindgen = { version = "=0.63.0", default-features = false, features = ["macros", "realloc"] }
```

Generate bindings for the `language-plugin` world and export your implementation:

```rust
wit_bindgen::generate!({
    world: "language-plugin",
    path: "../../../../../wit",
});

use exports::codeint::plugin::language::{
    DeclaredDep, DepKind, FileModel, FlaggedLine, Function, Guest, ImportRequest, LowerError,
    Manifest, ReferenceRange, Resolution, SourceSet, Token, TokenKind, WorkspaceError,
};

struct SampleMiniPlugin;

export!(SampleMiniPlugin);
```

### Required functions

Implement all eight functions on `Guest`:

| Function | Responsibility |
| -------- | -------------- |
| `extensions() -> list<string>` | Return file extensions without a leading dot, such as `["mini"]`. |
| `lower(path, text) -> result<file-model, lower-error>` | Lower one file into its `source-set`, `source-lines`, `functions` (each with optional `name` and nesting-weighted `cognitive-complexity`), `tokens`, `flagged-lines`, and `runtime-imports`. Return `declaration-only`, `unparsable`, `unsupported-file`, or `too-large` on error. |
| `manifest-paths(repository-paths) -> list<string>` | Select manifest paths from `repository-paths` that the host reads once on the Plugin's behalf. |
| `workspace-packages(manifests) -> result<list<string>, workspace-error>` | Return relative Workspace package roots from the manifest contents, or an empty list for a single-package repository. |
| `resolve-imports(repository-paths, manifests, requests) -> list<resolution>` | Return one `resolved(string)`, `external(string)` (carrying the canonical package name), `builtin` (for standard-library or runtime built-ins), or `unresolved(string)` entry per request, in request order. |
| `declared-dependencies(manifests) -> list<declared-dep>` | Extract manifest-declared external dependencies (`package-root`, normalized `name`, and `kind` of `runtime`, `dev`, or `optional`). |
| `entry-points(repository-paths, manifests) -> list<string>` | Discover standard entry-point source files from `repository-paths` and `manifests`, returning only paths from `repository-paths`. |
| `reference-ranges() -> list<reference-range>` | Return published Reference ranges for this language, at most one per Metric. |

Follow these invariants in `lower`:

- Keep `functions` sorted by start `line` (`line >= 1` and `line <= end-line`), with `name` set to `none` for anonymous lambdas or closures.
- Keep `tokens` sorted by 1-based `line`, with 64-bit FNV-1a `text-hash` values of each token's UTF-8 text.
- Produce identical `source-lines` and `functions` for LF and CRLF line endings.
- Do not count nested functions' decision points, cognitive complexity, or source lines in their enclosing function.

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

## Build a Metric plugin

[crates/plugin-host/tests/fixtures/sample-metric-plugin/](../crates/plugin-host/tests/fixtures/sample-metric-plugin/src/lib.rs) is a complete Metric plugin that measures `long_functions`, the share of functions with more than 10 source lines.

### Crate setup

Use the same `cdylib` and `wit-bindgen` configuration, and generate bindings for the `metric-plugin` world:

```rust
wit_bindgen::generate!({
    world: "metric-plugin",
    path: "../../../../../wit",
});

use exports::codeint::plugin::metric::{CodeModel, Finding, Guest, Measurement};

struct SampleLongFunctionsPlugin;

export!(SampleLongFunctionsPlugin);
```

### Required functions and host validation

Implement all four functions on `Guest`:

| Function | Responsibility |
| -------- | -------------- |
| `key() -> string` | Return the Metric key used in reports, Budgets, and Regression limits, such as `long_functions`. |
| `name() -> string` | Return the display name shown in human-readable reports, such as `Long functions`. |
| `description() -> string` | Return a short description used in SARIF rule metadata. |
| `measure(model) -> measurement` | Compute `ratio`, `numerator`, `denominator`, and `findings` for one Source set's Code model. |

The Plugin host enforces these rules at load and measurement time:

- `key` must be non-empty, start with a lowercase ASCII letter, and contain only lowercase ASCII letters, digits, `_`, or `-`.
- `key` must not collide with built-in Metric keys (`erosion`, `duplication`, `verbosity`, `import_cycles`), reserved report keys (`files`, `source_lines`, `packages`), or another external Metric plugin's key.
- `ratio` must be finite and between `0.0` and `1.0`.
- `numerator` and `denominator` must be finite and `>= 0.0`.
- Each Finding must have a non-empty `id`, a 1-based `line` (`>= 1`), and a valid repository-relative `path` using `/`.
- Compute each Finding `id` from file path and content (never from line numbers) so `codeint diff` tracks moved Findings as unchanged.
- Order `findings` worst first.

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
      "new_findings": "numerator_rise_with_new_findings"
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

### Run the Seam 2 conformance suite

Every Language plugin (built-in or external) must pass the shared Seam 2 conformance suite in [crates/plugin-host/tests/conformance.rs](../crates/plugin-host/tests/conformance.rs).
The suite verifies:

- Non-empty file extensions without leading dots.
- `UnsupportedFile`, empty file, `DeclarationOnly`, and `Unparsable` handling in `lower`.
- Production and Test Source set classification.
- LF and CRLF equivalence for `source-lines` and `functions`.
- Ordered `functions` and `tokens`, valid 1-based line spans, and non-empty Flagged line rules.
- `manifest-paths` returning only paths from `repository-paths`.
- `workspace-packages` parsing valid manifests and returning `WorkspaceError::Manifest` on invalid manifests.
- `resolve-imports` returning one `Resolved`, `External`, `Builtin`, or `Unresolved` result per request in request order.
- `declared-dependencies` returning manifest-declared dependencies and `entry-points` returning only paths from `repository-paths`.
- `reference-ranges` returning valid ranges.
- Safe concurrent `lower` calls across threads.

Run the conformance suite and the host sandbox tests:

```sh
cargo test --locked -p codeint-plugin-host --test conformance
cargo test --locked -p codeint-plugin-host --test load
```

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
package codeint:plugin@0.2.0;
```

This version identifies the exported interfaces `codeint:plugin/language@0.2.0` and `codeint:plugin/metric@0.2.0` in the compiled WebAssembly component.

### Compatibility rules

- **Pre-1.0 (`0.x.y`)**: While the major version is `0`, a minor version bump (`0.1.0` to `0.2.0`) may introduce breaking changes.
  A patch version bump (`0.1.0` to `0.1.1`) contains only non-breaking fixes.
- **Stable (`1.x.y` and later)**:
  - Additive, backward-compatible changes (such as adding a new optional interface or new world) bump the minor version (`1.0.0` to `1.1.0`).
  - Breaking changes (removing or renaming a function, changing record fields, or changing variant or enum cases) bump the major version (`1.0.0` to `2.0.0`).

### Host compatibility

`wasmtime::component::bindgen!` binds the codeint host to the exact WIT interface version compiled into the binary.
At load time, wasmtime checks that the component's exported interface name and version match what the host expects.

- A Plugin built against an incompatible WIT version (such as `codeint:plugin@0.1.0`) fails at load time with exit code 2, naming the plugin, its found interface version, the required `0.2.0` interface version, and how to rebuild it.
- Rebuild the Plugin against the updated `wit/` files and update its `sha256` in `codeint.jsonc` or `codeint.json` when upgrading to a breaking WIT version.
- Never silently mutate an existing released WIT version in place; every breaking change gets a new version number.
