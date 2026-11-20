# no-placeholders

**Invariant**:
No file in `.rules`, `docs/agents/`, or `docs/checks/` holds a Kit placeholder: text in angle brackets that starts with a lowercase word, such as the words "main branch" in angle brackets.
Code samples inside fenced blocks are exempt only when the block is marked as a template.

**Implemented by**:
`cargo xtask no-placeholders`, over the `placeholders.paths` globs in [checks.toml](../../checks.toml).
Curly braces such as `{name}` mark a value to fill in at use time; they are not placeholders.
A fenced block whose info string is `template` is exempt.

**Runs in**:
Hook (`cargo xtask hook`) and CI (`cargo xtask merged-result`).

**Error text**:

```text
{file}:{line} still holds the placeholder {text}. Replace it with the Project value.
```

Spec: [Factory Kit v0.4.0](https://github.com/avidianity/factory/blob/v0.4.0/checks/no-placeholders.md).
