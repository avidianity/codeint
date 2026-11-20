# doc-size

**Invariant**:
`.rules` and every page in its "Always read" column are each under 32,000 bytes.

**Implemented by**:
`cargo xtask doc-size`.
It reads the "Always read" column of `.rules` and the budget `rules.size_budget_bytes` in [checks.toml](../../checks.toml).

**Runs in**:
Hook (`cargo xtask hook`) and CI (`cargo xtask merged-result`).

**Error text**:

```text
{file} is {n} bytes; the budget is 32000. Split it or move detail to a reference page. Never raise the budget.
```

Spec: [Factory Kit v0.4.0](https://github.com/avidianity/factory/blob/v0.4.0/checks/doc-size.md).
