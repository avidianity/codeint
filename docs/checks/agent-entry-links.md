# agent-entry-links

**Invariant**:
Each agent entry file that [adapters/agents.md](https://github.com/avidianity/factory/blob/v0.4.0/adapters/agents.md) lists for the Project's agents is a symbolic link to `.rules`, and `.rules` exists.

**Implemented by**:
`cargo xtask agent-entry-links`, which reads `rules.file` and `rules.entry_files` in [checks.toml](../../checks.toml).

**Runs in**:
Hook (`cargo xtask hook`) and CI (`cargo xtask merged-result`).

**Error text**:

```text
{file} must be a link to .rules. Edit .rules only, then recreate the link.
```

Spec: [Factory Kit v0.4.0](https://github.com/avidianity/factory/blob/v0.4.0/checks/agent-entry-links.md).
