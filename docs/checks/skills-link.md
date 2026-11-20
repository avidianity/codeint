# skills-link

**Invariant**:
The Project has one skills folder.
Each other skills folder that an agent of the Project reads is a symbolic link to it.
Each skill in the folder is listed in the skills lock file or is a Project skill.

**Implemented by**:
`cargo xtask skills-link`, which reads the `skills` table in [checks.toml](../../checks.toml) and `skills-lock.json`.

**Runs in**:
Hook (`cargo xtask hook`) and CI (`cargo xtask merged-result`).

**Error text**:

```text
{folder} must be a link to {skills folder}.` or `{skill} is not pinned in the skills lock file. Install it with the skills installer.
```

Spec: [Factory Kit v0.4.0](https://github.com/avidianity/factory/blob/v0.4.0/checks/skills-link.md).
