# Go

codeint ships a built-in `go` Language plugin that measures every `.go` file with no configuration required.
Every built-in Metric measures Go: Erosion, Cognitive complexity, Duplication, Verbosity, Import cycles, Import boundaries, Orphan files, and Dependency hygiene.
The `go` plugin defines no Hazard line rules.

## Go terms and codeint terms

Go and codeint reuse some of the same words with different meanings:

| Go term | codeint term | Meaning |
| ------- | ------------ | ------- |
| Go module (folder with `go.mod`) | **Workspace package** | Each folder with a `go.mod` file below the repository root is one Workspace package, measured on its own and selectable with `--package <path>` |
| Go package (folder of `.go` files) | **Go package** | codeint always writes "Go package" for a folder of `.go` files; an import of a Go package leads to every non-test `.go` file in that folder |
| One `.go` source file | **module** | In codeint reports and [GLOSSARY.md](../../GLOSSARY.md), a module is one measured source file |

## Functions, complexity, and duplication

codeint parses Go through language version 1.27, including type parameters on functions, types, and methods, without adding external parser dependencies.
Files that call C via `import "C"` (cgo) are measured as Go.
Files that cannot be scanned, such as a file with an unclosed string literal, are listed under `skipped` and do not stop the run.

- **Functions and methods**: every function declaration and method declaration with a body counts as one function.
  Declarations without bodies are ignored.
  A method is named `ReceiverType.MethodName`, so methods with the same name on different types stay distinct.
  Pointer and value receivers produce the same name, and generic receivers use the base type name without type arguments.
- **Function literals**: a function literal is not a separate function.
  Its source lines and decision points count toward the enclosing function, and its body adds one nesting level for Cognitive complexity.
- **Decision points**: each `if`, `for`, `case` (except `default`) in a `switch` or `select`, `&&`, and `||` adds one decision point.
  A `case` with multiple comma-separated expressions counts as 1.
  `else` and `default` add 0.
- **Cognitive complexity**: follows codeint's standard nesting rules across `if`, `for`, `switch`, `select`, `else`, function literals, logical operator runs, labelled `break` and `continue`, `goto`, and direct recursion.
  An `else if` chain adds no extra nesting.
- **Duplication**: exact and near-clone detection uses Go tokens without comments and excludes `import` declarations from clone matching.

## Source sets

After checking any `source_sets` globs in `codeint.jsonc` or `codeint.json` (which always override built-in rules), the `go` plugin classifies each `.go` file as follows:

- **Vendored**: any file under a `vendor/` folder.
- **Generated**: any file with a line comment before the first non-comment code matching Go's documented header `// Code generated ... DO NOT EDIT.`.
  Block comments and comments after code do not match.
- **Test**: any file whose name ends in `_test.go`, or any file under a `testdata/` folder at any depth.
- **Production**: every other `.go` file, including platform-specific files such as `*_linux.go`, `*_darwin.go`, or files with `//go:build` constraints.

## Flagged line rules

The `go` plugin ships five zero-false-positive Flagged line rules that feed Verbosity:

- `else-after-return`: an `else` block after an `if` branch that always returns.
  Stays silent when the `if` header declares a variable (`:=`) that the `else` branch uses.
- `if-else-returns-boolean`: an `if` / `else`, or an `if` followed by a `return`, where one branch returns `true` and the other returns `false`.
  Stays silent in functions with named results and when either branch does more than return.
- `conditional-yields-boolean`: an `if` / `else` whose only statements assign `true` and `false` to the same variable.
- `trailing-bare-return`: a bare `return` as the last statement of a function with no return values.
  Stays silent in functions with named results.
- `redundant-break`: an unlabelled `break` as the last statement of a `case` in a `switch` or `select`.
  Stays silent for labelled `break` statements and for `break` inside a loop.

codeint never flags the idiomatic Go error check (`if err != nil { return ..., err }`).

## Imports, Import cycles, and Import boundaries

codeint reads single and grouped `import` declarations, with or without package aliases, `.`, or `_`, and resolves them from repository paths and `go.mod` files alone:

- **Built-in**: any import path whose first segment contains no dot (standard library) and cgo's `"C"`.
- **Repository Go packages**: any import path under the module path of a repository `go.mod` file (longest matching module path wins, including cross-module imports) leads to every `.go` file in that folder except `*_test.go` files ([ADR 0019](../adr/0019-go-imports-lead-to-packages.md)).
  A repository module path that matches no folder, or a folder with no non-test `.go` files, is reported as an Unresolved import.
- **External**: every other import with a dot in its first segment is classified as external under its Go module path.

Because one Go import leads to every non-test file of the imported Go package:

- **Import cycles** reports its measured value for Go (usually `0`, since the Go compiler rejects package cycles).
  Because codeint evaluates no build conditions, a cycle through platform-specific files such as `a_linux.go` and `b_darwin.go` is reported with the real source files involved.
- **Import boundaries** reports every target file in a forbidden Go package along with the importing file.
  Standard library and external imports never trigger boundary Findings.

## Entry points, Orphan files, and `codeint trace`

Orphan files works on Go repositories with no configuration:

- **Programs (`package main`)**: every Production `.go` file declaring `package main` receives the `program-start` mark, including programs under `internal/` such as code generators.
  `_test.go` files never become entry points.
- **Public Go packages**: every non-test Production `.go` file in a folder with no `internal` segment in its path (counted from its module root) is an entry point, so public library packages are not reported as Orphan files.
- **Unused internal packages**: Go packages under `internal/` that nothing reaches from an entry point are reported as `unreferenced`, or as `test-only` when only `_test.go` files import them.
- **Multi-file reachability**: importing a Go package makes every non-test `.go` file in that folder reachable, even when no file mentions a sibling filename directly.
- **Custom entry points**: `entry_points` globs in `codeint.jsonc` or `codeint.json` add entry points alongside `package main` and public packages.

Run `codeint trace <from> <to>` to inspect why a Go file is reachable.
`codeint trace` reports `program-start mark` for `package main` files and lists every file reached by each multi-file Go package import.

## Dependency hygiene (`go.mod`)

codeint reads `module`, `require`, `replace`, and `tool` directives from each `go.mod` file in single and grouped forms, and ignores other directives.
A malformed `go.mod` stops the run with exit code `2` and names the file.
In a multi-module repository, each Go module is checked independently against its own `go.mod`.

- **`require` without `// indirect`**: runtime dependency.
  Reported under `unused_dependencies` when no file in the module imports it.
  A module imported only by `_test.go` files stays a runtime dependency with no Finding, matching `go mod tidy`.
- **`require` with `// indirect`**: optional dependency.
  Satisfies imports (never `unlisted_dependencies`) and is never reported as `unused_dependencies`.
- **`tool`**: a module that provides a package named by a `tool` line, and that no plain `require` line in the same `go.mod` names, is a development dependency.
  Importing it from Production code triggers `dev_in_production_dependencies`.
- **`replace`**: a `replace` line whose target is a folder inside the repository resolves imports of that module to that folder's Go files instead of treating it as external.
  A `replace` line pointing to another module version or outside the repository changes nothing.
- **Unlisted imports and ignores**: an external import not covered by any `require` or `tool` entry in the owning `go.mod` is reported under `unlisted_dependencies`.
  List Go module paths in `dependencies.ignore` in `codeint.jsonc` or `codeint.json` to silence specific modules.

## What codeint does not read

- Build conditions (`//go:build`, `// +build`, or OS/architecture filename constraints).
- `go.work` workspace files (each folder with a `go.mod` file is discovered directly).
- `vendor/modules.txt`.
- C, assembly (`.s`), and template files.

## Reference ranges

Calibrated Go Reference ranges from a panel of 20 pre-2023 public Go repositories are pending calibration and will be published here and in [Built-in Languages](./languages.md).
See [docs/reference.md](../reference.md) for all CLI commands, options, and configuration keys.
