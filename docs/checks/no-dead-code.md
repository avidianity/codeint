# no-dead-code

**Invariant**:
A pull request adds no dead symbol.
A dead symbol is a file, function, type, constant, export, or dependency that nothing reachable from a Project entry point uses.
The exported API of a published package is an entry point.
A symbol in application code that only tests use is dead.

The check compares the dead symbols on the pull request head with those on the base branch.
It fails when the head has a dead symbol that the base branch lacks, wherever that symbol is.

**Implemented by**:
The workspace lints in [Cargo.toml](../../Cargo.toml) deny `unused`, which includes `dead_code`, and `unreachable_pub`.
CI runs Clippy with `-D warnings`.
A function that only tests use is dead, because the build without tests does not use it.
`cargo shear` (cargo-shear 1.14.1) finds unused dependencies.

**Accepted gap**:
The Rust compiler checks `dead_code` one crate at a time.
It does not report a `pub` item of one workspace library crate that no other crate uses.
The Project accepts this gap for three reasons:

- `unreachable_pub = "deny"` already forces every crate-internal item to be private or `pub(crate)`, so `dead_code` catches it.
- `cargo shear` already catches every unused crate dependency across the workspace.
- Exact cross-crate `pub` analysis requires whole-workspace type resolution.
  A syntax-only check gives false positives on integration-test helpers, types in public signatures, and macro-generated items.
  Whole-workspace analysis tools exceed the Feedback budget in [feedback-budget](./feedback-budget.md).

**Runs in**:
CI (`rust` job for Clippy, `repository` job for `cargo shear`), and every local build.

**Error text**:

```text
error: function `{name}` is never used
cargo shear names each unused dependency and exits with status 1.
```

Spec: [Factory Kit v0.4.0](https://github.com/avidianity/factory/blob/v0.4.0/checks/no-dead-code.md).
