# How to use and extend `codeint-cli`

## Call the library or CLI entry point

```rust
use std::path::Path;

use codeint_cli::{CheckOptions, ReportFormat, check};

let outcome = check(Path::new("."), &CheckOptions::default())?;
let rendered = outcome.report.render(ReportFormat::Toon)?;
if !outcome.passed {
    for failure in &outcome.failures {
        eprintln!("{failure}");
    }
}
```

- `crates/cli` (`codeint-cli`) is both the binary entry point (`src/main.rs` calls `codeint_cli::run_cli()`) and the Rust library crate backing programmatic callers ([ADR 0038](../../docs/adr/0038-programmatic-sdks.md)).
- User-facing CLI flags, exit codes, and configuration keys live in [docs/reference.md](../../docs/reference.md).
- `run_cli` returns exit code `0` on pass, `1` when any Budget, Ratchet, Regression, `doctor` blocking error, or `plugin test` check fails, and `2` on operational errors.
- Library entry points in `src/lib.rs` (`check`, `diff`, `trend`, `impact`, `trace`, `init`, `guard`, `schema`, `resolved_config`, `doctor`, `agent_status`, `agent_install`, `churn`, `coupling`, `ratchet_update`, `baseline_update`, `rerender_report`, `plugin_test`) return `Result<_, CodeintError>` (except `schema`), where `CodeintError` pairs `message()` with an actionable `remedy()`.
- Gated operations (`check`, `diff`, `doctor`, and `plugin_test`) return `GatedOutcome<T>` (aliased as `CheckOutcome`, `DiffOutcome`, `DoctorOutcome`, and `PluginTestOutcome`), which pairs `passed`, `failures`, and `report`.
- `init` takes `InitOptions` (`format`, `adopt`, `plugin_host`) and returns `InitOutcome` (`configuration` plus optional `AdoptedFiles` when `adopt` is `true`).
- `ratchet_update` and `baseline_update` accept `RatchetUpdateOptions` and `BaselineUpdateOptions` (both aliases of `StateUpdateOptions` with `reset`, `coverage_reports`, and `plugin_host`).
- `rerender_report` accepts `RerenderReportOptions` (`format`, `all`) to re-render a saved JSON report file, while `SavedReport::from_json` supports in-memory re-rendering without touching disk.
- `plugin_test` accepts `PluginTestOptions` (`kind`, `sources`, `limits`) and returns `Result<PluginTestOutcome, CodeintError>`.

## Check and measurement orchestration

`src/check.rs` coordinates the shared measurement pipeline used by `check`, `diff`, `trend`, `impact`, `trace`, `ratchet`, `baseline`, `churn`, `coupling`, and `mcp`:

1. **Workspace and plugins (`WorkspaceContext::load`, `LoadedPlugins::load`)**:
   - Discovers the Git repository, reads configuration, and loads built-in plus external Wasm Plugins with configured `PluginLimits`.
   - Validates configured Metrics, Dialects (`src/check/scope.rs::validate_configured_dialects`), disabled Hazard rules (`validate_disabled_hazard_rules`), and Framework profiles (`src/check/frameworks.rs::validate_configured_framework_profiles`).
2. **Manifests and Workspace packages (`src/check/manifests.rs`)**:
   - Calls `choose_manifests` and `analyze_manifests` for every Language plugin with `has_imports() == true`.
   - Drops invalid manifests from workspace discovery and records user-visible warnings with control characters escaped (`src/check/control_characters.rs`).
3. **Framework profiles and lowering (`src/check/frameworks.rs`, `src/check/lowering.rs`)**:
   - Resolves per-package Framework profiles, convention entry points, generated paths, and framework Reference ranges.
   - Resolves per-file Dialects (`scope::resolve_file_dialects`) and lowers files in parallel with `rayon`, caching lowered outcomes in `LoweringCache` by `(path, blob_id, plugin_name, plugin_version, dialect)`.
   - Settles Source sets via `codeint_model::settle_source_set` and validates Hazard lines and file diagnostics.
4. **Cross-file scope analysis (`src/check/scope.rs`)**:
   - For every Language plugin with `has_scope_analysis() == true`, groups opaque `scope_artifact` bytes by `(Workspace package, Dialect)` sorted by path.
   - Calls `LanguagePlugin::analyze_scope`, validates the output with `codeint_model::validate_scope_analysis`, merges extra Hazard lines into each file, and collects sorted `ScopeSummaryEntry` items.
5. **Imports, coverage, and package scoping (`src/check/manifests.rs`, `src/check/coverage.rs`, `src/check/package_filter.rs`)**:
   - Resolves runtime imports with per-package active Framework profiles.
   - Loads user-named LCOV or Cobertura XML Coverage reports from CLI flags or configuration (never guessing paths).
   - Evaluates cross-package Orphan files and Dependency hygiene before filtering the snapshot to `--package` and refreshing package-scoped Framework profiles and Reference ranges.
6. **Reporting, Baseline, and Ratchet (`src/baseline.rs`, `src/ratchet.rs`)**:
   - Summarizes the report via `codeint_report::Report::summarize_with_extras`, verifies `Test gap` Budget requirements and Baseline Metric versions, and enforces whole-repository Ratchets.

## MCP stdio server

- `src/mcp.rs` implements a JSON-RPC 2.0 server over stdio (`initialize`, `ping`, `tools/list`, `tools/call`) with default protocol version `2024-11-05`.
- It exposes eight tools: `check`, `diff`, `trend`, `guard`, `impact`, `trace`, `churn`, and `coupling`.
- Every tool defaults to `format: "toon"` and accepts `"json"`.
- `src/mcp/workspace_cache.rs` caches parsed configuration and loaded Wasm Plugins per repository root across tool calls in one server session.
