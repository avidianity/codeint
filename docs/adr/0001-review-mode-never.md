---
status: accepted
---

# Review mode is Never

The Owner chose this Review mode on 2026-11-21 during Bootstrap from the Factory Kit v0.4.0.

## Context

codeint is an open source command-line tool built mostly by coding agents.
The Factory Kit offers three Review modes: Always, Critical only, and Never.
The Owner wants agents to land work without reading every diff, while keeping control of risky changes.

## Decision

**codeint uses the Review mode Never.**

- Tier 0 to 2 changes: the agent merges on green CI and a complete Evidence bundle.
- Tier 3 changes: an Independent reviewer approves the code, then the Owner approves the Risk summary.
- Tier 4 changes: the Owner approves the Risk summary, and the agent never runs the change alone.

The Owner still makes every Decision and can override the mode for one pull request by saying so on that pull request.

## Considered options

- **Always.**
  Rejected: the Owner does not want to review every routine diff.
- **Critical only.**
  Rejected: the Owner prefers an Independent reviewer plus a plain-language Risk summary over reading tier 3 diffs.

## Consequences

- Routine Metric, Language plugin, and CLI work lands without waiting for the Owner.
- The tier check and the Independent reviewer carry the safety load, so the tier map must stay accurate.
- Changing the mode needs a new ADR that supersedes this one.
