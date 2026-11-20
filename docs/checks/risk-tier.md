# risk-tier

**Invariant**:
Every pull request gets a Tier from 0 to 4, computed from its changed paths with the Project's tier map.
The highest Tier among the changed paths applies.
A path in no list is Tier 2.
The check applies the `tier-{n}` label and replaces any `tier-*` label that someone else set.

**Implemented by**:
`cargo xtask risk-tier --base origin/main` computes the Tier in the `risk-tier` job, which stores it as the `tier` artifact.
The `label` workflow then applies the `tier-{n}` label with a write token, so pull requests from forks get it too.
It runs no code from the pull request: it reads the artifact, accepts only a Tier from 0 to 4, and skips a run whose commit is no longer the head of the pull request.
The tier map is the `tier` table in [checks.toml](../../checks.toml): a path in no rule is Tier 2, and the highest matching rule wins.
Tier 4 is not a path: publishing a package, creating a release, or pushing a tag is Tier 4 by the forbidden actions in [security.md](../agents/security.md#mandatory-forbidden-actions).
A change that writes outside the cache folder or the `--output` file is Tier 3 by review, because no path rule can see it.

**Runs in**:
CI, `risk-tier` job of the `ci` workflow, on every pull request event, then the `label` workflow when that run completes.

**Error text**:

```text
Tier {n}: {path} matches {tier map rule}. This Tier needs {approval rule from review-and-merge.md}.
```

Spec: [Factory Kit v0.4.0](https://github.com/avidianity/factory/blob/v0.4.0/checks/risk-tier.md).
