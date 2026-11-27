---
status: accepted
---

# Metric version 2 for Duplication and Import boundaries

The Owner decided this on 2026-11-28 during a full codebase review of codeint v0.8.0.

## Context

Reviewing `crates/metrics` and `crates/report` surfaced three measurement flaws in Duplication and Import boundaries:

1. In multi-language repositories, `Report::summarize_with_extras` ran Duplication once per Language plugin and then ran Duplication a second time over all languages merged together.
   Matching normalized tokens across unrelated languages (such as Rust and TypeScript) doubled the cost of the slowest Metric, forced repeated full-model cloning, and could report accidental cross-language clones that cannot be refactored together.
2. In opt-in near-clone detection (`crates/metrics/src/duplication/near_clones.rs`), single-linkage clustering via `DisjointSet` chained pairs A-B-C-D into one Clone group even when A and D fell below `min_similarity`, and locality-sensitive hashing buckets larger than 32 blocks capped pairs by alphabetical file order instead of similarity.
3. In Import boundaries (`crates/metrics/src/boundaries.rs`), enabling `strict_zones` added `UnzonedFile` findings (file count) to the numerator while dividing by `resolved_imports` (import edge count) and clamping the ratio at `1.0`.
   Adding imports to a repository could therefore shrink the ratio of unzoned files, and two unzoned files in a package with one import produced a clamped `1.0` ratio.

Per [ADR 0023](./0023-metric-versioning.md), any change to a Metric's calculation rules increments that Metric's positive integer Metric version.

## Decision

**Bump Duplication to Metric version `2` and Import boundaries to Metric version `2` with the following calculation rules.**

1. **Per-language Duplication aggregation**:
   Run Duplication strictly within each Language plugin's files and aggregate per-language Duplication measurements into the repository total.
   Never match tokens across distinct Language plugins.
   TypeScript, JavaScript, Vue, and Svelte remain inside one Language plugin, so clones across those files continue to match.
2. **Anchor-checked near-clone clustering and similarity-ordered bucket capping**:
   Replace single-linkage near-clone chaining with anchor-checked clustering so every member of a near-clone group meets `min_similarity` against the group anchor.
   When a locality-sensitive hashing bucket exceeds the all-pairs limit, rank candidate pairs by estimated MinHash similarity rather than alphabetical file order.
3. **Combined denominator for Import boundaries under `strict_zones`**:
   When `strict_zones` is `false`, keep the existing `resolved_imports` denominator unchanged.
   When `strict_zones` is `true`, set the denominator to `resolved_imports + evaluated_files` so both import findings and unzoned-file findings sit on a consistent, naturally bounded `[0, 1]` scale without clamping.

## Considered options

- **Keep cross-language Duplication matching in the repository total.**
  Rejected: normalized identifiers and literals across unrelated languages produce noise that cannot be deduplicated in code, and recomputing Duplication on merged models doubles multi-language latency and memory.
- **Keep single-linkage near-clone clustering and alphabetical bucket capping to stay on Duplication version 1.**
  Rejected: path-dependent recall and dissimilar transitive chains undermine trust in opt-in near-clones, and batching this fix with per-language Duplication requires only one Metric version bump.
- **Keep the clamped import-only denominator for `strict_zones` or split `strict_zones` into a separate Metric.**
  Rejected: clamping file counts divided by import counts distorts non-zero Budgets and Ratchets, while adding a separate Metric fragments architectural zone governance; expanding the denominator only when `strict_zones` is enabled matches how external package rules already expand the denominator when enabled.

## Consequences

- Duplication and Import boundaries move to Metric version `2` in `Metric::DESCRIPTORS`.
- Existing `.codeint/ratchet.json` and `.codeint/baseline.json` files must refresh via `codeint ratchet update` and `codeint baseline update`, and commit measurement caches invalidate automatically per [ADR 0023](./0023-metric-versioning.md).
- Built-in Language plugin Reference ranges for Duplication update their `metric-version` to `2` in lockstep; single-language exact-clone panel numbers do not change because single-language exact matching is unchanged.
- Multi-language `codeint check` runs drop redundant whole-repo Duplication work and full-model token clones.
- Repositories with `strict_zones: false` see unchanged Import boundaries ratios.
