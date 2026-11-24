---
status: accepted
---

# codeint reports Hazards next to Software entropy

The Owner decided this on 2026-11-24 while adding SQL support.

## Context

Every Metric before this one measures Software entropy, the maintainability cost carried in the shape of code.
The Owner wants codeint to check SQL for patterns that are likely to cause wrong results, slow execution, or a security exposure.
Examples are a `DELETE` without `WHERE`, a `LIKE` pattern with a leading wildcard, and a join on a column without an index.
These patterns are a risk of failure, not a maintainability cost.
A Flagged line is a verbose-code pattern, so these patterns do not fit it.
A Metric plugin works on every language that has a Language plugin, so a Metric for SQL alone does not fit either.

## Decision

**codeint reports a Hazards Metric, the share of source lines that are Hazard lines, and leaves the meaning of Software entropy unchanged.**
A Language plugin owns the Hazard line rules of its language and reports each match in the Code model.
The Hazards Metric counts Hazard lines without knowing the language.
A Hazard line names its rule and carries no severity level.
A Hazard line Finding shows the path, the line, and the rule, and never the source text, because a rule can match a line that holds a secret.
One configuration list turns named Hazard line rules off.
Hazards is absent, never 0, for a language whose Language plugin has no Hazard line rules.
A Metric is now one named measurement of a codebase, and every Metric except Hazards measures Software entropy.

## Considered options

- **Keep these patterns out of codeint.**
  Rejected: the Owner wants one tool to report them.
- **Widen Flagged line to hold them.**
  Rejected: Verbosity would count a `DELETE` without `WHERE` as verbose code.
- **A Metric for SQL alone.**
  Rejected: it breaks the rule that a Metric plugin knows no language.
- **Widen Software entropy to include hazards.**
  Rejected: the term would then mean anything that codeint reports.
- **Severity levels on a Hazard line, shown or used as weights.**
  Rejected: levels are one author's opinion and read as grades, and weights cannot be calibrated honestly, as ADR 0003 says.

## Consequences

- codeint does a second job, and its one-line description says so.
- Every Hazard line counts the same, so an advice rule weighs as much as a destructive statement.
  A team turns off a rule that does not fit its codebase.
- A Hazard line rule matches parsed source, never raw text, so a match is exactly what the rule names.
- Any Language plugin may add Hazard line rules later without a change to the Metric.
- The Code model gains Hazard lines, which needs a new version of the Plugin interface.
- Hazards has no Reference range until a panel of public repositories is measured and approved in an ADR.
