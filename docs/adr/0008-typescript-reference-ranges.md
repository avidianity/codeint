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

- codeint 0.1.0, built at commit `40a0f76` of the Integration branch `feat/10-codeint-v1`.
- Default rule set: minimum clone size 75 tokens, clone filter 14 distinct normalized tokens.
- Apple M5 with 10 cores and 32 GB of memory, on 2026-11-21.
- codeint took 0.01 s to 9.1 s per repository, and about 33 s for the whole panel.
- Every percentile interpolates linearly between the two nearest ranked values.
- Every statistic weighs each repository once, whatever its size.

### Applying the rule

In list order, the rule excluded these candidates:

- TypeScript: `freeCodeCamp/freeCodeCamp` (curriculum), `nilbuild/developer-roadmap` (roadmap), `iptv-org/iptv` (1,422 Production lines), and `yangshun/tech-interview-handbook` (interview questions).
- JavaScript: `trekhleb/javascript-algorithms` and `CyC2018/CS-Notes` (interview topic), `twbs/bootstrap` (4,621 Production lines, and more Generated than Production lines), `Snailclimb/JavaGuide` (interview topic), `airbnb/javascript` (1,741 Production lines), `Chalarangelo/30-seconds-of-code` (snippets), `axios/axios` (8,323 Generated lines against 6,882 Production lines), `microsoft/Web-Dev-For-Beginners` (curriculum), `ryanmcdermott/clean-code-javascript` (no source lines), `anuraghazra/github-readme-stats` (3,761 Production lines), and `typicode/json-server` (1,074 Production lines).
- `FortAwesome/Font-Awesome` has no default-branch commit before 2023-01-01, so it cannot be pinned; the rule did not name this case, and it is treated as an exclusion.

codeint exited with an error on no candidate.

### Panel

| Repository | Language | Stars band | Pinned commit | Production lines |
| ---------- | -------- | ---------- | ------------- | ---------------- |
| `vuejs/vue` | TypeScript | over 100k | `0e8511a8becf627e00443bd799dd99e5fd1b8a35` | 24,527 |
| `n8n-io/n8n` | TypeScript | over 100k | `99e3eb634bd34fa6791e09f112ce9e49407fed49` | 486,907 |
| `microsoft/vscode` | TypeScript | over 100k | `56e16bf51a7b620ba40b20c0ed94db2d1265295b` | 700,471 |
| `excalidraw/excalidraw` | TypeScript | over 100k | `fdd8552637c21d0b2317f33feb4247d2e46050dc` | 42,574 |
| `supabase/supabase` | TypeScript | over 100k | `42ad83d7429d3b2467a80cc5ce3a1e80a1cc9cde` | 127,101 |
| `angular/angular` | TypeScript | over 100k | `27da7338ea8f3bf5336d77ed1877cfa6a1974d52` | 529,042 |
| `microsoft/playwright` | TypeScript | over 100k | `a30aac56687598c373c51255308ef5833de0c9bb` | 173,251 |
| `ant-design/ant-design` | TypeScript | over 100k | `2f623fe5536395bbb4dfa694fae6dbf2042d9025` | 99,382 |
| `tailwindlabs/tailwindcss` | TypeScript | 50k to 100k | `8b13ecfe167861202f7f5c0d09d213cb68eeefb7` | 10,864 |
| `puppeteer/puppeteer` | TypeScript | 50k to 100k | `29a50764d42309ae88774cb25108acf609c51c4f` | 22,437 |
| `react/react` | JavaScript | over 100k | `de7d1c90718ea8f4844a2219991f7115ef2bd2c5` | 51,201 |
| `vercel/next.js` | JavaScript | over 100k | `fd018fd6cb1458cf1c0da154574e74f63950026b` | 236,380 |
| `nodejs/node` | JavaScript | over 100k | `e2dd139da03d71e190830a3d2e82e37da5ff2f71` | 1,286,810 |
| `mrdoob/three.js` | JavaScript | over 100k | `2a4451c430b6756e9856c242edba4ba80379031b` | 262,299 |
| `react/create-react-app` | JavaScript | over 100k | `d960b9e38c062584ff6cfb1a70e1512509a966e7` | 55,360 |
| `mui/material-ui` | JavaScript | 50k to 100k | `33a0e1c2ebd7575f694638559f42eb9a9d545e60` | 550,243 |
| `louislam/uptime-kuma` | JavaScript | 50k to 100k | `cd7a6e4019d30affe601e7a41decd235a4b2e049` | 28,277 |
| `sveltejs/svelte` | JavaScript | 50k to 100k | `de6821a1de6574b1aa987944fe9a5f0b5f7724bb` | 49,793 |
| `juliangarnier/anime` | JavaScript | 50k to 100k | `3ebfd913a04f7dc59cc3d52e38275272a5a12ae6` | 9,060 |
| `hakimel/reveal.js` | JavaScript | 50k to 100k | `4fe3946cb43de57f79aaa7b646aee7e78f4bcc75` | 5,670 |

The language is GitHub's primary language of the repository on the query date.
No panel repository falls in the 10k to 25k or the 25k to 50k band.

## Measurements

### Per Metric, over the 20 repositories

| Metric | Mean | Median | Standard deviation | 10th percentile | 90th percentile | Lowest | Highest |
| ------ | ---- | ------ | ------------------ | --------------- | --------------- | ------ | ------- |
| Erosion | 0.477 | 0.503 | 0.128 | 0.339 | 0.577 | 0.268 | 0.802 |
| Verbosity | 0.191 | 0.130 | 0.216 | 0.025 | 0.343 | 0.010 | 0.834 |
| Duplication | 0.190 | 0.129 | 0.217 | 0.025 | 0.343 | 0.007 | 0.834 |
| Import cycles | 0.080 | 0.022 | 0.154 | 0.000 | 0.163 | 0.000 | 0.677 |

### Per Metric, by half of the panel

| Metric | TypeScript median | TypeScript 10th to 90th percentile | JavaScript median | JavaScript 10th to 90th percentile |
| ------ | ----------------- | ---------------------------------- | ----------------- | ---------------------------------- |
| Erosion | 0.393 | 0.293 to 0.587 | 0.527 | 0.391 to 0.577 |
| Verbosity | 0.066 | 0.024 to 0.209 | 0.196 | 0.032 to 0.683 |
| Duplication | 0.066 | 0.024 to 0.208 | 0.195 | 0.032 to 0.682 |
| Import cycles | 0.088 | 0.015 to 0.286 | 0.004 | 0.000 to 0.050 |

### Per repository, in the Production Source set

| Repository | Erosion | Verbosity | Duplication | Import cycles |
| ---------- | ------- | --------- | ----------- | ------------- |
| `vuejs/vue` | 0.564 | 0.010 | 0.007 | 0.242 |
| `n8n-io/n8n` | 0.802 | 0.146 | 0.146 | 0.019 |
| `microsoft/vscode` | 0.355 | 0.079 | 0.078 | 0.077 |
| `excalidraw/excalidraw` | 0.488 | 0.031 | 0.031 | 0.677 |
| `supabase/supabase` | 0.389 | 0.114 | 0.112 | 0.108 |
| `angular/angular` | 0.344 | 0.200 | 0.199 | 0.022 |
| `microsoft/playwright` | 0.560 | 0.299 | 0.298 | 0.098 |
| `ant-design/ant-design` | 0.268 | 0.053 | 0.053 | 0.015 |
| `tailwindlabs/tailwindcss` | 0.396 | 0.026 | 0.026 | 0.013 |
| `puppeteer/puppeteer` | 0.295 | 0.050 | 0.049 | 0.154 |
| `react/react` | 0.622 | 0.307 | 0.307 | 0.000 |
| `vercel/next.js` | 0.565 | 0.306 | 0.305 | 0.007 |
| `nodejs/node` | 0.572 | 0.215 | 0.214 | 0.024 |
| `mrdoob/three.js` | 0.534 | 0.177 | 0.176 | 0.047 |
| `react/create-react-app` | 0.541 | 0.834 | 0.834 | 0.000 |
| `mui/material-ui` | 0.442 | 0.165 | 0.164 | 0.000 |
| `louislam/uptime-kuma` | 0.366 | 0.034 | 0.033 | 0.022 |
| `sveltejs/svelte` | 0.521 | 0.084 | 0.083 | 0.080 |
| `juliangarnier/anime` | 0.394 | 0.666 | 0.665 | 0.000 |
| `hakimel/reveal.js` | 0.519 | 0.018 | 0.017 | 0.000 |

### Outliers

An outlier lies more than 1.5 interquartile ranges outside the middle half of the panel.
All of them stay in the panel, as the selection rule says.

- Duplication and Verbosity, `react/create-react-app` at 0.834: almost every Clone group is in two bundled test fixtures of about 41,000 lines each under a `fixtures` folder.
  codeint puts them in the Production Source set, because no Source set rule knows that folder.
- Duplication and Verbosity, `juliangarnier/anime` at 0.665: the repository commits two build outputs of its one source file in `lib/`, plus copies of the library and of other libraries under its documentation folder.
  codeint puts them in the Production Source set.
- Import cycles, `excalidraw/excalidraw` at 0.677: one Import cycle holds 157 of 232 Production modules.
- Import cycles, `vuejs/vue` at 0.242: one Import cycle holds 56 of 264 Production modules.
- Erosion has no outlier by this test; `n8n-io/n8n` is the highest at 0.802, and its three heaviest functions have a Function mass of 7,354 to 20,941.

### Limits of codeint at this commit

These limits bias the JavaScript half of the panel more than the TypeScript half.

- The Language plugin parses a `.js` file without JSX.
  It skips each `.js` file that uses JSX as "Unexpected JSX expression".
  It skipped 2,632 files in `mui/material-ui`, 2,885 in `vercel/next.js`, and 377 in `react/react`.
- The Language plugin does not parse Flow.
  It skipped 624 files in `react/react` as "Flow is not supported".
  Together with JSX, codeint skipped 1,025 of the 1,871 JavaScript files of `react/react`.
- The Test Source set rules know a `__tests__` folder and `.test.` and `.spec.` file names, but not a `test` or `tests` folder.
  So `nodejs/node` counts 7 Test lines, and its test suite is in its 1.29 million Production lines.
- Committed build output and bundled fixtures outside the known build output folders count as Production, as the outliers above show.
- Verbosity is within 0.003 of Duplication in every panel repository, because the Language plugin flags few lines.
  Its range is, in effect, the Duplication range.

The Python ranges in ADR 0003 used other rules, so they do not compare with these numbers.

## Decision

**The TypeScript and JavaScript Language plugin shows, for each Metric, the 10th to 90th percentile of this panel as its Reference range.**
The source label reads `codeint panel of 20 TypeScript and JavaScript repositories, ADR 0008, codeint 0.1.0`.

| Metric | Reference range |
| ------ | --------------- |
| Erosion | 0.339 to 0.577 |
| Verbosity | 0.025 to 0.343 |
| Duplication | 0.025 to 0.343 |
| Import cycles | 0.000 to 0.163 |

- The 10th to 90th percentile holds the middle 80% of the panel, and two extreme repositories cannot move it.
- The mean and standard deviation suit Erosion, but not Duplication and Import cycles: those are skewed, and a mean minus one standard deviation falls below zero.
- The Plugin interface holds a range as a low and a high ratio, which a percentile band fills directly.
- One range covers both languages, because the plugin covers both and the halves of the panel are too small to split.

## Considered options

- **Mean plus or minus one standard deviation, as the Python ranges in ADR 0003 do.**
  Rejected: Duplication would run from below zero to 0.41, and two repositories with committed copies set its width.
- **Separate ranges for TypeScript and for JavaScript.**
  Rejected: 10 repositories per language give unstable percentiles, and one Language plugin serves both languages.
- **The most-starred repositories at their current commits.**
  Rejected: recent commits may hold agent-written code, which a range for human-written code must leave out.
- **Remove the outliers and measure again.**
  Rejected: the selection rule keeps them, and the percentile band already limits their effect.
- **Fix the limits above first, then measure.**
  This is the option to choose if the Owner wants the JavaScript half to be measured on all of its files; the run takes about one minute once the fixes land.

## Consequences

- TypeScript and JavaScript reports show a Reference range next to each Metric, with a source label that names this record and the codeint version.
- A change to a Metric's rules, to the default rule set, or to the Source set rules changes what the range means; such a change needs a new measurement and an amendment to this record.
- The panel and the rule are written down, so anyone can repeat the measurement.
- The ranges carry the limits above; when they are fixed, the panel is measured again and the ranges are amended.
- No code, fixture, or text of a panel repository enters codeint; this record holds only names, commits, and numbers.
