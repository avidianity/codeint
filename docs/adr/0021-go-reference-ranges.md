---
status: proposed
---

# Go Reference ranges come from a panel of 20 public repositories

This record proposes calibrated Go Reference ranges for Spec #228 (Tickets #425 to #428) using the pre-2023 human-written selection rule established in [ADR 0008](./0008-typescript-reference-ranges.md).

## Context

A Reference range must describe human-written code, or it cannot show how far agent-written code drifts from it.
This record applies the selection rule and measurement method of [ADR 0008](./0008-typescript-reference-ranges.md) to 20 public Go repositories.

### Selection rule and run

- Query on 2026-11-24: `language:Go created:<2022-01-01 fork:false archived:false`, sorted by stars descending.
- Walk the result list in strict descending star order and take the first 20 repositories passing every ADR 0008 exclusion: not a fork, mirror, or archived; not a list, book, tutorial, roadmap, curriculum, course, or interview-question collection; at least 5,000 Go Production source lines; Generated plus Vendored Go source lines no larger than Production source lines; and `codeint check` exits with code 0.
- Pin each repository at the last commit on its default branch dated before `2023-01-01` by committer date.
- Shallow-clone each pinned commit into a temporary scratch folder outside codeint, measure in place with `/usr/bin/time -l codeint check --format json --all`, never build or run any panel repository, and delete the clone immediately afterward.
- On multi-language repositories, read the `languages.go` section so every count and Metric ratio measures Go code only.
- Measured with `codeint 0.6.0` release build (default minimum clone size 75 tokens, clone filter 14 distinct normalized tokens) on Apple Silicon.
- Every percentile interpolates linearly between the two nearest ranked values (`idx = p * (n - 1)`), and standard deviation is the sample standard deviation (`n - 1`).
- Run time across the 20 panel repositories: `0.01s` to `9.67s` per repository (`16.66s` total across 12,052 Go files and 1,189,802 Go Production source lines, with 0 unparsable Go files).
  Pure-Go panel repositories ran in `0.01s` to `0.30s`; the two slowest runs (`fatedier/frp` at `9.67s` and `gogs/gogs` at `3.51s`) also measured bundled TypeScript and JavaScript web assets.
- Largest panel repository by Go Production source lines: `go-gitea/gitea` (1,902 Go files, 182,304 Go Production source lines), which completed in `0.51s` wall time with `253,771,776` bytes (`242.0 MiB`) maximum resident set size (`235.6 MiB` peak memory footprint).

### Exclusions

In strict descending star order, the rule excluded these 11 candidates before completing the 20-repository panel:

| Rank | Repository | Stars band | Exclusion reason |
| ---- | ---------- | ---------- | ---------------- |
| 1 | `avelino/awesome-go` | over 100k | Metadata: curated awesome list |
| 2 | `golang/go` | over 100k | Generated (432,319) + Vendored (324,899) = 757,218 exceeds Production (681,549) |
| 3 | `kubernetes/kubernetes` | over 100k | Generated (657,556) + Vendored (1,856,683) = 2,514,239 exceeds Production (641,574) |
| 4 | `microsoft/TypeScript` | over 100k | 0 Go Production source lines (< 5,000) at pre-2023 commit |
| 8 | `gin-gonic/gin` | 50k to 100k | 4,515 Go Production source lines (< 5,000) |
| 10 | `jesseduffield/lazygit` | 50k to 100k | Generated (0) + Vendored (304,310) = 304,310 exceeds Production (36,894) |
| 11 | `netdata/netdata` | 50k to 100k | 0 Go Production source lines (< 5,000) at pre-2023 commit |
| 14 | `moby/moby` | 50k to 100k | Generated (6,700) + Vendored (996,726) = 1,003,426 exceeds Production (134,124) |
| 19 | `FiloSottile/mkcert` | 50k to 100k | 1,151 Go Production source lines (< 5,000) |
| 21 | `wagoodman/dive` | 50k to 100k | 4,864 Go Production source lines (< 5,000) |
| 22 | `jesseduffield/lazydocker` | 50k to 100k | Generated (0) + Vendored (262,663) = 262,663 exceeds Production (7,792) |

`codeint check` exited with code 0 on every candidate.
On excluded candidate `golang/go`, 25 `.go` files were listed as skipped; manual inspection confirmed all 25 are intentional negative syntax-error compiler tests under `test/` and `testdata/`.

### Panel

| Repository | Stars band | Pinned commit | Go files | Production lines |
| ---------- | ---------- | ------------- | -------- | ---------------- |
| `fatedier/frp` | over 100k | `804f2910fd7612729af14e75c3558fe2d3765fa4` | 167 | 17,203 |
| `syncthing/syncthing` | 50k to 100k | `fb4209e382ffa08d53f96569d91f5c4fe172d622` | 520 | 50,924 |
| `gohugoio/hugo` | 50k to 100k | `e754d5cb3e6545ab5c21668a3e775ca5e9084761` | 750 | 63,343 |
| `junegunn/fzf` | 50k to 100k | `ec20dfe312fb31dca2ebf7aa0caa05cd582cf131` | 59 | 11,332 |
| `caddyserver/caddy` | 50k to 100k | `bd74f9449601b6d7dee1cb1db8b960ee1b811de3` | 225 | 31,425 |
| `nektos/act` | 50k to 100k | `e3a722e7c6c697a1b0a38aa314bfb227da4deb5f` | 94 | 9,848 |
| `prometheus/prometheus` | 50k to 100k | `2d61d012ff7c96a12a8d1aba52d3eb19e8d2db46` | 438 | 62,689 |
| `traefik/traefik` | 50k to 100k | `a2016a295332e24101307aca112a37f54689fa3d` | 534 | 32,527 |
| `usememos/memos` | 50k to 100k | `24b21aa9d75d8792d195cc127583c096f3114765` | 57 | 5,317 |
| `rclone/rclone` | 50k to 100k | `98fa93f6d17909e9c6b9778a4dc9e67c58151f6a` | 840 | 130,708 |
| `go-gitea/gitea` | 50k to 100k | `3fef47b41c98392865d13fd21bbcec34236daf4f` | 1,902 | 182,304 |
| `etcd-io/etcd` | 50k to 100k | `4ae4d9fe6c98b617338ce7519253b2d145290349` | 1,001 | 70,192 |
| `ethereum/go-ethereum` | 50k to 100k | `db82ea2ee375b80809673316ec79578c9e883166` | 1,216 | 153,549 |
| `AlistGo/alist` | 50k to 100k | `1c8d895fc09a71c50e05430ae86c2052a3ca6c49` | 340 | 29,637 |
| `hashicorp/terraform` | 50k to 100k | `28d7a685a9e16c8da6b752c520ce0edf9bd5f9b4` | 1,235 | 100,150 |
| `gogs/gogs` | 25k to 50k | `c53b7e9698524da9a2c80ab2c129ca4e623a25e9` | 288 | 35,719 |
| `cli/cli` | 25k to 50k | `6618baaf63cbae2eb4162082496f6171bb9b3fa3` | 518 | 45,463 |
| `v2ray/v2ray-core` | 25k to 50k | `6a747255bc2dddeca7c93b6d27c90cab3007798c` | 643 | 30,798 |
| `milvus-io/milvus` | 25k to 50k | `772e5a4a3b187e3a9cb5670842dcc0ebc8e36032` | 1,140 | 115,366 |
| `juanfont/headscale` | 25k to 50k | `7bb6f1a7eb60fa0e1f8b753137d994b0c14a7372` | 85 | 11,308 |

## Measurements

### Per Metric, over the 20 repositories

| Metric | Mean | Median | Standard deviation | 10th percentile | 90th percentile | Lowest | Highest |
| ------ | ---- | ------ | ------------------ | --------------- | --------------- | ------ | ------- |
| Erosion | 0.523 | 0.501 | 0.133 | 0.379 | 0.692 | 0.291 | 0.862 |
| Verbosity | 0.126 | 0.110 | 0.097 | 0.045 | 0.183 | 0.009 | 0.479 |
| Duplication | 0.125 | 0.110 | 0.097 | 0.044 | 0.181 | 0.009 | 0.479 |
| Import cycles | 0.000 | 0.000 | 0.000 | 0.000 | 0.000 | 0.000 | 0.000 |

### Per repository, in the Production Source set

| Repository | Erosion | Verbosity | Duplication | Import cycles |
| ---------- | ------- | --------- | ----------- | ------------- |
| `fatedier/frp` | 0.291 | 0.181 | 0.178 | 0.000 |
| `syncthing/syncthing` | 0.495 | 0.040 | 0.039 | 0.000 |
| `gohugoio/hugo` | 0.517 | 0.045 | 0.045 | 0.000 |
| `junegunn/fzf` | 0.862 | 0.009 | 0.009 | 0.000 |
| `caddyserver/caddy` | 0.716 | 0.084 | 0.084 | 0.000 |
| `nektos/act` | 0.472 | 0.053 | 0.052 | 0.000 |
| `prometheus/prometheus` | 0.567 | 0.117 | 0.117 | 0.000 |
| `traefik/traefik` | 0.481 | 0.179 | 0.178 | 0.000 |
| `usememos/memos` | 0.690 | 0.479 | 0.479 | 0.000 |
| `rclone/rclone` | 0.507 | 0.101 | 0.101 | 0.000 |
| `go-gitea/gitea` | 0.612 | 0.111 | 0.111 | 0.000 |
| `etcd-io/etcd` | 0.402 | 0.072 | 0.071 | 0.000 |
| `ethereum/go-ethereum` | 0.478 | 0.101 | 0.100 | 0.000 |
| `AlistGo/alist` | 0.429 | 0.120 | 0.116 | 0.000 |
| `hashicorp/terraform` | 0.620 | 0.155 | 0.154 | 0.000 |
| `gogs/gogs` | 0.534 | 0.097 | 0.097 | 0.000 |
| `cli/cli` | 0.591 | 0.114 | 0.113 | 0.000 |
| `v2ray/v2ray-core` | 0.379 | 0.110 | 0.109 | 0.000 |
| `milvus-io/milvus` | 0.372 | 0.204 | 0.204 | 0.000 |
| `juanfont/headscale` | 0.446 | 0.151 | 0.149 | 0.000 |

### Outliers

An outlier lies more than 1.5 interquartile ranges outside the middle half of the panel.
Both outliers stay in the panel, as [ADR 0008](./0008-typescript-reference-ranges.md) requires.

- Erosion, `junegunn/fzf` at `0.862`: core terminal and matcher routines live in a small number of large, branch-heavy functions.
- Duplication and Verbosity, `usememos/memos` at `0.479`: repetitive CRUD store and HTTP handler blocks account for a large share of its 5,317 Production source lines.

### Flagged line hand check and defects fixed

During calibration, we sampled and hand-checked Flagged line matches for all five Go rules against the actual source lines in the panel repositories and audited every non-zero Import cycle and skipped file.
This audit uncovered two real defects in the Go Language plugin, which we fixed with new synthetic tests before re-measuring all 20 panel repositories:

1. **`else-after-return` on `if` / `else if` / `else` chains (`plugins/languages/go/src/rules.rs`)**: the initial pass flagged a trailing `else` after a returning `else if` branch even when an earlier `if` branch in the same chain did not return (for example, `if err == ErrNotFound { } else if err != nil { return err } else { ... }`).
   Dropping `else` in that pattern changes program behavior because the earlier non-returning branch would fall through into the `else` body.
   We updated the rule to track whether every preceding branch in the chain returns and added a synthetic regression test.
   Re-measurement dropped false-positive `else-after-return` lines across the panel (for example, from 25 to 0 in `rclone/rclone`, 28 to 9 in `caddyserver/caddy`, and 69 to 32 in `go-gitea/gitea`).
2. **Self-import exclusion in package resolution (`plugins/languages/go/src/resolution.rs`)**: in `ethereum/go-ethereum`, a build-constrained `package main` generator file (`core/mkalloc.go`) inside the `core/` folder imported `github.com/ethereum/go-ethereum/core`.
   Because package resolution returned every non-test `.go` file in `core/` including the importer itself, it created a spurious single-file self-cycle (`0.0012` Import cycles).
   We updated Go package resolution to exclude the importing file from its own target list and added a synthetic regression test, bringing `ethereum/go-ethereum` and the entire 20-repository panel to `0.000` Import cycles.

Post-fix hand check across the 20 panel repositories:

| Flagged line rule | Total flagged lines across panel | Hand-checked occurrences | False positives post-fix |
| ----------------- | -------------------------------- | ------------------------ | ------------------------ |
| `else-after-return` | 543 | 40 | 0 |
| `if-else-returns-boolean` | 481 | 45 | 0 |
| `conditional-yields-boolean` | 15 | 3 (all) | 0 |
| `trailing-bare-return` | 5 | 5 (all) | 0 |
| `redundant-break` | 25 | 20 | 0 |

## Decision

**The Go Language plugin shows, for each Metric, the 10th to 90th percentile of this panel as its Reference range.**
The source label reads `codeint panel of 20 Go repositories, ADR 0021, codeint 0.6.0`.

| Metric | Reference range |
| ------ | --------------- |
| Erosion | 0.379 to 0.692 |
| Verbosity | 0.045 to 0.183 |
| Duplication | 0.044 to 0.181 |
| Import cycles | 0.000 to 0.000 |

- The 10th to 90th percentile holds the middle 80% of the panel and limits the effect of outliers such as `junegunn/fzf` and `usememos/memos`.
- Go's compiler rejects import cycles between Go packages, so every repository in the panel measured `0.000`; codeint publishes `0.000 to 0.000` as measured so reports state what is typical and flag real cycles introduced across build-constrained files ([ADR 0019](./0019-go-imports-lead-to-packages.md)).

## Considered options

- **Omit the Import cycles Reference range because the measured band is `0.000 to 0.000`.**
  Rejected: Spec #228 requires publishing the measured Import cycles range even when it is `0` to `0`, so Go developers see that zero cycles is the baseline.
- **Remove the two statistical outliers and recompute the band.**
  Rejected: [ADR 0008](./0008-typescript-reference-ranges.md) keeps all qualifying repositories in the panel and relies on the 10th to 90th percentile band to bound outlier influence.

## Consequences

- Go reports show a Reference range next to Erosion, Verbosity, Duplication, and Import cycles with the source label above.
- No code, fixture, or text of a panel repository enters codeint; this record holds only names, commits, and numbers.
- Any future change to default Metric rules, Go Flagged line rules, or Go Source set rules requires re-measuring the panel and amending this record.
- This ADR moves from `proposed` to `accepted` once the Owner approves it.
