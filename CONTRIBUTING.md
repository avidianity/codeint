# Contributing to codeint

Thank you for your interest in improving codeint.
Whether you are fixing a bug, improving documentation, or adding a Language or Metric feature, this guide explains how to get set up and how changes move through this repository.

## Built with `avidianity/factory`

This repository is developed using [`avidianity/factory`](https://github.com/avidianity/factory), an opinionated workflow kit created personally by the author for building software with AI coding agents.
It brings together the repository's rules (`.rules`), automated checks (`xtask`, Git hooks, and `checks.toml`), agent skills, and review loops.

Contributors and `codeint` users are **not** expected or required to use `avidianity/factory` in their own projects.
It is simply how `codeint` itself is built.
If you are curious about structured AI-assisted software development, feel free to explore it and borrow any ideas that fit your own workflow.

## Prerequisites

- Rust via rustup ([rust-toolchain.toml](./rust-toolchain.toml) pins the toolchain version).
- Node.js 24 and npm (for Markdown linting and repository tools in [package.json](./package.json)).
- `gitleaks` and `lychee` on your `PATH` (used by Git hooks).

## Local setup

Clone the repository, install the npm tools and Git hooks, and run the affected test loop:

```sh
npm ci
npx lefthook install
cargo xtask affected-tests
```

For day-to-day development, run `cargo xtask affected-tests` or targeted crate tests (for example `cargo test --locked -p codeint-cli --test reference`) rather than running the entire workspace suite locally on every edit.
CI runs the full workspace suite (`cargo test --workspace --locked`) on every pull request.

Never skip Git hooks.
All repository checks are documented in [docs/checks/README.md](./docs/checks/README.md).

## Making a change

People and coding agents follow the same rules in this repository: read [.rules](./.rules) and [GLOSSARY.md](./GLOSSARY.md) before making changes.

1. Open an issue first for new features or behavior changes so the scope and design are clear.
2. Create a short-lived branch from `main`.
3. Write tests first for behavior changes, and keep code comments within [docs/checks/no-comments.md](./docs/checks/no-comments.md).
4. Update [docs/reference.md](./docs/reference.md) and the affected user-facing docs (`README.md`, `docs/guide/`, `llms.txt`, `GLOSSARY.md`) in the same change whenever user-visible commands, Metrics, or `codeint.jsonc` / `codeint.json` options change.
5. Use Conventional Commits (`feat:`, `fix:`, `docs:`, `refactor:`, `chore:`) with no agent attribution or co-author lines.
6. Open a pull request against `main` and fill in every section of [.github/pull_request_template.md](./.github/pull_request_template.md).

## Running the Evaluation suite

The Evaluation suite measures accuracy, native-parser recall, speed, and memory on the public repositories pinned in [eval/corpus.json](./eval/corpus.json).
Its runner clones each repository into a temporary folder outside the checkout and deletes it after measuring.
Read [eval/HOW-TO.md](./eval/HOW-TO.md) for the command, the oracle runtimes, and the report files.
The `corpus-eval` workflow runs it in CI on push to `main`, weekly, and on manual dispatch, never on pull requests.
The planted-defect suite runs with the other tests: `cargo test --locked -p codeint-cli --test integration planted_defects`.

## Adding a Language or Metric plugin

Read [docs/plugins.md](./docs/plugins.md) to build an external WebAssembly Plugin against `codeint:plugin@0.5.0`, or inspect `plugins/languages/` to see how built-in Language plugins implement the Code model contract.

## Code of Conduct

All contributors follow [CODE_OF_CONDUCT.md](./CODE_OF_CONDUCT.md).
