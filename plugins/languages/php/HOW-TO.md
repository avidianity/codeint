# How to change the rules of the PHP Language plugin

## Overview

`plugins/languages/php` (`codeint-lang-php`, plugin name `php`) is a built-in Language plugin that lowers `.php` files into the language-neutral Code model using a pure-Rust `#![forbid(unsafe_code)]` lexer and structural analyzer.

## Source sets and workspaces

- `src/source_set.rs` classifies a `.php` file as `Generated` when a leading comment before the first PHP code token contains a generated-file marker, `Test` when its path is under `tests/` or `test/` or its name ends in `Test.php` or `.test.php`, and `Production` otherwise (`vendor/` is settled by the core).
- `src/workspaces.rs` reads local Composer `repositories` entries of `"type": "path"` from the root `composer.json` to report Workspace packages.

## Functions, tokens, and Flagged lines

- `src/functions.rs` extracts `function` definitions, closures, and PHP arrow functions (`fn(...) => ...`) and counts decision points (`if`, `elseif`, `for`, `foreach`, `while`, `case`, `catch`, `&&`, `||`, `and`, `or`, `??`, ternary `?`, and `match` arms minus one).
- `src/imports.rs` marks `namespace` and top-level `use` declarations as `TokenKind::ImportOrExport`.
- `src/rules.rs` implements six zero-false-positive structural Flagged-line rules: `else-after-return`, `if-else-returns-boolean`, `catch-only-rethrows`, `ternary-yields-boolean`, `trailing-bare-return`, and `constructor-only-forwards`.

## Imports and PSR-4 resolution

- `src/imports.rs` and `src/resolution.rs` extract top-level `use` statements (including grouped and aliased imports) and static relative `require`/`include` paths, resolving PSR-4 namespaces through `composer.json` `autoload.psr-4` and `autoload-dev.psr-4` (falling back to declared Composer sub-packages when no local file or directory matches), classifying PHP built-in classes, built-in namespaces, and single-segment global symbols as `Builtin`, and normalizing multi-segment external namespaces to Composer `vendor/package` names.
- `src/dependencies.rs` parses `composer.json` `require` (`Runtime`), `require-dev` (`Dev`), and `suggest` (`Optional`) dependencies per Workspace package while excluding platform requirements (`php`, `ext-*`, `lib-*`) and local workspace package names.

## Reference ranges

- `reference_ranges` returns the 10th to 90th percentile bands from the 20-repository pre-2023 PHP panel recorded in [ADR 0011](../../../docs/adr/0011-php-reference-ranges.md).
