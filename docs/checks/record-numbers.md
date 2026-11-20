# record-numbers

**Invariant**:
Numbered records, such as ADRs and migrations, have unique numbers with no gaps in their sequence.

**Implemented by**:
`cargo xtask record-numbers`, over the `records.folders` in [checks.toml](../../checks.toml).

**Runs in**:
Hook (`cargo xtask hook`) and CI (`cargo xtask merged-result`).

**Error text**:

```text
Record number {n} is used by {file a} and {file b}. Renumber the newer one to {next free number}.
```

Spec: [Factory Kit v0.4.0](https://github.com/avidianity/factory/blob/v0.4.0/checks/record-numbers.md).
