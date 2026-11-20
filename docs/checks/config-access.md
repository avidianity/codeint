# config-access

**Invariant**:
Only the Project's configuration modules read environment variables or other process configuration.

**Implemented by**:
`cargo xtask config-access`.
It rejects `env::var`, `env::args`, and `env::{` reads in Rust code outside the `config_access.modules` globs in [checks.toml](../../checks.toml).

**Runs in**:
Hook (`cargo xtask hook`) and CI (`cargo xtask merged-result`).

**Error text**:

```text
{file}:{line} reads configuration directly. Read it in {config module} and pass the typed value.
```

Spec: [Factory Kit v0.4.0](https://github.com/avidianity/factory/blob/v0.4.0/checks/config-access.md).
