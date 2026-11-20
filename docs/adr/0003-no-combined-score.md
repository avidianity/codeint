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
Sources checked on 2026-11-21: arXiv:2603.24755, appendix G, and <https://earendil.com/posts/measuring-code-sloppiness>.
They give Verbosity 0.15 ± 0.06 for established repositories against 0.33 ± 0.10 for agent-written code, and Erosion 0.31 ± 0.17 against 0.68 ± 0.20.

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
