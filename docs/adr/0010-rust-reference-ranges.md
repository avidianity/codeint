---
status: superseded by ADR 0035
---

# Rust Reference ranges come from a panel of 20 public repositories

The Owner approved running the calibrated Rust panel on 2026-11-22 using the pre-2023 human-written selection rule established in [ADR 0008](./0008-typescript-reference-ranges.md).

## Context

A Reference range must describe human-written code, or it cannot show how far agent-written code drifts from it.
This record applies the same selection rule and measurement method as ADR 0008 to 20 public Rust repositories.

### Selection rule and run

- Query on 2026-11-22: `language:Rust created:<2022-01-01 fork:false archived:false`, sorted by stars descending.
- Take the first 20 repositories passing the ADR 0008 exclusions (no forks/archived/lists/tutorials/books/collections, >= 5,000 Production source lines, Generated + Vendored <= Production, and `codeint check` exits 0).
- Pin each repository at the last commit on its default branch dated before `2023-01-01`.
- Measured with `codeint 0.2.1` release build at commit `ac42e2f` (default clone size 75 tokens, filter 14 distinct normalized tokens) on Apple Silicon.
- Run time across the 20 repositories: 0.025s to 1.054s per repository (3.6s total).

### Exclusions

- `rust-lang/rust`: codeint exited -6: thread '<unknown>' (2077022) has overflowed its stack
fatal runtime error: stack overflow, aborting.
- `warpdotdev/warp`: no default-branch commit before 2023-01-01.
- `rust-lang/rustlings`: 1858 Production lines (< 5,000).
- `rust-unofficial/awesome-rust`: topic/description (awesome).
- `sharkdp/fd`: 2662 Production lines (< 5,000).
- `ajeetdsouza/zoxide`: 1402 Production lines (< 5,000).

### Panel

| Repository | Stars band | Pinned commit | Production lines |
| ---------- | ---------- | ------------- | ---------------- |
| `rustdesk/rustdesk` | over 100k | `0dcf10616d0f0e45a1f15341d72f34ba86413326` | 76,263 |
| `tauri-apps/tauri` | over 100k | `ce76d95ab186f4556166a2a22360067eab000fc8` | 37,940 |
| `denoland/deno` | over 100k | `f729576b2db2aa6ce000a598ad2e45533f686213` | 309,944 |
| `oven-sh/bun` | 50k to 100k | `9b3db963081d5556a9fca41e5baa2670cc5cba49` | 79,120 |
| `zed-industries/zed` | 50k to 100k | `eeb21af841d3bae73f313902a82126497339c7c9` | 113,624 |
| `dani-garcia/vaultwarden` | 50k to 100k | `613b2519edc53dfcc7f82ea4e402ff846ab9cc04` | 34,953 |
| `BurntSushi/ripgrep` | 50k to 100k | `61101289fabc032fd8e90009c41d0b78e6dfc9a2` | 18,661 |
| `alacritty/alacritty` | 50k to 100k | `2bd26fbeb0c51cd8a98ae31d58ea105d1274387a` | 18,582 |
| `sharkdp/bat` | 50k to 100k | `4e34b362f844dae57b017e56a24235d8163ffbf7` | 5,343 |
| `FuelLabs/sway` | 50k to 100k | `9a0e658ed70b429c5fe39537620ef6f9d11d4729` | 88,825 |
| `starship/starship` | 50k to 100k | `67b6376e2ef0835350e3e856ade6602b6c187c42` | 15,242 |
| `meilisearch/meilisearch` | 50k to 100k | `776acb5ed3b825c971f1de36e6ac908eb7b98945` | 17,422 |
| `typst/typst` | 50k to 100k | `54962e6dcd002fd27918827996155fd7dc4e1cff` | 26,070 |
| `FuelLabs/fuel-core` | 50k to 100k | `a3f584f12bb3e160aa00e08616a27c81734ebd02` | 23,207 |
| `bevyengine/bevy` | 25k to 50k | `b027d402e29d7d4f26062051161bc2187f168e80` | 68,565 |
| `helix-editor/helix` | 25k to 50k | `febe1a5172f346bcb52138e1f9c3d16c212d5a3c` | 35,872 |
| `parcel-bundler/parcel` | 25k to 50k | `90a886fb2a1cc15fd6092dcbfae338f6419d694b` | 9,652 |
| `FuelLabs/fuels-rs` | 25k to 50k | `b08c8a89748841736b85eedb686f1b8a843e6297` | 8,518 |
| `nushell/nushell` | 25k to 50k | `ececca7ad2912283ab4dbb6ae2600a76503c1b57` | 121,731 |
| `pola-rs/polars` | 25k to 50k | `f8f9a7cae38c2d92de9032ae57da93547211bd70` | 104,024 |

## Measurements

### Per Metric, over the 20 repositories

| Metric | Mean | Median | Standard deviation | 10th percentile | 90th percentile | Lowest | Highest |
| ------ | ---- | ------ | ------------------ | --------------- | --------------- | ------ | ------- |
| Erosion | 0.523 | 0.547 | 0.111 | 0.380 | 0.642 | 0.253 | 0.700 |
| Verbosity | 0.184 | 0.153 | 0.109 | 0.060 | 0.329 | 0.048 | 0.438 |
| Duplication | 0.182 | 0.151 | 0.109 | 0.050 | 0.325 | 0.047 | 0.437 |
| Import cycles | 0.224 | 0.169 | 0.202 | 0.021 | 0.506 | 0.008 | 0.748 |

### Per repository, in the Production Source set

| Repository | Erosion | Verbosity | Duplication | Import cycles |
| ---------- | ------- | --------- | ----------- | ------------- |
| `rustdesk/rustdesk` | 0.444 | 0.060 | 0.051 | 0.365 |
| `tauri-apps/tauri` | 0.614 | 0.167 | 0.165 | 0.108 |
| `denoland/deno` | 0.605 | 0.140 | 0.137 | 0.039 |
| `oven-sh/bun` | 0.537 | 0.438 | 0.437 | 0.008 |
| `zed-industries/zed` | 0.387 | 0.108 | 0.105 | 0.126 |
| `dani-garcia/vaultwarden` | 0.511 | 0.109 | 0.108 | 0.621 |
| `BurntSushi/ripgrep` | 0.322 | 0.307 | 0.304 | 0.418 |
| `alacritty/alacritty` | 0.569 | 0.064 | 0.060 | 0.493 |
| `sharkdp/bat` | 0.575 | 0.048 | 0.047 | 0.108 |
| `FuelLabs/sway` | 0.655 | 0.224 | 0.224 | 0.203 |
| `starship/starship` | 0.475 | 0.338 | 0.336 | 0.045 |
| `meilisearch/meilisearch` | 0.641 | 0.228 | 0.227 | 0.256 |
| `typst/typst` | 0.549 | 0.214 | 0.214 | 0.748 |
| `FuelLabs/fuel-core` | 0.412 | 0.098 | 0.097 | 0.110 |
| `bevyengine/bevy` | 0.544 | 0.260 | 0.258 | 0.162 |
| `helix-editor/helix` | 0.499 | 0.110 | 0.108 | 0.231 |
| `parcel-bundler/parcel` | 0.612 | 0.053 | 0.049 | 0.021 |
| `FuelLabs/fuels-rs` | 0.253 | 0.123 | 0.123 | 0.176 |
| `nushell/nushell` | 0.700 | 0.328 | 0.323 | 0.021 |
| `pola-rs/polars` | 0.549 | 0.270 | 0.267 | 0.216 |

## Decision

**The Rust Language plugin shows, for each Metric, the 10th to 90th percentile of this panel as its Reference range.**
The source label reads `codeint panel of 20 Rust repositories, ADR 0010, codeint 0.2.1`.

| Metric | Reference range |
| ------ | --------------- |
| Erosion | 0.380 to 0.642 |
| Verbosity | 0.060 to 0.329 |
| Duplication | 0.050 to 0.325 |
| Import cycles | 0.021 to 0.506 |

## Consequences

- Rust reports show a Reference range next to each Metric with the source label above.
- No code, fixture, or text of a panel repository enters codeint; this record holds only names, commits, and numbers.
- Any future change to default Metric rules or Source set rules requires re-measuring the panel and amending this record.
