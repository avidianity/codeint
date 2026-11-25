---
status: accepted
---

# Framework profiles live inside Language plugins as package-level rule sets

## Context

Modern applications built on TypeScript, JavaScript, PHP, Python, and Dart follow framework conventions that sit above the base language grammar.
Frameworks declare convention entry points (such as `src/main.tsx` or `vue.config.js`), emit framework-generated files (such as `auto-imports.d.ts`), define fallback import aliases (such as `@/*` or `$lib/*`), and carry calibrated Reference ranges (such as React ADR 0020, Vue 2 ADR 0014, and Vue 3 ADR 0015).

Previously, React and Vue Reference ranges were wired through a temporary host helper in `crates/plugin-host` that inspected manifests directly for `typescript-javascript`.
That approach could not scale to additional frameworks, could not supply framework entry points, generated paths, or fallback import hints, and could not work for external WebAssembly Language plugins.

## Decision

**Each framework is modeled as a Framework profile inside its owning Language plugin, keeping per-file lowering framework-blind and executing package-level rules through the WIT `0.4.0` `known-framework-profiles` and `resolve-framework-profiles` exports.**

1. **Per-file lowering remains framework-blind**:
   Stateless syntax extraction (such as `.vue` and `.svelte` `<script>` block extraction in `sfc.rs`) stays in `lower` so source files still parse cleanly when no framework manifest is present or when Framework profiles are disabled.
2. **Isolated per-framework modules**:
   Inside each Language plugin, each Framework profile lives in its own isolated module (for example `plugins/languages/ts/src/frameworks/{react,vue2,vue3,svelte}.rs`), and sibling framework modules never import one another.
3. **Hoisted-monorepo isolation and library safety**:
   The host passes each Workspace package only its own files and manifests.
   A monorepo root manifest never activates a Framework profile on a child Workspace package without a local signal, and `peerDependencies`-only UI libraries do not activate app-only conventions (while `svelte` permits `devDependencies`).
4. **Parent-child precedence and sibling union**:
   When a child framework activates alongside its parent in one Workspace package, the child wins Reference range selection while entry points, generated paths, and fallback import hints union across all active profiles.
   When two unrelated sibling profiles both supply Reference ranges in the same scope (such as React and Vue), their conventions still union cleanly while Reference ranges fall back to the base language ranges with a clear `reference_range_note`.
5. **Strict four-layer Source set priority**:
   Source sets settle in exact order from lowest to highest:
   - Per-file lowering by the Language plugin.
   - Core Vendored (`vendor/`, `third_party/`) and standard build-folder (`dist/`, `build/`, `out/`, `coverage/`, minified lines) rules.
   - Framework profile `generated_paths`, which reclassify matching non-Vendored files to `Generated` and never override `Vendored`.
   - Explicit `source_sets` overrides in `codeint.json`, which always win.
6. **Global and per-package configuration with fail-fast validation**:
   Users may disable all profiles (`false`), re-enable auto-detection (`true`), allow-list profiles (`["react"]` or `{"only": ["react"]}`), or exclude profiles (`{"exclude": ["vue3"]}`) globally under `framework_profiles` and per Workspace package under `packages.<path>.framework_profiles`.
   Configured profile names are validated against the union of `known-framework-profiles()` from all loaded Language plugins (built-in and external), failing fast with exit code 2 on unknown names.
7. **Report visibility**:
   Active Framework profile names are emitted as sorted `framework_profiles` lists per language and per Workspace package across JSON, TOON, Markdown, and text reports, omitted when empty.

## Options rejected

- **Separate WebAssembly Language plugins per framework**:
  Rejected because React, Vue, Svelte, and plain TypeScript share the same extensions (`.ts`, `.tsx`, `.js`, `.jsx`) and cross-file import graph.
  Splitting them would break cross-file Import cycles, Orphan files, and Boundaries across mixed packages.
- **Keep framework detection in the host (`crates/plugin-host`)**:
  Rejected because it violates [ADR 0002](./0002-plugins-as-wasm-components.md) by duplicating language-specific manifest logic in the core host and prevents external Language plugins from shipping Framework profiles.

## Consequences

- `crates/plugin-host` removes its temporary React and Vue Reference range helpers and routes every built-in and external Language plugin through the uniform WIT `0.4.0` Framework profile contract.
- Disabling a Framework profile cleanly turns off its convention entry points, generated paths, fallback import hints, and framework Reference ranges in one place.
- Future Framework profiles (Next.js, Nuxt, SvelteKit, Angular, NestJS, Laravel, Symfony, CakePHP, CodeIgniter, Django, FastAPI, Flask, and Flutter) drop directly into their owning Language plugins without changing the host or WIT contract.
