---
status: superseded by ADR-0035
---

# Python Reference ranges come from a panel of 20 public repositories

The Owner approved running the calibrated Python panel on 2026-11-22 using the pre-2023 human-written selection rule established in [ADR 0008](./0008-typescript-reference-ranges.md).

## Context

A Reference range must describe human-written code, or it cannot show how far agent-written code drifts from it.
This record applies the same selection rule and measurement method as ADR 0008 to 20 public Python repositories.

### Selection rule and run

- Query on 2026-11-22: `language:Python created:<2022-01-01 fork:false archived:false`, sorted by stars descending.
- Take the first 20 repositories passing the ADR 0008 exclusions (no forks/archived/lists/tutorials/books/collections, >= 5,000 Production source lines, Generated + Vendored <= Production, and `codeint check` exits 0).
- Pin each repository at the last commit on its default branch dated before `2023-01-01`.
- Measured with `codeint 0.4.0` release build (default clone size 75 tokens, filter 14 distinct normalized tokens) on Apple Silicon.
- Run time across the 20 repositories: 0.010s to 0.851s per repository (4.40s total, 0 unparsable Python files).

### Exclusions

- `public-apis/public-apis`: topic/description (list of).
- `EbookFoundation/free-programming-books`: topic/description (book).
- `donnemartin/system-design-primer`: topic/description (interview).
- `vinta/awesome-python`: topic/description (awesome).
- `practical-tutorials/project-based-learning`: topic/description (list of).
- `TheAlgorithms/Python`: topic/description (interview).
- `521xueweihan/HelloGitHub`: topic/description (awesome).
- `sherlock-project/sherlock`: 1140 Production lines (< 5,000).
- `bregman-arie/devops-exercises`: topic/description (interview).
- `fighting41love/funNLP`: topic/description (awesome).
- `swisskyrepo/PayloadsAllTheThings`: topic/description (list of).
- `d2l-ai/d2l-zh`: topic/description (book).
- `Z4nzu/hackingtool`: 1967 Production lines (< 5,000).
- `Asabeneh/30-Days-Of-Python`: topic/description (100 days).
- `josephmisiti/awesome-machine-learning`: topic/description (awesome).
- `labmlai/annotated_deep_learning_paper_implementations`: topic/description (tutorial).

### Panel

| Repository | Stars band | Pinned commit | Production lines |
| ---------- | ---------- | ------------- | ---------------- |
| `yt-dlp/yt-dlp` | over 100k | `2fb0f858686c46abc50a0e253245afe750746775` | 173,655 |
| `huggingface/transformers` | over 100k | `375801d5e65457d2ef86e6b269703c5f9ad67252` | 580,324 |
| `ytdl-org/youtube-dl` | over 100k | `195f22f679330549882a8234e7234942893a4902` | 131,379 |
| `pytorch/pytorch` | over 100k | `39d49dbe4546c9f4d153ec748f62371e4546158a` | 471,422 |
| `fastapi/fastapi` | over 100k | `efc12c5cdbede6922a033a5234c70cb22c4d204a` | 12,611 |
| `nvbn/thefuck` | 50k to 100k | `ceeaeab94b5df5a4fe9d94d61e4f6b0bbea96378` | 5,049 |
| `3b1b/manim` | 50k to 100k | `a31c4ae3c23f060a34f3a4f5062c1d67fc4c268a` | 17,579 |
| `PaddlePaddle/PaddleOCR` | 50k to 100k | `4f735dbde167684cce5dcd14c035e9f67b4d187d` | 66,093 |
| `home-assistant/core` | 50k to 100k | `6d4489e82207916a811e936be00246c5cbc10545` | 671,942 |
| `django/django` | 50k to 100k | `c179ad9fe7e82dcb80261aa016f2fe18c8fcc181` | 116,838 |
| `python/cpython` | 50k to 100k | `baf234971f20e5e6c7438f46889e9c8f172cc25e` | 291,724 |
| `tensorflow/models` | 50k to 100k | `755617b99ca8b647367d806cad30d463693da02b` | 319,102 |
| `openbq-org/OpenBB` | 50k to 100k | `21247ac302dfa98e187ec70b832b76eb46e9c9e8` | 148,307 |
| `apache/superset` | 50k to 100k | `734d423ca094746ee1c2f2c1e72138ed9e69cdee` | 83,725 |
| `pallets/flask` | 50k to 100k | `74e5263c88e51bb442879ab863a5d03292fc0a33` | 7,935 |
| `ansible/ansible` | 50k to 100k | `45df14a2e05573e6213ce789aa3278f4f30fa54f` | 93,017 |
| `scikit-learn/scikit-learn` | 50k to 100k | `96a0bc861ad2ba3c63db0326b42b41ddabeb2aff` | 189,155 |
| `scrapy/scrapy` | 50k to 100k | `deaf1fb6cf513db8119acfe70fd3e08f761f7ede` | 16,683 |
| `keras-team/keras` | 50k to 100k | `ed99e34f279a2d2d6a44af87ee64f8fc98c7e8b9` | 131,710 |
| `commaai/openpilot` | 50k to 100k | `6c8aca245ae78433a4f8e4acf3b1e5270fcb8768` | 34,524 |

## Measurements

### Per Metric, over the 20 repositories

| Metric | Mean | Median | Standard deviation | 10th percentile | 90th percentile | Lowest | Highest |
| ------ | ---- | ------ | ------------------ | --------------- | --------------- | ------ | ------- |
| Erosion | 0.396 | 0.406 | 0.182 | 0.144 | 0.624 | 0.012 | 0.681 |
| Verbosity | 0.084 | 0.043 | 0.094 | 0.014 | 0.204 | 0.007 | 0.406 |
| Duplication | 0.078 | 0.035 | 0.095 | 0.012 | 0.201 | 0.002 | 0.403 |
| Import cycles | 0.106 | 0.032 | 0.130 | 0.000 | 0.321 | 0.000 | 0.456 |

### Per repository, in the Production Source set

| Repository | Erosion | Verbosity | Duplication | Import cycles |
| ---------- | ------- | --------- | ----------- | ------------- |
| `yt-dlp/yt-dlp` | 0.590 | 0.031 | 0.028 | 0.006 |
| `huggingface/transformers` | 0.621 | 0.406 | 0.403 | 0.011 |
| `ytdl-org/youtube-dl` | 0.649 | 0.034 | 0.031 | 0.011 |
| `pytorch/pytorch` | 0.395 | 0.059 | 0.048 | 0.332 |
| `fastapi/fastapi` | 0.382 | 0.207 | 0.201 | 0.021 |
| `nvbn/thefuck` | 0.012 | 0.038 | 0.015 | 0.047 |
| `3b1b/manim` | 0.151 | 0.007 | 0.002 | 0.108 |
| `PaddlePaddle/PaddleOCR` | 0.359 | 0.140 | 0.134 | 0.016 |
| `home-assistant/core` | 0.234 | 0.083 | 0.082 | 0.026 |
| `django/django` | 0.423 | 0.029 | 0.018 | 0.186 |
| `python/cpython` | 0.450 | 0.048 | 0.040 | 0.031 |
| `tensorflow/models` | 0.315 | 0.120 | 0.116 | 0.034 |
| `openbq-org/OpenBB` | 0.317 | 0.204 | 0.204 | 0.000 |
| `apache/superset` | 0.417 | 0.085 | 0.084 | 0.320 |
| `pallets/flask` | 0.251 | 0.023 | 0.020 | 0.188 |
| `ansible/ansible` | 0.681 | 0.031 | 0.023 | 0.000 |
| `scikit-learn/scikit-learn` | 0.502 | 0.025 | 0.019 | 0.199 |
| `scrapy/scrapy` | 0.089 | 0.009 | 0.008 | 0.136 |
| `keras-team/keras` | 0.478 | 0.086 | 0.076 | 0.456 |
| `commaai/openpilot` | 0.597 | 0.015 | 0.012 | 0.000 |

## Decision

**The Python Language plugin shows, for each Metric, the 10th to 90th percentile of this panel as its Reference range.**
The source label reads `codeint panel of 20 Python repositories, ADR 0013, codeint 0.4.0`.

| Metric | Reference range |
| ------ | --------------- |
| Erosion | 0.144 to 0.624 |
| Verbosity | 0.014 to 0.204 |
| Duplication | 0.012 to 0.201 |
| Import cycles | 0.000 to 0.321 |

## Consequences

- Python reports show a Reference range next to each Metric with the source label above.
- No code, fixture, or text of a panel repository enters codeint; this record holds only names, commits, and numbers.
- Any future change to default Metric rules or Source set rules requires re-measuring the panel and amending this record.
