# behavior-has-test

**Invariant**:
A pull request that changes application source files also changes or adds a test file, or its body says "No behavior change" with a reason.

**Implemented by**:
`cargo xtask behavior-has-test --base origin/main --body-file {body file}`.
Source and test paths are the `behavior` globs in [checks.toml](../../checks.toml).
Unit tests live in sibling `tests.rs` files, as [ADR 0005](../adr/0005-unit-tests-in-sibling-files.md) says.

**Runs in**:
CI, `pull-request` workflow, `body` job, on every pull request event that changes the body or the commits.

**Error text**:

```text
This pull request changes source files but no tests. Add a fail-first test, or state "No behavior change" with a reason.
```

Spec: [Factory Kit v0.4.0](https://github.com/avidianity/factory/blob/v0.4.0/checks/behavior-has-test.md).
