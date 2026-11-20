---
status: proposed
---

# TypeScript and JavaScript Reference ranges come from a panel of 20 public repositories

Ticket #74 of Spec #10 proposes this record on 2026-11-21, for the Owner's approval.
The published Reference ranges cover Python only, as ADR 0003 records, so codeint measures its own for TypeScript and JavaScript.

## Context

A Reference range must describe human-written code, or it cannot show how far agent-written code drifts from it.
A panel picked after looking at results can be bent toward any range, so the selection rule below was written and committed before any repository was measured.

### Selection rule

The rule was written on 2026-11-21, before any measurement.

1. Run two GitHub repository searches on 2026-11-21, each sorted by stars in descending order:
   - `language:TypeScript created:<2022-01-01 fork:false archived:false`
   - `language:JavaScript created:<2022-01-01 fork:false archived:false`
2. Walk each result list in order, and take the first 10 repositories that pass every exclusion below.
   The panel is those 20 repositories: 10 from each search.
3. Pin each repository at the last commit of its default branch dated before 2023-01-01, by the committer date.
   Code written before 2023 predates the wide use of coding agents, so it stands for human-written code.
4. Record each repository's stars band from its star count on the query date: 10k to 25k, 25k to 50k, 50k to 100k, or over 100k.

A repository is excluded, and the next one in its list takes its place, when any of these is true:

- It is a fork, a mirror, or archived.
- Its description or topics say it is a list, a roadmap, a curriculum, a course, a book, a tutorial, a set of interview questions, a cheat sheet, or a collection of snippets or examples.
- At the pinned commit, codeint counts fewer than 5,000 source lines in the Production Source set.
- At the pinned commit, the Generated and Vendored Source sets together hold more source lines than the Production Source set.
- codeint exits with an error on it; the record lists each such repository and the error.

No repository is removed after measurement for any other reason.
An outlier stays in the panel and is named, with its likely cause.

### Method

- Build codeint at the commit that this record names, with `cargo build --release --locked`.
- Use the default rule set: a minimum clone size of 75 tokens and a clone filter of 14 distinct normalized tokens, as ADR 0007 decided.
- Fetch each pinned commit with depth 1 into a scratch folder outside the codeint repository.
- Run `codeint check --format json --all` on each, and read each Metric's value in the Production Source set.
- Report, per Metric, the mean, the median, the standard deviation, and the 10th to 90th percentile.
- Record only numbers; no code, fixture, or text from a panel repository enters codeint.

### The run

- codeint 0.1.0, built at commit `b92758b` of the Integration branch `feat/10-codeint-v1`, after #79 (JSX in `.js`, `.mjs`, `.cjs`), #80 (`test/` and `tests/` folders as Test code), and #81 (Verbosity audit) landed.
- Default rule set: minimum clone size 75 tokens, clone filter 14 distinct normalized tokens.
- Apple M5 with 10 cores and 32 GB of memory, on 2026-11-21.
- codeint took 0.01 s to 21.55 s per repository, and about 56 s of total `codeint` run time across the panel (measured across 6 parallel workers).
- Every percentile interpolates linearly between the two nearest ranked values.
- Every statistic weighs each repository once, whatever its size.

### Applying the rule

In list order, the rule excluded these candidates:

- TypeScript: `freeCodeCamp/freeCodeCamp` (curriculum), `nilbuild/developer-roadmap` (roadmap), `iptv-org/iptv` (1,422 Production lines), and `yangshun/tech-interview-handbook` (interview questions).
- JavaScript: `trekhleb/javascript-algorithms` and `CyC2018/CS-Notes` (interview topic), `twbs/bootstrap` (4,281 Production lines, and more Generated plus Vendored than Production lines), `Snailclimb/JavaGuide` (interview topic), `airbnb/javascript` (1,566 Production lines), `Chalarangelo/30-seconds-of-code` (snippets), `axios/axios` (3,523 Production lines, and 8,323 Generated lines exceeding Production), `microsoft/Web-Dev-For-Beginners` (curriculum), `ryanmcdermott/clean-code-javascript` (no source lines), `anuraghazra/github-readme-stats` (3,761 Production lines), and `typicode/json-server` (1,074 Production lines).
- `FortAwesome/Font-Awesome` has no default-branch commit before 2023-01-01, so it cannot be pinned; the rule did not name this case, and it is treated as an exclusion.

codeint exited with an error on no candidate.

### Panel

| Repository | Language | Stars band | Pinned commit | Production lines |
| ---------- | -------- | ---------- | ------------- | ---------------- |
| `vuejs/vue` | TypeScript | over 100k | `0e8511a8becf627e00443bd799dd99e5fd1b8a35` | 20,830 |
| `n8n-io/n8n` | TypeScript | over 100k | `99e3eb634bd34fa6791e09f112ce9e49407fed49` | 484,161 |
| `microsoft/vscode` | TypeScript | over 100k | `56e16bf51a7b620ba40b20c0ed94db2d1265295b` | 672,047 |
| `excalidraw/excalidraw` | TypeScript | over 100k | `fdd8552637c21d0b2317f33feb4247d2e46050dc` | 41,852 |
| `supabase/supabase` | TypeScript | over 100k | `42ad83d7429d3b2467a80cc5ce3a1e80a1cc9cde` | 132,341 |
| `angular/angular` | TypeScript | over 100k | `27da7338ea8f3bf5336d77ed1877cfa6a1974d52` | 293,759 |
| `microsoft/playwright` | TypeScript | over 100k | `a30aac56687598c373c51255308ef5833de0c9bb` | 68,252 |
| `ant-design/ant-design` | TypeScript | over 100k | `2f623fe5536395bbb4dfa694fae6dbf2042d9025` | 98,848 |
| `tailwindlabs/tailwindcss` | TypeScript | 50k to 100k | `8b13ecfe167861202f7f5c0d09d213cb68eeefb7` | 10,238 |
| `puppeteer/puppeteer` | TypeScript | 50k to 100k | `29a50764d42309ae88774cb25108acf609c51c4f` | 22,240 |
| `react/react` | JavaScript | over 100k | `de7d1c90718ea8f4844a2219991f7115ef2bd2c5` | 58,971 |
| `vercel/next.js` | JavaScript | over 100k | `fd018fd6cb1458cf1c0da154574e74f63950026b` | 236,836 |
| `nodejs/node` | JavaScript | over 100k | `e2dd139da03d71e190830a3d2e82e37da5ff2f71` | 446,364 |
| `mrdoob/three.js` | JavaScript | over 100k | `2a4451c430b6756e9856c242edba4ba80379031b` | 246,288 |
| `react/create-react-app` | JavaScript | over 100k | `d960b9e38c062584ff6cfb1a70e1512509a966e7` | 56,306 |
| `mui/material-ui` | JavaScript | 50k to 100k | `33a0e1c2ebd7575f694638559f42eb9a9d545e60` | 663,301 |
| `louislam/uptime-kuma` | JavaScript | 50k to 100k | `cd7a6e4019d30affe601e7a41decd235a4b2e049` | 28,219 |
| `sveltejs/svelte` | JavaScript | 50k to 100k | `de6821a1de6574b1aa987944fe9a5f0b5f7724bb` | 22,879 |
| `juliangarnier/anime` | JavaScript | 50k to 100k | `3ebfd913a04f7dc59cc3d52e38275272a5a12ae6` | 9,060 |
| `hakimel/reveal.js` | JavaScript | 50k to 100k | `4fe3946cb43de57f79aaa7b646aee7e78f4bcc75` | 5,666 |

The language is GitHub's primary language of the repository on the query date.
No panel repository falls in the 10k to 25k or the 25k to 50k band.

## Measurements

### Per Metric, over the 20 repositories

| Metric | Mean | Median | Standard deviation | 10th percentile | 90th percentile | Lowest | Highest |
| ------ | ---- | ------ | ------------------ | --------------- | --------------- | ------ | ------- |
| Erosion | 0.478 | 0.454 | 0.128 | 0.352 | 0.600 | 0.271 | 0.804 |
| Verbosity | 0.173 | 0.095 | 0.218 | 0.018 | 0.347 | 0.008 | 0.827 |
| Duplication | 0.172 | 0.093 | 0.218 | 0.017 | 0.346 | 0.005 | 0.827 |
| Import cycles | 0.113 | 0.027 | 0.185 | 0.000 | 0.308 | 0.000 | 0.692 |

### Per Metric, by half of the panel

| Metric | TypeScript median | TypeScript 10th to 90th percentile | JavaScript median | JavaScript 10th to 90th percentile |
| ------ | ----------------- | ---------------------------------- | ----------------- | ---------------------------------- |
| Erosion | 0.413 | 0.292 to 0.606 | 0.544 | 0.391 to 0.600 |
| Verbosity | 0.057 | 0.015 to 0.116 | 0.233 | 0.019 to 0.682 |
| Duplication | 0.057 | 0.015 to 0.115 | 0.233 | 0.018 to 0.681 |
| Import cycles | 0.092 | 0.015 to 0.326 | 0.003 | 0.000 to 0.136 |

### Per repository, in the Production Source set

| Repository | Erosion | Verbosity | Duplication | Import cycles |
| ---------- | ------- | --------- | ----------- | ------------- |
| `vuejs/vue` | 0.584 | 0.008 | 0.005 | 0.286 |
| `n8n-io/n8n` | 0.804 | 0.145 | 0.145 | 0.019 |
| `microsoft/vscode` | 0.359 | 0.079 | 0.077 | 0.078 |
| `excalidraw/excalidraw` | 0.485 | 0.034 | 0.034 | 0.692 |
| `supabase/supabase` | 0.382 | 0.112 | 0.109 | 0.105 |
| `angular/angular` | 0.422 | 0.113 | 0.111 | 0.030 |
| `microsoft/playwright` | 0.423 | 0.061 | 0.060 | 0.135 |
| `ant-design/ant-design` | 0.271 | 0.054 | 0.054 | 0.015 |
| `tailwindlabs/tailwindcss` | 0.404 | 0.016 | 0.016 | 0.014 |
| `puppeteer/puppeteer` | 0.295 | 0.050 | 0.049 | 0.185 |
| `react/react` | 0.599 | 0.272 | 0.272 | 0.000 |
| `vercel/next.js` | 0.564 | 0.311 | 0.311 | 0.007 |
| `nodejs/node` | 0.577 | 0.228 | 0.227 | 0.094 |
| `mrdoob/three.js` | 0.547 | 0.173 | 0.172 | 0.058 |
| `react/create-react-app` | 0.542 | 0.827 | 0.827 | 0.000 |
| `mui/material-ui` | 0.420 | 0.239 | 0.239 | 0.000 |
| `louislam/uptime-kuma` | 0.366 | 0.034 | 0.033 | 0.024 |
| `sveltejs/svelte` | 0.609 | 0.019 | 0.018 | 0.512 |
| `juliangarnier/anime` | 0.394 | 0.666 | 0.665 | 0.000 |
| `hakimel/reveal.js` | 0.519 | 0.018 | 0.017 | 0.000 |

### Outliers

An outlier lies more than 1.5 interquartile ranges outside the middle half of the panel.
All of them stay in the panel, as the selection rule says.

- Duplication and Verbosity, `react/create-react-app` at 0.827: almost every Clone group is in two bundled test fixtures of about 41,000 lines each under a `fixtures` folder.
  codeint puts them in the Production Source set, because no Source set rule knows that folder.
- Duplication and Verbosity, `juliangarnier/anime` at 0.665 (Verbosity 0.666): the repository commits two build outputs of its one source file in `lib/`, plus copies of the library and of other libraries under its documentation folder.
  codeint puts them in the Production Source set.
- Import cycles, `excalidraw/excalidraw` at 0.692: one Import cycle holds most Production modules.
- Import cycles, `sveltejs/svelte` at 0.512: once `test/` moved out of Production (#80), the compiler's internal module cycle accounts for half of its Production modules.
- Import cycles, `vuejs/vue` at 0.286: one Import cycle holds a large share of its Production modules.
- Erosion has no outlier by this test; `n8n-io/n8n` is the highest at 0.804.

### Limits of codeint at this commit

Before this run, #79 added JSX parsing for `.js`, `.mjs`, and `.cjs` files (cutting skipped files in `mui/material-ui` from 2,633 to 1, in `vercel/next.js` from 2,893 to 5, and in `react/react` from 1,025 to 631) and #80 moved `test/` and `tests/` folders into the Test Source set (moving 840,453 lines of `nodejs/node` and 26,914 lines of `sveltejs/svelte` out of Production).
These limits remain:

- The Language plugin does not parse Flow (`oxc_parser` does not support Flow).
  It skipped 607 Flow files in `react/react` (plus 24 other syntax files).
- Committed build output and bundled fixtures outside the known build output folders count as Production, as the outliers above show.
- Verbosity is within 0.003 of Duplication in every panel repository.
  Ticket #81 confirmed that all six Verbosity rules match their specifications: mature, linted pre-2023 repositories already enforce equivalent lint rules, and single-line Verbosity matches are small next to duplicated blocks.
  On human-written reference repositories the Verbosity range therefore tracks Duplication, while Verbosity still adds signal through `flagged_lines` and per-rule Findings on unlinted or agent-written code (Owner Decision on #81, 2026-11-21).

The Python ranges in ADR 0003 used other rules, so they do not compare with these numbers.

## Decision

**The TypeScript and JavaScript Language plugin shows, for each Metric, the 10th to 90th percentile of this panel as its Reference range.**
The source label reads `codeint panel of 20 TypeScript and JavaScript repositories, ADR 0008, codeint 0.1.0`.

| Metric | Reference range |
| ------ | --------------- |
| Erosion | 0.352 to 0.600 |
| Verbosity | 0.018 to 0.347 |
| Duplication | 0.017 to 0.346 |
| Import cycles | 0.000 to 0.308 |

- The 10th to 90th percentile holds the middle 80% of the panel, and two extreme repositories cannot move it.
- The mean and standard deviation suit Erosion, but not Duplication and Import cycles: those are skewed, and a mean minus one standard deviation falls below zero.
- The Plugin interface holds a range as a low and a high ratio, which a percentile band fills directly.
- One range covers both languages, because the plugin covers both and the halves of the panel are too small to split.

## Considered options

- **Mean plus or minus one standard deviation, as the Python ranges in ADR 0003 do.**
  Rejected: Duplication would run from below zero to 0.39, and two repositories with committed copies set its width.
- **Separate ranges for TypeScript and for JavaScript.**
  Rejected: 10 repositories per language give unstable percentiles, and one Language plugin serves both languages.
- **The most-starred repositories at their current commits.**
  Rejected: recent commits may hold agent-written code, which a range for human-written code must leave out.
- **Remove the outliers and measure again.**
  Rejected: the selection rule keeps them, and the percentile band already limits their effect.
- **Change the Verbosity definition to exclude clone lines (#81).**
  Rejected by Owner Decision (2026-11-21): keep the v1 Verbosity union and record why it tracks Duplication on mature, linted human repositories.

## Consequences

- TypeScript and JavaScript reports show a Reference range next to each Metric, with a source label that names this record and the codeint version.
- A change to a Metric's rules, to the default rule set, or to the Source set rules changes what the range means; such a change needs a new measurement and an amendment to this record.
- The panel and the rule are written down, so anyone can repeat the measurement.
- The ranges carry the limits above; when they are fixed, the panel is measured again and the ranges are amended.
- No code, fixture, or text of a panel repository enters codeint; this record holds only names, commits, and numbers.
