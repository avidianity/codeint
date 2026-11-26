---
status: accepted
---

# Context-anchored Finding matching and Baseline schema version 2

## Context

`FindingId` hashes a Metric key, file paths, token content, and a 0-based source-order occurrence index.
That keeps every Finding stable when unrelated lines shift, and gives duplicate Findings in one file distinct identifiers.
When a developer inserts an identical Finding above an existing legacy Finding in the same file, the new Finding takes occurrence `0` and the legacy Finding shifts to occurrence `1`.
Matching by `FindingId` alone then hides or marks unchanged the new Finding at the top, and blames the untouched legacy Finding further down (Issue #542).

Changing `FindingId` to include surrounding context would break every existing `FindingId`, bump every Metric version, invalidate committed Ratchet files, and require Reference range updates across every Language plugin.
Instead, we want to keep `FindingId` and every Metric version unchanged while pairing duplicate Findings accurately in both `codeint check` (Baseline) and `codeint diff`.

## Decision

**Amend [ADR 0030](./0030-finding-baseline.md) so `.codeint/baseline.json` uses `schema_version` `2` with hashed surrounding-line context anchors, and use the same context-anchored matching in both Baseline hiding and `codeint diff`.**

1. **`FindingId` and Metric versions stay unchanged**:
   No `FindingId` formula changes, no Metric version bumps, and no Ratchet or Reference range invalidates.
2. **Hashed context anchors only**:
   Each Baseline entry in `schema_version` `2` stores the Finding's `id`, its occurrence-free `content` hash, optional one-way hashes of the nearest preceding (`before`) and following (`after`) non-empty token lines, and an optional 1-based `line` hint.
   No raw source text is ever stored in `.codeint/baseline.json` or cached commit measurements.
3. **Matching rule for identical-content Findings**:
   Built-in Findings group by occurrence-free `content` hash.
   When a group has one reference and one candidate, they always match, so editing neighboring lines around a single Finding never creates a false positive.
   When a group has multiple Findings on either side, exact maximum-weight bipartite matching pairs `min(|references|, |candidates|)` Findings by context score (both sides matching > one side matching > neither, with token-line matches outranking file-boundary matches), then closest line distance, then identical `id`.
   Groups larger than 64 entries use a linear-time hash-bucketed fallback with the same priority order.
4. **Backward compatibility with `schema_version` `1`**:
   `codeint check` and `codeint baseline update` continue to read `schema_version` `1` Baseline files cleanly by falling back to exact `id` matching.
   The next `codeint baseline update` upgrades the file to `schema_version` `2`, preserving unmeasured Metrics as id-only entries until they are measured.
5. **Shared matching in `codeint diff`**:
   `CommitMeasurement` persists each Finding's anchor alongside its row, and `codeint diff` uses the same matcher to classify `new`, `resolved`, and `unchanged` Findings.
   The commit measurement cache version increments so cached commits are re-measured with context anchors.

## Considered options

- **Fold surrounding-line hashes directly into `FindingId` and bump all Metric versions.**
  Rejected: breaks every single-Finding match when an adjacent line changes, forces Metric version bumps across every Metric, and invalidates Ratchet and Reference range records.
- **Run Git blame or line-level Git diff hunks to map legacy lines.**
  Rejected: `codeint check` has no base commit for Baseline hiding, and adding Git blame adds process overhead and fails on untracked or unstaged edits.

## Consequences

- Inserting an identical Finding above an existing legacy Finding reports the new line and keeps the legacy line hidden in `codeint check` and unchanged in `codeint diff`.
- Inserting an adjacent duplicate Finding hides or marks unchanged one occurrence and reports one new Finding, never two.
- Editing lines next to a single baselined or base-commit Finding never breaks matching.
- Existing `schema_version` `1` `.codeint/baseline.json` files keep working without error and upgrade automatically on `codeint baseline update`.
- `.codeint/baseline.json` grows slightly per Finding to hold compact hexadecimal hashes and line hints, while containing zero raw source text.
