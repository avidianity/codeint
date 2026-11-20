# commit-message

**Invariant**:
Each commit message follows Conventional Commits with an allowed scope.
It holds no co-author line, no agent attribution, and no marker that tells CI to skip the commit.

**Implemented by**:
`cargo xtask commit-message --file {message file}` and `cargo xtask commit-message --range origin/main`.
Allowed types and scopes are `commit.types` and `commit.scopes` in [checks.toml](../../checks.toml).
A subject that Git writes for a merge, starting with the word "Merge" and a space, is accepted.

**Runs in**:
The `commit-msg` hook in [lefthook.yml](../../lefthook.yml), and CI (`repository` job) for every commit of a pull request.

**Error text**:

```text
Commit message: {problem}. Use "{type}({scope}): {summary}" with no co-author or attribution lines.
```

Spec: [Factory Kit v0.4.0](https://github.com/avidianity/factory/blob/v0.4.0/checks/commit-message.md).
