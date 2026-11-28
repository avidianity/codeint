# How to use and change the Go Language plugin

## Call the plugin

```rust
use codeint_lang_go::{Go, flagged_line_rules};
use codeint_model::{LanguagePlugin, RepoPath};

let plugin = Go;
let path = RepoPath::new("cmd/app/main.go")?;
let model = plugin.lower(&path, "package main\nfunc main() {}\n", None)?;
let rules: Vec<&str> = flagged_line_rules().collect();
```

- `plugins/languages/go` (`codeint-lang-go`, plugin name `go`) is a built-in Language plugin for `.go` files under `#![forbid(unsafe_code)]`.
- It implements `LanguagePlugin` with a hand-written lexer ([ADR 0029](../../../docs/adr/0029-hand-written-lexers.md)).
- `has_imports` is `true`, `has_hazard_rules` is `false`, `has_scope_analysis` is `false`, `known_dialects` is empty, and `known_framework_profiles` is empty.
- `lower` ignores the optional `dialect` argument and returns `LowerError::UnsupportedFile` for any path that does not end with `.go`.

## Source sets

- `src/source_set.rs` classifies a `.go` file in this priority order:
  1. `Vendored` when any folder segment is `vendor`.
  2. `Generated` when a leading header line comment matches `// Code generated ... DO NOT EDIT.`.
  3. `Test` when the path ends with `_test.go` or has a `testdata` folder segment.
  4. `Production` otherwise.

## Functions, tokens, and Flagged lines

- `src/functions.rs` extracts top-level `func` declarations and methods, computing decision points and nesting-weighted cognitive complexity.
- `src/lib.rs` emits Duplication tokens while skipping inserted `AutoSemi` tokens and marking `import` declaration tokens as `TokenKind::ImportOrExport`.
- `FileModel::program_start` is `true` when the file declares `package main`.
- `src/rules.rs` implements five Flagged line rules returned by `flagged_line_rules()`:
  - `conditional-yields-boolean`
  - `else-after-return`
  - `if-else-returns-boolean`
  - `redundant-break`
  - `trailing-bare-return`

## Manifests, Workspace packages, and dependencies

- `manifest_paths` (`src/resolution.rs::find_go_mods`) selects every `go.mod` file outside `vendor/` and `testdata/`.
- `src/go_mod.rs` parses `module`, `require`, `replace`, and `tool` directives (both single-line and grouped blocks) and rejects unknown directives, missing or duplicate `module` directives, and unclosed blocks as `WorkspaceError`.
- `analyze_manifests` (`src/workspaces.rs`) reports each subdirectory `go.mod` directory as a Workspace package, records `DependencyManifestOwner` entries, and collects malformed files in `invalid_manifests`.
- `src/dependencies.rs` maps direct `require` entries to `DepKind::Runtime`, `// indirect` entries to `DepKind::Optional`, and `tool` providers to `DepKind::Dev` (when not already `Runtime`), excluding repository modules and local relative `replace` targets.

## Imports and entry points

- `resolve_imports` (`src/resolution.rs`) resolves package-level Go imports ([ADR 0019](../../../docs/adr/0019-go-imports-lead-to-packages.md)):
  - Empty or relative specifiers (`.`, `..`, `./`, `../`) return `Resolution::Unresolved`.
  - `"C"` and specifiers whose first segment contains no `.` return `Resolution::Builtin`.
  - Local relative `replace` directives in the owning `go.mod` and in-repository `go.mod` module paths resolve to every non-test `.go` file in the target package folder (excluding the importer itself), or `Resolution::Unresolved` when no `.go` files exist there.
  - Other specifiers return `Resolution::External` normalized against the owning `go.mod` `require` and `tool` prefixes or inferred from known host path layouts.
- `entry_points` (`src/entry_points.rs`) returns every non-test, non-vendored `.go` file whose path relative to its owning `go.mod` root contains no `internal` folder segment.

## Reference ranges

- `reference_ranges` returns the 10th to 90th percentile bands from the stratified 100-repository Go panel under [ADR 0035](../../../docs/adr/0035-stratified-100-repository-reference-panels.md).
- Its source label is `codeint stratified panel of 100 Go repositories, ADR 0035, codeint 0.8.0`.
