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

- `src/imports.rs` and `src/resolution.rs` extract top-level `use` statements (including grouped and aliased imports) and static relative `require`/`include` paths.
  They resolve PSR-4 namespaces through `composer.json` `autoload.psr-4` and `autoload-dev.psr-4`, falling back to declared Composer sub-packages when no local file or directory matches.
  They classify PHP built-in classes, PHP 8.x built-in extension namespaces, Composer runtime API namespaces (`Composer\InstalledVersions`, `Composer\Autoload\...`), and single-segment global symbols as `Builtin`.
  Multi-segment external namespaces match against the repository's declared `composer.json` dependencies across 2-segment, 3-segment joined, PSR-17 factory and 3-segment class-word forms, framework structural sub-layer (`Component`, `Contracts`, `Bridge`, `Bundle`, `Common`, `UX`), and unique multi-word package-name forms before falling back to `vendor/package` normalization.
- `src/dependencies.rs` parses `composer.json` `require` (`Runtime`), `require-dev` (`Dev`), and `suggest` (`Optional`) dependencies per Workspace package while excluding platform requirements (`php`, `ext-*`, `lib-*`) and local workspace package names.

## Framework profiles

- `src/frameworks/` implements isolated package-level Framework profiles (`laravel`, `symfony`, `cakephp`, `codeigniter`) with zero sibling cross-imports ([ADR 0025](../../../docs/adr/0025-framework-profiles.md)).
- Detection requires strict app-level `composer.json` packages plus local bootstrap markers so Laravel's internal `symfony/*` dependencies never trigger Symfony, standalone `illuminate/*` packages never trigger Laravel, and `symfony-bundle` / `cakephp-plugin` libraries never activate app conventions (CodeIgniter also supports manual non-Composer installs with `spark`, `app/Config/Paths.php`, and `system/Boot.php`).
- Active profiles contribute convention entry points, framework-generated `Source set` paths (`bootstrap/cache/`, `storage/framework/`, `var/cache/`, `tmp/`, `writable/`, and related framework output folders), and PSR-4 fallback import hints scoped to the importer's package root when `composer.json` PSR-4 does not already match.

## Reference ranges

- `reference_ranges` returns the 10th to 90th percentile bands from the stratified 100-repository current-codebase PHP panel under [ADR 0035](../../../docs/adr/0035-stratified-100-repository-reference-panels.md).
