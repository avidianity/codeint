---
status: accepted
---

# Stratified 100-repository Reference panels with round-robin star bands

## Context

[ADR 0028](./0028-stratified-reference-panels.md) established 50-repository panels per built-in **Language plugin** and 20-repository panels per calibrated **Framework profile**, queried across three GitHub star bands (`10,000+`, `1,000-9,999`, and `200-999`).
Measuring the initial 410-repository panels revealed three concrete weaknesses:

1. **Sequential star-band walk biased flagship panels**: walking bands from top to bottom filled 100% of the TypeScript/JavaScript (minimum 49,588 stars), Python (minimum 48,978 stars), Go (minimum 24,881 stars), React (minimum 22,812 stars), and Vue 3 (minimum 11,254 stars) panels from the `10,000+` band alone.
   [ADR 0031](./0031-shell-reference-ranges.md) corrected this for Shell using round-robin sampling across all three star bands, leaving Shell inconsistent with the other seven languages.
2. **Wide sampling noise on 20-repository Framework profile panels**: at 20 repositories, 90th-percentile bootstrap 95% confidence intervals spanned `+/- 18%` on React Erosion and `+/- 31%` on React Duplication.
3. **Uncalibrated Cognitive complexity and separate Shell manifest**: reports reserve a **Reference range** column for **Cognitive complexity**, yet the **Evaluation suite** collected only Erosion, Duplication, Verbosity, and Import cycles, and kept Shell outside `eval/panels.json`.

Raising base-language panels to 200 repositories would cut sampling error further, but SQL already drops to 348 stars at 50 repositories and 60 Large (50,000+ Production lines) non-generated SQL repositories risk exhausting the `200+`-star pool while quadrupling recalibration cost.

## Decision

**Supersede [ADR 0028](./0028-stratified-reference-panels.md) with a unified stratified current-codebase methodology targeting 100 repositories per Language plugin and 50 repositories per calibrated Framework profile, sampled round-robin across three star bands with an explicit pool-exhaustion floor.**

1. **Target panel sizes and Production Source set line-count buckets**:
   - **Base Language plugin panels (target 100 repositories)**:
     - 30 Small repositories (`2,000-9,999` Production source lines).
     - 40 Medium repositories (`10,000-49,999` Production source lines).
     - 30 Large repositories (`50,000+` Production source lines).
   - **Calibrated Framework profile panels (target 50 repositories)**:
     - 15 Small repositories (`2,000-9,999` Production source lines).
     - 20 Medium repositories (`10,000-49,999` Production source lines).
     - 15 Large repositories (`50,000+` Production source lines).
2. **Mandatory round-robin star-band sampling**:
   - Query public GitHub repositories with `fork:false archived:false` across three star bands: `10,000+`, `1,000-9,999`, and `200-999`.
   - Sort candidates in strict descending star order within each band.
   - Rotate round-robin across the three bands (`10,000+`, then `1,000-9,999`, then `200-999`), taking one candidate per band per turn.
   - Skip any band once its candidates are exhausted and continue rotating through the remaining bands.
3. **Pool-exhaustion fallback (never breach the 200-star quality floor)**:
   - Never lower the star floor below `200` stars to force a quota.
   - When a panel exhausts qualifying `200+`-star candidates for Large or Small before reaching its target size, spill unfilled Large or Small slots into Medium.
   - When total qualifying `200+`-star repositories still fall short of the target size, stop at the honest count (minimum 30 repositories to publish a **Reference range**) and record that exact count in the source label (for example, `codeint stratified panel of 68 SQL repositories, ADR 0035, codeint 0.8.0`).
4. **Proportional shape minimums (base Language plugins only)**:
   - Compute base-language minimums proportionally against the actual panel size `N`:
     - At least `30%` single-package repositories (`floor(0.30 * N)`).
     - At least `20%` multi-package workspaces (`floor(0.20 * N)`), where the language ecosystem has workspace manifests.
     - At least `20%` applications or command-line tools (`floor(0.20 * N)`).
     - At least `20%` libraries or packages (`floor(0.20 * N)`).
   - Classify package topology and app-versus-library shape mechanically from repository manifests and entry points.
   - For SQL, neither workspace manifests nor app-versus-library manifests exist, so both shape minimums are omitted (`["single", "n/a"]`).
   - For Shell, apply [ADR 0031](./0031-shell-reference-ranges.md)'s mechanical shebang rule (library-shaped when Production shell lines in files without `#!` exceed lines in files with `#!`) with no workspace minimum.
   - **Framework profiles** carry no app-versus-library minimums because web framework repositories skew naturally toward applications.
5. **Exclusions and quality filters**:
   - Exclude mirrors, books, curated awesome lists, templates, boilerplate starters, tutorials, courses, cheatsheets, interview lists, roadmaps, and exercise collections via a deterministic name, description, and topic filter.
   - Exclude repositories where `Generated + Vendored` source lines exceed `Production` source lines for the measured language.
   - Exclude repositories where `codeint check` fails to exit `0` or lowers zero files of the declared language.
   - Exclude **Framework profile** candidates where the target **Framework profile** is not active.
6. **Metrics, unified manifest, and percentile publication**:
   - Collect all five core **Reference range** **Metrics** in the **Evaluation suite**: **Erosion**, **Cognitive complexity**, **Duplication**, **Verbosity**, and **Import cycles** (omitting **Import cycles** for languages without runtime imports, such as SQL and Shell).
   - Pin all eight built-in **Language plugins** (including Shell) and calibrated **Framework profiles** in `eval/panels.json`, with full selection provenance in `eval/panel-selection.json`.
   - Retain all qualifying repositories including statistical outliers once admitted.
   - Compute and publish the 10th and 90th percentiles (linear interpolation over sorted ratios, rounded to three decimals) as `[low, high]` alongside each **Metric**'s exact integer **Metric version**.
   - Never copy third-party source code, fixtures, or text into codeint; commit only repository identifiers, pinned commit SHAs, selection metadata, and aggregate measurement numbers.

## Considered options

- **Raise base-language panels to 200 repositories.**
  Rejected: quadrupling panel size only halves sampling error over 50 (while 100 already captures ~70% of that gain), quadruples recalibration runtime and GitHub clone load, and risks exhausting `200+`-star Large repositories in SQL, Dart, and Shell.
- **Keep 50-repository base panels and 20-repository Framework panels, fixing only star-band sampling.**
  Rejected: 20-repository **Framework profile** panels remain too noisy (`+/- 18%` to `+/- 31%` on p90), and 50 repositories spread across 9 star-band-by-size cells leaves only ~5 repositories per cell.
- **Drop the 200-star floor when a niche language falls short of 100.**
  Rejected: scraping sub-200-star repositories pulls in homework repositories and raw data dumps, degrading calibration quality more than reporting a smaller honest panel size.

## Consequences

- Every built-in **Language plugin** (including Shell) and every calibrated **Framework profile** (`react`, `vue2`, `vue3`, and future calibrated profiles) shares one unified manifest (`eval/panels.json`) and one round-robin selection rule.
- **Cognitive complexity** joins **Erosion**, **Duplication**, **Verbosity**, and **Import cycles** as a calibrated **Reference range** across all built-in languages.
- Recalibrating all 11 current panels spans up to 950 repository entries (8 x 100 base languages plus 3 x 50 Framework profiles), keeping full measurement runs feasible on developer hardware while cutting flagship star bias and Framework panel noise.
- Existing 50/20 ranges from [ADR 0028](./0028-stratified-reference-panels.md) and [ADR 0031](./0031-shell-reference-ranges.md) remain superseded once the recalibrated panels and plugin tables land.
