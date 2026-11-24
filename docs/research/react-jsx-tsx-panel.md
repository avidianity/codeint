# React (JSX and TSX) 20-repository evaluation

Date checked: 2026-11-24.
Binary: `codeint 0.6.0` release build at commit `df9ffed4e318920f660d335cf6672bbc14ff218e` on Apple Silicon.

## Summary

The built-in `typescript-javascript` Language plugin parses `.tsx`, `.jsx`, and JSX syntax inside `.js`, `.mjs`, and `.cjs` files through `oxc_parser`.
All 20 public React repositories in this panel (10 TypeScript `.tsx` repositories and 10 JavaScript or mixed `.jsx` repositories) completed `codeint check --format json` cleanly.

- Total files scanned across the 20 repositories: 13,816 `.tsx` files and 1,242 `.jsx` files (15,058 JSX/TSX files), totaling 26,077 Production files and 2,115,282 Production source lines.
- Wall-clock time: `0.03s` to `1.86s` per repository (`7.07s` combined across all 20 repositories).
- Skipped files: 11 across 26,077 Production files (zero real JSX or TSX parser errors).
  All 11 skipped files are non-JavaScript generator templates (`<%= ... %>` scaffolds in `carbon-design-system/carbon` and `reactjs/react-rails`) or intentional invalid-syntax test fixtures (`badSyntax.js` and a Babel config fixture in `meta/docusaurus`).
- Reference range fit: the panel medians (Erosion `46.6%`, Duplication `8.1%`, Verbosity `8.1%`, Import cycles `0.8%`) sit closely within the existing TypeScript and JavaScript Reference ranges in [ADR 0008](../adr/0008-typescript-reference-ranges.md), which already includes seven React repositories.
  React repositories therefore continue using the ADR 0008 Reference ranges with no separate override required.

## Summary statistics (Production Source set)

Every statistic weighs each repository once, with linear interpolation between adjacent ranked values for percentiles.

| Metric | Mean | Median | 10th percentile | 90th percentile | Lowest | Highest |
| ------ | ---- | ------ | --------------- | --------------- | ------ | ------- |
| Erosion | 45.1% | 46.6% | 21.2% | 62.6% | 16.1% | 63.8% |
| Cognitive complexity | 34.3% | 35.8% | 14.2% | 55.1% | 6.0% | 58.9% |
| Duplication | 12.5% | 8.1% | 2.4% | 22.9% | 0.0% | 31.5% |
| Verbosity | 12.5% | 8.1% | 2.4% | 22.9% | 0.0% | 31.5% |
| Import cycles | 3.5% | 0.8% | 0.0% | 7.4% | 0.0% | 32.2% |

## Per-repository results

Each repository was shallow-cloned from GitHub on 2026-11-24 into a scratch folder outside the codeint repository, measured with `codeint check --format json`, and deleted immediately afterward.
For polyglot repositories, the numbers below report the `typescript-javascript` Production Source set.

| Repository | Group | Commit | `.tsx` | `.jsx` | Prod files | Prod lines | Erosion | Cognitive | Duplication | Verbosity | Import cycles | Time |
| ---------- | ----- | ------ | ------ | ------ | ---------- | ---------- | ------- | --------- | ----------- | --------- | ------------- | ---- |
| `ant-design/ant-design` | TSX | `5d2ba4176869` | 2,043 | 0 | 2,424 | 175,216 | 32.9% | 16.6% | 7.6% | 7.6% | 0.0% | 0.34s |
| `mantinedev/mantine` | TSX | `8f2ae606f8a3` | 3,972 | 0 | 5,230 | 331,943 | 38.2% | 27.1% | 22.0% | 22.0% | 0.8% | 0.74s |
| `refinedev/refine` | TSX | `2352eb5b6539` | 4,008 | 19 | 5,639 | 412,234 | 33.9% | 15.0% | 31.4% | 31.4% | 4.8% | 1.17s |
| `outline/outline` | TSX | `fd66b5e020c4` | 786 | 0 | 2,095 | 221,988 | 38.7% | 20.7% | 8.5% | 8.5% | 4.3% | 0.48s |
| `TanStack/query` | TSX | `9cd0abcf0cf3` | 538 | 0 | 2,060 | 82,112 | 21.8% | 20.7% | 31.5% | 31.5% | 0.0% | 0.55s |
| `meta/docusaurus` | TSX | `809ffffd5c84` | 412 | 3 | 810 | 55,465 | 16.1% | 6.9% | 2.6% | 2.7% | 1.6% | 0.19s |
| `excalidraw/excalidraw` | TSX | `9c2e462ec23e` | 329 | 0 | 553 | 129,178 | 62.5% | 47.4% | 4.7% | 4.7% | 32.2% | 0.18s |
| `xyflow/xyflow` | TSX | `9635a064f9d4` | 204 | 0 | 846 | 45,226 | 42.4% | 27.7% | 6.6% | 6.6% | 7.2% | 0.09s |
| `vadimdemedes/ink` | TSX | `0c0ebc048ccf` | 203 | 0 | 133 | 9,761 | 52.3% | 42.7% | 4.0% | 4.0% | 0.0% | 0.07s |
| `react-hook-form/react-hook-form` | TSX | `074d174816ae` | 183 | 0 | 257 | 16,047 | 58.9% | 55.0% | 16.8% | 16.8% | 0.8% | 0.15s |
| `gethomepage/homepage` | JSX | `a1e3a5fd26d6` | 0 | 548 | 582 | 29,325 | 63.2% | 34.4% | 19.9% | 19.9% | 0.0% | 0.14s |
| `getredash/redash` | JSX | `f561a53dc55e` | 118 | 207 | 514 | 40,866 | 16.2% | 6.0% | 7.8% | 7.8% | 2.9% | 0.24s |
| `steemit/condenser` | JSX | `453926a2039d` | 0 | 166 | 499 | 190,811 | 61.0% | 50.7% | 16.8% | 16.9% | 0.0% | 0.23s |
| `preactjs/preact` | JSX | `1fbc07f67bd8` | 21 | 132 | 94 | 6,148 | 62.1% | 56.6% | 0.0% | 0.0% | 9.6% | 0.06s |
| `react-dates/react-dates` | JSX | `b7bad38dcff0` | 0 | 49 | 111 | 13,210 | 61.0% | 47.3% | 21.7% | 21.7% | 0.0% | 0.06s |
| `carbon-design-system/carbon` | JSX | `c2443c3736fb` | 592 | 39 | 2,861 | 302,953 | 36.7% | 24.5% | 17.2% | 17.2% | 1.0% | 1.86s |
| `vitejs/vite-plugin-react` | JSX | `447ab6208f5f` | 369 | 27 | 1,229 | 28,605 | 35.0% | 37.2% | 20.2% | 20.2% | 0.5% | 0.17s |
| `react-grid-layout/react-grid-layout` | JSX | `ec4ab79c9744` | 36 | 26 | 54 | 8,003 | 50.8% | 42.6% | 5.0% | 5.0% | 3.7% | 0.09s |
| `mdx-js/mdx` | JSX | `52285a6758fa` | 0 | 14 | 61 | 4,188 | 63.8% | 58.9% | 0.4% | 0.4% | 0.0% | 0.03s |
| `reactjs/react-rails` | JSX | `e48a3263d6a7` | 2 | 12 | 25 | 12,003 | 54.2% | 47.6% | 5.9% | 6.0% | 0.0% | 0.23s |

## Known syntax exclusions

- Non-standard Babel Stage 1 `export default from './foo'` syntax (`@babel/plugin-proposal-export-default-from`, observed in legacy repositories such as `casesandberg/react-color` and `Semantic-Org/Semantic-UI-React`) and Flow type syntax are not part of standard ECMAScript or TypeScript and are reported under `skipped`.
- Code-generation templates with ERB or placeholder delimiters inside `.js`, `.jsx`, or `.tsx` files are reported under `skipped` rather than failing the run.
