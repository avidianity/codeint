# codeint Guide

Welcome to the modular user guide for codeint.
Each page below focuses on one topic and links to [docs/reference.md](../reference.md) for exact command flags, exit codes, and `codeint.jsonc` / `codeint.json` schema keys.

## Reading map

1. [Philosophy and Architecture](./philosophy-and-architecture.md): why codeint measures Software entropy, how its independent Metrics (including Hazards) and structural checks work, and how the codebase is structured.
2. [Getting Started](./getting-started.md): install codeint, initialize `codeint.jsonc`, and run `check`, `diff`, `trend`, `doctor`, `config`, `report`, `guard`, `trace`, `impact`, `churn`, and `coupling`.
3. [Configuration (`codeint.jsonc` / `codeint.json`)](./configuration.md): configure JSONC files, Budgets, Regression limits, Source-set globs, architectural boundaries, near-clone detection, entry points, dependency hygiene, Coverage reports, SQL Dialects, Hazards disabled rules, and multi-language folders.
4. [Import boundaries](./import-boundaries.md): define architectural zones, `public_files` encapsulation, built-in `layered` and `hexagonal` presets, `allow_external` / `deny_external` third-party package globs, and pre-edit `codeint guard` checks.
5. [Test gap](./test-gap.md): measure untested Production Function mass from LCOV and Cobertura XML Coverage reports, set up each test tool, and enforce a Test gap Budget.
6. [Hazards](./hazards.md): measure Hazard lines next to Software entropy with strict non-leakage, disable named rules, and enforce Budgets and Regression limits.
7. [Built-in Languages](./languages.md): how TypeScript/JavaScript (with Vue and Svelte), Rust, PHP, Dart, Python, Go, and SQL are measured, plus their calibrated Reference ranges.
8. [CI and GitHub Actions](./ci-and-github-actions.md): gate pull requests with the reusable GitHub Action, single-scan multi-format `codeint report` workflows, `CODEOWNERS` scoping, and SARIF upload.
9. [Adopting codeint on an existing codebase](./adopting-on-an-existing-codebase.md): freeze legacy Findings with `codeint init --adopt`, then stop new decay with `.codeint/ratchet.json`, `.codeint/baseline.json`, zero Budgets, `new_file_budgets`, and `per_file` checks.
10. [Coding-Agent Integration (MCP Server)](./mcp.md): set up `codeint mcp` with `codeint agent install`, inspect capabilities with `codeint schema`, and call all eight MCP tools.
11. [Programmatic API](../programmatic.md): call codeint from JavaScript, TypeScript, or Rust.
12. [Plugins](../plugins.md): build external WebAssembly Language and Metric plugins with `codeint-plugin-sdk` (`wasm32-wasip2` `no_std` or `wasm32-unknown-unknown` `std`), pin SHA-256 digests, and test them.

## Reference and domain docs

- [CLI and Configuration Reference](../reference.md): tested reference for every command, option, exit code, report format, and schema key.
- [Glossary](../../GLOSSARY.md): canonical definitions of every domain term.
- [Architecture Decision Records](../adr/0001-review-mode-never.md): recorded design decisions and calibrated Reference range panels.
- [llms.txt](../../llms.txt): compact documentation index for coding agents and LLM tools.
