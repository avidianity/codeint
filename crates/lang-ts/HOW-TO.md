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

## Change the Test rules

- `src/source_set.rs` puts a file in the Test Source set when its name ends in `.test.{extension}` or `.spec.{extension}` after a non-empty stem, or when any of its folders is named `__tests__`.
  Every other file is Production.
- A file named only `test.ts` or `spec.ts` has an empty stem, so it stays Production.
- Prove a change with `tests/conformance.rs`: list each path that must be Test and each look-alike path that must stay Production.

## Change how imports resolve

- `src/resolution.rs` resolves each import with `oxc_resolver`, the way TypeScript resolves it: relative paths, tsconfig `paths` and `baseUrl`, and the `exports`, `main`, and `index` entries of packages.
- `manifest_paths` asks the host for every `package.json` file and every file whose name starts with `tsconfig` and ends with `.json`.
  A tsconfig `extends` entry that names any other file, or a package outside the repository, cannot be read; the importer then resolves without that tsconfig.
- `src/resolution/snapshot.rs` is the virtual file system that `oxc_resolver` reads: the repository paths, the manifest texts, and a `node_modules` link for each package of the repository.
  It reads no real file, so a tsconfig never reaches outside the snapshot.
- `src/resolution/packages.rs` links each `package.json` below the root that has a valid `name`, outside `node_modules`, so an import of a workspace package follows its `exports` field.
  Two packages with the same name are both left unlinked.
- The `exports` conditions are `types`, `import`, `require`, `node`, and `default`, so an export whose `types` entry names a source file resolves to that file.
- A bare specifier that fails to resolve is `External`, unless it names a linked package or matches a tsconfig `paths` key of the importer; then it is `Unresolved`.
- Prove a change with `tests/conformance.rs` through `manifest_paths` and `resolve_imports`, and with a cycle test in `crates/cli/tests/check.rs`.
