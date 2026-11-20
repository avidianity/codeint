# test-oracle-guard

**Invariant**:
A pull request that deletes a test, skips a test, removes an assertion, adds a lint or type suppression, raises a test timeout, or changes CI or check configuration is at least Tier 3.
Check configuration includes the Feedback budget file, the comment allow-list, and the duplicate-code thresholds.

**Implemented by**:
Part of `cargo xtask risk-tier`.
It raises a change to Tier 3 when it deletes a test file, adds `#[ignore`, removes or changes a line with an assertion in a test file, changes a test timeout, adds a suppression, or changes a `lints` table in a `Cargo.toml`.
Check configuration paths are Tier 3 in the tier map.

**Runs in**:
CI, as an input to the `risk-tier` job.

**Error text**:

```text
Tier 3: {file} weakens a test or check ({what changed}). This needs approval under review-and-merge.md.
```

Spec: [Factory Kit v0.4.0](https://github.com/avidianity/factory/blob/v0.4.0/checks/test-oracle-guard.md).
