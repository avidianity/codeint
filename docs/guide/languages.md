# Built-in Languages

codeint ships eight built-in Language plugins in pure, safe Rust (`#![forbid(unsafe_code)]`).
Each plugin parses the modern superset of its language family with no language-version pin needed in `codeint.jsonc` or `codeint.json`.

Every built-in Language plugin extracts:

- **Function names and complexity**: named functions (with `none` for anonymous lambdas or unnamed SQL statements) for Erosion and nesting-weighted Cognitive complexity Findings.
- **Workspaces and entry points**: for languages with imports, Workspace package roots and standard entry-point files discovered from language manifests for `--package` scoping and Orphan files detection; Language plugins can also mark Production files as program starts from source text.
  A manifest that a Language plugin finds invalid is skipped with a warning instead of stopping the run; see [Invalid manifests](../reference.md#invalid-manifests).
- **Dependency manifests and built-ins**: for languages with imports, declared runtime, dev, and optional dependencies from each ecosystem's manifest, plus standard-library and runtime built-in classification so built-in imports are never flagged as unlisted dependencies.
- **Hazard line rules**: Language plugins may also define AST-only Hazard line rules; `sql` ships 32 Hazard line rules (including 5 schema-backed rules) and `shell` ships 10, while the other six built-in Language plugins define none (leaving Hazards absent rather than `0`).

## Summary

| Plugin name | Extensions | Workspace & dependency manifest | Default Test / Generated rules |
| ----------- | ---------- | ------------------------------- | ------------------------------ |
| `typescript-javascript` | `.ts`, `.tsx`, `.mts`, `.cts`, `.js`, `.jsx`, `.mjs`, `.cjs`, `.vue`, `.svelte` | `package.json` (npm, Yarn, Bun) / `pnpm-workspace.yaml` | `*.test.*`, `*.test-d.*`, `*.spec.*`, `__tests__/`, `test/`, `tests/`; built-in Framework profiles (`react`, `vue2`, `vue3`, `svelte`, `nextjs`, `nuxt`, `sveltekit`, `angular`, `nestjs`) add package-level entry points, generated paths, fallback import hints, and calibrated Reference ranges while `.vue` and `.svelte` `<script>` lowering stays framework-blind |
| `rust` | `.rs` | `Cargo.toml` | `tests/`, `benches/`, `examples/`, `tests.rs`, `*_test.rs`, `*_tests.rs`; inline `#[cfg(test)]` and `#[cfg(all(test, ...))]` excluded from Production |
| `php` | `.php` | `composer.json` (`repositories` of type `path`) | `tests/`, `test/`, `*Test.php`, `*.test.php`; built-in Framework profiles (`laravel`, `symfony`, `cakephp`, `codeigniter`) add package-level entry points, generated paths, and PSR-4 fallback import hints |
| `dart` | `.dart` | `pubspec.yaml` (`workspace`) | `test/`, `tests/`, `integration_test/`, `*_test.dart`; `*.g.dart`, `*.freezed.dart`, `*.gr.dart`, `*.mocks.dart`, `*.pb*.dart` -> Generated; built-in Framework profile (`flutter`) adds app vs package/plugin entry points, Flutter-generated paths (`.dart_tool/`, `build/`, `lib/generated_plugin_registrant.dart`, `*.gen.dart`, `lib/l10n/app_localizations*.dart`), and fallback import hints |
| `python` | `.py` | `pyproject.toml` (`[tool.uv.workspace].members`), `requirements*.txt`, `setup.cfg`, `Pipfile` | `test/`, `tests/`, `test_*.py`, `*_test.py`, `conftest.py`; `*_pb2.py`, `*_pb2_grpc.py` -> Generated; `if TYPE_CHECKING:` imports excluded from Import cycles; built-in Framework profiles (`django`, `fastapi`, `flask`) add convention entry points, generated paths (`**/migrations/[0-9][0-9][0-9][0-9]_*.py`, `staticfiles/`, `instance/`), and fallback import hints |
| `go` | `.go` | `go.mod` | `*_test.go`, `testdata/`; `// Code generated ... DO NOT EDIT.` -> Generated; `vendor/` -> Vendored; see [Go](./go.md) |
| `sql` | `.sql` | none (no imports; joins enclosing Workspace package) | `test/`, `tests/`, `fixtures/`; dump headers, migration markers, `prisma/migrations/`, `drizzle/` -> Generated; generic SQL by default plus `postgresql`, `mysql`, `sqlite`, `duckdb` Dialects; 32 Hazard line rules; see [SQL](./sql.md) |
| `shell` | `.sh`, `.bash`, `.zsh` | none (no imports; joins enclosing Workspace package) | `test/`, `tests/`, `spec/`, `test_*.sh`, `*_test.sh`; `ltmain.sh`, `configure.sh`, generated-file headers -> Generated; POSIX `sh`, Bash, or Zsh from the shebang or extension; 10 Hazard line rules; see [Shell](./shell.md) |

Core rules also classify files under `vendor/` or `third_party/` as `vendored` and files with generated-file header comments (`@generated`, `GENERATED CODE`, `DO NOT EDIT`) as `generated`.
See [Go](./go.md) for Go terminology mapping, multi-file Go package imports, entry points, Flagged line rules, and `go.mod` Dependency hygiene.
See [SQL](./sql.md) for generic SQL measurement, Dialects, templated SQL and rejected statements, automatic SQL schemas, and [Hazards](./hazards.md) for the Hazards Metric.
See [Shell](./shell.md) for shell flavor detection, top-level `<script>` functions, Flagged line rules, and Hazard line rules.

## Calibrated Reference ranges

Measured across stratified panels of 50 current public repositories per built-in language and 20 current public repositories per calibrated Framework profile (10th to 90th percentile, [ADR 0028](../adr/0028-stratified-reference-panels.md)):

| Language | Erosion | Duplication | Verbosity | Import cycles | Record |
| -------- | ------- | ----------- | --------- | ------------- | ------ |
| TypeScript / JavaScript | 20.6% to 73.1% | 0.6% to 19.4% | 0.0% to 0.1% | 0.0% to 18.4% | [ADR 0028](../adr/0028-stratified-reference-panels.md) |
| React (`.jsx` / `.tsx`) | 27.9% to 55.1% | 2.9% to 45.6% | 0.0% to 0.1% | 0.0% to 8.3% | [ADR 0028](../adr/0028-stratified-reference-panels.md) |
| Vue 2 (`.vue`) | 9.9% to 57.6% | 1.5% to 28.7% | 0.0% to 0.3% | 0.0% to 11.5% | [ADR 0028](../adr/0028-stratified-reference-panels.md) |
| Vue 3 (`.vue`) | 10.1% to 45.2% | 1.2% to 19.4% | 0.0% to 0.1% | 0.0% to 7.4% | [ADR 0028](../adr/0028-stratified-reference-panels.md) |
| Rust | 21.2% to 71.6% | 4.1% to 31.4% | 0.0% to 0.5% | 0.0% to 57.4% | [ADR 0028](../adr/0028-stratified-reference-panels.md) |
| PHP | 8.4% to 63.0% | 1.7% to 18.9% | 0.0% to 0.5% | 0.0% to 14.5% | [ADR 0028](../adr/0028-stratified-reference-panels.md) |
| Dart | 2.3% to 47.9% | 2.4% to 26.2% | 0.0% to 0.8% | 1.3% to 48.5% | [ADR 0028](../adr/0028-stratified-reference-panels.md) |
| Python | 33.1% to 73.4% | 0.8% to 17.2% | 0.0% to 0.9% | 0.0% to 43.7% | [ADR 0028](../adr/0028-stratified-reference-panels.md) |
| Go | 32.5% to 67.4% | 2.5% to 18.1% | 0.0% to 0.2% | 0.0% to 0.0% | [ADR 0028](../adr/0028-stratified-reference-panels.md) |
| SQL | 0.0% to 79.5% | 0.1% to 90.9% | 0.0% to 0.0% | N/A | [ADR 0028](../adr/0028-stratified-reference-panels.md) |
| Shell | 41.6% to 92.7% | 0.8% to 75.1% | 0.0% to 2.5% | N/A | [ADR 0031](../adr/0031-shell-reference-ranges.md) |

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
