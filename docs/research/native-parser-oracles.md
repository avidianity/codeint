# Native parser oracles for PHP, Dart, Python, and Go recall checks

Date checked: 2026-11-25.
Scope: Official standard-library parsers and first-party tooling for measuring Recall (at least 98% on functions, decision points, and static imports) of the hand-written `python`, `go`, `php`, and `dart` Language plugins without adding any dependencies to codeint.

## Summary

Each language has an official first-party parser or tokenizer that runs from an external scratch directory outside the codeint repository.
Three of the four runtimes (`python3`, `go`, and `php`) ship their oracle API in the standard runtime with zero external packages.
Dart has no source parser in `dart:*` core libraries and requires the first-party `package:analyzer` package inside an isolated scratch directory.

| Language | Official oracle API | Minimum version | Local runtime (2026-11-25) | External packages in scratch dir | Oracle fidelity |
| -------- | ------------------- | --------------- | -------------------------- | -------------------------------- | --------------- |
| Python | `ast` stdlib module (`ast.parse`) | Python 3.12+ (3.10+ min for `match`) | `Python 3.15.2` (`/opt/homebrew/bin/python3`) | None | Full syntax tree |
| Go | `go/parser`, `go/ast`, `go/token` stdlib | Go 1.24+ (1.18+ min for generics) | `go1.27.2 darwin/arm64` (`/opt/homebrew/bin/go`) | None | Full syntax tree |
| PHP | `PhpToken::tokenize(..., TOKEN_PARSE)` (`ext/tokenizer`) | PHP 8.1+ (8.0+ min for `PhpToken`) | `PHP 8.5.11` (`/opt/homebrew/bin/php`) | None (optional `nikic/php-parser` or PECL `ast` for full tree) | Zend lexer + syntax check |
| Dart | `package:analyzer` (`parseString` + `RecursiveAstVisitor`) | Dart SDK 3.0+ | `Dart SDK 3.11.0` (`/Users/jmm/fvm/default/bin/dart`) | `analyzer` via `dart pub get` in scratch dir | Full syntax tree |

All four runtimes are installed locally on this machine.
Evaluation scripts and CI jobs must gate each language oracle on binary availability (`command -v`) and minimum runtime version, and skip unparsable corpus files from Recall denominators.

## Alignment with codeint rules

Each oracle must align with the exact extraction rules implemented in the corresponding Language plugin:

- Python: [functions.rs](../../plugins/languages/python/src/functions.rs) and [imports.rs](../../plugins/languages/python/src/imports.rs).
- Go: [functions.rs](../../plugins/languages/go/src/functions.rs) and [imports.rs](../../plugins/languages/go/src/imports.rs).
- PHP: [functions.rs](../../plugins/languages/php/src/functions.rs) and [imports.rs](../../plugins/languages/php/src/imports.rs).
- Dart: [functions.rs](../../plugins/languages/dart/src/functions.rs) and [imports.rs](../../plugins/languages/dart/src/imports.rs).

Every Recall comparison measures three outputs per file:

1. Functions: extracted function spans (start line, end line, and optional name).
2. Decision points: per-function branch and boolean-operator count used for cyclomatic complexity (`1 + decision_points`) and Function mass.
3. Static imports: deduplicated runtime import specifiers extracted by the Language plugin.

## Python

### Official API and minimum version (Python)

Python ships the `ast` module in the standard library with zero external packages.
Use `ast.parse(source, filename=path, mode="exec")` and subclass `ast.NodeVisitor`.

- `end_lineno` and `end_col_offset` were added in Python 3.8.
- `ast.Match` and `ast.match_case` were added in Python 3.10.
- `ast.TryStar` (`except*`) was added in Python 3.11.
- PEP 695 generic `type_params` on `FunctionDef` were added in Python 3.12.

Require Python 3.12+ to parse modern 2026 Python corpora (Python 3.10+ is the hard floor for `match` and `case`).
Local runtime: `/opt/homebrew/bin/python3` (`Python 3.15.2`).

### Scratch-folder invocation (Python)

Create a standalone script outside the repository and invoke `python3` directly:

```sh
python3 /tmp/oracle-py/oracle.py path/to/file.py
```

Minimal extraction skeleton:

```python
import ast, json, sys

tree = ast.parse(open(sys.argv[1], encoding="utf-8").read(), filename=sys.argv[1])
```

### Recall mapping (Python)

- **Functions**: Visit `ast.FunctionDef`, `ast.AsyncFunctionDef`, and `ast.Lambda`.
  Record `node.lineno` and `node.end_lineno`, plus `node.name` (or `None` for `ast.Lambda`).
  Stop descending into nested `FunctionDef`, `AsyncFunctionDef`, and `Lambda` bodies when counting the enclosing function's decision points; launch a new counter for each nested function instead.
- **Decision points**: Count nodes inside each function's own body (matching `decision_weight` in `plugins/languages/python/src/functions.rs`):
  - `ast.If`: +1 (Python represents `elif` as a nested `ast.If` inside `orelse`, so both `if` and `elif` count +1).
  - `ast.IfExp`: +1 (ternary `a if cond else b`).
  - `ast.For` and `ast.AsyncFor`: +1 each.
  - `ast.comprehension` (inside `ListComp`, `SetComp`, `DictComp`, `GeneratorExp`): +1 for each generator clause plus +1 for each filter in `comp.ifs`.
  - `ast.While`: +1.
  - `ast.ExceptHandler` (under both `ast.Try` and `ast.TryStar`): +1 each.
  - `ast.match_case`: +1 unless `pattern` is an unguarded bare wildcard (`isinstance(c.pattern, ast.MatchAs) and c.pattern.pattern is None and c.pattern.name is None and c.guard is None`).
  - `ast.BoolOp` (`ast.And` or `ast.Or`): add `len(node.values) - 1`, because Python flattens `a and b and c` into one `BoolOp` with three values.
- **Static imports**: Skip any `ast.If` block whose `test` is `TYPE_CHECKING` or `<ident>.TYPE_CHECKING` (matching `is_type_checking_guard` in `plugins/languages/python/src/imports.rs`).
  - `ast.Import`: emit each `alias.name`.
  - `ast.ImportFrom`: skip `level == 0 and module == "__future__"`.
    When `module` is present, emit `"." * level + module`.
    When `module is None` and `level > 0`, emit `"." * level + alias.name` (or `"." * level` for `alias.name == "*"`).

### Blind spots and semantic mismatches (Python)

- **Match guard `if` double-count**: For `case x if cond:`, codeint counts both the `case` header and the `if` token (+2).
  A pure `match_case` count yields +1 unless the oracle also adds +1 when `c.guard is not None`.
- **Decorated function start lines**: Since Python 3.8, `FunctionDef.lineno` points to the `def` line rather than `@decorator` lines, which matches codeint's `start_line`.
- **Interpreter grammar coupling**: `ast.parse` parses only syntax valid for the running Python version.
  Legacy Python 2 files or syntax newer than the installed interpreter raise `SyntaxError` and must be excluded from Recall denominators.
- **Soft keyword `case` false positives**: Codeint detects `case` headers heuristically, whereas `ast.parse` distinguishes true `match_case` clauses from dict keys or type annotations starting with `case:`.
  Differences here reveal real codeint false positives rather than oracle defects.

### Primary sources (Python)

- Python 3 standard library `ast` documentation (checked 2026-11-25): <https://docs.python.org/3/library/ast.html>

## Go

### Official API and minimum version (Go)

Go ships `go/parser`, `go/ast`, and `go/token` in the standard library with zero external packages.
Call `parser.ParseFile(fset, filename, src, parser.SkipObjectResolution)` and traverse with `ast.Inspect`.

- `parser.SkipObjectResolution` was added in Go 1.17 and skips deprecated object resolution for faster parsing.
- Generics syntax (`TypeParams`) was added in Go 1.18.
- Newer language syntax (range-over-int in Go 1.22, range-over-func in Go 1.23, generic type aliases in Go 1.24) requires matching or newer Go toolchains.

Require Go 1.24+ for modern 2026 Go repositories (Go 1.18+ is the hard floor for generics).
Local runtime: `/opt/homebrew/bin/go` (`go1.27.2 darwin/arm64`).

### Scratch-folder invocation (Go)

Run directly with `go run` or compile once inside the scratch folder for fast multi-repository batches:

```sh
go build -o /tmp/oracle-go/oracle /tmp/oracle-go/main.go
/tmp/oracle-go/oracle path/to/file.go
```

Minimal parsing call:

```go
fset := token.NewFileSet()
file, err := parser.ParseFile(fset, path, nil, parser.SkipObjectResolution)
```

### Recall mapping (Go)

- **Functions**: Inspect top-level `*ast.FuncDecl` nodes where `decl.Body != nil`.
  Use `fset.Position(decl.Pos()).Line` as `start_line` and `fset.Position(decl.End()).Line` as `end_line`.
  Format method names as `{RecvType}.{Name}` by unwrapping pointer `*ast.StarExpr`, parenthesized `*ast.ParenExpr`, and generic index `*ast.IndexExpr` or `*ast.IndexListExpr` on `decl.Recv.List[0].Type`.
  Log `*ast.FuncLit` closures in a separate diagnostic field (see mismatch below).
- **Decision points**: Traverse `decl.Body` with `ast.Inspect` (matching `count_decision_points` in `plugins/languages/go/src/functions.rs`):
  - `*ast.IfStmt`: +1 (`else if` is an `*ast.IfStmt` inside `Else`).
  - `*ast.ForStmt` and `*ast.RangeStmt`: +1 each.
  - `*ast.CaseClause` with `len(n.List) > 0`: +1 (excludes `default:`).
  - `*ast.CommClause` with `n.Comm != nil`: +1 (excludes `default:` in `select`).
  - `*ast.BinaryExpr` with `n.Op == token.LAND || n.Op == token.LOR`: +1 each.
- **Static imports**: Iterate `file.Imports` (`[]*ast.ImportSpec`) and call `strconv.Unquote(spec.Path.Value)`.
  This covers single imports, grouped `import (...)` blocks, aliased imports, dot `.` imports, blank `_` imports, and raw backtick strings.

### Blind spots and semantic mismatches (Go)

- **`FuncLit` closure policy mismatch**: `plugins/languages/go/src/functions.rs` extracts only top-level `func` declarations (`collect_top_level_functions`) and does not emit `*ast.FuncLit` closures as separate functions.
  Its `count_decision_points` scans all tokens inside `span.body_open + 1..span.body_close`, including tokens inside nested function literals.
  To compare Recall apples-to-apples against current codeint rules, the oracle must emit only `*ast.FuncDecl` with `Body != nil` and allow `ast.Inspect` to walk through nested `*ast.FuncLit` bodies when counting the enclosing function's decision points.
- **Bodyless declarations**: Assembly, Cgo, and `go:linkname` forward declarations produce `*ast.FuncDecl` with `Body == nil`.
  Codeint skips them; the oracle must skip `Body == nil`.
- **`default:` clauses**: Both `CaseClause` (`List == nil`) and `CommClause` (`Comm == nil`) represent `default:`.
  Excluding `nil` clauses aligns with codeint counting only `case`.
- **Template files**: Go `.go` template fixtures containing `{{ ... }}` fail `parser.ParseFile` and must be excluded from Recall denominators.

### Primary sources (Go)

- Go standard library `go/parser` documentation (checked 2026-11-25): <https://pkg.go.dev/go/parser>
- Go standard library `go/ast` documentation (checked 2026-11-25): <https://pkg.go.dev/go/ast>
- Go standard library `go/token` documentation (checked 2026-11-25): <https://pkg.go.dev/go/token>
- Go 1.17 release notes for `SkipObjectResolution` (checked 2026-11-25): <https://go.dev/doc/go1.17>

## PHP

### Official API and minimum version (PHP)

Core PHP does not expose Zend Engine's internal syntax tree to userland code (`php -l` runs a syntax check only).
Core PHP ships the built-in `tokenizer` module (`ext/tokenizer`, enabled by default), which exposes Zend Engine's exact lexer via two APIs:

- `PhpToken::tokenize(string $code, int $flags = 0): array` (added in PHP 8.0.0): returns an array of `PhpToken` objects with `$id`, `$text`, `$line`, `$pos`, `getTokenName()`, `is()`, and `isIgnorable()`.
- `token_get_all(string $code, int $flags = 0): array` (legacy procedural API): returns mixed arrays and single-character strings.

Always pass the `TOKEN_PARSE` flag (added in PHP 7.0.0).
With `TOKEN_PARSE`, Zend reclassifies context-sensitive semi-reserved keywords (such as `const PUBLIC = 1`) as `T_STRING` and throws `\ParseError` on syntactically invalid PHP files (verified locally on PHP 8.5.11).

Two non-core syntax-tree options exist if a corpus ever needs full tree-level verification beyond tokenization:

1. PECL `ast` (`nikic/php-ast`, `pecl install ast` or `pie install nikic/php-ast`): a third-party C module exposing Zend Engine's internal syntax tree (`ast\parse_code`); not bundled in core PHP or Homebrew PHP (`php -m` confirms it is not installed locally).
2. `nikic/php-parser` (`composer require nikic/php-parser` inside `/tmp/oracle-php/`): pure-PHP userland parser that produces full syntax-tree nodes without compiling C modules.

Use `PhpToken::tokenize($code, TOKEN_PARSE)` as the zero-dependency baseline oracle, and escalate to `nikic/php-parser` in an isolated scratch directory only if ternary or arrow-function boundary noise exceeds 1%.

Require PHP 8.1+ for `PhpToken`, `T_MATCH`, `T_NAME_QUALIFIED`, `T_ATTRIBUTE`, `T_ENUM`, and `T_READONLY` (PHP 8.0+ is the hard floor for `PhpToken`).
Local runtime: `/opt/homebrew/bin/php` (`PHP 8.5.11`, `tokenizer` enabled).

### Scratch-folder invocation (PHP)

Run directly with the `php` CLI and zero packages:

```sh
php /tmp/oracle-php/oracle.php path/to/file.php
```

Minimal tokenization call:

```php
<?php
$tokens = PhpToken::tokenize(file_get_contents($argv[1]), TOKEN_PARSE);
```

### Recall mapping (PHP)

- **Lexical accuracy**: Zend's own lexer handles heredocs, nowdocs, double-quoted string interpolation, inline HTML outside `<?php` tags (`T_INLINE_HTML`), `#` comments versus `#[Attribute]` (`T_ATTRIBUTE`), case-insensitive keywords, and semi-reserved member names (`TOKEN_PARSE`).
- **Functions**: Scan non-ignorable tokens for `T_FUNCTION` (named functions, methods, and anonymous closures with `{ ... }` bodies, skipping abstract or interface `;` declarations) and `T_FN` (arrow functions `fn(...) => expr`).
  Track `start_line` from `$token->line` and `end_line` from the body's closing token line.
  Exclude nested `T_FUNCTION` and `T_FN` spans when counting an enclosing function's decision points (matching `score_function_spans`).
- **Decision points** (matching `decision_increment` in `plugins/languages/php/src/functions.rs`):
  - `T_IF`, `T_ELSEIF`, `T_FOR`, `T_FOREACH`, `T_WHILE`, `T_CASE`, `T_CATCH`, `T_LOGICAL_AND` (`and`), `T_LOGICAL_OR` (`or`), `T_BOOLEAN_AND` (`&&`), `T_BOOLEAN_OR` (`||`), `T_COALESCE` (`??`): +1 each.
  - `T_MATCH`: count depth-0 `T_DOUBLE_ARROW` (`=>`) inside the `match (...) { ... }` block, then subtract 1 (`max(0, arms - 1)`).
  - Ternary `?`: +1 for single-character `?` tokens that follow an expression-ending token (not nullable types like `?int` or `?string`).
- **Static imports** (matching `plugins/languages/php/src/imports.rs`):
  - Top-level `T_USE` outside class, interface, trait, and enum bodies (excluding closure `use (` and `use function` or `use const`): assemble full names from `T_NAME_QUALIFIED`, `T_NAME_FULLY_QUALIFIED`, `T_STRING`, `T_NS_SEPARATOR`, and group `{ ... }` blocks while skipping `T_AS` aliases; emit with `use:` prefix.
  - `T_REQUIRE`, `T_REQUIRE_ONCE`, `T_INCLUDE`, `T_INCLUDE_ONCE`: extract simple `T_CONSTANT_ENCAPSED_STRING` literals (with optional `(` and optional `T_DIR` `.` prefix); emit with `req:` prefix.

### Blind spots and semantic mismatches (PHP)

- **Tokenizer is not a full syntax tree**: Zend emits the exact same single-character `?` token for nullable types (`?int`) and ternary expressions (`$x ? 1 : 0`), as verified on PHP 8.5.11.
  A `PhpToken` oracle still relies on token context for ternary `?`, arrow-function expression ends, and brace matching.
  If those token heuristics diverge from ground truth on real repositories, run `nikic/php-parser` in a scratch folder as a second-stage syntax-tree oracle.
- **PHP 8.0+ combined name tokens**: `PhpToken::tokenize` emits `Foo\Bar` as one `T_NAME_QUALIFIED` token and `\Foo\Bar` as one `T_NAME_FULLY_QUALIFIED` token.
  The oracle must handle these token IDs in `use` statements, whereas codeint splits identifiers and `\` tokens.
- **PHP 8.1+ enum `case` tokens**: `T_CASE` appears in both `switch` blocks and `enum` declarations.
  Counting decision points strictly inside function bodies prevents enum `case` declarations from inflating counts.
- **PHP 8.4+ property hooks**: Neither codeint nor `T_FUNCTION` extracts PHP 8.4 property hooks (`get { ... }`, `set { ... }`) as functions.
  A full syntax-tree oracle must exclude property hooks to stay aligned with current codeint rules.
- **Short open tags**: `PhpToken::tokenize` follows the runtime `short_open_tag` INI setting for `<?` tags (`<?=` is always `T_OPEN_TAG_WITH_ECHO`).
  Keep the default `short_open_tag=0` unless testing legacy short-tag repositories.

### Primary sources (PHP)

- PHP manual for the Tokenizer module (checked 2026-11-25): <https://www.php.net/manual/en/book.tokenizer.php>
- PHP manual for `PhpToken` class (checked 2026-11-25): <https://www.php.net/manual/en/class.phptoken.php>
- PHP manual for `PhpToken::tokenize` (checked 2026-11-25): <https://www.php.net/manual/en/phptoken.tokenize.php>
- PHP manual for `token_get_all` (checked 2026-11-25): <https://www.php.net/manual/en/function.token-get-all.php>
- PHP manual list of parser tokens (checked 2026-11-25): <https://www.php.net/manual/en/tokens.php>
- PECL `ast` package page (checked 2026-11-25): <https://pecl.php.net/package/ast>
- `nikic/php-ast` source repository (checked 2026-11-25): <https://github.com/nikic/php-ast>

## Dart

### Official API and minimum version (Dart)

Dart does not ship a Dart source parser in its built-in `dart:*` core libraries (verified in the local Dart 3.11.0 SDK `lib/` directory and on `dart.dev/libraries`).
`dart:mirrors` provides JIT runtime reflection only, cannot parse arbitrary source files, and is deprecated for removal in Dart 4.0.
The `dart analyze` CLI emits static diagnostics only and does not dump a syntax tree.

The official first-party parser is `package:analyzer`, maintained by the Dart SDK team on `pub.dev`.
Use `parseString` (or `parseFile`) from `package:analyzer/dart/analysis/utilities.dart` together with `RecursiveAstVisitor` from `package:analyzer/dart/ast/visitor.dart`.
Pass `featureSet: FeatureSet.latestLanguageVersion()` and `throwIfDiagnostics: false` so standalone files parse cleanly without a surrounding project `package_config.json`.

Require Dart SDK 3.0+ for Dart 3 pattern matching (`SwitchExpression`, `SwitchPatternCase`, records, and sealed classes) and an `analyzer` version compatible with the installed SDK.
Local runtime: `/Users/jmm/fvm/default/bin/dart` (`Dart SDK 3.11.0`).

### Scratch-folder invocation (Dart)

Create an isolated scratch package outside codeint with a minimal `pubspec.yaml`:

```yaml
name: dart_oracle
environment:
  sdk: ">=3.0.0 <4.0.0"
dependencies:
  analyzer: any
```

Install once and compile to a native executable for fast batch evaluation across repositories:

```sh
cd /tmp/oracle-dart
dart pub get
dart compile exe bin/oracle.dart -o /tmp/oracle-dart/oracle
/tmp/oracle-dart/oracle path/to/file.dart
```

Minimal parsing call:

```dart
import 'package:analyzer/dart/analysis/features.dart';
import 'package:analyzer/dart/analysis/utilities.dart';

final result = parseString(
  content: source,
  featureSet: FeatureSet.latestLanguageVersion(),
  throwIfDiagnostics: false,
);
final unit = result.unit;
final lineInfo = result.lineInfo;
```

### Recall mapping (Dart)

- **Functions**: Visit four node types and skip any declaration whose body is `EmptyFunctionBody` (abstract methods, external declarations, and bodyless constructors such as `const Point(this.x, this.y);`):
  - `FunctionDeclaration` (top-level and local named functions, getters, setters).
  - `MethodDeclaration` (instance and static methods, getters, setters, operators).
  - `ConstructorDeclaration` (generative and factory constructors with bodies).
  - `FunctionExpression` (anonymous closures, skipping nodes whose direct parent is a `FunctionDeclaration` to avoid double-counting).
  Translate offsets to 1-based lines with `lineInfo.getLocation(offset).lineNumber`.
  Exclude nested functions and closures when counting the enclosing function's decision points (matching `score_function_spans`).
- **Decision points** (matching `decision_weight` in `plugins/languages/dart/src/functions.rs`):
  - `IfStatement` and collection `IfElement`: +1 each.
  - `ForStatement` and collection `ForElement`: +1 each.
  - `WhileStatement` and `DoStatement`: +1 each.
  - `SwitchCase` and `SwitchPatternCase`: +1 each (exclude `SwitchDefault`).
  - `SwitchExpression`: `max(0, node.cases.length - 1)` (matching `count_switch_expr_arms`).
  - `CatchClause` where `node.catchKeyword != null`: +1 (see mismatch below).
  - `BinaryExpression` with operator `&&`, `||`, or `??`: +1 each.
  - `ConditionalExpression` (`cond ? a : b`): +1 each (cleanly distinguishes ternary `?` from nullable types `int?`, null-aware `?.`, and pattern null-checks).
- **Static imports** (matching `plugins/languages/dart/src/imports.rs`):
  - `ImportDirective` and `ExportDirective`: emit `node.uri.stringValue` plus each `cfg.uri.stringValue` in `node.configurations` (conditional `if (dart.library.io) '...'` URIs).
  - `PartDirective`: emit `node.uri.stringValue`.
  - Skip `PartOfDirective` and `LibraryDirective`.

### Blind spots and semantic mismatches (Dart)

- **Scratch pub resolution required**: Unlike Python, Go, and PHP, the Dart oracle requires one-time `dart pub get` network access (or a pre-populated pub cache) in the scratch directory.
  Compiling once with `dart compile exe` removes Dart VM startup overhead on multi-repository runs and runs offline thereafter.
- **Function start-line offset**: Codeint sets `start_line` to the `(` token line (or the `get` keyword line for getters), whereas `analyzer`'s `node.offset` starts on the first annotation or return-type line.
  Align oracle start lines by taking `parameters.leftParenthesis.offset` (or `propertyKeyword.offset` for getters), or match functions by `(name, end_line)` with header-line tolerance.
- **Bare `on Type { ... }` catch clauses**: Codeint's `decision_weight` counts only the `catch` keyword, not bare `on Type { ... }` blocks (even though `compute_dart_cognitive` scores bare `on` clauses).
  The oracle must filter `CatchClause` to `node.catchKeyword != null` when measuring `decision_points` Recall.
- **Diagnostics on invalid or pre-null-safety files**: Check `result.errors.isNotEmpty` when `throwIfDiagnostics: false` and exclude files with syntax errors from Recall denominators.

### Primary sources (Dart)

- Dart core libraries overview (checked 2026-11-25): <https://dart.dev/libraries>
- Dart CLI tool documentation (checked 2026-11-25): <https://dart.dev/tools/dart-tool>
- `package:analyzer` package page (checked 2026-11-25): <https://pub.dev/packages/analyzer>
- `parseString` API reference (checked 2026-11-25): <https://pub.dev/documentation/analyzer/latest/dart_analysis_utilities/parseString.html>
- `parseFile` API reference (checked 2026-11-25): <https://pub.dev/documentation/analyzer/latest/dart_analysis_utilities/parseFile.html>
- `RecursiveAstVisitor` API reference (checked 2026-11-25): <https://pub.dev/documentation/analyzer/latest/dart_ast_visitor/RecursiveAstVisitor-class.html>

## Local availability and CI gating

All four runtimes are present on the local evaluation machine:

- `python3`: `Python 3.15.2` (`/opt/homebrew/bin/python3`)
- `go`: `go1.27.2 darwin/arm64` (`/opt/homebrew/bin/go`)
- `php`: `PHP 8.5.11` with `ext/tokenizer` enabled (`/opt/homebrew/bin/php`)
- `dart`: `Dart SDK 3.11.0` (`/Users/jmm/fvm/default/bin/dart`)

Evaluation harnesses and CI workflows should gate each oracle independently instead of assuming all four runtimes exist on every runner:

1. Check runtime presence (`command -v python3`, `command -v go`, `command -v php`, `command -v dart`) and assert minimum versions before running that language's Recall suite.
2. Check `php -r 'exit(class_exists(PhpToken::class) ? 0 : 1);'` to verify PHP 8.0+ and `ext/tokenizer` together.
3. For Dart, cache or pre-build `/tmp/oracle-dart/oracle` in CI; skip the Dart oracle with an explicit reason when offline and no pub cache is present.
4. Exclude files that fail native parsing (`SyntaxError`, `ParseFile` error, `ParseError`, or `result.errors`) from Recall denominators and report the skipped count separately.
