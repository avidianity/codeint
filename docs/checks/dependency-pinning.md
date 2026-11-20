# dependency-pinning

**Invariant**:
Every direct dependency has an exact version, and the lockfile is committed and matches the manifest.

**Implemented by**:
`cargo xtask dependency-pinning`.
It requires `=x.y.z` in every `Cargo.toml` and exact versions in every `package.json`.
It checks that `Cargo.lock` is current with `cargo metadata --locked`, and compares each `package-lock.json` with its manifest.
Every Cargo command in CI also runs with `--locked`, and `npm ci` refuses a stale lockfile.
[ADR 0006](../adr/0006-exact-dependency-pins.md) records the Rust side.

**Runs in**:
Hook (`cargo xtask hook`) and CI (`cargo xtask merged-result`).

**Error text**:

```text
{dependency} is not pinned to an exact version, or the lockfile is stale. Pin it and update the lockfile.
```

Spec: [Factory Kit v0.4.0](https://github.com/avidianity/factory/blob/v0.4.0/checks/dependency-pinning.md).
