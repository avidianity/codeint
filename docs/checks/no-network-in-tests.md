# no-network-in-tests

**Invariant**:
Automated tests make no network connection beyond the local machine.

**Implemented by**:
The `rust` job runs `cargo test` inside a new network namespace (`unshare --net`) with only the loopback interface, so every non-local connection fails.
Local test runs are not guarded.

**Runs in**:
CI, `rust` job, on every run.

**Error text**:

```text
Test {name} tried to connect to {host kind}. Tests use injected adapters; no network beyond the local machine.
```

Spec: [Factory Kit v0.4.0](https://github.com/avidianity/factory/blob/v0.4.0/checks/no-network-in-tests.md).
