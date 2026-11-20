# record-numbers

**Invariant**:
Numbered records, such as ADRs and migrations, have unique numbers with no gaps in their sequence.

**Implemented by**:
`cargo xtask record-numbers`, over the `records.folders` in [checks.toml](../../checks.toml).

**Runs in**:
Hook (`cargo xtask hook`) and CI (`cargo xtask merged-result`).

**Error text**:

```text
Record number {number} is used by {first} and {second}. Renumber the newer one to {next_free}.
Record number {expected} is missing in {folder}. Renumber the records so the sequence has no gaps.
```

Spec: [Factory Kit v0.4.0](https://github.com/avidianity/factory/blob/v0.4.0/checks/record-numbers.md).
