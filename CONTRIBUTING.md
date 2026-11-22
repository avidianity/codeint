# Contributing to codeint

Thank you for your interest in improving codeint.
People and coding agents follow the same rules in this repository: read [.rules](./.rules) and [GLOSSARY.md](./GLOSSARY.md) before making changes.

## Prerequisites

- Rust via rustup ([rust-toolchain.toml](./rust-toolchain.toml) pins the version).
- Node.js 24 and npm (for formatting and documentation tools in [package.json](./package.json)).
- `gitleaks` and `lychee` on your `PATH` (used by Git hooks).

## Local setup

Clone the repository and install hooks and tests:

```sh
npm ci
npx lefthook install
cargo test --workspace --locked
```

Never skip Git hooks.
All repository checks are documented in [docs/checks/README.md](./docs/checks/README.md).

## Making a change

1. Open an issue first for new features or behavior changes so the scope and design are clear.
2. Create a short-lived branch from `main`.
3. Write tests first for behavior changes, and keep code comments within [docs/checks/no-comments.md](./docs/checks/no-comments.md).
4. Use Conventional Commits (`feat:`, `fix:`, `docs:`, `refactor:`, `chore:`) with no agent attribution lines.
5. Open a pull request against `main` and fill in every section of [.github/pull_request_template.md](./.github/pull_request_template.md).

## Adding a Language or Metric plugin

Read [docs/plugins.md](./docs/plugins.md) to build an external WebAssembly Plugin, or inspect `plugins/languages/` for how built-in Language plugins implement the Code model contract.

## Code of Conduct

All contributors follow [CODE_OF_CONDUCT.md](./CODE_OF_CONDUCT.md).
