# Hazards

**Hazards** (`hazards`) measures the share of source lines that are **Hazard lines**: patterns in parsed source code that are likely to cause wrong results, slow execution, or a security exposure.
Hazards sits next to **Software entropy** rather than inside it ([ADR 0016](../adr/0016-hazards-next-to-software-entropy.md)).
Every other Metric in codeint measures Software entropy, the maintainability cost carried in the shape of code.

## How Hazards works

- **Ratio**: `hazard_lines / source_lines`, measured per Source set (`production`, `test`, `generated`, `vendored`), per language, and per Workspace package.
  A line that matches several rules counts as one Hazard line in `hazard_lines` and produces one Finding per matched rule.
- **Absent when no rules exist**: for any language whose Language plugin defines no Hazard line rules (`typescript-javascript`, `rust`, `php`, `dart`, `python`, and `go`), Hazards is absent rather than `0`.
  In a multi-language repository, repository Hazards totals count only files of languages that define Hazard line rules on both numerator and denominator.
- **Equal weight, no severity levels**: every Hazard line counts equally, with no severity tiers, grades, or weights ([ADR 0016](../adr/0016-hazards-next-to-software-entropy.md)).
  Turn off any rule that does not fit your codebase in `hazards.disabled_rules`.
- **AST-only matching**: every Hazard line rule reads parsed statements, never raw text.
  A keyword inside a comment or string literal never matches.
- **Strict non-leakage**: a Hazard line Finding holds only its `id`, repository-relative `path`, 1-based `line`, and `rule` name (plus optional `owners`).
  No report format, log line, or error message prints the matched source text, so a line matched by `plain-text-secret` or `hardcoded-secret` never copies a secret into a report or CI log.
- **No Reference range**: Hazards has no published Reference range.

Currently, the built-in `sql` Language plugin defines 32 Hazard line rules (27 statement rules and 5 schema-backed rules), and the built-in `shell` Language plugin defines 10.
External WebAssembly Language plugins can also export Hazard line rules through `codeint:plugin@0.5.0`; see [docs/plugins.md](../plugins.md).
See the complete 32-rule SQL list in [docs/reference.md#sql](../reference.md#sql) and the 10-rule Shell list in [docs/reference.md#shell](../reference.md#shell).

## Configuration

Turn off named rules, enforce per-Source-set Budgets, and configure `codeint diff` Regression limits in `codeint.jsonc` or `codeint.json`:

```jsonc
{
  "hazards": {
    "disabled_rules": ["select-star", "order-by-random"],
  },
  "budgets": {
    "production": {
      "hazards": 0.02,
    },
    "test": {
      "hazards": 0.1,
    },
  },
  "regression_limits": {
    "hazards": {
      "ratio_rise": 0.01,
      "new_findings": "any_new_finding",
    },
    "production": {
      "hazards": {
        "ratio_rise": 0,
        "new_findings": "any_new_finding",
      },
    },
  },
}
```

### Disabling rules (`hazards.disabled_rules`)

- List rule names under `hazards.disabled_rules` to turn them off across the repository.
- A turned-off rule adds no Hazard line and no Finding in `codeint check`, `codeint diff`, or `codeint trend`.
- In `codeint diff`, the working tree's `hazards.disabled_rules` applies to both the merge-base and the head side.
- An unknown rule name stops the run with exit code `2` and names the unknown rule.

### Budgets (`budgets.{source_set}.hazards`)

- Each Source set (`production`, `test`, `generated`, `vendored`) accepts a `hazards` Budget from `0` to `1`.
- A ratio equal to its Budget passes, compared to four decimal places.
- When Hazards is absent because no measured language has Hazard line rules, a Hazards Budget passes so one shared configuration works across repositories with and without SQL.

### Regression limits (`regression_limits.hazards` and `regression_limits.{source_set}.hazards`)

- Defaults match the structural Metrics: `ratio_rise` of `0.01` and `new_findings` of `any_new_finding` (plus numerator rise, where the numerator is `hazard_lines`), so introducing a brand-new Hazard line fails even when an old Hazard line is removed elsewhere.
- Set `new_findings` to `numerator_rise` to allow replacing one Hazard line with another when `hazard_lines` does not rise, or to `never` (with `ratio_rise: 1`) on `generated` or `vendored` SQL.
- A per-Source-set setting wins over the top-level `regression_limits.hazards` setting.

## Commands, report formats, and MCP

- **`codeint check`**: reports `hazards` (`ratio`, `hazard_lines`, `source_lines`, and `findings`) per Source set, per language, and per Workspace package in TOON, JSON, Markdown, and SARIF.
- **`codeint diff <base>`**: computes Hazards on both the merge-base and head sides, classifies each Finding as `new`, `resolved`, or `unchanged` by its content-based `id` (the rule name and line tokens, never the line number), and checks Budgets and Regression limits.
- **`codeint trend`**: includes the `hazards` ratio for each commit when a language with Hazard line rules is present.
- **SARIF (`--format sarif`)**: emits one SARIF rule per matched Hazard line rule name at level `note` with message `Hazard line: rule <rule>.` and no source text.
  In `codeint diff --format sarif`, only new Hazard lines are listed so code scanning annotates what the change added.
- **MCP (`codeint mcp`)**: the `check`, `diff`, and `trend` MCP tools return Hazards ratios, Findings, Budget failures, and Regressions in their JSON payloads so coding agents can locate and fix risky statements directly.

See [SQL](./sql.md) for SQL Dialect and schema setup, and [docs/reference.md#hazards](../reference.md#hazards) for exact schema keys and report fields.
