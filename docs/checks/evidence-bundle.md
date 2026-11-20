# evidence-bundle

**Invariant**:
Every pull request body has the sections of the Evidence bundle template: Tier, Task, Fail-first test, Checks, Safety proof, Runtime evidence, and Gaps.
Each section has content, or the words "Not applicable" with a reason.

**Implemented by**:
`cargo xtask evidence-bundle --body-file {body file}`.
It requires each section of [the pull request template](../../.github/pull_request_template.md), with content and no unfilled placeholder.

**Runs in**:
CI, `pull-request` workflow, `body` job.

**Error text**:

```text
The pull request body lacks {section}. Use templates/pr-evidence.md.
```

Spec: [Factory Kit v0.4.0](https://github.com/avidianity/factory/blob/v0.4.0/checks/evidence-bundle.md).
