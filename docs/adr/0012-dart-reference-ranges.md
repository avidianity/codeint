---
status: accepted
---

# Dart Reference ranges come from a panel of 20 public repositories

The Owner approved running the calibrated Dart panel on 2026-11-22 using the pre-2023 human-written selection rule established in [ADR 0008](./0008-typescript-reference-ranges.md).

## Context

A Reference range must describe human-written code, or it cannot show how far agent-written code drifts from it.
This record applies the same selection rule and measurement method as ADR 0008 to 20 public Dart repositories.

### Selection rule and run

- Query on 2026-11-22: `language:Dart created:<2022-01-01 fork:false archived:false`, sorted by stars descending.
- Take the first 20 repositories passing the ADR 0008 exclusions (no forks/archived/lists/tutorials/books/collections, >= 5,000 Production source lines, Generated + Vendored <= Production, and `codeint check` exits 0).
- Pin each repository at the last commit on its default branch dated before `2023-01-01`.
- Measured with `codeint 0.2.1` release build at commit `ac42e2f` (default clone size 75 tokens, filter 14 distinct normalized tokens) on Apple Silicon.
- Run time across the 20 repositories: 0.011s to 8.736s per repository (11.1s total).

### Exclusions

- `Solido/awesome-flutter`: topic/description (awesome).
- `GopeedLab/gopeed`: 2440 Production lines (< 5,000).
- `iampawan/FlutterExampleApps`: 5 Production lines (< 5,000).
- `flutter/samples`: topic/description (collection of).
- `getlantern/lantern`: no default-branch commit before 2023-01-01.
- `cfug/dio`: 4980 Production lines (< 5,000).
- `firebase/flutterfire`: topic/description (collection of).
- `kaina404/FlutterDouBan`: topic/description (awesome).
- `ComicSparks/pikapika`: topic/description (book).
- `alibaba/fish-redux`: 4484 Production lines (< 5,000).
- `OpenFlutter/Flutter-Notebook`: topic/description (book).
- `abuanwar072/Flutter-Responsive-Admin-Panel-or-Dashboard`: 959 Production lines (< 5,000).
- `vandadnp/flutter-tips-and-tricks`: topic/description (collection of).
- `2d-inc/HistoryOfEverything`: 4833 Production lines (< 5,000).
- `iampawan/Flutter-UI-Kit`: topic/description (collection of).
- `samarthagarwal/FlutterScreens`: topic/description (collection of).
- `TheLastGimbus/GooglePhotosTakeoutHelper`: 472 Production lines (< 5,000).
- `flutterchina/flukit`: 4586 Production lines (< 5,000).

### Panel

| Repository | Stars band | Pinned commit | Production lines |
| ---------- | ---------- | ------------- | ---------------- |
| `flutter/flutter` | over 100k | `0196e6050b75dd92def07e4a45900a7ff51662d8` | 510,218 |
| `AppFlowy-IO/AppFlowy` | 50k to 100k | `a2b5d6fa996ad76f422b77e85ce6a85f87b79437` | 102,905 |
| `alibaba/flutter-go` | 10k to 25k | `a41b2124278e9f17d222ec1cea2ace3042be03f7` | 26,578 |
| `mitesh77/Best-Flutter-UI-Templates` | 10k to 25k | `1033b06fbc5d022a9d439876bc8cb8a831e694dd` | 9,473 |
| `CarGuo/gsy_github_app_flutter` | 10k to 25k | `428dbfed1fc17917d6528a32f47b5b389da786cc` | 20,027 |
| `Notsfsssf/pixez-flutter` | 10k to 25k | `41edc91b678d8de5dc49461664c558e40f6cedf7` | 27,511 |
| `felangel/bloc` | 10k to 25k | `554db6c066f3078addf4de5bb6a3f09a77e3681d` | 10,425 |
| `dart-lang/sdk` | 10k to 25k | `9383820f15fd5a7910064eb1267bb733ab07f713` | 1,406,078 |
| `jonataslaw/getx` | 10k to 25k | `c3e9a1d29c12f1c97806cbf24a8a3681dc5f4002` | 14,091 |
| `flame-engine/flame` | 10k to 25k | `6c5ef0540aff778212a1f96d50c6df8cc4fe4279` | 42,811 |
| `lollipopkit/flutter_server_box` | 5k to 10k | `b2e2d4c603094b372f3ea176639103cbb1dc506d` | 7,012 |
| `brianegan/flutter_architecture_samples` | 5k to 10k | `41a033f6e67ec51bba2edf669cfcb857498db58c` | 18,955 |
| `toly1994328/FlutterUnit` | 5k to 10k | `e67a5377d5859c90bd4dc30b127a44991b8d8d49` | 47,239 |
| `simplezhli/flutter_deer` | 5k to 10k | `fa299060bb1dd5a2f3b8b469d67c6fcce4a44a98` | 16,582 |
| `imaNNeo/fl_chart` | 5k to 10k | `42e9863589072c33c0ae1bd9418da698c98eb075` | 15,067 |
| `rrousselGit/riverpod` | 5k to 10k | `d34d7400730f40db447a410c99d3a05027b0ce15` | 22,875 |
| `alibaba/flutter_boost` | 5k to 10k | `47fc55fd4bb41d6e69bced3de4ac464c5bc19974` | 6,921 |
| `nisrulz/flutter-examples` | 5k to 10k | `f761e553e45af904f036a9e38208eaa1381d9fec` | 12,396 |
| `Sky24n/flutter_wanandroid` | 5k to 10k | `51061b34a4923bfeaad02f9d1fa53f296391d9e7` | 7,314 |
| `deckerst/aves` | 5k to 10k | `44eb9c7fdf73ae115d1c6e6afa00da622d7b2a35` | 68,119 |

## Measurements

### Per Metric, over the 20 repositories

| Metric | Mean | Median | Standard deviation | 10th percentile | 90th percentile | Lowest | Highest |
| ------ | ---- | ------ | ------------------ | --------------- | --------------- | ------ | ------- |
| Erosion | 0.216 | 0.173 | 0.167 | 0.047 | 0.419 | 0.016 | 0.699 |
| Verbosity | 0.179 | 0.172 | 0.089 | 0.080 | 0.259 | 0.057 | 0.391 |
| Duplication | 0.174 | 0.167 | 0.091 | 0.073 | 0.256 | 0.050 | 0.390 |
| Import cycles | 0.299 | 0.312 | 0.180 | 0.073 | 0.524 | 0.032 | 0.647 |

### Per repository, in the Production Source set

| Repository | Erosion | Verbosity | Duplication | Import cycles |
| ---------- | ------- | --------- | ----------- | ------------- |
| `flutter/flutter` | 0.403 | 0.112 | 0.110 | 0.208 |
| `AppFlowy-IO/AppFlowy` | 0.168 | 0.167 | 0.163 | 0.246 |
| `alibaba/flutter-go` | 0.149 | 0.248 | 0.244 | 0.480 |
| `mitesh77/Best-Flutter-UI-Templates` | 0.143 | 0.217 | 0.202 | 0.471 |
| `CarGuo/gsy_github_app_flutter` | 0.178 | 0.145 | 0.137 | 0.405 |
| `Notsfsssf/pixez-flutter` | 0.107 | 0.215 | 0.212 | 0.549 |
| `felangel/bloc` | 0.059 | 0.134 | 0.132 | 0.331 |
| `dart-lang/sdk` | 0.410 | 0.176 | 0.171 | 0.180 |
| `jonataslaw/getx` | 0.221 | 0.114 | 0.104 | 0.397 |
| `flame-engine/flame` | 0.200 | 0.082 | 0.074 | 0.521 |
| `lollipopkit/flutter_server_box` | 0.048 | 0.057 | 0.050 | 0.076 |
| `brianegan/flutter_architecture_samples` | 0.016 | 0.391 | 0.390 | 0.111 |
| `toly1994328/FlutterUnit` | 0.495 | 0.244 | 0.242 | 0.032 |
| `simplezhli/flutter_deer` | 0.263 | 0.067 | 0.060 | 0.323 |
| `imaNNeo/fl_chart` | 0.282 | 0.185 | 0.180 | 0.427 |
| `rrousselGit/riverpod` | 0.156 | 0.358 | 0.354 | 0.040 |
| `alibaba/flutter_boost` | 0.180 | 0.086 | 0.080 | 0.145 |
| `nisrulz/flutter-examples` | 0.115 | 0.236 | 0.234 | 0.087 |
| `Sky24n/flutter_wanandroid` | 0.039 | 0.245 | 0.244 | 0.300 |
| `deckerst/aves` | 0.699 | 0.107 | 0.104 | 0.647 |

## Decision

**The Dart Language plugin shows, for each Metric, the 10th to 90th percentile of this panel as its Reference range.**
The source label reads `codeint panel of 20 Dart repositories, ADR 0012, codeint 0.2.1`.

| Metric | Reference range |
| ------ | --------------- |
| Erosion | 0.047 to 0.419 |
| Verbosity | 0.080 to 0.259 |
| Duplication | 0.073 to 0.256 |
| Import cycles | 0.073 to 0.524 |

## Consequences

- Dart reports show a Reference range next to each Metric with the source label above.
- No code, fixture, or text of a panel repository enters codeint; this record holds only names, commits, and numbers.
- Any future change to default Metric rules or Source set rules requires re-measuring the panel and amending this record.
