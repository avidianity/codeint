# no-duplicate-code

**Invariant**:
A change adds no block of code that duplicates another block in the Project.
A duplicate is a block of 11 or more lines, or 100 or more tokens, that appears twice when formatting is ignored.
Test files count.
Generated files are exempt.
A clone whose every copy lies under `crates/*/tests/fixtures/` is exempt, because clone-detection fixtures must hold duplicates on purpose.
A clone between a fixture and any other file still fails.

The thresholds are Defaults.
The Project keeps them in the check's configuration, changes them only by an ADR, and a change to them is Tier 3; see [test-oracle-guard](./test-oracle-guard.md).

**Implemented by**:
`cargo xtask no-duplicate-code`, which `npm run duplicates` calls.
It runs jscpd 5.4.3 with [.jscpd.json](../../.jscpd.json) over Rust, JavaScript, JSX, TypeScript, and TSX files, once per pass in `duplicates.passes` of [checks.toml](../../checks.toml), so a duplicate fails when it has 11 or more lines (`min_lines = 10` in jscpd) or 100 or more tokens.
It reads the jscpd JSON report and drops a clone only when both copies match `test_input.paths` in [checks.toml](../../checks.toml), the same list that [no-comments](./no-comments.md) reads.
A clone found by both passes may print twice if the passes report different line ranges.
The thresholds and the test input paths live in `checks.toml`; a change to them is Tier 3.

**Runs in**:
Hook (`duplicates` in [lefthook.yml](../../lefthook.yml)) and CI (`repository` job).

**Error text**:

```text
{file}:{start}-{end} duplicates {other file}:{start}-{end}. Extract the shared code into one place; see docs/checks/no-duplicate-code.md.
```

If jscpd cannot start, exits non-zero, or writes an unreadable report, the check prints that tool error instead and omits the clone line above.

Spec: [Factory Kit v0.4.0](https://github.com/avidianity/factory/blob/v0.4.0/checks/no-duplicate-code.md).
