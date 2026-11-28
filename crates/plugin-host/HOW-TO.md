# How to use the Plugin host

## Discover built-in Language plugins

```rust
use codeint_plugin_host::{
    built_in_language_descriptors, built_in_language_flagged_line_rules,
    built_in_language_framework_profiles, built_in_language_hazard_rules,
    built_in_language_names, built_in_language_plugin, built_in_language_plugins,
};

let plugins = built_in_language_plugins();
let reserved = built_in_language_names();
let descriptors = built_in_language_descriptors();
let flagged_line_rules = built_in_language_flagged_line_rules();
let hazard_rules = built_in_language_hazard_rules();
let framework_profiles = built_in_language_framework_profiles();
let ts_plugin = built_in_language_plugin("typescript-javascript");
```

- `built_in_language_plugins` returns the static slice of all eight built-in `NamedLanguagePlugin` entries (`typescript-javascript`, `rust`, `php`, `dart`, `python`, `go`, `sql`, `shell`).
- `built_in_language_plugin` looks up a single built-in Language plugin by name.
- `built_in_language_names` returns the reserved built-in plugin names in registration order.
- `built_in_language_descriptors` returns each built-in Language plugin's name and claimed extensions for schema and discovery reporting.
- `built_in_language_flagged_line_rules` returns each built-in Language plugin's name mapped to the Flagged line rule names its crate declares; every built-in Language plugin has an entry.
- `built_in_language_hazard_rules` returns the map of built-in Language plugins that expose Hazard line rules to their declared rule names.
- `built_in_language_framework_profiles` returns the map of built-in Language plugins that expose Framework profiles to their declared profile names.

## Lower SQL files and run the cross-file SQL schema pass

```rust
use std::collections::BTreeMap;

use codeint_config::SqlDialect;
use codeint_plugin_host::{apply_sql_schema_pass, lower_sql_source};

let lowered = lower_sql_source(&path, source, SqlDialect::Postgresql)?;
let mut parsed_by_path = BTreeMap::new();
parsed_by_path.insert(path, lowered.parsed);
let summaries = apply_sql_schema_pass(&mut sql_model, &parsed_by_path);
```

- `lower_sql_source` lowers a SQL source file with the configured `codeint_config::SqlDialect` and returns its `FileModel`, opaque `SqlParsedFile`, and any `SqlRejectedStatement` entries.
- `apply_sql_schema_pass` builds per-(package, dialect) schemas from non-vendored, non-down-migration SQL files, appends cross-file schema Hazard lines to `sql_model`, and returns sorted `SqlSchemaScopeSummary` records.

## Resolve Framework profiles

- Built-in and external Language plugins resolve package-level Framework profiles, convention entry points, framework-generated Source set paths, and framework Reference ranges through `LanguagePlugin::known_framework_profiles` and `LanguagePlugin::resolve_framework_profiles` (`codeint:plugin@0.5.0`).
- Resolved active profile names are passed per package (`Vec<PackageFrameworkProfiles>`) into `LanguagePlugin::resolve_imports` so fallback import hints stay coupled to each package's active profiles.

## Load external plugins declared in configuration

```rust
use codeint_config::PluginHostConfig;
use codeint_plugin_host::{built_in_language_names, load_language_plugins};

let reserved = built_in_language_names();
let host_config = PluginHostConfig::default();
let external = load_language_plugins(repository.root(), &configuration.plugins, &reserved, &host_config)?;
```

- `load_language_plugins` reads each local plugin file or fetches and caches each HTTPS plugin once, verifies its SHA-256 digest against the declared `sha256` before caching, compiling, or instantiating it, and returns `Vec<(String, WasmLanguagePlugin)>`.
- Relative paths in `PluginSource::Path` resolve against `repository_root`; absolute paths are used as given.
- `PluginSource::HttpsUrl` downloads through `curl` with `--proto =https --tlsv1.2 --proto-redir =https --max-filesize <max_download_bytes>` and a bounded streaming reader (50 MiB default via `PluginHostConfig::max_download_bytes`, overridden by `CODEINT_PLUGIN_MAX_DOWNLOAD_MB`), verifies the SHA-256 digest of the downloaded bytes before atomically renaming into `<cache_dir>/<sha256>.wasm`, and reuses a valid cached entry without network I/O.
- Every `WasmLanguagePlugin` is instantiated with an empty `wasmtime::component::Linker` with zero host capabilities (no WASI, no filesystem, no network, no environment variables, no clocks).
  A component that imports any host interface fails to instantiate with `PluginHostError::Instantiate`.
- Every `WasmLanguagePlugin` and `WasmMetricPlugin` validates its exported WIT interface version against `codeint:plugin@0.5.0` before instantiation; an incompatible version (such as `codeint:plugin@0.4.0`, `codeint:plugin@0.3.0`, or `codeint:plugin@0.2.0`) fails with `PluginHostError::IncompatibleContractVersion`.
- Every `WasmMetricPlugin` also validates its `version()` export at load time through `validate_metric_version`, rejecting `0` with `PluginHostError::InvalidMetricVersion`.
- `verify_plugin_component` compiles component bytes and checks the exported WIT contract version and zero-capability sandbox requirements without instantiating or running untrusted guest code.
- Every `WasmLanguagePlugin` and `WasmMetricPlugin` enforces `codeint_config::PluginLimits` (linear memory cap via `wasmtime::ResourceLimiter`, host lifted-size budget via `enforce_lifted_budget`, and wall-clock time limit via `wasmtime` epoch interruption).
  Use `load_language_plugins_with_limits` and `load_metric_plugins_with_limits` to pass configured limits from the configuration file.
- All plugin loads share one process-wide `wasmtime::Engine`, a bounded compiled-component cache keyed by verified SHA-256 digest, and one reference-counted epoch-ticker thread that shuts down once all loaded plugins drop.
  Every plugin call instantiates a fresh `wasmtime::Store` from pre-compiled `InstancePre` with an empty zero-capability `Linker` and `PluginLimits`, then drops the store immediately after the call so linear memory never stays pinned and guest state cannot leak across calls.
- `validate_metric_key` enforces external Metric key syntax and rejects every reserved report key from `codeint_model::is_reserved_report_key` (`Metric::ALL` and `STRUCTURAL_REPORT_KEYS`), every `SourceSet::ALL` key (`production`, `test`, `generated`, `vendored`), and the trend and top-level configuration keys (`commit`, `committed_on`, `dependencies`).
- `WasmLanguagePlugin` implements `codeint_model::LanguagePlugin: Sync` by instantiating a fresh store from its pre-compiled `InstancePre` per call, so callers can invoke `lower` concurrently across threads without shared mutable guest state.

## Verify a single plugin or compute a digest

```rust
use codeint_config::PluginHostConfig;
use codeint_plugin_host::{load_language_plugin, sha256, sha256_hex};

let digest = sha256_hex(&sha256(&wasm_bytes));
let plugin = load_language_plugin(repository.root(), &declaration, &PluginHostConfig::default())?;
```

- `sha256` is a pure-Rust, `#![forbid(unsafe_code)]`, std-only FIPS 180-4 SHA-256 implementation.
- On a hash mismatch, `load_language_plugin` returns `PluginHostError::HashMismatch` (local path) or `PluginHostError::DownloadHashMismatch` (HTTPS URL) naming the plugin, its path or URL, and both expected and actual digests, without caching or passing the bytes to `wasmtime`.

## Run the Conformance runner on one Wasm Plugin

```rust
use codeint_config::{PluginKind, PluginLimits};
use codeint_plugin_host::{ConformanceSource, run_conformance};

let sources = [ConformanceSource {
    file_name: "sample.mini".to_owned(),
    text: "alpha\nbeta\n".to_owned(),
}];
let report = run_conformance(PluginKind::Language, path, PluginLimits::default(), &sources)?;
if !report.passed() {
    for check in report.checks.iter().filter(|check| check.hint.is_some()) {
        eprintln!("{}: {} ({})", check.id, check.message, check.hint.as_deref().unwrap_or_default());
    }
}
```

- `run_conformance` reads the file, loads it through the same zero-capability sandbox as `codeint check`, and returns one `ConformanceCheck` per rule with `ConformanceStatus::{Pass, Fail, Skipped}`, a message, and a fix hint on every fail.
- It returns `Err(PluginHostError::Read)` only when the file cannot be read; every contract violation is a failed check, never an `Err`.
- A check that depends on a failed load or an earlier fail is `Skipped`, and so is every import check of a Language plugin whose `has-imports` is false.
- The report lists every check exactly once, in a fixed order.
- `WasmLanguagePlugin::manifest_paths_checked`, `WasmLanguagePlugin::analyze_manifests_checked`, and `WasmLanguagePlugin::analyze_scope_checked` expose a trap or a limit hit as `Err(PluginHostError)`; `LanguagePlugin::manifest_paths`, `LanguagePlugin::analyze_manifests`, `LanguagePlugin::analyze_scope`, and `LanguagePlugin::reference_ranges` propagate traps and limit hits as `Err(LanguagePluginError)`.
- `sources` adds Language plugin probe sources to the built-in ones; each file name carries the extension that the Plugin claims.
- `run_conformance` leaves `end-to-end` as `Skipped` and attaches an `EndToEndProbe` in `report.end_to_end` when earlier checks pass; callers such as `codeint-cli` run `codeint check` on that probe and fill in the `end-to-end` entry.
- `codeint plugin test` is the command-line front end; see [docs/reference.md](../../docs/reference.md#codeint-plugin).

## Verify WIT contract lockstep

Run `cargo test --locked -p codeint-plugin-host --test wit_lockstep` to verify that `wit/language.wit` and `wit/metric.wit` match `PLUGIN_INTERFACE_VERSION`, the `LanguagePlugin` and `MetricPlugin` traits in `codeint-model`, the host wrappers in `codeint-plugin-host`, and built-in Language plugin registration.
