# test-oracle-guard

**Invariant**:
A pull request that deletes a test, skips a test, removes an assertion, adds a lint or type suppression, raises a test timeout, or changes CI or check configuration is at least Tier 3.
Check configuration includes the Feedback budget file, the comment allow-list, and the duplicate-code thresholds.

**Implemented by**:
Part of `cargo xtask risk-tier`.
It raises a change to Tier 3 when it:

- deletes a test file;
- adds a Rust attribute that mentions `ignore`, or a `.skip(`, `.only(`, `.todo(`, `xit(`, or `xdescribe(` call in a script test;
- removes a `#[test]` attribute or a `cfg(test)` condition from a Rust file;
- removes or changes any line of an assertion in a test file, including the lines inside a multi-line assertion;
- changes a test timeout;
- adds a suppression;
- changes a `lints` table in a `Cargo.toml`.

Check configuration paths are Tier 3 in the tier map, wherever they sit in the tree: `.gitattributes`, `clippy.toml`, `rustfmt.toml`, markdownlint, jscpd, lychee, gitleaks, and lefthook files.

**Runs in**:
CI, as an input to the `risk-tier` job.

**Error text**:

```text
Tier 3: {path} weakens a test or check: it {reason}.
```

Spec: [Factory Kit v0.4.0](https://github.com/avidianity/factory/blob/v0.4.0/checks/test-oracle-guard.md).
