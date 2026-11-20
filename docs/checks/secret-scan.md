# secret-scan

**Invariant**:
No commit adds text that matches a known secret format or a high-entropy token assigned to a name such as key, token, secret, or password.

**Implemented by**:
gitleaks 8.30.1: `gitleaks git --pre-commit --staged` in the hook, and `gitleaks git` over the whole history in CI.
CI downloads the release and verifies it against the checksum pinned in [the gitleaks action](../../.github/actions/gitleaks/action.yml).

**Runs in**:
Hook (`secrets` in [lefthook.yml](../../lefthook.yml)) and CI (`repository` job).

**Error text**:

```text
WRN leaks found: {count}
```

Spec: [Factory Kit v0.4.0](https://github.com/avidianity/factory/blob/v0.4.0/checks/secret-scan.md).
