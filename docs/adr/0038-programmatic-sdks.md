---
status: accepted
---

# Client SDK architecture, parity, and Dart client SDK

The Owner decided this on 2026-11-28 while expanding codeint v0.8.0 client SDKs beyond TypeScript and Rust.

## Context

codeint ships two client SDKs documented in `docs/programmatic.md`: the `codeint` npm package (`npm/codeint`) and the Rust library (`codeint-cli`).
Both client SDKs drifted behind the CLI as new commands and Metrics landed:

1. `npm/codeint` exposes only four of eighteen CLI operations (`check`, `diff`, `trend`, and `init`).
   Its TypeScript declarations list only four of twelve built-in Metrics.
   They also omit newer options such as `package`, `owner`, `coverage`, `adopt`, and `format`.
   Its Node test suite (`npm/codeint/test/sdk.test.mjs`) does not run in CI.
2. `codeint-cli` exposes fourteen in-process operations and omits four (`ratchet update`, `baseline update`, `report`, and `plugin test`).
   Its `init` function hardcodes JSONC and omits both `format` and `adopt`.
3. Dart and Flutter developers have no official pub.dev package to run codeint from code or via `dart run`.
4. Without exported report JSON Schemas, cross-SDK CI tests, and unified version synchronization, every client SDK will drift behind the CLI.

## Decision

**Standardize client SDKs in waves, starting with TypeScript parity, Rust parity, and pure Dart after ADR 0036 and ADR 0037.**

1. **Scope and rollout waves**:
   This decision covers client SDKs that invoke codeint from user code, distinct from WebAssembly Plugin authoring (`codeint-plugin-sdk`).
   Wave 1 brings TypeScript and Rust to full parity and adds Dart.
   Wave 2 adds Python, Go, and PHP after Wave 1 ships, while SQL and Shell remain CLI-only.
   Wave 1 targets the post-[ADR 0036](./0036-plugin-contract-0-5-0.md) and post-[ADR 0037](./0037-metric-v2-duplication-and-boundaries.md) JSON Schema (`scope_summaries`, `file_diagnostics`, and Metric version 2).
2. **CLI parity and failure semantics**:
   Every client SDK exposes all eighteen operations across the seventeen non-interactive CLI commands.
   These cover `check`, `diff`, `trend`, `impact`, `trace`, `init` (with `format` and `adopt`), `guard`, `schema`, `config`, `report`, and `doctor`.
   They also cover `agent status`, `agent install`, `churn`, `coupling`, `ratchet update`, `baseline update`, and `plugin test`.
   Only `mcp` and `update` stay CLI-only.
   Every client SDK accepts full CLI option parity for each exposed operation.
   Gated commands (`check`, `diff`, `doctor`, and `plugin test`) return pass/fail results on exit codes `0` and `1`.
   Operational errors (exit code `2`) throw or return `CodeintError`.
   Report models expose typed fields for all twelve built-in Metrics plus an extensible map for custom Metric plugins.
3. **JSON subprocess seam and binary delivery**:
   Non-Rust client SDKs run the native `codeint` binary with JSON output instead of linking through a C-ABI foreign-function interface.
   This preserves `#![forbid(unsafe_code)]` per [ADR 0004](./0004-forbid-unsafe-code.md) and avoids C header maintenance.
   TypeScript keeps npm per-platform `optionalDependencies` (`@avidian/codeint-{os}-{arch}`).
   Wave 2 Python will use PyPI per-platform wheels.
   pub.dev caps package archives at 100 MB and lacks per-platform optional dependencies.
   Dart (and Wave 2 Go and PHP) therefore resolves the binary in four steps:
   - explicit `binaryPath` option;
   - `CODEINT_BIN` environment variable;
   - a version-matching `codeint` binary on `PATH`;
   - a version-pinned GitHub Release download verified against `SHA256SUMS` in a local user cache.
   Verified-cache client SDKs support `allowDownload: false` for offline CI.
   They also expose an explicit pre-install helper (`dart run codeint:install` and `ensureBinary`).
4. **In-process Rust parity and deferred crates.io publishing**:
   Keep the Rust client SDK in-process inside `codeint-cli` at full eighteen-operation parity for workspace and Git callers.
   Defer publishing workspace crates to crates.io while [ADR 0006](./0006-exact-dependency-pins.md) enforces exact version pins.
   Exact pins across `codeint-cli` and its internal runtime crates would conflict with downstream Cargo graphs.
   A thin subprocess-wrapper Rust crate can publish to crates.io later without relaxing ADR 0006.
5. **Monorepo layout, JSON Schema contract, and CI checks**:
   Keep all client SDKs in this repository so CLI changes, JSON Schemas, and client SDKs update together.
   Keep `npm/` at its current path and place new client SDKs under `sdks/<language>/`, starting with `sdks/dart/`.
   Export report JSON Schemas from `codeint-report` and validate live CLI JSON outputs in automated tests.
   Run TypeScript, Rust, and Dart client SDK tests offline against the built binary in CI on non-docs pull requests.
   Extend `cargo xtask version-sync` and `prepare-release` to lock `sdks/dart/pubspec.yaml` and future `sdks/*` manifests to the workspace version.
6. **Pure Dart package, ADR 0006 amendment, and pub.dev publishing**:
   Ship the Dart client SDK as a pure Dart package named `codeint` on pub.dev (`sdk: ^3.6.0`, zero Flutter dependency).
   Include a thin CLI forwarder so users can run `dart run codeint`.
   This decision partially amends [ADR 0006](./0006-exact-dependency-pins.md) for published pub.dev library manifests.
   `sdks/dart/pubspec.yaml` uses standard pub.dev caret version ranges for its two small runtime dependencies (SHA-256 hashing and archive extraction).
   Rust workspace crates and npm manifests keep exact pins per [ADR 0006](./0006-exact-dependency-pins.md).
   Publish `codeint` on version tags via pub.dev GitHub Actions OIDC after the Owner completes one manual initial publish.

## Considered options

- **Expose a C-ABI shared library (`cdylib`) for non-Rust client SDKs.**
  Rejected: a C-ABI foreign-function interface requires `unsafe` Rust violating [ADR 0004](./0004-forbid-unsafe-code.md), adds C headers, and still ships per-platform binaries.
- **Bundle all six platform binaries directly inside the pub.dev Dart archive.**
  Rejected: pub.dev caps archives at 100 MB and lacks per-platform packages, so bundling six binaries bloats every install.
- **Require Dart users to install `codeint` on `PATH` with no download fallback.**
  Rejected: a separate manual install hurts ergonomics when verified `SHA256SUMS` downloads work automatically and `allowDownload: false` supports offline CI.
- **Publish `codeint-cli` and its internal workspace crates to crates.io in Wave 1.**
  Rejected: exact `=` dependency pins from [ADR 0006](./0006-exact-dependency-pins.md) conflict with downstream Rust dependency graphs on crates.io.
- **Host each client SDK in a separate Git repository.**
  Rejected: separate repositories split JSON Schema updates across pull requests and cause the drift seen in `npm/codeint`.

## Consequences

- Every non-docs pull request verifies TypeScript, Rust, and Dart client SDK parity offline against the built binary.
- `cargo xtask version-sync` and `prepare-release` manage `Cargo.toml`, `npm/`, and `sdks/dart/pubspec.yaml` as one versioned unit.
- Dart and Flutter projects gain a pure Dart client SDK and `dart run codeint` with SHA-256-verified delivery.
- The Owner runs one manual `dart pub publish` to register `codeint` and enable pub.dev OIDC before automated tag publishing runs.
- Wave 2 client SDKs (Python, Go, and PHP) inherit the same monorepo layout, report JSON Schemas, and CI checks.
