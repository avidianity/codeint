# Built-in Languages

codeint ships five built-in Language plugins in pure, safe Rust (`#![forbid(unsafe_code)]`).
Each plugin parses the modern superset of its language family with no language-version pin needed in `codeint.json`.

## Summary

| Plugin name | Extensions | Workspace manifest | Default Test / Generated rules |
| ----------- | ---------- | ------------------ | ------------------------------ |
| `typescript-javascript` | `.ts`, `.tsx`, `.mts`, `.cts`, `.js`, `.jsx`, `.mjs`, `.cjs` | `package.json` (npm, Yarn, Bun) / `pnpm-workspace.yaml` | `*.test.*`, `*.spec.*`, `__tests__/`, `test/`, `tests/` |
| `rust` | `.rs` | `Cargo.toml` | `tests/`, `benches/`, `examples/`, `tests.rs`, `*_test.rs`, `*_tests.rs`; inline `#[cfg(test)]` excluded from Production |
| `php` | `.php` | `composer.json` (`repositories` of type `path`) | `tests/`, `test/`, `*Test.php`, `*.test.php` |
| `dart` | `.dart` | `pubspec.yaml` (`workspace`) | `test/`, `tests/`, `integration_test/`, `*_test.dart`; `*.g.dart`, `*.freezed.dart`, `*.gr.dart`, `*.mocks.dart`, `*.pb*.dart` -> Generated |
| `python` | `.py` | `pyproject.toml` (`[tool.uv.workspace].members`) | `test/`, `tests/`, `test_*.py`, `*_test.py`, `conftest.py`; `*_pb2.py`, `*_pb2_grpc.py` -> Generated; `if TYPE_CHECKING:` imports excluded from Import cycles |

Core rules also classify files under `vendor/` or `third_party/` as `vendored` and files with generated-file header comments (`@generated`, `GENERATED CODE`, `DO NOT EDIT`) as `generated`.

## Calibrated Reference ranges

Measured across 20 pre-2023 public repositories per language (10th to 90th percentile):

| Language | Erosion | Duplication | Verbosity | Import cycles | Record |
| -------- | ------- | ----------- | --------- | ------------- | ------ |
| TypeScript / JavaScript | 35.2% to 60.0% | 1.7% to 34.6% | 1.8% to 34.7% | 0.0% to 30.8% | [ADR 0008](../adr/0008-typescript-reference-ranges.md) |
| Rust | 38.0% to 64.2% | 5.0% to 32.5% | 6.0% to 32.9% | 2.1% to 50.6% | [ADR 0010](../adr/0010-rust-reference-ranges.md) |
| PHP | 5.5% to 67.7% | 4.1% to 29.0% | 4.2% to 29.2% | 0.0% to 10.7% | [ADR 0011](../adr/0011-php-reference-ranges.md) |
| Dart | 4.7% to 41.9% | 7.3% to 25.6% | 8.0% to 25.9% | 7.3% to 52.4% | [ADR 0012](../adr/0012-dart-reference-ranges.md) |
| Python | 14.4% to 62.4% | 1.2% to 20.1% | 1.4% to 20.4% | 0.0% to 32.1% | [ADR 0013](../adr/0013-python-reference-ranges.md) |

To add another language as an external WebAssembly plugin, see [docs/plugins.md](../plugins.md).

## Sample repository measurements and speed

Sample Production measurements from `codeint check` (release build, Apple Silicon):

| Language | Repository | Files | Prod lines | Erosion | Duplication | Verbosity | Import cycles | Time |
| -------- | ---------- | ----- | ---------- | ------- | ----------- | --------- | ------------- | ---- |
| Rust | `avidianity/codeint` | 154 | 16,400 | 31.0% | 5.2% | 5.3% | 2.3% | 0.06s |
| Rust | `BurntSushi/ripgrep` | 105 | 24,961 | 36.4% | 31.1% | 31.3% | 29.3% | 0.07s |
| Rust | `sharkdp/fd` | 25 | 3,423 | 44.6% | 0.0% | 0.1% | 56.5% | 0.02s |
| Rust | `dtolnay/serde` | 189 | 19,425 | 45.7% | 40.2% | 40.2% | 0.0% | 0.06s |
| PHP | `laravel/laravel` | 29 | 762 | 0.0% | 0.0% | 0.0% | 8.0% | 0.03s |
| PHP | `laravel/framework` | 3,208 | 132,791 | 16.9% | 8.1% | 8.2% | 4.4% | 0.47s |
| Dart | `dart-lang/pub` | 547 | 26,170 | 44.4% | 0.5% | 1.7% | 78.6% | 0.88s |
| Dart | `flutter/packages` | 4,137 | 434,407 | 45.9% | 23.1% | 23.3% | 18.5% | 3.68s |
| Python | `django/django` | 865 | 116,838 | 42.3% | 1.8% | 2.9% | 18.6% | 0.33s |
| Python | `home-assistant/core` | 5,592 | 671,942 | 23.4% | 8.2% | 8.3% | 2.6% | 0.73s |
