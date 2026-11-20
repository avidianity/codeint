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
The exempt paths are `no_comments.exempt` in [checks.toml](../../checks.toml).
This page is the allow-list file; a change to it is Tier 3.

**Runs in**:
Hook (`cargo xtask hook`) and CI (`repository` job).

**Error text**:

```text
{path}:{line} adds a comment. Delete it. Put the reason in a name, an ADR, a test name, the commit message, or a tracker issue; see docs/agents/engineering.md.
```

Spec: [Factory Kit v0.4.0](https://github.com/avidianity/factory/blob/v0.4.0/checks/no-comments.md).
