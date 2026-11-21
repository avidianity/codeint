---
status: accepted
---

# codeint reports each Metric on its own and has no combined score

The Owner decided this on 2026-11-21 while choosing the v1 Metrics.

## Context

A single number is easy to read and easy to gate on.
A weighted blend of Metrics needs weights that cannot be calibrated honestly, and it hides which Metric moved.
Once a number becomes a target it stops measuring well (Goodhart's law).
Published Reference ranges exist per Metric, not for a blend.
Source checked on 2026-11-21: arXiv:2603.24755 version 2, section 3.3, Table 2.
It gives Verbosity 0.19 ± 0.11 for 473 human-written repositories against 0.44 ± 0.18 for agent-written code, and Erosion 0.34 ± 0.22 against 0.68 ± 0.20.
These ranges cover Python only, measured with 137 Python ast-grep rules; the paper publishes no range for any other language.

## Decision

**codeint reports every Metric separately, each next to its Reference ranges when they exist, and never computes a blended score, index, or grade.**
CI gates on per-Metric budgets and on per-Metric regressions against a base Git ref.

## Considered options

- **A weighted 0 to 100 index.**
  Rejected: arbitrary weights, hidden movement, and an easy Goodhart target.
- **Letter grades per Metric.**
  Rejected: grade boundaries are as arbitrary as weights and lose precision.

## Consequences

- A user sees exactly which Metric changed and by how much.
- Reports and the JSON schema have no score field, and adding one later needs a new ADR.
- Users set one budget per Metric instead of one overall threshold.
- Amended 2026-11-21: a Reference range is shown only for the language it was measured in.
  TypeScript and JavaScript show no Reference range until codeint measures its own panel of public repositories and the Owner approves the result in an ADR.
