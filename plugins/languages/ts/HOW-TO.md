# How to change the rules of the TypeScript and JavaScript Language plugin

## Write a Flagged-line rule

```rust
use oxc_ast::ast::{Program, Statement};
use oxc_span::{GetSpan, Span};

use super::Rule;

pub(super) const RULE: Rule = Rule {
    id: "empty-statement",
    find: find_empty_statements,
};

fn find_empty_statements(program: &Program<'_>, _code: &str) -> Vec<Span> {
    program
        .body
        .iter()
        .filter(|statement| matches!(statement, Statement::EmptyStatement(_)))
        .map(GetSpan::span)
        .collect()
}
```

- Put each rule in its own module under `src/rules/`, and add its `RULE` to the `RULES` table in `src/rules.rs`.
- `id` is the stable identifier that the report prints next to each Flagged line.
  Use kebab case, and never change it once released.
- `find` receives the parsed program and `code`, the file text with every comment blanked out to spaces.
  `code` has the same byte offsets as the file, so search it, not the original text, for keywords between AST nodes.
- `find` returns the spans of the code that can be deleted or collapsed.
  The table flags every source line a span touches, and skips lines that hold only comments or whitespace.
- A rule passes the admission test: the flagged code can be deleted or collapsed with no behavior change.
  When a case is unsure, do not flag it.

## Prove the rule

- Add one `RuleFixtures` entry to `FIXTURES` in `tests/rules.rs`, with at least one flagged fixture and one passing fixture.
- A flagged fixture lists the exact line numbers, counted from 1, that the rule flags.
- A passing fixture is code that looks close to the pattern but must not be flagged, such as a case that would change behavior if collapsed.
- The test `every_flagged_line_rule_ships_flagged_and_passing_fixtures` fails until every rule in `RULES` has both kinds of fixture.
- Keep fixtures as string literals in the test, never as files under `tests/fixtures/`, while the no-comments check scans fixture files.

## Change the Test and Generated rules

- `src/source_set.rs` puts a file in the Generated Source set when a comment before its first token of code holds a generated marker.
  `GENERATED_MARKERS` lists the markers; the match ignores case.
  A marker in a later comment or in a string never counts.
- Otherwise it puts a file in the Test Source set when its name ends in `.test.{extension}`, `.spec.{extension}`, `.test-d.{extension}`, or `.spec-d.{extension}` after a non-empty stem, or when any of its folders, at any depth, is named `__tests__`, `test`, `tests`, `test-dts`, or `type-tests`.
  `TEST_FOLDERS` lists the folder names; the match is exact and keeps case.
  Every other file is Production.
- A file named only `test.ts` or `spec.ts` has an empty stem, so it stays Production.
- A file named `test.ts` or `tests.ts` stays Production unless one of its folders is a test folder.
- Build output folders, very long lines, and vendor folders are rules of the core, in `codeint_model::settle_source_set`; never repeat them here.
- Prove a change with `tests/conformance.rs`: list each path or text that must be Test or Generated, and each look-alike that must stay Production.

## Change which tokens clone detection skips

- `src/tokens.rs` gives `TokenKind::ImportOrExport` to every token of an `import` declaration, an `import x = require()` declaration, an `export * from` declaration, an `export { a } from` declaration, and an `export { a }` declaration.
- A declaration that declares code, such as `export const` or `export default function`, keeps its usual token kinds, and so does a dynamic `import()`.
- Prove a change with `tests/conformance.rs`: list each declaration whose tokens must be skipped, and each look-alike that must keep its kinds.

## Change how imports resolve

- `src/resolution.rs` resolves each import with `oxc_resolver`, the way TypeScript resolves it: relative paths, tsconfig `paths` and `baseUrl`, and the `exports`, `main`, and `index` entries of packages.
- `manifest_paths` asks the host for every `package.json` file, every file whose name starts with `tsconfig` and ends with `.json`, and the `pnpm-workspace.yaml` at the repository root.
  A tsconfig `extends` entry that names any other file, or a package outside the repository, cannot be read; the importer then resolves without that tsconfig.
- `src/resolution/snapshot.rs` is the virtual file system that `oxc_resolver` reads: the repository paths, the manifest texts, and a `node_modules` link for each package of the repository.
  It reads no real file, so a tsconfig never reaches outside the snapshot.
- `manifest_paths` is also how the plugin asks for the `package.json` files that `workspace_packages` reads, so the host reads each manifest once.
- `src/resolution/packages.rs` links the `package.json` of each workspace package that the workspace declares, plus the root `package.json` when its `name` does not collide with a workspace member, so an import of a workspace package follows its `exports` field (falling back to source files or `Builtin` when build outputs are absent).
  It finds the workspace packages with `src/workspaces.rs`, the same rule that splits the report by package.
  A non-root package outside the declared workspaces (including subfolder packages in a repository without workspaces) is not linked, because npm, Yarn, and Bun do not install it in `node_modules`.
  Two packages with the same name are both left unlinked.
- The `exports` conditions are `types`, `bun`, `import`, `require`, `node`, and `default`, so an export whose `types` or `bun` entry names a source file resolves to that file.
- `bun`, `bun:*`, `node:*`, `cloudflare:*`, and `deno:*` specifiers are always `Builtin` runtime modules, and `npm:` / `jsr:` specifiers strip their registry prefix and optional `@version` before normalizing the external package name.
- When `oxc_resolver` fails to resolve an import, `classify_error` falls back to `Resolution::Builtin` for framework virtual modules (`virtual:*`, `$app`, `$app/*`, `$env/*`, and `$service-worker`), so real `tsconfig.json` `paths` aliases resolve first while unbuilt framework virtual modules never trigger false `Unresolved` or `unlisted_dependencies` Findings.
- A bare specifier that fails to resolve is `External`, unless it names a linked package or matches a tsconfig `paths` key of the importer; then it is `Unresolved`.
- `src/dependencies.rs` parses `package.json` dependency sections (`dependencies` as `Runtime`, `devDependencies` as `Dev`, and `optionalDependencies` / `peerDependencies` as `Optional`), excluding `workspace:` entries and local package names.
  Ambient `@types/*` declarations emit as `Optional` so they are never flagged as unused and satisfy explicit imports without hiding undeclared `@types/*` imports.
- Prove a change with `tests/conformance.rs` through `manifest_paths` and `resolve_imports`, and with a cycle test in `crates/cli/tests/check.rs`.

## Change the workspace rules

- `src/workspaces.rs` finds the workspace packages from the manifests that `manifest_paths` chose.
- A `pnpm-workspace.yaml` at the repository root decides the packages, and the `workspaces` field of `package.json` is then ignored, because pnpm reads only `pnpm-workspace.yaml`.
  Without it, the `workspaces` field of the root `package.json` decides, as npm, Yarn, and Bun read it.
- `src/workspaces/pnpm.rs` matches pnpm package patterns as pnpm does: a `!` pattern excludes in any position, a wildcard skips folder names that begin with a dot unless the pattern is negated, `./`, `..`, and repeated slashes resolve first, and an absolute pattern selects nothing.
- `pnpm_package_patterns` reads `pnpm-workspace.yaml` with `serde-saphyr` into a derived struct, so YAML that does not parse and a `packages` field of the wrong shape each give an error with the line and column.
  A missing or empty `packages` field means the repository is one package.
- Prove the matching in `src/workspaces/tests.rs`, each error text in `tests/conformance.rs`, and the report in `crates/cli/tests/check.rs`.

## JavaScript JSX and Flow

- `source_type` in `src/lib.rs` enables JSX on every JavaScript extension (`.js`, `.mjs`, `.cjs`, and `.jsx`), while preserving each extension's module kind (`Unambiguous` for `.js` and `.jsx`, `Module` for `.mjs`, `CommonJS` for `.cjs`).
  TypeScript files enable JSX only on `.tsx`.
- `oxc_parser` does not support Flow syntax; when a JavaScript file with a `@flow` comment fails to parse, `oxc_parser` reports `Flow is not supported`, and the plugin returns `LowerError::Unparsable` so the report skips and counts it with the parse error.
- Prove JSX and Flow handling in `tests/conformance.rs` and `crates/cli/tests/check.rs`.

## Reference ranges

- `reference_ranges` in `src/lib.rs` returns the 10th to 90th percentile ranges from [ADR 0008](../../../docs/adr/0008-typescript-reference-ranges.md).
  Its source label is `codeint panel of 20 TypeScript and JavaScript repositories, ADR 0008, codeint 0.1.0`.
- `vue_reference_ranges` in `src/sfc.rs` selects the calibrated Vue 2 Reference ranges ([ADR 0014](../../../docs/adr/0014-vue2-reference-ranges.md)) or Vue 3 Reference ranges ([ADR 0015](../../../docs/adr/0015-vue3-reference-ranges.md)) when `.vue` files are measured and `package.json` declares `vue` major version 2 or 3.
- `react_reference_ranges` in `src/sfc.rs` selects the calibrated React (JSX and TSX) Reference ranges ([ADR 0020](../../../docs/adr/0020-react-reference-ranges.md)) when `.jsx` or `.tsx` files are measured (without `.vue` files) and `package.json` declares `react`.
- `src/sfc.rs` extracts `<script>`, `<script setup>`, `<script context="module">`, and `<script module>` blocks from `.vue` and `.svelte` Single-File Components while preserving original 1-based line numbers and skipping HTML templates and CSS styles.
- Measure the panel again and amend the relevant ADR whenever Metric rules, the default rule set, or Source set rules change.
- Prove a change in `tests/conformance.rs`, `tests/sfc.rs`, and `crates/cli/tests/check.rs`.
