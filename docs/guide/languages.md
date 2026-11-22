# Built-in Languages

codeint ships four built-in Language plugins in pure, safe Rust (`#![forbid(unsafe_code)]`).
Each plugin parses the modern superset of its language family with no language-version pin needed in `codeint.json`.

## Summary

| Plugin name | Extensions | Workspace manifest | Default Test / Generated rules |
| ----------- | ---------- | ------------------ | ------------------------------ |
| `typescript-javascript` | `.ts`, `.tsx`, `.mts`, `.cts`, `.js`, `.jsx`, `.mjs`, `.cjs` | `package.json` / `pnpm-workspace.yaml` | `*.test.*`, `*.spec.*`, `__tests__/`, `test/`, `tests/` |
| `rust` | `.rs` | `Cargo.toml` | `tests/`, `benches/`, `examples/`, `tests.rs`, `*_test.rs`, `*_tests.rs`; inline `#[cfg(test)]` excluded from Production |
| `php` | `.php` | `composer.json` (`repositories` of type `path`) | `tests/`, `test/`, `*Test.php`, `*.test.php` |
| `dart` | `.dart` | `pubspec.yaml` (`workspace`) | `test/`, `tests/`, `integration_test/`, `*_test.dart`; `*.g.dart`, `*.freezed.dart`, `*.gr.dart`, `*.mocks.dart`, `*.pb*.dart` -> Generated |

Core rules also classify files under `vendor/` or `third_party/` as `vendored` and files with generated-file header comments (`@generated`, `GENERATED CODE`, `DO NOT EDIT`) as `generated`.

## Calibrated Reference ranges

Measured across 20 pre-2023 public repositories per language (10th to 90th percentile):

| Language | Erosion | Duplication | Verbosity | Import cycles | Record |
| -------- | ------- | ----------- | --------- | ------------- | ------ |
| TypeScript / JavaScript | 35.2% to 60.0% | 1.7% to 34.6% | 1.8% to 34.7% | 0.0% to 30.8% | [ADR 0008](../adr/0008-typescript-reference-ranges.md) |
| Rust | 38.0% to 64.2% | 5.0% to 32.5% | 6.0% to 32.9% | 2.1% to 50.6% | [ADR 0010](../adr/0010-rust-reference-ranges.md) |
| PHP | 5.5% to 67.7% | 4.1% to 29.0% | 4.2% to 29.2% | 0.0% to 10.7% | [ADR 0011](../adr/0011-php-reference-ranges.md) |
| Dart | 4.7% to 41.9% | 7.3% to 25.6% | 8.0% to 25.9% | 7.3% to 52.4% | [ADR 0012](../adr/0012-dart-reference-ranges.md) |

To add another language as an external WebAssembly plugin, see [docs/plugins.md](../plugins.md).
