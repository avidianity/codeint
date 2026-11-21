# How to load external Language plugins

## Load plugins declared in codeint.json

```rust
use codeint_plugin_host::load_language_plugins;

let reserved = ["typescript-javascript"];
let external = load_language_plugins(repository.root(), &configuration.plugins, &reserved)?;
```

- `load_language_plugins` reads each plugin file once, verifies its SHA-256 digest against the declared `sha256` before compiling or instantiating it, and returns `Vec<(String, WasmLanguagePlugin)>`.
- Relative paths in `PluginDeclaration::path` resolve against `repository_root`; absolute paths are used as given.
- Every `WasmLanguagePlugin` is instantiated with an empty `wasmtime::component::Linker` with zero host capabilities (no WASI, no filesystem, no network, no environment variables, no clocks). A component that imports any host interface fails to instantiate with `PluginHostError::Instantiate`.
- Every `WasmLanguagePlugin` and `WasmMetricPlugin` enforces `codeint_config::PluginLimits` (linear memory cap via `wasmtime::ResourceLimiter` and wall-clock time limit via `wasmtime` epoch interruption). Use `load_language_plugins_with_limits` and `load_metric_plugins_with_limits` to pass configured limits from `codeint.json`.
- `WasmLanguagePlugin` implements `codeint_model::LanguagePlugin: Sync` using an internal pool of pre-instantiated component instances, so callers can invoke `lower` concurrently across threads.

## Verify a single plugin or compute a digest

```rust
use codeint_plugin_host::{load_language_plugin, sha256, sha256_hex};

let digest = sha256_hex(&sha256(&wasm_bytes));
let plugin = load_language_plugin(repository.root(), &declaration)?;
```

- `sha256` is a pure-Rust, `#![forbid(unsafe_code)]`, std-only FIPS 180-4 SHA-256 implementation.
- On a hash mismatch, `load_language_plugin` returns `PluginHostError::HashMismatch` naming the plugin, its resolved path, and both expected and actual digests, without passing the bytes to `wasmtime`.
