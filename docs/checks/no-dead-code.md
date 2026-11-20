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
Known gap: the compiler does not report a `pub` item of one workspace crate that no other crate uses.

**Runs in**:
CI (`rust` job for Clippy, `repository` job for `cargo shear`), and every local build.

**Error text**:

```text
{symbol} in {file} is dead after this change: nothing reachable from an entry point uses it. Delete it.
```

Spec: [Factory Kit v0.4.0](https://github.com/avidianity/factory/blob/v0.4.0/checks/no-dead-code.md).
