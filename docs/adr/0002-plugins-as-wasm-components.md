---
status: accepted
---

# Plugins are WebAssembly components behind a WIT interface

The Owner decided this on 2026-11-21 while designing the Plugin system before the first line of code.

## Context

codeint ships as prebuilt binaries through npm, GitHub Releases, and an install script.
Users cannot recompile codeint, so third-party Plugins must load at runtime.
Rust has no stable ABI.
A Plugin may come from an untrusted author and must not read files or reach the network on its own.
Checked on 2026-11-21: `wasmtime` 50.0.3 and `wit-bindgen` 0.63.0 on crates.io.

## Decision

**The Plugin interface is defined once in WIT, and external Plugins load as WebAssembly components through wasmtime.**
Built-in Plugins, including the TypeScript and JavaScript Language plugin and the v1 Metric plugins, compile natively against the same WIT interface, so the interface is exercised by every run.
A Plugin gets no file system, network, or clock access unless codeint grants it.

## Considered options

- **Dynamic libraries.**
  Rejected: no stable Rust ABI, one build per OS and architecture, and no sandbox.
- **Subprocess with JSON over standard input and output.**
  Rejected: serializing a large Code model on every run is slow.
- **Compiled-in Plugins only.**
  Rejected: a third party could add a language or Metric only by forking codeint.

## Consequences

- One `.wasm` file runs on every supported platform.
- Plugin authors can use any language that targets WebAssembly components.
- The binary grows, and the Code model is copied across the component boundary.
- The WIT interface is a public contract; a breaking change needs a new interface version.
- The Plugin host is tier 3, because its capability grants are a security boundary.
- Amended 2026-11-21: codeint is language-neutral, and TypeScript and JavaScript are only the first Language plugin.
  Every rule that differs between languages lives in a Language plugin, so a new language never changes the core or a Metric plugin.
- Amended 2026-11-21: every Plugin runs under a memory cap and a time limit, with defaults set by measurement and raised in configuration.
  A Language plugin that hits a limit on a file skips that file, which the report lists; a Metric plugin that hits a limit stops the run with an operational error.
