---
status: accepted
---

# Narrow ADR 0004 amendment for `codeint-plugin-sdk` guest runtime shim

## Context

[ADR 0004](./0004-forbid-unsafe-code.md) requires `#![forbid(unsafe_code)]` in every crate root across codeint so the host, core crates, CLI, and built-in Language plugins contain zero `unsafe` blocks.
When shipping the official Rust Plugin SDK (`codeint-plugin-sdk`) for external WebAssembly Language and Metric plugins targeting `wasm32-wasip2` (`no_std`) and `wasm32-unknown-unknown`, `no_std` WebAssembly guests require an unavoidable low-level guest runtime shim implementing `GlobalAlloc`, `cabi_realloc`, `memcmp`, `memcpy`, `memmove`, `memset`, and `wit_bindgen::generate!` FFI exports.
Without an SDK crate encapsulating that runtime shim, every external plugin author must hand-roll identical `unsafe` boilerplate in their own crate.

## Decision

**Amend [ADR 0004](./0004-forbid-unsafe-code.md) narrowly so that `codeint-plugin-sdk`'s isolated WebAssembly guest runtime shim module (and the generated `wit_bindgen` export bridge) may use the minimal `unsafe` required by Rust's `GlobalAlloc`, `#[unsafe(export_name = "cabi_realloc")]`, `#[unsafe(no_mangle)]` `mem*` intrinsics, and Component Model guest bindings, while every other workspace crate continues to enforce `#![forbid(unsafe_code)]`.**

1. **Strict boundary**:
   Only the dedicated guest runtime shim module inside `codeint-plugin-sdk` may contain `unsafe` code; all safe SDK traits, helper types, and conversion logic outside that module remain safe Rust.
2. **Host, core, CLI, and built-in plugins unchanged**:
   Every host, core, CLI, `xtask`, and built-in Language plugin crate continues to declare `#![forbid(unsafe_code)]` at its crate root with zero exceptions.
3. **No comment-rule relaxation**:
   `xtask no-comments` remains unchanged; the guest runtime shim keeps its `unsafe` surface minimal, auditable, and strictly scoped to WebAssembly linear-memory allocation and ABI shims.

## Considered options

- **Keep `#![forbid(unsafe_code)]` everywhere and omit `no_std` guest runtime support from `codeint-plugin-sdk`.**
  Rejected: forces every external `no_std` Rust plugin author to duplicate ~150 lines of error-prone allocator and `cabi_realloc` boilerplate.
- **Relax ADR 0004 workspace-wide.**
  Rejected: weakens memory-safety guarantees across the host and built-in plugins where safe dependencies already suffice.

## Consequences

- External Rust plugin authors write 100% safe Rust against `codeint-plugin-sdk` in both `no_std` (`wasm32-wasip2`) and `std` modes.
- Every crate in the codeint host binary retains `#![forbid(unsafe_code)]`.
- Security audits of `unsafe` code remain bounded to one isolated guest runtime shim module inside `codeint-plugin-sdk`.
