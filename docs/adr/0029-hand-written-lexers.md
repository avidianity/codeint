---
status: accepted
---

# Hand-written lexers for PHP, Dart, Python, and Go Language plugins

## Context

codeint's built-in TypeScript/JavaScript plugin uses `oxc`, Rust uses `syn`, and SQL uses `sqlparser`, because those three crates are pure Rust, fast, zero-`unsafe`, and match codeint's binary-size and build-time constraints.
For PHP, Dart, Python, and Go, mature pure-Rust full AST parser crates either do not exist, pull in C toolchains (`tree-sitter` grammars violating [ADR 0004](./0004-forbid-unsafe-code.md) and complicating cross-compilation), or inflate binary size and compile time beyond the tool's budget.
Meanwhile, codeint's Code model requires only source lines, function boundaries and names, cyclomatic and cognitive decision points, normalized token streams, runtime imports, Source set classification, and language-specific Flagged line rules.

## Decision

**Keep lightweight, zero-`unsafe`, hand-written lexers and structural scanners inside `plugins/languages/php`, `plugins/languages/dart`, `plugins/languages/python`, and `plugins/languages/go`, and continuously verify their accuracy against each language's official native parser in the Evaluation suite.**

1. **Zero external parser crates or C FFI**:
   None of `codeint-lang-php`, `codeint-lang-dart`, `codeint-lang-python`, or `codeint-lang-go` depends on `tree-sitter` or C bindings; all four retain `#![forbid(unsafe_code)]`.
2. **Native-parser recall bar**:
   Each hand-written lexer is audited against its language's official native parser running in an ephemeral scratch environment outside the repository (`PhpToken::tokenize` for PHP, `package:analyzer` for Dart, standard library `ast` for Python, and `go/parser` for Go), enforcing at least 98% recall on functions, decision points, and runtime imports and under 2% false positives on Flagged lines.
3. **Synthetic regression tests**:
   Every edge case discovered during scratch-folder corpus audits is fixed with original Rust code and locked down by owned synthetic regression tests inside the plugin crate without copying external code.

## Considered options

- **Adopt `tree-sitter` grammars for PHP, Dart, Python, and Go.**
  Rejected: introduces C compilation and FFI `unsafe` code, complicates cross-platform release builds (including Windows), and increases binary size and scan latency.
- **Spawn each language's native interpreter or compiler during `codeint check`.**
  Rejected: violates codeint's zero-runtime-dependency single-binary design and adds unacceptable per-file process overhead.

## Consequences

- `codeint` stays a single self-contained, `#![forbid(unsafe_code)]` Rust binary with fast scan latency across all built-in languages.
- Lexer accuracy is measured objectively and continuously in `.github/workflows/corpus-eval.yml` against official native parsers rather than assumed.
- Adding new syntax constructs in future PHP, Dart, Python, or Go releases requires targeted scanner updates and synthetic tests rather than waiting on third-party parser crate releases.
