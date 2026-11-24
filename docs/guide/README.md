# codeint Guide

Welcome to the modular user guide for codeint.
Each page below focuses on one topic and links to [docs/reference.md](../reference.md) for exact command flags, exit codes, and `codeint.jsonc` / `codeint.json` schema keys.

## Reading map

1. [Philosophy and Architecture](./philosophy-and-architecture.md): why codeint measures Software entropy, how its independent Metrics and structural checks work, and how the codebase is structured.
2. [Getting Started](./getting-started.md): install codeint, initialize `codeint.jsonc`, and run `check`, `diff`, `trend`, `doctor`, `config`, `report`, `guard`, `trace`, `impact`, `churn`, and `coupling`.
3. [Configuration (`codeint.jsonc` / `codeint.json`)](./configuration.md): configure JSONC files, Budgets, Regression limits, Source-set globs, architectural boundaries, near-clone detection, entry points, dependency hygiene, Coverage reports, and multi-language folders.
4. [Test gap](./test-gap.md): measure untested Production Function mass from LCOV and Cobertura XML Coverage reports, set up each test tool, and enforce a Test gap Budget.
5. [Built-in Languages](./languages.md): how TypeScript/JavaScript (with Vue and Svelte), Rust, PHP, Dart, Python, and Go are measured, plus their calibrated Reference ranges.
6. [Go](./go.md): Go terminology mapping, Source sets, Flagged line rules, multi-file Go package imports, entry points, and `go.mod` Dependency hygiene.
7. [CI and GitHub Actions](./ci-and-github-actions.md): gate pull requests with the reusable GitHub Action, single-scan multi-format `codeint report` workflows, `CODEOWNERS` scoping, and SARIF upload.
8. [Coding-Agent Integration (MCP Server)](./mcp.md): set up `codeint mcp` with `codeint agent install`, inspect capabilities with `codeint schema`, and call all eight MCP tools.
9. [Programmatic API](../programmatic.md): call codeint from JavaScript, TypeScript, or Rust.
10. [Plugins](../plugins.md): build, hash, declare, and test external WebAssembly Language and Metric plugins.

## Reference and domain docs

- [CLI and Configuration Reference](../reference.md): tested reference for every command, option, exit code, report format, and schema key.
- [Glossary](../../GLOSSARY.md): canonical definitions of every domain term.
- [Architecture Decision Records](../adr/0001-review-mode-never.md): recorded design decisions and calibrated Reference range panels.
- [llms.txt](../../llms.txt): compact documentation index for coding agents and LLM tools.
