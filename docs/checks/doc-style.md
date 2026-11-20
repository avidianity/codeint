# doc-style

**Invariant**:
Markdown files contain no em dash character (U+2014), and pass the Project's Markdown linter.

**Implemented by**:
`cargo xtask doc-style` for the em dash, and markdownlint-cli2 0.24.2 with [.markdownlint-cli2.jsonc](../../.markdownlint-cli2.jsonc).
Installed third-party skills are exempt, because the Project does not edit them.

**Runs in**:
Hook (`cargo xtask hook` and `markdown` in [lefthook.yml](../../lefthook.yml)) and CI (`repository` job).

**Error text**:

```text
{path}:{line} uses an em dash. Use a plain dash "-" or split the sentence.
```

Spec: [Factory Kit v0.4.0](https://github.com/avidianity/factory/blob/v0.4.0/checks/doc-style.md).
