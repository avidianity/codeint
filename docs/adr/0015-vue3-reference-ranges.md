---
status: superseded by ADR-0035
---

# Vue 3 Reference ranges come from a panel of 20 public repositories

The Owner approved calibrating separate Vue 2 and Vue 3 Reference ranges on 2026-11-22 using the pre-2023 human-written selection rule established in [ADR 0008](./0008-typescript-reference-ranges.md).

## Context

Vue 3 Single-File Components (`.vue`) adopt the Composition API and `<script setup>` alongside TypeScript, producing different entropy distributions from Vue 2 Options API codebases.
This record applies the ADR 0008 selection rule and measurement method to 20 public Vue 3 repositories.

### Selection rule and run

- Query on 2026-11-22: public Vue repositories created before `2022-01-01` (`fork:false archived:false`) whose `package.json` declares `vue` major version `3` and that contain `.vue` files.
- Take 20 repositories passing the ADR 0008 exclusions (>= 5,000 Production source lines, Generated + Vendored <= Production, and `codeint check` exits 0).
- Pin each repository at the last commit on its default branch dated before `2023-01-01`.
- Measured with `codeint 0.4.0` release build (default clone size 75 tokens, filter 14 distinct normalized tokens) on Apple Silicon.
- Run time across the 20 repositories: 0.008s to 0.490s per repository (0 unparsable files).

### Panel

| Repository | Stars band | Pinned commit | Production lines |
| ---------- | ---------- | ------------- | ---------------- |
| `AutomaApp/automa` | 10k to 25k | `f0dca888589437acc49ba1f54c9d0dc221dd7077` | 32,720 |
| `snapshot-labs/snapshot-v1` | 5k to 10k | `1a50981f459079bf3a1236f5cc7cac0bccde97e5` | 14,660 |
| `heyui/heyui` | under 5k | `c5b42abf2c37d7fde21a88229beb0b3592bdde6d` | 15,150 |
| `tobe-fe-dalao/fast-vue3` | under 5k | `470105d576e90ae40a5aa433a78d62dfa7a617d8` | 14,607 |
| `MelihAltintas/vue3-openlayers` | under 5k | `eb300feeecb9d1b6747857568af47b8bdea5a8d2` | 5,011 |
| `antoniandre/wave-ui` | under 5k | `efc69ca46b41c34d857685400812bdd5279e307d` | 21,931 |
| `vuefront/vuefront` | under 5k | `06083a96dc46a455c98737ef35868dbc549a92c8` | 31,113 |
| `logaretm/vee-validate` | 10k to 25k | `5265af5c75b7809ebd73b22d4c8319a8a146da5a` | 5,523 |
| `JosephusPaye/Keen-UI` | under 5k | `9d6ff42b02569a2fc5541faa275339b06eae1cfa` | 85,013 |
| `buqiyuan/vite-vue3-lowcode` | under 5k | `035663eb3b8198404245f68df49fe00799ca27c3` | 7,302 |
| `intlify/vue-i18n` | under 5k | `fb6775a40448a0023b0edb281f7dd25e8a52f478` | 17,286 |
| `imzbf/md-editor-v3` | under 5k | `8a78897d6d6ec9149621e79dea0112a2f76da1c4` | 6,115 |
| `epicmaxco/vuestic-admin` | 10k to 25k | `5a3204f24aa1586c98316cd6208dabbf7f3fd8bc` | 7,183 |
| `varletjs/varlet` | 5k to 10k | `54e0ab80323437f226b869e564e7d0d04dba8bb1` | 24,034 |
| `epicmaxco/vuestic-ui` | under 5k | `5ef276e4e7423c5690b7c265c99472225cdd5343` | 36,379 |
| `antares-sql/antares` | under 5k | `6fa430adf68013a9d0a093031f56dd741bdc0299` | 29,449 |
| `open-source-labs/OverVue` | under 5k | `81ddfe9772f971f65f3585f97638c4ca482550bf` | 8,608 |
| `staniel359/muffon` | under 5k | `d58b54ab42fea3c8dedbc76eb90818905bb4604e` | 66,836 |
| `hoppscotch/hoppscotch` | 50k to 100k | `acafc072db28db247321d9c624734d3620d2c145` | 32,986 |
| `nuxt/nuxt` | 50k to 100k | `0a701ae7b3cae6e18ee26439c20eae8a3f63236f` | 13,043 |

## Measurements

### Per Metric, over the 20 repositories

| Metric | Mean | Median | Standard deviation | 10th percentile | 90th percentile | Lowest | Highest |
| ------ | ---- | ------ | ------------------ | --------------- | --------------- | ------ | ------- |
| Erosion | 0.211 | 0.191 | 0.167 | 0.019 | 0.388 | 0.000 | 0.636 |
| Verbosity | 0.164 | 0.113 | 0.201 | 0.024 | 0.346 | 0.010 | 0.870 |
| Duplication | 0.163 | 0.112 | 0.201 | 0.023 | 0.345 | 0.010 | 0.870 |
| Import cycles | 0.032 | 0.013 | 0.041 | 0.000 | 0.077 | 0.000 | 0.133 |

### Per repository, in the Production Source set

| Repository | Erosion | Verbosity | Duplication | Import cycles |
| ---------- | ------- | --------- | ----------- | ------------- |
| `AutomaApp/automa` | 0.269 | 0.028 | 0.028 | 0.000 |
| `snapshot-labs/snapshot-v1` | 0.084 | 0.025 | 0.024 | 0.062 |
| `heyui/heyui` | 0.277 | 0.047 | 0.045 | 0.000 |
| `tobe-fe-dalao/fast-vue3` | 0.259 | 0.326 | 0.326 | 0.000 |
| `MelihAltintas/vue3-openlayers` | 0.127 | 0.192 | 0.192 | 0.000 |
| `antoniandre/wave-ui` | 0.128 | 0.076 | 0.076 | 0.000 |
| `vuefront/vuefront` | 0.057 | 0.523 | 0.523 | 0.000 |
| `logaretm/vee-validate` | 0.206 | 0.041 | 0.041 | 0.026 |
| `JosephusPaye/Keen-UI` | 0.289 | 0.870 | 0.870 | 0.000 |
| `buqiyuan/vite-vue3-lowcode` | 0.022 | 0.074 | 0.074 | 0.044 |
| `intlify/vue-i18n` | 0.559 | 0.153 | 0.151 | 0.041 |
| `imzbf/md-editor-v3` | 0.636 | 0.114 | 0.113 | 0.128 |
| `epicmaxco/vuestic-admin` | 0.000 | 0.010 | 0.010 | 0.000 |
| `varletjs/varlet` | 0.127 | 0.111 | 0.111 | 0.052 |
| `epicmaxco/vuestic-ui` | 0.106 | 0.044 | 0.044 | 0.030 |
| `antares-sql/antares` | 0.335 | 0.182 | 0.181 | 0.133 |
| `open-source-labs/OverVue` | 0.198 | 0.145 | 0.144 | 0.000 |
| `staniel359/muffon` | 0.000 | 0.145 | 0.143 | 0.000 |
| `hoppscotch/hoppscotch` | 0.184 | 0.164 | 0.163 | 0.071 |
| `nuxt/nuxt` | 0.369 | 0.010 | 0.010 | 0.060 |

## Decision

**When a repository measures `.vue` files and `package.json` declares `vue` major version `3`, codeint shows the 10th to 90th percentile of this panel as its Reference range.**
The source label reads `codeint panel of 20 Vue 3 repositories, ADR 0015, codeint 0.4.0`.

| Metric | Reference range |
| ------ | --------------- |
| Erosion | 0.019 to 0.388 |
| Verbosity | 0.024 to 0.346 |
| Duplication | 0.023 to 0.345 |
| Import cycles | 0.000 to 0.077 |

## Consequences

- Vue 3 repositories keep full cross-file `.ts`/`.js`/`.vue` Import cycle resolution inside `typescript-javascript` while displaying Vue 3 Reference ranges automatically.
- No code, fixture, or text of a panel repository enters codeint; this record holds only names, commits, and numbers.
- Any future change to default Metric rules or Source set rules requires re-measuring the panel and amending this record.
