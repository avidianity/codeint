---
status: accepted
---

# Vue 2 Reference ranges come from a panel of 20 public repositories

The Owner approved calibrating separate Vue 2 and Vue 3 Reference ranges on 2026-11-22 using the pre-2023 human-written selection rule established in [ADR 0008](./0008-typescript-reference-ranges.md).

## Context

Vue 2 Single-File Components (`.vue`) use the Options API and distinct module conventions from plain TypeScript/JavaScript or Vue 3 `<script setup>` codebases.
This record applies the ADR 0008 selection rule and measurement method to 20 public Vue 2 repositories.

### Selection rule and run

- Query on 2026-11-22: public Vue repositories created before `2022-01-01` (`fork:false archived:false`) whose `package.json` declares `vue` major version `2` and that contain `.vue` files.
- Take 20 repositories passing the ADR 0008 exclusions (>= 5,000 Production source lines, Generated + Vendored <= Production, and `codeint check` exits 0).
- Pin each repository at the last commit on its default branch dated before `2023-01-01`.
- Measured with `codeint 0.4.0` release build (default clone size 75 tokens, filter 14 distinct normalized tokens) on Apple Silicon.
- Run time across the 20 repositories: 0.010s to 1.375s per repository (0 unparsable files).

### Panel

| Repository | Stars band | Pinned commit | Production lines |
| ---------- | ---------- | ------------- | ---------------- |
| `FreeTubeApp/FreeTube` | 10k to 25k | `b96c6b554222412014e9e8ac7e2e48ce4e33fffe` | 17,794 |
| `museui/muse-ui` | 5k to 10k | `e799a4ff3854b9341e2207720c0937b1ba040bbc` | 12,609 |
| `VueTorrent/VueTorrent` | 5k to 10k | `b53c331d4c98635f38945d9898ab72358bb7bf34` | 8,737 |
| `hinesboy/mavonEditor` | 5k to 10k | `d5c6284037e21909c8d2558f500b82a1241e548a` | 7,356 |
| `xyxiao001/vue-cropper` | under 5k | `823a1d88d0871b69c0e6d2aa6e3a893a53348a45` | 5,906 |
| `NervJS/taro` | 25k to 50k | `9f4baeefcb5ef82356b672fa53611ec6ea2a241d` | 93,970 |
| `surveyjs/survey-library` | under 5k | `21f4f11b6c29c52e5feb235ef74d50c01229f866` | 79,041 |
| `statping/statping` | 5k to 10k | `aa515545dba227a97a5e705bb23bfc87d5e312e7` | 16,625 |
| `KlausSchaefers/quant-ux` | under 5k | `788961f3b01efd89fc071b8479a6fcdc8f0a4c15` | 93,644 |
| `Geek-James/ddBuy` | under 5k | `62495996adc35cc7f075b71b5a19cec49f8d1c63` | 7,865 |
| `tookit/vue-material-admin` | under 5k | `6d67e0fe82e1b01e4e89d868614d4773de67e745` | 7,419 |
| `jsbroks/coco-annotator` | under 5k | `9cce5d2f64944e2aa7ca829ca4032624e3305138` | 8,074 |
| `4xmen/anubias` | under 5k | `94df5c585e9b81451e09a4a903233b57f097e59c` | 5,491 |
| `mainsail-crew/mainsail` | under 5k | `c2ecc845cadcb998875f3ab8069a5f76afe86ed9` | 23,718 |
| `tuandm/laravue` | under 5k | `b56ec7526e47f06a37ca79cc356f545fcef21f34` | 11,048 |
| `getgridea/gridea` | 10k to 25k | `0374add4ff98afcab2c2a8b0c5d1473408917bd9` | 6,528 |
| `vform666/variant-form` | under 5k | `9e20d046c507a83ab85972f2d4003d93bc6dc6b4` | 12,256 |
| `bootstrap-vue/bootstrap-vue` | 10k to 25k | `c597779a8296edc68be719e10330c4d9b74cbc8e` | 29,234 |
| `requarks/wiki` | 25k to 50k | `bfbb64a749b9d93f45f4df7e5ef846faf992f390` | 33,058 |
| `owncloud/web` | under 5k | `57ca0720be49891dd283eac99769555774e1fdf2` | 34,950 |

## Measurements

### Per Metric, over the 20 repositories

| Metric | Mean | Median | Standard deviation | 10th percentile | 90th percentile | Lowest | Highest |
| ------ | ---- | ------ | ------------------ | --------------- | --------------- | ------ | ------- |
| Erosion | 0.259 | 0.287 | 0.156 | 0.020 | 0.452 | 0.000 | 0.599 |
| Verbosity | 0.129 | 0.103 | 0.173 | 0.036 | 0.192 | 0.011 | 0.844 |
| Duplication | 0.128 | 0.101 | 0.173 | 0.036 | 0.192 | 0.009 | 0.844 |
| Import cycles | 0.024 | 0.000 | 0.047 | 0.000 | 0.114 | 0.000 | 0.160 |

### Per repository, in the Production Source set

| Repository | Erosion | Verbosity | Duplication | Import cycles |
| ---------- | ------- | --------- | ----------- | ------------- |
| `FreeTubeApp/FreeTube` | 0.286 | 0.108 | 0.105 | 0.010 |
| `museui/muse-ui` | 0.185 | 0.055 | 0.055 | 0.000 |
| `VueTorrent/VueTorrent` | 0.102 | 0.011 | 0.011 | 0.015 |
| `hinesboy/mavonEditor` | 0.314 | 0.228 | 0.228 | 0.000 |
| `xyxiao001/vue-cropper` | 0.599 | 0.844 | 0.844 | 0.000 |
| `NervJS/taro` | 0.352 | 0.105 | 0.103 | 0.013 |
| `surveyjs/survey-library` | 0.202 | 0.102 | 0.099 | 0.160 |
| `statping/statping` | 0.478 | 0.108 | 0.107 | 0.000 |
| `KlausSchaefers/quant-ux` | 0.288 | 0.142 | 0.141 | 0.004 |
| `Geek-James/ddBuy` | 0.000 | 0.011 | 0.009 | 0.000 |
| `tookit/vue-material-admin` | 0.022 | 0.060 | 0.059 | 0.112 |
| `jsbroks/coco-annotator` | 0.327 | 0.057 | 0.057 | 0.000 |
| `4xmen/anubias` | 0.312 | 0.189 | 0.188 | 0.000 |
| `mainsail-crew/mainsail` | 0.331 | 0.124 | 0.124 | 0.007 |
| `tuandm/laravue` | 0.000 | 0.072 | 0.071 | 0.000 |
| `getgridea/gridea` | 0.219 | 0.038 | 0.038 | 0.000 |
| `vform666/variant-form` | 0.187 | 0.104 | 0.103 | 0.000 |
| `bootstrap-vue/bootstrap-vue` | 0.384 | 0.041 | 0.041 | 0.000 |
| `requarks/wiki` | 0.449 | 0.120 | 0.118 | 0.022 |
| `owncloud/web` | 0.137 | 0.060 | 0.060 | 0.131 |

## Decision

**When a repository measures `.vue` files and `package.json` declares `vue` major version `2`, codeint shows the 10th to 90th percentile of this panel as its Reference range.**
The source label reads `codeint panel of 20 Vue 2 repositories, ADR 0014, codeint 0.4.0`.

| Metric | Reference range |
| ------ | --------------- |
| Erosion | 0.020 to 0.452 |
| Verbosity | 0.036 to 0.192 |
| Duplication | 0.036 to 0.192 |
| Import cycles | 0.000 to 0.114 |

## Consequences

- Vue 2 repositories keep full cross-file `.ts`/`.js`/`.vue` Import cycle resolution inside `typescript-javascript` while displaying Vue 2 Reference ranges automatically.
- No code, fixture, or text of a panel repository enters codeint; this record holds only names, commits, and numbers.
- Any future change to default Metric rules or Source set rules requires re-measuring the panel and amending this record.
