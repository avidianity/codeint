---
status: superseded by ADR-0035
---

# React (JSX and TSX) Reference ranges come from a panel of 20 public repositories

The Owner requested calibrating React (`.jsx` and `.tsx`) Reference ranges on 2026-11-24 using the pre-2023 human-written selection rule established in [ADR 0008](./0008-typescript-reference-ranges.md).

## Context

React repositories compose user interfaces from small component functions and hooks with unidirectional data flow, producing lower Erosion and Import cycle ratios than general TypeScript and JavaScript repositories.
This record applies the ADR 0008 selection rule and measurement method to 20 public React repositories (10 TypeScript and 10 JavaScript) containing `.tsx` and `.jsx` files.

### Selection rule and run

- Queries on 2026-11-24: two GitHub repository searches sorted by stars descending:
  - `topic:react language:TypeScript created:<2022-01-01 fork:false archived:false`
  - `topic:react language:JavaScript created:<2022-01-01 fork:false archived:false`
- Walk each result list in order and take the first 10 repositories that pass every exclusion below (20 repositories total):
  - Exclude forks, mirrors, archived repositories, curricula, roadmaps, interview question lists, cheat sheets, tutorials, books, and study guides.
  - Require at least one `.tsx` or `.jsx` file in the repository and a `react` dependency declared in a `package.json` manifest.
  - Pin each repository at the last commit on its default branch dated before `2023-01-01`.
  - Exclude repositories with fewer than 5,000 Production source lines, repositories whose Generated plus Vendored source lines exceed Production source lines, and repositories that fail to fetch within the 50-second shallow-fetch timeout or exit with an error.
- Measured with `codeint 0.6.0` release build (`df9ffed`, default clone size 75 tokens, filter 14 distinct normalized tokens) on Apple Silicon.
- Run time across the 20 repositories: `0.02s` to `24.84s` per repository (`30.76s` total across 6,883 `.tsx` files, 256 `.jsx` files, and 1,858,963 Production source lines; zero `codeint` errors).

### Panel

| Repository | Language | Stars band | Pinned commit | `.tsx` | `.jsx` | Production lines |
| ---------- | -------- | ---------- | ------------- | ------ | ------ | ---------------- |
| `ant-design/ant-design` | TypeScript | over 100k | `2f623fe5536395bbb4dfa694fae6dbf2042d9025` | 1,216 | 0 | 98,848 |
| `tldraw/tldraw` | TypeScript | 50k to 100k | `70ca73555b01c153903976047dcb1d86df8e3a2e` | 222 | 0 | 34,034 |
| `TanStack/query` | TypeScript | 50k to 100k | `7d3f80386d7a33ed401ee9260ad9e9130951a9b3` | 87 | 37 | 19,773 |
| `reactive-resume/reactive-resume` | TypeScript | 25k to 50k | `c9685d4ce7a61d5cf8e4309819c4ba0c8b94b05c` | 111 | 0 | 11,517 |
| `outline/outline` | TypeScript | 25k to 50k | `dc795604a4a53d8bca69e08930c0463c7c2d6948` | 410 | 0 | 81,473 |
| `appsmithorg/appsmith` | TypeScript | 25k to 50k | `a8885dca3caddfeb28ec6fc436096db8790c7e05` | 1,147 | 0 | 347,491 |
| `styled-components/styled-components` | TypeScript | 25k to 50k | `d5aacbd55667e9b9264d190505ef42615f73adba` | 32 | 0 | 5,102 |
| `trpc/trpc` | TypeScript | 25k to 50k | `1bc95c837dfb777390adfefc1a9b7b8158243922` | 111 | 1 | 19,840 |
| `chakra-ui/chakra-ui` | TypeScript | 25k to 50k | `05c31f4097be0973565532aed4c611bcb0bcdb2e` | 444 | 0 | 40,102 |
| `novuhq/novu` | TypeScript | 25k to 50k | `01c87fb0cba61bb3c7f6c71863a4e635d80ab6ef` | 353 | 1 | 71,175 |
| `vercel/next.js` | JavaScript | over 100k | `fd018fd6cb1458cf1c0da154574e74f63950026b` | 1,118 | 62 | 236,836 |
| `react/create-react-app` | JavaScript | over 100k | `d960b9e38c062584ff6cfb1a70e1512509a966e7` | 7 | 0 | 56,306 |
| `mui/material-ui` | JavaScript | 50k to 100k | `33a0e1c2ebd7575f694638559f42eb9a9d545e60` | 1,536 | 4 | 663,301 |
| `enzymejs/enzyme` | JavaScript | 10k to 25k | `a4980e14d9adf7adc0de125744213cbd1c11b168` | 0 | 102 | 7,556 |
| `decaporg/decap-cms` | JavaScript | 10k to 25k | `0c5efd19d6aa28e1e06a90c221ea0e8ac834ef33` | 2 | 0 | 54,751 |
| `emotion-js/emotion` | JavaScript | 10k to 25k | `acb72a45592881d9d1f72003b6db2e488b981599` | 26 | 0 | 6,573 |
| `Semantic-Org/Semantic-UI-React` | JavaScript | 10k to 25k | `c8aace9c1860f806f830817ca0222eca47af30fc` | 1 | 0 | 53,167 |
| `segmentio/evergreen` | JavaScript | 10k to 25k | `105e02f9edd00cdfb4f7ba6935873af843a53195` | 58 | 0 | 32,324 |
| `downshift-js/downshift` | JavaScript | 10k to 25k | `99bd9d936b46620d0e8f27dd3a35ca15149ec7b5` | 2 | 0 | 5,584 |
| `react-dates/react-dates` | JavaScript | 10k to 25k | `b7bad38dcff024d374ee98f972b55b3de9e61289` | 0 | 49 | 13,210 |

## Measurements

### Per Metric, over the 20 repositories

| Metric | Mean | Median | Standard deviation | 10th percentile | 90th percentile | Lowest | Highest |
| ------ | ---- | ------ | ------------------ | --------------- | --------------- | ------ | ------- |
| Erosion | 0.304 | 0.274 | 0.153 | 0.106 | 0.544 | 0.064 | 0.610 |
| Verbosity | 0.178 | 0.094 | 0.187 | 0.047 | 0.368 | 0.002 | 0.827 |
| Duplication | 0.177 | 0.094 | 0.187 | 0.046 | 0.368 | 0.000 | 0.827 |
| Import cycles | 0.057 | 0.013 | 0.079 | 0.000 | 0.144 | 0.000 | 0.293 |

### Per repository, in the Production Source set

| Repository | Erosion | Verbosity | Duplication | Import cycles |
| ---------- | ------- | --------- | ----------- | ------------- |
| `ant-design/ant-design` | 0.271 | 0.053 | 0.053 | 0.015 |
| `tldraw/tldraw` | 0.335 | 0.098 | 0.098 | 0.099 |
| `TanStack/query` | 0.277 | 0.193 | 0.192 | 0.000 |
| `reactive-resume/reactive-resume` | 0.248 | 0.360 | 0.360 | 0.042 |
| `outline/outline` | 0.344 | 0.077 | 0.077 | 0.174 |
| `appsmithorg/appsmith` | 0.323 | 0.091 | 0.090 | 0.293 |
| `styled-components/styled-components` | 0.220 | 0.002 | 0.000 | 0.139 |
| `trpc/trpc` | 0.140 | 0.093 | 0.093 | 0.125 |
| `chakra-ui/chakra-ui` | 0.070 | 0.073 | 0.073 | 0.020 |
| `novuhq/novu` | 0.213 | 0.131 | 0.131 | 0.141 |
| `vercel/next.js` | 0.564 | 0.311 | 0.311 | 0.006 |
| `react/create-react-app` | 0.542 | 0.827 | 0.827 | 0.000 |
| `mui/material-ui` | 0.419 | 0.239 | 0.239 | 0.000 |
| `enzymejs/enzyme` | 0.377 | 0.446 | 0.446 | 0.000 |
| `decaporg/decap-cms` | 0.224 | 0.047 | 0.046 | 0.075 |
| `emotion-js/emotion` | 0.459 | 0.046 | 0.046 | 0.000 |
| `Semantic-Org/Semantic-UI-React` | 0.110 | 0.095 | 0.095 | 0.010 |
| `segmentio/evergreen` | 0.064 | 0.066 | 0.066 | 0.000 |
| `downshift-js/downshift` | 0.262 | 0.091 | 0.091 | 0.000 |
| `react-dates/react-dates` | 0.610 | 0.217 | 0.217 | 0.000 |

## Decision

**When a repository measures `.jsx` or `.tsx` files (without `.vue` files) and `package.json` declares `react`, codeint shows the 10th to 90th percentile of this panel as its Reference range.**
The source label reads `codeint panel of 20 React (JSX and TSX) repositories, ADR 0020, codeint 0.6.0`.

| Metric | Reference range |
| ------ | --------------- |
| Erosion | 0.106 to 0.544 |
| Verbosity | 0.047 to 0.368 |
| Duplication | 0.046 to 0.368 |
| Import cycles | 0.000 to 0.144 |

## Consequences

- React repositories keep full cross-file `.ts`/`.tsx`/`.js`/`.jsx` Import cycle resolution inside `typescript-javascript` while displaying React Reference ranges automatically.
- Vue repositories that also use Vue JSX (`.tsx`/`.jsx` alongside `.vue`) continue displaying Vue 2 or Vue 3 Reference ranges; hybrid repositories declaring both `vue` and `react` with both `.vue` and `.jsx`/`.tsx` files fall back to the general TypeScript and JavaScript Reference ranges ([ADR 0008](./0008-typescript-reference-ranges.md)).
- No code, fixture, or text of a panel repository enters codeint; this record holds only names, commits, and numbers.
- Any future change to default Metric rules or Source set rules requires re-measuring the panel and amending this record.
