# How to change the rules of the Dart Language plugin

## Overview

`plugins/languages/dart` (`codeint-lang-dart`, plugin name `dart`) is a built-in Language plugin that lowers `.dart` files into the language-neutral Code model using a pure-Rust `#![forbid(unsafe_code)]` lexer and structural analyzer.

## Source sets and workspaces

- `src/source_set.rs` classifies a `.dart` file as `Generated` when its filename ends with a standard Dart or Flutter generated suffix (`.g.dart`, `.freezed.dart`, `.gr.dart`, `.mocks.dart`, `.pb.dart`, `.pbenum.dart`, `.pbjson.dart`, `.pbserver.dart`) or when a leading comment contains a generated-file marker, `Test` when its path is under `test/`, `tests/`, or `integration_test/` or ends with `_test.dart`, and `Production` otherwise (`vendor/` and `third_party/` are settled by the core).
- `src/workspaces.rs` reads `workspace` entries from the root `pubspec.yaml` to report Workspace packages.

## Functions, tokens, and Flagged lines

- `src/functions.rs` extracts top-level functions, methods, getters, constructors, and closures (block and `=>` expression bodies) and counts decision points (`if`, `for`, `while`, `do`/`while`, switch `case`, Dart 3 `switch` expression arms minus one, `catch`, `&&`, `||`, `??`, and ternary `?`).
- `src/imports.rs` marks `library`, `import`, `export`, `part`, and `part of` directives as `TokenKind::ImportOrExport`.
- `src/rules.rs` implements five zero-false-positive structural Flagged-line rules: `else-after-return`, `if-else-returns-boolean`, `catch-only-rethrows`, `ternary-yields-boolean`, and `trailing-bare-return`.

## Imports and Pub resolution

- `src/imports.rs` and `src/resolution.rs` extract `import`, `export` (including conditional `if (...)` URIs), and `part` URIs (ignoring `part of` back-references to avoid false cycles), resolving relative URIs and `package:<name>/...` URIs through `pubspec.yaml` `name` -> `lib/`, classifying `dart:*` URIs as `Builtin`, and classifying third-party `package:<name>/...` URIs as `External(<name>)`.
- `src/entry_points.rs` registers public `lib/**/*.dart` libraries outside `lib/src/` (or top-level `lib/*.dart` for `lib/main*.dart` applications), `bin/*.dart`, declared `executables`, and native-asset hooks (`hook/build.dart`, `hook/link.dart`) per `pubspec.yaml`.
- `src/dependencies.rs` parses `pubspec.yaml` `dependencies` (`Runtime`) and `dev_dependencies` (`Dev`) per Workspace package while excluding local workspace package names.

## Reference ranges

- `reference_ranges` returns the 10th to 90th percentile bands from the 20-repository pre-2023 Dart panel recorded in [ADR 0012](../../../docs/adr/0012-dart-reference-ranges.md).
