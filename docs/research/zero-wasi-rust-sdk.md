# Zero-WASI Rust WebAssembly component SDK ergonomics

Date checked: 2026-11-25.
Pinned local toolchain verified in this checkout: `rustc 1.99.1`, `wasm-component-ld 0.5.27`, `wit-bindgen 0.63.0`, and `wasmtime 51.0.0`.

## Summary

A single Rust SDK crate (`codeint-plugin-sdk`) can support zero-host-import Plugins on two complementary Tier 2 targets without changing codeint's empty-`Linker` sandbox:

1. **Primary path (zero extra tools): `wasm32-wasip2` with `#![cfg_attr(target_arch = "wasm32", no_std)]`.**
   Plugin authors install only `rustup target add wasm32-wasip2` and run `cargo build --target wasm32-wasip2 --release`.
   The SDK crate hides the global freeing allocator, panic handler, `cabi_realloc`, C memory intrinsics (`memcmp`, `memcpy`, `memmove`, `memset`), and `wit-bindgen` wiring behind one safe trait and one export macro.
   The only line the SDK cannot hide in the Plugin crate is the one-line `#![cfg_attr(..., no_std)]` crate attribute.
2. **Escape hatch (`std` crate ecosystem): `wasm32-unknown-unknown` with `std` plus `wasm-tools component new`.**
   On `wasm32-unknown-unknown`, Rust's `std` imports zero host functions, uses `dlmalloc` over `memory.grow`, provides `compiler_builtins` memory intrinsics, and gets `cabi_realloc` directly from `wit-bindgen`.
   Authors who need `std`-only crates (such as `regex` or `std`-bound parsers) can build with the same SDK crate and convert the output core module into a component with one `wasm-tools component new` command.

A critical finding in the existing fixtures is that `sample-plugin` and `probe-runtime` use a non-freeing `BumpAllocator` while `crates/plugin-host/src/wasm.rs` reuses component instances through `InstancePool`.
A production SDK must replace the bump allocator with a freeing allocator (`talc` v5.1+ `WasmDynamicTalc` or `dlmalloc` v0.2 `GlobalDlmalloc`) so pooled instances reclaim linear memory between files under codeint's 64 MiB default cap.

## Host constraints in codeint

Every external Language plugin and Metric plugin runs under strict host boundaries enforced in `crates/plugin-host`:

- **Zero host imports**: `wit/language.wit` and `wit/metric.wit` (`package codeint:plugin@0.3.0`) export one interface and import nothing.
  `crates/plugin-host/src/wasm.rs` instantiates every component with an empty `wasmtime::component::Linker::new(&engine)`.
  Any component importing a WASI interface (such as `wasi:cli/*`, `wasi:io/*`, `wasi:filesystem/*`, `wasi:clocks/*`, or `wasi:random/*`) fails instantiation with exit code 2.
- **64 MiB default memory cap**: `crates/config/src/codeint_json.rs` sets `DEFAULT_PLUGIN_MEMORY_MB = 64` and `DEFAULT_PLUGIN_TIMEOUT_MS = 5_000`.
  `crates/plugin-host/src/limits.rs` enforces this cap per `Store` via `StoreLimitsBuilder::new().memory_size(max_memory_bytes).trap_on_grow_failure(true)` plus `set_hostcall_fuel(limits.memory_bytes())` and `enforce_lifted_budget`.
- **Instance pooling**: `InstancePool` checks out and returns live component instances across calls (`lower`, `resolve_imports`, `measure`).
  Any memory leaked inside a guest instance persists across files until the instance traps with `MemoryLimitExceeded`.
- **Workspace `unsafe` policy**: [ADR 0004](../adr/0004-forbid-unsafe-code.md) enforces `#![forbid(unsafe_code)]` on every workspace crate root and states that any future need for `unsafe` requires a new ADR.

## Target and toolchain comparison

| Target and mode | Host imports | Extra CLI tools | Standard library | Boilerplate hidden by SDK | Verdict |
| --------------- | ------------ | --------------- | ---------------- | ------------------------- | ------- |
| `wasm32-wasip2` + `no_std` (`alloc`) | Zero | None (`wasm-component-ld` ships with `rustc`) | `core` + `alloc` only | Allocator, panic handler, `cabi_realloc`, `mem*`, WIT bindings; 1-line `cfg_attr` stays in Plugin | **Recommended default** |
| `wasm32-unknown-unknown` + `std` | Zero | `wasm-tools` (`component new`) | Full `std` (OS calls stubbed or error) | WIT bindings and trait only; `std` + `wit-bindgen` supply allocator, panic, `cabi_realloc`, `mem*` | **Recommended `std` escape hatch** |
| `wasm32-wasip2` + `std` | WASI 0.2 (`wasi:cli/*`, `wasi:io/*`, etc.) | None | Full `std` | N/A | **Incompatible** (rejected by empty `Linker`) |
| `wasm32-none-component` (Tier 3) | Zero | None | `core` + `alloc` only | Allocator, panic handler, WIT bindings | **Not ready** (absent from `rustc 1.99.1`; requires nightly `-Z build-std`) |
| `wasm32v1-none` (Tier 2) | Zero | `wasm-tools` (`component new`) | `core` + `alloc` only | Same as `wasip2` + `no_std` | **Dominated** (combines `no_std` limits with `wasm-tools` step) |

### Why `wasm32-wasip2` + `std` fails codeint's sandbox

`wasm32-wasip2` is a Tier 2 `rustc` target whose default linker is `wasm-component-ld` (bundled as `wasm-component-ld 0.5.27` inside the `rustc 1.99.1` host sysroot).
When a crate links `std` for `wasm32-wasip2`, `rustc` links `self-contained/libc.a` (wasi-libc) and WASI 0.2 bindings.
Even a pure computation function then imports WASI interfaces such as `wasi:cli/environment` and `wasi:io/streams`.
Wasmtime's Component Model linker enforces every declared import strictly, so codeint's empty `Linker::new(&engine)` rejects the component at load time.

### Why `sample-plugin` currently carries 151 lines of unsafe boilerplate

Inspection of `crates/plugin-host/tests/fixtures/sample-plugin/src/lib.rs`, `wit-bindgen 0.63.0`, and the installed `rustc 1.99.1` sysroot reveals the exact root causes of every block in that boilerplate:

1. **`#![no_std]` and `extern crate alloc;`**: Required on `wasm32-wasip2` to prevent `rustc` from linking `std` and `wasi-libc`.
2. **Custom `#[global_allocator]` and `#[panic_handler]`**: Required in `#![no_std]` binaries that allocate with `alloc`.
3. **Manual `#[unsafe(export_name = "cabi_realloc")]`**: In `wit-bindgen 0.63.0`, the `realloc` Cargo feature (`realloc = []` in `wit-bindgen-0.63.0/Cargo.toml`) is an empty backward-compatibility flag.
   `wit-bindgen-0.63.0/build.rs` links its prebuilt `libwit_bindgen_cabi_realloc.a` weak shim only when `target_env == "p1" || target_env == ""` (`wasm32-wasip1` and `wasm32-unknown-unknown`).
   `wit-bindgen-0.63.0/src/rt/mod.rs` applies the same `#[cfg(any(not(target_family = "wasm"), target_env = "p1", target_env = ""))]` gate to `maybe_link_cabi_realloc()` and `rt::cabi_realloc`.
   On `wasm32-wasip2` (`target_env = "p2"`), `wit-bindgen` assumes `std`/`wasi-libc` exports `cabi_realloc`, so a `#![no_std]` crate on `wasm32-wasip2` must export `cabi_realloc` itself.
4. **Manual `memcmp`, `memcpy`, `memmove`, and `memset`**: Inspecting `libcompiler_builtins-*.rlib` inside the `rustc 1.99.1` `wasm32-wasip2` sysroot with `llvm-nm --defined-only` shows zero definitions for `memcpy`, `memmove`, `memset`, or `memcmp`.
   The `wasm32-wasip2` target expects `self-contained/libc.a` to supply C memory functions, whereas `wasm32-unknown-unknown` builds `compiler_builtins` with the `mem` feature enabled.
   A `#![no_std]` crate on `wasm32-wasip2` therefore needs those four symbols supplied by the SDK runtime.
5. **Inline `wit_bindgen::generate!` and raw `Guest` implementation**: Each fixture copies the path to `wit/` and implements every method on the generated `Guest` trait without defaults.

### Why `wasm32-unknown-unknown` + `std` works with zero WASI imports

According to the `rustc` platform support guide for `wasm32-unknown-unknown`, its standard library imports no host functions:

- Allocation uses `dlmalloc` backed by WebAssembly `memory.grow`.
- OS calls either no-op (`println!`), return errors (`std::fs`, `std::net`), or panic (`std::thread::spawn`).
- `std::collections::HashMap` works without host entropy (using a non-randomized hasher).
- `compiler_builtins` provides `memcpy`, `memmove`, `memset`, and `memcmp`.
- `wit-bindgen 0.63.0` (`target_env == ""`) automatically links `libwit_bindgen_cabi_realloc.a` and embeds the `component-type:wit-bindgen:...` custom section via `wit_bindgen::generate!`.

Because `wit_bindgen::generate!` already embeds the component type custom section into the core `.wasm` file, converting the core module into a zero-import component requires only `wasm-tools component new` (no `wasm-tools component embed` and no `--adapt` WASI adapter):

```sh
cargo build --target wasm32-unknown-unknown --release
wasm-tools component new \
  target/wasm32-unknown-unknown/release/my_plugin.wasm \
  -o my_plugin.wasm
```

Using `wasm-component-ld` directly as a custom linker for `wasm32-unknown-unknown` is not a supported `rustc` workflow because `wasm32-unknown-unknown` is configured for core modules (`wasm-ld`), not `wasm-component-ld`.
`wasm-tools component new` is the documented Bytecode Alliance path for turning a `wasm32-unknown-unknown` core module into a component.

## SDK crate architecture (`codeint-plugin-sdk`)

### Packaging `wit-bindgen` inside the SDK crate

`wit-bindgen 0.63.0` supports cross-crate SDK wrappers natively through four `generate!` options verified in `wit-bindgen-0.63.0/src/lib.rs` and `wit-bindgen-rust-0.63.0/src/lib.rs`:

- **`default-features = false, features = ["macros"]`**: Codeint's WIT worlds are synchronous and use no `bitflags` types, so `std`, `async`, `bitflags`, and `macro-string` are unnecessary (`realloc` is a no-op in 0.63.0).
- **`pub_export_macro: true`**: Makes the generated `export!` macro `#[macro_export] pub`.
  Crucially, `wit-bindgen-rust 0.63.0` (`src/lib.rs` lines 1527-1569) handles `pub_export_macro: true` by emitting an exports-stripped `component-type` section inside the SDK crate and deferring the full exports `component-type` section until the downstream Plugin invokes `export!`.
  This lets one `codeint-plugin-sdk` crate contain bindings for both `language-plugin` and `metric-plugin` without causing a Language plugin to emit `metric` export metadata.
- **`default_bindings_module`**: Points the generated `export!` macro at the SDK's hidden bindings module (for example `codeint_plugin_sdk::__bindings::language`).
- **`runtime_path`**: Points generated code at a re-exported runtime path (`codeint_plugin_sdk::__wit_bindgen::rt`), so Plugin crates never add `wit-bindgen` to their own `Cargo.toml`.
- **`export_macro_name` and `type_section_suffix`**: Give distinct names (`__export_language`, `__export_metric`) and distinct custom section suffixes (`-language`, `-metric`) when both worlds live in one SDK crate.

The SDK crate can bundle `wit/language.wit` and `wit/metric.wit` inside its package (matching the crate version to the WIT contract version) so external Plugin authors never copy `.wit` files by hand.

### Hiding runtime boilerplate behind one macro and trait

Everything except `#![no_std]` can live inside `codeint-plugin-sdk`:

1. **Allocator, panic handler, `cabi_realloc`, and `mem*` in `codeint-plugin-sdk`**:
   - Gate the `no_std` Wasm runtime module with `#[cfg(all(target_arch = "wasm32", target_os = "wasi", target_env = "p2"))]`.
   - Put the global allocator behind a default-on Cargo feature `default-allocator` and the panic handler behind a default-on feature `panic-handler`.
     Advanced authors who need a custom allocator or panic handler can opt out with `default-features = false` without symbol collisions.
   - Keep `extern crate codeint_plugin_sdk;` or a linker anchor call inside `export_language_plugin!` / `export_metric_plugin!` (such as referencing a `#[used]` static holding `cabi_realloc`, matching `wit-bindgen`'s `maybe_link_cabi_realloc()` pattern) so `wasm-ld` and LTO never dead-strip the runtime object unit from the `cdylib`.
   - Codeint's own `crates/plugin-host/tests/fixtures/sandbox-probes/probe-runtime` already proves that `#[global_allocator]`, `#[panic_handler]`, `#[unsafe(export_name = "cabi_realloc")]`, and `#[unsafe(no_mangle)]` `mem*` symbols defined in a dependency `rlib` link cleanly into a zero-import `wasm32-wasip2` `cdylib` component.
   - On `wasm32-unknown-unknown` (with the SDK's optional `std` feature) and on native hosts (`cargo test`), the `wasip2` `no_std` runtime module compiles out cleanly, so the exact same Plugin code runs native unit tests under `cargo test` with full `std` test assertions.
2. **What cannot be hidden by a macro**:
   - `#![no_std]` is a crate-level inner attribute and cannot be emitted by a `macro_rules!` expansion.
   - On `wasm32-wasip2`, the Plugin's `src/lib.rs` must keep one attribute line:

     ```rust
     #![cfg_attr(all(target_arch = "wasm32", target_os = "wasi", target_env = "p2"), no_std)]
     ```

   - `codeint-plugin-sdk` can re-export `alloc::string::{String, ToString}`, `alloc::vec::Vec`, `alloc::vec`, `alloc::format`, and `alloc::collections::{BTreeMap, BTreeSet}` from `codeint_plugin_sdk::prelude::*`, eliminating `extern crate alloc;` in Plugin code.
3. **Ergonomic safe traits with defaults**:
   - The SDK can expose `LanguagePlugin` and `MetricPlugin` traits using the generated WIT types (or thin re-exports of them) with default methods for optional capabilities:
     - `has_hazard_rules() -> bool` defaults to `false`, and `hazard_rules() -> Vec<String>` defaults to `Vec::new()`.
     - `reference_ranges() -> Vec<ReferenceRange>` defaults to `Vec::new()`.
     - `has_imports() -> bool` defaults to `false`, with default empty implementations of `manifest_paths`, `workspace_packages`, `resolve_imports`, `declared_dependencies`, and `entry_points`.
   - The `export_language_plugin!(MyPlugin);` macro generates a private bridge struct that implements `wit-bindgen`'s `Guest` trait by forwarding to `MyPlugin`'s `LanguagePlugin` impl, anchors the runtime symbols, and invokes the hidden `wit-bindgen` export macro.
   - Helper utilities such as `fnv1a_64(&str) -> u64` can live directly in the SDK so every Language plugin computes token hashes identically.

### Resulting Plugin author ergonomics

With `codeint-plugin-sdk`, a minimal `wasm32-wasip2` Language plugin shrinks from ~170 lines of unsafe runtime setup to a clean safe crate:

```toml
[lib]
crate-type = ["cdylib"]

[dependencies]
codeint-plugin-sdk = "0.3"

[profile.release]
opt-level = "s"
lto = true
panic = "abort"
```

```rust
#![cfg_attr(all(target_arch = "wasm32", target_os = "wasi", target_env = "p2"), no_std)]

use codeint_plugin_sdk::language::{FileModel, LanguagePlugin, LowerError, SourceSet};
use codeint_plugin_sdk::prelude::*;

struct MyLanguage;

impl LanguagePlugin for MyLanguage {
    fn extensions() -> Vec<String> {
        vec!["mylang".into()]
    }

    fn lower(path: String, text: String) -> Result<FileModel, LowerError> {
        Ok(FileModel {
            source_set: SourceSet::Production,
            source_lines: text.lines().filter(|line| !line.trim().is_empty()).count() as u32,
            ..FileModel::empty()
        })
    }
}

codeint_plugin_sdk::export_language_plugin!(MyLanguage);
```

Build and test commands for Plugin authors:

```sh
# One-time toolchain setup
rustup target add wasm32-wasip2

# Run native unit tests on host with full std
cargo test

# Build zero-WASI WebAssembly component (no external tools)
cargo build --target wasm32-wasip2 --release
```

### Interaction with ADR 0004 (`#![forbid(unsafe_code)]`)

Two separate `unsafe` boundaries matter:

1. **Inside the `codeint` workspace**:
   `codeint-plugin-sdk` requires `unsafe` in three places: its `wasip2` runtime module (`cabi_realloc`, `mem*`, allocator wiring), `#[unsafe(export_name = ...)]` / `#[unsafe(no_mangle)]` (required by Rust 2024 edition), and the code emitted by `wit_bindgen::generate!`.
   [ADR 0004](../adr/0004-forbid-unsafe-code.md) currently forbids `unsafe` in every crate root and explicitly notes that a future need for `unsafe` requires a new ADR.
   Shipping `codeint-plugin-sdk` in the workspace therefore requires either a narrow superseding ADR permitting `unsafe` solely in the guest-side `codeint-plugin-sdk` runtime crate (keeping all host crates `#![forbid(unsafe_code)]`), or housing the SDK in a separate package workspace.
2. **Inside third-party Plugin crates**:
   According to the Rust Reference on lint check attributes, `#![forbid(unsafe_code)]` cannot be overridden by `#[allow(unsafe_code)]` inside a macro expansion.
   Because `wit-bindgen`'s `export!` macro expands `unsafe` blocks and `#[unsafe(export_name = ...)]` at the invocation site, a Plugin crate with `#![forbid(unsafe_code)]` at the crate root will fail to compile when it calls `export_language_plugin!`.
   Plugin authors who want strict `unsafe` rejection can either:
   - Place `#![deny(unsafe_code)]` at the crate root and let the SDK export macro wrap its generated `const _: () = { ... };` block in `#[allow(unsafe_code)]`, or
   - Put all Plugin logic inside an inner `mod plugin` marked `#![forbid(unsafe_code)]` and keep only the `export_*!` invocation in `src/lib.rs`.

## Allocator, binary size, and memory behavior under the 64 MiB cap

### Why `BumpAllocator` is unsafe for real Plugins

`crates/plugin-host/src/wasm.rs` pools component instances in `InstancePool` and reuses each instance across sequential calls.
In `sample-plugin` and `probe-runtime`, `BumpAllocator::dealloc` is a no-op.
Every `lower(path, text)` call allocates the input strings, intermediate parse buffers, and the returned `FileModel` in linear memory; when `wit-bindgen`'s generated `cabi_post_*` runs `Cleanup::drop` and calls `alloc::alloc::dealloc`, the bump allocator discards the call.
Across a repository with thousands of files, a pooled instance leaks monotonically until `wasm32::memory_grow` hits codeint's 64 MiB `StoreLimits` cap and traps with `MemoryLimitExceeded`.

### Freeing allocator comparison for `wasm32-wasip2` `no_std`

| Allocator | Crate & API (checked 2026-11-25) | Allocation performance | Binary size impact | Fragmentation under 64 MiB cap | Trade-offs |
| --------- | -------------------------------- | ---------------------- | ------------------ | ------------------------------ | ---------- |
| **`talc`** | `talc` v5.1+ (`talc::wasm::WasmDynamicTalc` via `new_wasm_dynamic_allocator()`) | Fast (segregated free lists, single-threaded `TalcSyncCell`) | Small (~a few KiB; optional `disable-grow-in-place` and `disable-realloc-in-place` features trim size further) | Low (`WasmGrowAndExtend` default since v5.1.0 avoids `Vec` growth bloat) | **Best technical fit**; newer third-party maintainer compared to `rust-lang/dlmalloc-rs` |
| **`dlmalloc`** | `dlmalloc` v0.2 (`dlmalloc::GlobalDlmalloc` with `features = ["global"]`) | Moderate to fast | Larger (~10-20+ KiB over bare module) | Low (mature boundary-tag coalescing) | **Lowest supply-chain risk** (maintained under `rust-lang/dlmalloc-rs`, identical to `wasm32-unknown-unknown`'s `std` allocator) |
| **`lol_alloc`** | `lol_alloc` (`AssumeSingleThreaded<FreeListAllocator>`) | Slow under churn (`O(n)` free list) | Minimal (hundreds of bytes) | Moderate | Poor fit for token-heavy Language plugins with thousands of `Vec` / `String` allocations per file |
| **`BumpAllocator`** | Hand-written (current fixtures) | Fastest per-alloc | Minimal | **Unbounded leak** across `InstancePool` calls | **Unsuitable** for real Plugins; safe only for tiny tests |

**Recommendation**: Use `talc` v5.1+ (`WasmDynamicTalc`) if binary size and allocation throughput lead, or `dlmalloc` v0.2 (`GlobalDlmalloc`) if minimizing non-`rust-lang` dependencies leads.
Either allocator eliminates the `InstancePool` leak and reclaims freed linear memory immediately between files.

### Binary size and Wasmtime 64 MiB memory behavior

- **Observed baseline sizes in this checkout**: Unoptimized debug builds in `target/wasm-fixtures/wasm32-wasip2/debug/` measure 270 KiB for `sample_mini_plugin.wasm` and 180 KiB for `sample_long_functions_plugin.wasm`.
  Release builds with `opt-level = "s"`, `lto = true`, `panic = "abort"`, and `strip = true` drop this substantially (typical minimal `no_std` components land in the tens of KiB).
- **Wasm linear memory does not shrink**: WebAssembly `memory.grow` expands linear memory in 64 KiB pages and never returns pages to the host OS while an instance remains checked into `InstancePool`.
  The 64 MiB `StoreLimits` cap therefore constrains each instance's **peak live working set plus fragmentation**, not cumulative allocations across a run, as long as the guest uses a freeing allocator.
- **Lifted-budget ceiling**: `crates/plugin-host/src/limits.rs` also caps each call's lifted return value to `memory_bytes` (64 MiB) and `memory_bytes / 32` elements (`enforce_lifted_budget`) plus `set_hostcall_fuel(memory_bytes)`, so a single file's `FileModel` or `CodeModel` stays bounded even before linear memory fills.
- **Optimization tip for `mem*`**: Enabling `+bulk-memory` (standard in modern Wasmtime and `wasm32-wasip2`) allows LLVM to emit native `memory.copy` and `memory.fill` instructions instead of calling byte-loop `memcpy`/`memset` shims for most operations, keeping the SDK's C-intrinsic shims as fallback symbols rather than hot paths.

## Ecosystem trade-offs and gotchas

1. **`serde_json` works in `no_std` + `alloc`**:
   Use `serde_json = { version = "1", default-features = false, features = ["alloc"] }` and `serde = { version = "1", default-features = false, features = ["derive", "alloc"] }`.
   Dynamic `Value`, `from_str`, and `to_string` all work without `std`.
2. **`regex` requires `std`**:
   The top-level `regex` crate does not support `no_std`.
   Authors writing a `no_std` `wasm32-wasip2` Plugin must either write a scanner/lexer directly, use `regex-automata` with precompiled/deserialized automata, or switch to the `wasm32-unknown-unknown` (`std`) + `wasm-tools component new` build path.
3. **`HashMap` vs `BTreeMap` and host entropy (`getrandom`)**:
   `std::collections::HashMap` is unavailable in `no_std`, and crates that depend on `getrandom` (such as `ahash` or `hashbrown` with default random state) fail or pull in WASI `wasi:random` imports that violate codeint's empty `Linker`.
   Prefer `alloc::collections::BTreeMap` and `BTreeSet` (which also guarantee deterministic output ordering for `Findings` and `FileModel`), or use `hashbrown` with a deterministic non-random hasher (such as `foldhash` or `rustc-hash` with fixed seed).
4. **C-backed parsers (such as `tree-sitter`)**:
   C parsers compiled for `wasm32-wasip2` link against `wasi-libc`, pulling in WASI imports unless cross-compiled with custom freestanding C shims.
   Pure-Rust parsers (`no_std` + `alloc` or `wasm32-unknown-unknown` `std`) avoid C runtime imports cleanly.
5. **Supporting both targets in one SDK crate**:
   Because `codeint-plugin-sdk` only gates its `no_std` runtime shim on `#[cfg(all(target_arch = "wasm32", target_os = "wasi", target_env = "p2"))]`, adding an optional `std` Cargo feature lets authors start with zero-tool `wasm32-wasip2` (`no_std`) and move to `wasm32-unknown-unknown` (`std`) + `wasm-tools component new` without rewriting any Plugin code if they later need `regex` or a `std`-only parser crate.

## Primary sources

All sources checked on `2026-11-25`:

- `wit-bindgen` 0.63.0 crate documentation (`generate!` options `pub_export_macro`, `default_bindings_module`, `runtime_path`, `export_macro_name`, `type_section_suffix`): <https://docs.rs/wit-bindgen/0.63.0/wit_bindgen/>
- `wit-bindgen` repository and installed 0.63.0 source (`Cargo.toml`, `build.rs`, `src/rt/mod.rs`, `wit-bindgen-rust/src/lib.rs`): <https://github.com/bytecodealliance/wit-bindgen>
- Rust compiler platform support for `wasm32-wasip2`: <https://doc.rust-lang.org/rustc/platform-support/wasm32-wasip2.html>
- Rust compiler platform support for `wasm32-unknown-unknown`: <https://doc.rust-lang.org/rustc/platform-support/wasm32-unknown-unknown.html>
- Rust compiler platform support for `wasm32-none-component` (Tier 3): <https://doc.rust-lang.org/rustc/platform-support/wasm32-none-component.html>
- The Rust Reference - Diagnostic attributes (`forbid` cannot be overridden in macros): <https://doc.rust-lang.org/reference/attributes/diagnostics.html>
- The Rust Reference - Linkage (`cdylib`): <https://doc.rust-lang.org/reference/linkage.html>
- Bytecode Alliance Component Model - Rust language support: <https://component-model.bytecodealliance.org/language-support/rust.html>
- `wasm-component-ld` repository and bundled `wasm-component-ld 0.5.27 --help`: <https://github.com/bytecodealliance/wasm-component-ld>
- `wasm-tools` repository (`wasm-tools component new` / `wit-component`): <https://github.com/bytecodealliance/wasm-tools>
- `wasmtime` 51.0.0 `component::Linker`: <https://docs.rs/wasmtime/latest/wasmtime/component/struct.Linker.html>
- `wasmtime` 51.0.0 `StoreLimitsBuilder`: <https://docs.rs/wasmtime/latest/wasmtime/struct.StoreLimitsBuilder.html>
- `talc` WebAssembly allocator documentation (`talc::wasm`, `WasmDynamicTalc`, `WasmGrowAndExtend`): <https://docs.rs/talc/latest/talc/wasm/index.html> and <https://github.com/SFBdragon/talc>
- `dlmalloc` allocator documentation (`GlobalDlmalloc`, `global` feature): <https://docs.rs/dlmalloc/latest/dlmalloc/struct.GlobalDlmalloc.html> and <https://github.com/rust-lang/dlmalloc-rs>
- `serde_json` `no_std` + `alloc` documentation: <https://docs.rs/serde_json/latest/serde_json/>
- `regex` crate `std` requirement and `regex-automata` `no_std` notes: <https://docs.rs/regex/latest/regex/> and <https://docs.rs/regex-automata/latest/regex_automata/>
- codeint repository files inspected at commit `73fa4543a74a8711698e5415d20d56c6b5a0d6c2`: `docs/plugins.md`, `docs/adr/0002-plugins-as-wasm-components.md`, `docs/adr/0004-forbid-unsafe-code.md`, `wit/language.wit`, `wit/metric.wit`, `crates/plugin-host/src/wasm.rs`, `crates/plugin-host/src/limits.rs`, `crates/config/src/codeint_json.rs`, `crates/plugin-host/tests/fixtures/sample-plugin/src/lib.rs`, `crates/plugin-host/tests/fixtures/sandbox-probes/probe-runtime/src/lib.rs`.
