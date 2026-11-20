# no-suppressions

**Invariant**:
No source file holds an inline directive that turns off a lint or type rule, except in a list of approved exceptions that changes only at Tier 3.

**Implemented by**:
`cargo xtask no-suppressions`, plus the Clippy lints `allow_attributes` and `allow_attributes_without_reason`, set to deny in [Cargo.toml](../../Cargo.toml).
It rejects Rust `allow` and `expect` attributes, script directives such as `eslint-disable` and `@ts-ignore`, `shellcheck disable`, `yamllint disable`, `gitleaks:allow`, and Markdown comments that disable or configure markdownlint, in any letter case.
It reads files that are not valid UTF-8 too, and skips only binary files.
The approved exceptions are `suppressions.approved` in [checks.toml](../../checks.toml), empty at Bootstrap.

**Runs in**:
Hook (`cargo xtask hook`) and CI (`cargo xtask merged-result`).

**Error text**:

```text
{file}:{line} suppresses {rule}. Fix the cause, or change the rule for the whole Project through an ADR.
```

Spec: [Factory Kit v0.4.0](https://github.com/avidianity/factory/blob/v0.4.0/checks/no-suppressions.md).
