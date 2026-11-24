# Built-in Languages

codeint ships six built-in Language plugins in pure, safe Rust (`#![forbid(unsafe_code)]`).
Each plugin parses the modern superset of its language family with no language-version pin needed in `codeint.jsonc` or `codeint.json`.

Every built-in Language plugin extracts:

- **Function names and complexity**: named functions (with `none` for anonymous lambdas) for Erosion and nesting-weighted Cognitive complexity Findings.
- **Workspaces and entry points**: Workspace package roots and standard entry-point files discovered from language manifests for `--package` scoping and Orphan files detection; Language plugins can also mark Production files as program starts from source text.
- **Dependency manifests and built-ins**: declared runtime, dev, and optional dependencies from each ecosystem's manifest, plus standard-library and runtime built-in classification so built-in imports are never flagged as unlisted dependencies.

## Summary

| Plugin name | Extensions | Workspace & dependency manifest | Default Test / Generated rules |
| ----------- | ---------- | ------------------------------- | ------------------------------ |
| `typescript-javascript` | `.ts`, `.tsx`, `.mts`, `.cts`, `.js`, `.jsx`, `.mjs`, `.cjs`, `.vue`, `.svelte` | `package.json` (npm, Yarn, Bun) / `pnpm-workspace.yaml` | `*.test.*`, `*.test-d.*`, `*.spec.*`, `__tests__/`, `test/`, `tests/`; supports React (`.tsx`, `.jsx`, and JSX in `.js`/`.mjs`/`.cjs`); `.vue` and `.svelte` measure `<script>` blocks only |
| `rust` | `.rs` | `Cargo.toml` | `tests/`, `benches/`, `examples/`, `tests.rs`, `*_test.rs`, `*_tests.rs`; inline `#[cfg(test)]` and `#[cfg(all(test, ...))]` excluded from Production |
| `php` | `.php` | `composer.json` (`repositories` of type `path`) | `tests/`, `test/`, `*Test.php`, `*.test.php` |
| `dart` | `.dart` | `pubspec.yaml` (`workspace`) | `test/`, `tests/`, `integration_test/`, `*_test.dart`; `*.g.dart`, `*.freezed.dart`, `*.gr.dart`, `*.mocks.dart`, `*.pb*.dart` -> Generated |
| `python` | `.py` | `pyproject.toml` (`[tool.uv.workspace].members`) | `test/`, `tests/`, `test_*.py`, `*_test.py`, `conftest.py`; `*_pb2.py`, `*_pb2_grpc.py` -> Generated; `if TYPE_CHECKING:` imports excluded from Import cycles |
| `go` | `.go` | `go.mod` | `*_test.go`, `testdata/`; `// Code generated ... DO NOT EDIT.` -> Generated; `vendor/` -> Vendored; see [Go](./go.md) |

Core rules also classify files under `vendor/` or `third_party/` as `vendored` and files with generated-file header comments (`@generated`, `GENERATED CODE`, `DO NOT EDIT`) as `generated`.
See [Go](./go.md) for Go terminology mapping, multi-file Go package imports, entry points, Flagged line rules, and `go.mod` Dependency hygiene.

## Calibrated Reference ranges

Measured across 20 pre-2023 public repositories per language (10th to 90th percentile):

| Language | Erosion | Duplication | Verbosity | Import cycles | Record |
| -------- | ------- | ----------- | --------- | ------------- | ------ |
| TypeScript / JavaScript | 35.2% to 60.0% | 1.7% to 34.6% | 1.8% to 34.7% | 0.0% to 30.8% | [ADR 0008](../adr/0008-typescript-reference-ranges.md) |
| React (`.jsx` / `.tsx`) | 10.6% to 54.4% | 4.6% to 36.8% | 4.7% to 36.8% | 0.0% to 14.4% | [ADR 0020](../adr/0020-react-reference-ranges.md) |
| Vue 2 (`.vue`) | 2.0% to 45.2% | 3.6% to 19.2% | 3.6% to 19.2% | 0.0% to 11.4% | [ADR 0014](../adr/0014-vue2-reference-ranges.md) |
| Vue 3 (`.vue`) | 1.9% to 38.8% | 2.3% to 34.5% | 2.4% to 34.6% | 0.0% to 7.7% | [ADR 0015](../adr/0015-vue3-reference-ranges.md) |
| Rust | 38.0% to 64.2% | 5.0% to 32.5% | 6.0% to 32.9% | 2.1% to 50.6% | [ADR 0010](../adr/0010-rust-reference-ranges.md) |
| PHP | 5.5% to 67.7% | 4.1% to 29.0% | 4.2% to 29.2% | 0.0% to 10.7% | [ADR 0011](../adr/0011-php-reference-ranges.md) |
| Dart | 4.7% to 41.9% | 7.3% to 25.6% | 8.0% to 25.9% | 7.3% to 52.4% | [ADR 0012](../adr/0012-dart-reference-ranges.md) |
| Python | 14.4% to 62.4% | 1.2% to 20.1% | 1.4% to 20.4% | 0.0% to 32.1% | [ADR 0013](../adr/0013-python-reference-ranges.md) |

To add another language as an external WebAssembly plugin, see [docs/plugins.md](../plugins.md).

## Sample repository measurements and speed

Sample Production measurements from `codeint check` (release build, Apple Silicon):

| Language | Repository | Files | Prod lines | Erosion | Duplication | Verbosity | Import cycles | Time |
| -------- | ---------- | ----- | ---------- | ------- | ----------- | --------- | ------------- | ---- |
| React (TSX) | `ant-design/ant-design` | 2,424 | 175,216 | 32.9% | 7.6% | 7.6% | 0.0% | 0.34s |
| React (TSX) | `mantinedev/mantine` | 5,230 | 331,943 | 38.2% | 22.0% | 22.0% | 0.8% | 0.74s |
| React (JSX) | `gethomepage/homepage` | 582 | 29,325 | 63.2% | 19.9% | 19.9% | 0.0% | 0.14s |
| Rust | `avidianity/codeint` | 176 | 43,487 | 10.1% | 1.2% | 1.3% | 1.7% | 0.11s |
| Rust | `BurntSushi/ripgrep` | 105 | 24,961 | 36.4% | 31.1% | 31.3% | 29.3% | 0.07s |
| Rust | `sharkdp/fd` | 25 | 3,423 | 44.6% | 0.0% | 0.1% | 56.5% | 0.02s |
| Rust | `dtolnay/serde` | 189 | 19,425 | 45.7% | 40.2% | 40.2% | 0.0% | 0.06s |
| PHP | `laravel/laravel` | 29 | 762 | 0.0% | 0.0% | 0.0% | 8.0% | 0.03s |
| PHP | `laravel/framework` | 3,208 | 132,791 | 16.9% | 8.1% | 8.2% | 4.4% | 0.47s |
| Dart | `dart-lang/pub` | 547 | 26,170 | 44.4% | 0.5% | 1.7% | 78.6% | 0.88s |
| Dart | `flutter/packages` | 4,137 | 434,407 | 45.9% | 23.1% | 23.3% | 18.5% | 3.68s |
| Python | `django/django` | 865 | 116,838 | 42.3% | 1.8% | 2.9% | 18.6% | 0.33s |
| Python | `home-assistant/core` | 5,592 | 671,942 | 23.4% | 8.2% | 8.3% | 2.6% | 0.73s |

See the [20-repository React (JSX and TSX) evaluation](../research/react-jsx-tsx-panel.md) for full JSX and TSX verification results across 15,058 JSX/TSX files and 2.1M Production lines.
