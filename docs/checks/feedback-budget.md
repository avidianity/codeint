# feedback-budget

**Invariant**:
Each Feedback loop finishes within its Feedback budget, measured in CI on the Project's standard runner.
The Default budgets are 30 seconds for the Git hook, 2 minutes for the Relevant tier, and 10 minutes for the CI gate.
When a loop takes 80% of its budget or more, the check opens one issue with the `budget` label for that loop, or updates the open one.
When a loop takes more than its budget, the check fails.

**Implemented by**:
`cargo xtask feedback-budget --loop {hook|relevant|ci-gate} ... --report`.
The budget file is the `feedback_budget` table in [checks.toml](../../checks.toml).
At 80% of a budget it opens or updates one issue labelled `budget`; over the budget it fails.

**Runs in**:
CI, `budget` job of the `ci` workflow, on every pull request.
It times the Git hook on the changed files, `cargo xtask affected-tests`, and the wall clock of the other jobs in the run.
It opens or updates the `budget` issue only for pull requests from this repository, because a fork's token cannot write issues.

**Error text**:

```text
{loop} took {seconds} seconds; its Feedback budget is {budget} seconds. Make the loop faster in this pull request. Never raise the budget to pass; see docs/agents/testing.md.
```

Spec: [Factory Kit v0.4.0](https://github.com/avidianity/factory/blob/v0.4.0/checks/feedback-budget.md).
