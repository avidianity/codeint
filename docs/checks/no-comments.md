# no-comments

**Invariant**:
No line that a change adds to a source file holds a comment, except a comment on the allow-list.
The allow-list is:

- a shebang line;
- a license header that the Project's license requires;
- a directive that the language, compiler, or build tool reads, such as an encoding declaration or a build constraint;
- a doc comment on an exported symbol of a published package, when the package's API reference is generated from it.

Generated files are exempt.
Test input files under `crates/*/tests/fixtures/` are exempt, because their comments are data that codeint reads, not explanations of codeint's code.
A suppression directive is not on the allow-list; [no-suppressions](./no-suppressions.md) owns it.

**Implemented by**:
`cargo xtask no-comments --staged` in the hook, and `cargo xtask no-comments --base origin/main` in CI.
It scans the added lines of Rust, WIT, JavaScript, JSX, TypeScript, TSX, TOML, YAML, shell, and PowerShell files.
It reads the diff as text even when `.gitattributes` marks a file as binary.

The allow-list:

- a shebang (`#!/`) on line 1 of a shell, PowerShell, JavaScript, or TypeScript file;
- a doc comment (`///` or `/** */`) in a `.wit` file, because the WIT interface is the published Plugin API;
- the tool directives `# yaml-language-server:` and `# shellcheck shell=`.

Rust doc comments are not on the allow-list, because no Rust crate is published.
The test input paths are `test_input.paths` in [checks.toml](../../checks.toml); [no-duplicate-code](./no-duplicate-code.md) reads the same list.
This page is the allow-list file; a change to it is Tier 3.

**Scanner design**:
The check uses a zero-dependency line lexer in `xtask/src/lexer.rs` instead of per-language parsers.
Full parsers for every supported language would add build dependencies and threaten the 30-second hook and 120-second relevant-tier [feedback budgets](./feedback-budget.md).
The lexer tracks strings, Rust raw strings, PowerShell block comments, YAML block scalars, and script regular expression literals.
In JavaScript and TypeScript, a slash opens a regular expression literal only after an expression-start token.
Inside a regular expression literal, the lexer tracks backslash escapes and `[...]` character classes until the closing slash or line end.
Known trade-offs remain where full parsing is needed, such as a regular expression literal directly after a closing parenthesis or brace.

**Runs in**:
Hook (`cargo xtask hook`) and CI (`repository` job).

**Error text**:

```text
{path}:{line} adds a comment. Delete it. Put the reason in a name, an ADR, a test name, the commit message, or a tracker issue; see docs/agents/engineering.md.
```

Spec: [Factory Kit v0.4.0](https://github.com/avidianity/factory/blob/v0.4.0/checks/no-comments.md).
