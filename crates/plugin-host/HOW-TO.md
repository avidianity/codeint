# How to load external Language plugins

## Load plugins declared in codeint.json

```rust
use codeint_config::PluginHostConfig;
use codeint_plugin_host::load_language_plugins;

let reserved = ["typescript-javascript"];
let host_config = PluginHostConfig::default();
let external = load_language_plugins(repository.root(), &configuration.plugins, &reserved, &host_config)?;
```

- `load_language_plugins` reads each local plugin file or fetches and caches each HTTPS plugin once, verifies its SHA-256 digest against the declared `sha256` before caching, compiling, or instantiating it, and returns `Vec<(String, WasmLanguagePlugin)>`.
- Relative paths in `PluginSource::Path` resolve against `repository_root`; absolute paths are used as given.
- `PluginSource::HttpsUrl` downloads through `curl` with `--proto =https --tlsv1.2 --proto-redir =https`, verifies the SHA-256 digest of the downloaded bytes before atomically renaming into `<cache_dir>/<sha256>.wasm`, and reuses a valid cached entry without network I/O.
- Every `WasmLanguagePlugin` is instantiated with an empty `wasmtime::component::Linker` with zero host capabilities (no WASI, no filesystem, no network, no environment variables, no clocks). A component that imports any host interface fails to instantiate with `PluginHostError::Instantiate`.
- Every `WasmLanguagePlugin` and `WasmMetricPlugin` validates its exported WIT interface version against `codeint:plugin@0.3.0` before instantiation; an incompatible version (such as `codeint:plugin@0.2.0`) fails with `PluginHostError::IncompatibleContractVersion`.
- `verify_plugin_component` compiles component bytes and checks the exported WIT contract version and zero-capability sandbox requirements without instantiating or running untrusted guest code.
- Every `WasmLanguagePlugin` and `WasmMetricPlugin` enforces `codeint_config::PluginLimits` (linear memory cap via `wasmtime::ResourceLimiter` and wall-clock time limit via `wasmtime` epoch interruption). Use `load_language_plugins_with_limits` and `load_metric_plugins_with_limits` to pass configured limits from `codeint.json`.
- `WasmLanguagePlugin` implements `codeint_model::LanguagePlugin: Sync` using an internal pool of pre-instantiated component instances, so callers can invoke `lower` concurrently across threads.

## Verify a single plugin or compute a digest

```rust
use codeint_config::PluginHostConfig;
use codeint_plugin_host::{load_language_plugin, sha256, sha256_hex};

let digest = sha256_hex(&sha256(&wasm_bytes));
let plugin = load_language_plugin(repository.root(), &declaration, &PluginHostConfig::default())?;
```

- `sha256` is a pure-Rust, `#![forbid(unsafe_code)]`, std-only FIPS 180-4 SHA-256 implementation.
- On a hash mismatch, `load_language_plugin` returns `PluginHostError::HashMismatch` (local path) or `PluginHostError::DownloadHashMismatch` (HTTPS URL) naming the plugin, its path or URL, and both expected and actual digests, without caching or passing the bytes to `wasmtime`.
