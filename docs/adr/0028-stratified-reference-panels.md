---
status: accepted
---

# Stratified 50-repository current-codebase Reference panels

## Context

[ADR 0008](./0008-typescript-reference-ranges.md) established codeint's initial Reference range selection rule: 20 top-starred public repositories per language pinned before 2023-01-01.
That initial rule skews heavily toward mega-starred flagship libraries and compilers, excludes modern language and framework syntax introduced since 2023, and leaves newly added Metrics and Framework profiles uncalibrated.
Before collecting recalibrated measurements in the Evaluation suite, codeint needs a single objective, repeatable panel selection methodology that removes cherry-picking while representing everyday real-world repositories.

## Decision

**Supersede [ADR 0008](./0008-typescript-reference-ranges.md)'s top-20 pre-2023 panel selection rule with a stratified 50-repository current-codebase panel methodology for every base Language plugin and a stratified 20-repository current-codebase panel methodology for every calibrated Framework profile.**

1. **Panel sizes and Production line-count buckets**:
   - **Base-language panels (50 repositories)**:
     - 15 Small repositories (`2,000-9,999` Production source lines).
     - 20 Medium repositories (`10,000-49,999` Production source lines).
     - 15 Large repositories (`50,000+` Production source lines).
   - **Framework profile panels (20 repositories)**:
     - 6 Small repositories (`2,000-9,999` Production source lines).
     - 8 Medium repositories (`10,000-49,999` Production source lines).
     - 6 Large repositories (`50,000+` Production source lines).
2. **Objective GitHub discovery and star bands**:
   - Query public GitHub repositories with `fork:false archived:false` across three star bands: `10,000+`, `1,000-9,999`, and `200-999`.
   - Iterate candidates in strict descending star order within each band, filling open size and shape buckets deterministically without manual cherry-picking.
3. **Shape minimums (per 50-repository base-language panel)**:
   - At least 15 single-package repositories.
   - At least 10 multi-package workspaces (where the language ecosystem has workspace manifests).
   - At least 10 applications or command-line tools.
   - At least 10 libraries or packages.
   - Classify package topology and app-versus-library shape mechanically from repository manifests and entry points.
4. **Exclusions and quality filters**:
   - Exclude mirrors, books, curated awesome lists, templates, boilerplate starters, and tutorial repositories.
   - Exclude repositories where `Generated + Vendored` source lines exceed `Production` source lines.
   - Exclude repositories where `codeint check` fails to exit `0` due to invalid repository layout.
5. **Commit pinning and current-codebase framing**:
   - Pin every selected repository to its latest default-branch commit SHA on the measurement date, with no pre-2023 cutoff.
   - Frame published Reference ranges transparently as typical values across a stratified panel of current public repositories.
6. **Outliers and percentile computation**:
   - Retain all qualifying repositories including statistical outliers once admitted by the objective rules.
   - Compute and publish the 10th and 90th percentiles (linear interpolation over sorted ratios) as `[low, high]` alongside the exact integer Metric version each range was calibrated against.
   - Never copy third-party source code, fixtures, or text into codeint; commit only repository identifiers, pinned commit SHAs, and aggregate measurement numbers.

## Considered options

- **Keep the top-20 pre-2023 rule from ADR 0008.**
  Rejected: 20 top-starred repositories bias percentiles toward unusually large frameworks and compilers and miss post-2022 language and framework idioms.
- **Trim statistical outliers before computing percentiles.**
  Rejected: manual trimming reintroduces subjectivity, while the 10th-to-90th percentile band already bounds extreme tails.

## Consequences

- Every Language plugin and Framework profile Reference range recalibration follows the same committed, auditable selection specification.
- Every recalibrated Reference range carries its exact current integer Metric version and passes CI's Metric-version lockstep check.
- Existing pre-recalibration ranges remain valid until replaced by a full stratified run under this methodology.
