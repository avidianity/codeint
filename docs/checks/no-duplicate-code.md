# no-duplicate-code

**Invariant**:
A change adds no block of code that duplicates another block in the Project.
A duplicate is a block of 10 or more lines, or 100 or more tokens, that appears twice when formatting is ignored.
Test files count.
Generated files are exempt.

The thresholds are Defaults.
The Project keeps them in the check's configuration, changes them only by an ADR, and a change to them is Tier 3; see [test-oracle-guard](./test-oracle-guard.md).

**Implemented by**:
jscpd 5.4.3 with [.jscpd.json](../../.jscpd.json), run as `npm run duplicates`, over Rust, JavaScript, JSX, TypeScript, and TSX files.
The script runs two passes, so a duplicate fails when it has 10 or more lines or 100 or more tokens.
The thresholds live in the `duplicates` script of [package.json](../../package.json); a change to them is Tier 3.

**Runs in**:
Hook (`duplicates` in [lefthook.yml](../../lefthook.yml)) and CI (`repository` job).

**Error text**:

```text
{file}:{lines} duplicates {other file}:{lines}. Extract one shared unit and call it from both places.
```

Spec: [Factory Kit v0.4.0](https://github.com/avidianity/factory/blob/v0.4.0/checks/no-duplicate-code.md).
