# merged-result

**Invariant**:
The whole-repository checks run on the tree that a merge will produce, right before each merge.

**Implemented by**:
`cargo xtask merged-result`.
It runs every whole-repository check: agent-entry-links, skills-link, doc-size, doc-style, no-placeholders, record-numbers, dependency-pinning, config-access, and no-suppressions.

**Runs in**:
The merge step of a request to land, as [git.md](../agents/git.md#mandatory-check-the-merged-result-before-a-merge) says, and CI (`repository` job).

**Error text**:

```text
The error text of the whole-repository check that failed.
```

Spec: [Factory Kit v0.4.0](https://github.com/avidianity/factory/blob/v0.4.0/checks/merged-result.md).
