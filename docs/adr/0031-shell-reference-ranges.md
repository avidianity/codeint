---
status: superseded by ADR-0035
---

# Shell Reference ranges come from a stratified panel of 50 public repositories

This record establishes calibrated Shell Reference ranges for Ticket #512 (Spec #454) with the stratified current-codebase methodology from [ADR 0028](./0028-stratified-reference-panels.md).

## Context

The `shell` Language plugin shipped without Reference ranges, so reports showed `no published Reference range for this language` for every shell Metric.
A Reference range must come from a committed, repeatable panel, and the plugin's lexer must parse real scripts the way a shell does before its numbers mean anything.
This record therefore covers two steps: an external parser audit of the hand-written lexer, then a 50-repository panel measured with the audited lexer.

### Shell adaptations to ADR 0028

ADR 0028 leaves three choices to each language, and this record fixes them for Shell before measurement:

1. **Round-robin across star bands**: the walk takes one candidate from each star band in turn (`10,000+`, then `1,000-9,999`, then `200-999`), each band in strict descending star order.
   A band-by-band walk would fill every bucket from the `10,000+` band alone, so round-robin keeps all three bands in play.
2. **Mechanical app-versus-library rule**: shell has no package manifests or declared entry points.
   A repository is library-shaped when its Production shell source lines in files without a `#!` first line exceed those in files with one, because shebang-less files are sourced rather than run.
   Every other repository is app-shaped.
3. **No package-topology minimum**: shell has no workspace manifests, so the single-package and multi-package minimums of ADR 0028 do not apply.

### Selection rule and run

- Query on 2026-11-25: `language:Shell fork:false archived:false` per star band, sorted by stars descending.
  The `1,000-9,999` and `200-999` bands were split into star slices so each slice stays under the 1,000-result search cap, then merged and re-sorted by stars.
- Pre-screen: skip a candidate when its GitHub languages-API Shell bytes divided by 12 cannot reach the lower bound of any still-open bucket.
  This skipped 457 candidates without cloning them.
- Exclude a candidate whose name, description, or topics contain `awesome`, `curated`, `tutorial`, `book`, `course`, `template`, `boilerplate`, `starter`, `cheatsheet`, `cheat-sheet`, `cheat sheet`, `interview`, `roadmap`, `list of`, `mirror of`, `learn-`, `learning`, `examples`, or `exercises`.
  The match is a plain substring test, so it also drops a few real tools (such as names containing `notebook` or `playbook`); it is kept mechanical on purpose.
- Shallow-clone the default branch into a temporary scratch folder outside codeint and record the head commit and its committer date.
- Measure in place with `/usr/bin/time -l codeint check --format json --all`, reading the `languages.shell` section on multi-language repositories and the top-level report otherwise.
- Admit a candidate when it has at least 2,000 Production shell source lines, its Generated plus Vendored shell source lines do not exceed Production, `codeint check` exits with code 0, and its bucket is still open.
- Shape phase: when all buckets were full with only 8 library-shaped repositories, the walk continued in the same order.
  Each later library-shaped candidate that qualified for a bucket replaced the most recently admitted app-shaped repository in that bucket, until libraries reached 10.
  `scop/bash-completion` replaced `google/oss-fuzz` (Medium), and `void-linux/void-packages` replaced `huifer/WellAlly-health` (Small).
- Measured with a `codeint 0.6.0` release build of the lexer and review fixes (`4f7ffad`, `3130b77`), default minimum clone size 75 tokens, on Apple Silicon.
- Every percentile interpolates linearly between the two nearest ranked values (`idx = p * (n - 1)`), and standard deviation is the sample standard deviation (`n - 1`).
- Run time across the 50 panel repositories: `0.00s` to `2.05s` per repository (`9.83s` total across 22,202 Production shell files and 2,168,197 Production shell source lines).
- Largest panel repository: `kaldi-asr/kaldi` (3,396 Production shell files, 298,400 Production shell source lines), which completed in `1.36s` with `362,659,840` bytes maximum resident set size, the panel peak.
- codeint skipped 2 shell files across the panel: one with an unterminated single quote that `bash -n` also rejects (`kaldi-asr/kaldi`, `egs/wsj/s5/steps/tandem/mk_aslf_lda_mllt.sh`), and one non-UTF-8 file (`akinomyoga/ble.sh`, `memo/D0589/D0589.test-printf-s2c.sh`).
- Re-measurement note: after the initial calibration build, PR review fixes (`4f7ffad` and `3130b77`) tightened Hazard and Flagged rules, capped lexer nesting, and indexed function spans once.
  Re-measuring all 50 panel repositories at `3130b77` left Erosion, Verbosity, Duplication, Cognitive complexity, and all published Reference ranges unchanged.
  Hazards and Hazard line counts dropped in 18 repositories (mean `0.054` to `0.051`, median `0.025` to `0.023`, 10th to 90th percentile unchanged at `0.004` to `0.133`), `HariSekhon/DevOps-Bash-tools` Flagged lines dropped from 269 to 265 (Verbosity unchanged at `0.0041`), and total run time dropped from `18.74s` to `9.83s`.

The full walk, with every measured candidate, its pinned commit, status, and numbers, is in [`data/0031-shell-reference-ranges.csv`](./data/0031-shell-reference-ranges.csv).

### Exclusions

The walk reached rank 289 in its bands.
Besides the 457 pre-screened candidates, it excluded 183 measured or named candidates:

- 86 by metadata keyword (27 `awesome`, 15 `book`, 13 `tutorial`, 7 `learning`, and 24 across the other words).
- 52 with fewer than 2,000 Production shell source lines.
- 42 because their bucket was already full (26 Small, 16 Medium).
- 2 app-shaped candidates that the shape phase did not need.
- 1 invalid layout: `BlackArch/blackarch` makes `codeint check` exit with code 2 because a tracked path contains a backslash.

### Panel

| Repository | Stars band | Pinned commit | Committer date | Bucket | Shape | Production files | Production lines |
| ---------- | ---------- | ------------- | -------------- | ------ | ----- | ---------------- | ---------------- |
| `ohmyzsh/ohmyzsh` | 10,000+ | `799f9a8a8ae16e2b6a46dd3e9a9bd657bf614825` | `2026-11-24T21:22:08-08:00` | Medium | library | 404 | 26,349 |
| `msitarzewski/agency-agents` | 10,000+ | `8ae6164602c95ca78828b148b9052460ee426885` | `2026-11-08T18:27:04-06:00` | Small | app | 31 | 4,429 |
| `nvm-sh/nvm` | 10,000+ | `3bf66860904e752a128637e423815b899a004155` | `2020-11-21T13:33:12-08:00` | Small | library | 6 | 7,190 |
| `tw93/Mole` | 10,000+ | `ff7aac5abb24a806a00081656e70b4e71e506b04` | `2026-11-25T17:11:05+08:00` | Large | app | 64 | 50,353 |
| `pi-hole/pi-hole` | 10,000+ | `f7c0220e63e753c664a1ec08d42d6b647b86005d` | `2026-11-11T19:35:12+01:00` | Small | app | 19 | 4,867 |
| `romkatv/powerlevel10k` | 10,000+ | `50a35cc8d9f1488d18fb8aea6cddcfc31189cc9f` | `2026-11-19T08:07:49+01:00` | Medium | library | 16 | 14,978 |
| `dockur/windows` | 10,000+ | `013ea27498e0e820a46b3861a0fa047d5b19d846` | `2026-10-05T23:27:34+02:00` | Medium | app | 12 | 12,602 |
| `acmesh-official/acme.sh` | 10,000+ | `5567d7fe7298fd37aca53194885ba444c6678669` | `2026-11-19T11:56:53+08:00` | Large | app | 303 | 52,037 |
| `omacom/omarchy` | 10,000+ | `e27997133d51f36aa00b7c3b8cbbd0d52e7d2a1e` | `2026-11-25T11:53:41+01:00` | Medium | library | 488 | 10,067 |
| `Donchitos/Claude-Code-Game-Studios` | 10,000+ | `abc0de79d6ceb0b5d7aa693945849b3937afbe6a` | `2026-11-09T23:39:37+11:00` | Medium | app | 68 | 14,919 |
| `community-scripts/ProxmoxVE` | 10,000+ | `dd694c4dfe171cdd43f07537d9aa40e229e445fd` | `2026-11-25T10:18:05Z` | Large | app | 1,516 | 109,705 |
| `233boy/v2ray` | 10,000+ | `707ecf7601ff49f91c2d12dd22b98e8f89588d1c` | `2026-01-15T08:49:53+08:00` | Small | app | 12 | 2,757 |
| `hwdsl2/setup-ipsec-vpn` | 10,000+ | `3a5a4548b027bf3fb8897f0f749a3c60f3da4bc7` | `2026-11-20T22:31:13-06:00` | Small | app | 18 | 6,430 |
| `OthmanAdi/planning-with-files` | 10,000+ | `189fd09fecb246a4064f2cd0635153b5297f5369` | `2026-11-25T08:04:43+01:00` | Large | app | 198 | 56,626 |
| `mbadolato/iTerm2-Color-Schemes` | 10,000+ | `72a504c56196a4bf14cfa8ff5718128dcd62b646` | `2026-11-23T08:22:17-07:00` | Large | app | 3,657 | 83,337 |
| `yuaotian/go-cursor-help` | 10,000+ | `2d00ebc01eb977b2575589d19f4201de9757368e` | `2026-09-23T23:53:20+08:00` | Small | app | 3 | 4,186 |
| `zsh-users/zsh-syntax-highlighting` | 10,000+ | `e6683f20e5c38d3e5447f45845d04ddda75d0f23` | `2026-10-09T17:16:49Z` | Small | library | 296 | 4,313 |
| `mack-a/v2ray-agent` | 10,000+ | `b5d76b921f825891155a1272181df8f380bfe907` | `2026-11-23T16:14:28+08:00` | Medium | app | 8 | 26,985 |
| `termux/termux-packages` | 10,000+ | `0e0d1929ab4047ce7407d881fa1b6ff421024640` | `2026-11-25T10:38:37Z` | Large | library | 3,997 | 104,481 |
| `kaldi-asr/kaldi` | 10,000+ | `e02e35f0254bb033fab73d1df99fc34123e31d56` | `2025-09-22T09:05:59+02:00` | Large | app | 3,396 | 298,400 |
| `Bash-it/bash-it` | 10,000+ | `0191fca5df3ee8528786af1cd70eef5f900b4410` | `2026-11-22T09:41:47+02:00` | Medium | library | 333 | 16,534 |
| `bin456789/reinstall` | 10,000+ | `d9ede3db41ddbf0a026a7109f2d1a469375eaaad` | `2026-11-23T23:27:27+08:00` | Medium | app | 11 | 11,253 |
| `hq450/fancyss` | 10,000+ | `b19e82cb6a05e08990a96905353921819d44d134` | `2026-06-02T00:00:28+08:00` | Medium | app | 62 | 40,218 |
| `juewuy/ShellCrash` | 10,000+ | `34b768b5a5df6b6ed7fe8793ed0e545a499f5a5d` | `2026-11-18T16:30:46+08:00` | Medium | app | 88 | 12,514 |
| `Gogh-Co/Gogh` | 10,000+ | `fd715c810bb0ad35f630bd73fb71f5b5a1311bd6` | `2026-11-14T17:46:43Z` | Large | app | 1,261 | 52,137 |
| `telekom-security/tpotce` | 1,000-9,999 | `9c3ae4aea976a63ef1f655598a4383c0cb7e140c` | `2026-11-19T12:47:16+01:00` | Small | app | 13 | 3,001 |
| `HyDE-Project/HyDE` | 1,000-9,999 | `cc824554ac63417d58253ea6764dc0045d963dce` | `2026-11-12T09:12:58+08:00` | Medium | app | 126 | 12,372 |
| `frankbria/ralph-claude-code` | 1,000-9,999 | `11f9df5becc8093ff2265542107e8e53a42e9970` | `2026-11-03T22:44:54-07:00` | Medium | app | 29 | 10,158 |
| `getsentry/self-hosted` | 1,000-9,999 | `1baa2d65cd1785251f7beff1bf5253b64306ce6c` | `2026-11-19T14:24:12+07:00` | Small | library | 60 | 2,294 |
| `ublue-os/bazzite` | 1,000-9,999 | `b42cdec64f91b8218ff710724099cce6245805e0` | `2026-11-24T19:31:30-08:00` | Small | app | 58 | 2,461 |
| `testssl/testssl.sh` | 1,000-9,999 | `0f87aca237ae52a35e69a14157c37377a1779908` | `2026-11-20T20:24:40+01:00` | Medium | app | 19 | 23,690 |
| `kunchenguid/firstmate` | 1,000-9,999 | `2f000eb4301c2d733bef163621e9ac3d740f21cf` | `2026-11-24T16:28:03-08:00` | Large | app | 251 | 102,556 |
| `RROrg/rr` | 1,000-9,999 | `ccf322ff697fa08eee8cd56f8fd623aad0dece6a` | `2026-11-24T13:12:20+08:00` | Small | app | 18 | 6,394 |
| `xiaoyaDev/xiaoya-alist` | 1,000-9,999 | `dae54c1b2bcd3506c50a2e32f74dc3bac75197bb` | `2026-11-25T07:40:46Z` | Small | app | 14 | 9,319 |
| `prasanthrangan/hyprdots` | 1,000-9,999 | `3c8b0dfb5e7f8e41a67b80463513f10d57cab1a4` | `2025-03-23T13:31:38+08:00` | Small | app | 59 | 3,832 |
| `HariSekhon/DevOps-Bash-tools` | 1,000-9,999 | `0ab20dad68521357169714cfe8e4ed7d4f048921` | `2026-11-19T01:30:59+08:00` | Large | app | 1,560 | 65,117 |
| `six2dez/reconftw` | 1,000-9,999 | `39252113a854bdac0b785f144b9fae1fa8ca35fc` | `2026-10-29T18:13:47+01:00` | Medium | app | 16 | 14,298 |
| `v1s1t0r1sh3r3/airgeddon` | 1,000-9,999 | `40c6beb3082aaf3144e14185b79f8e7e3ec00f78` | `2026-11-12T17:31:25+01:00` | Medium | app | 4 | 30,345 |
| `SuperManito/LinuxMirrors` | 1,000-9,999 | `7f8989fd237314722e64a23dc4e873f48bafbb9b` | `2026-11-19T23:58:06+08:00` | Medium | app | 10 | 20,653 |
| `ohmybash/oh-my-bash` | 1,000-9,999 | `94c2977eda06f230aafa9628fcd869123f01896b` | `2026-10-28T18:22:47+09:00` | Medium | app | 232 | 22,736 |
| `akinomyoga/ble.sh` | 1,000-9,999 | `152942758f567e51fd17bdc7b3783d27b3c476aa` | `2026-10-31T17:18:03+09:00` | Large | app | 146 | 63,530 |
| `nyldn/claude-octopus` | 1,000-9,999 | `c3a279b33930deb11403d8d26f65abc8c32b44c5` | `2026-11-25T06:32:18-05:00` | Large | app | 255 | 76,323 |
| `bitnami/containers` | 1,000-9,999 | `bf8167f212ffe230e49daa3ce03e7c803f7f0937` | `2026-11-25T11:58:36+01:00` | Large | app | 2,115 | 230,403 |
| `kejilion/sh` | 1,000-9,999 | `cd10a6be821a51ca9912efb691b6165301c14708` | `2026-11-24T13:26:03+08:00` | Large | app | 32 | 217,024 |
| `scop/bash-completion` | 1,000-9,999 | `ddf4f5e44818e00c51e984c019c15b6d73a2e79b` | `2026-11-22T11:28:44+02:00` | Medium | library | 543 | 25,796 |
| `void-linux/void-packages` | 1,000-9,999 | `89918ee0a8461cf29f4f2878e67ae50cc8cc4c0b` | `2026-11-25T03:54:55+01:00` | Small | library | 265 | 9,323 |
| `deajan/osync` | 200-999 | `c3f77008b1acf19de72fba92eddce068d5b5437b` | `2026-10-28T16:15:43+01:00` | Medium | app | 14 | 18,782 |
| `Danilop95/Proxmox-Enhanced-Configuration-Utility` | 200-999 | `d00fd0d1abaeeba5e6abd1c0b42263270da3005e` | `2026-05-12T01:04:44+02:00` | Small | app | 5 | 5,681 |
| `woniuzfb/aios` | 200-999 | `a813919b5a59e9ab1d2cdfbc3da91ddbaa7974fb` | `2026-11-25T16:40:19+08:00` | Large | app | 5 | 129,386 |
| `shibco/ableton-linux` | 200-999 | `1b1f596d17bb86675117f2ec315acd21b143e88c` | `2026-10-25T10:52:02+01:00` | Medium | app | 86 | 35,056 |

## Lexer audit against an external parser

The audit compared the plugin's functions and decision points with the `shfmt` 3.14.1 syntax tree (`shfmt -ln bash` or `-ln zsh --to-json`) on every Production shell file of the 50 panel repositories.
Decision points from the tree are `if`, each `elif`, `for` and `select`, `while` and `until`, each `case` item except a lone `*`, and every `&&` and `||` in command lists, `[[ ]]` tests, and arithmetic commands.
Recall is the matched count over the tree's count per function, and the false-positive rate is codeint's surplus over its own count.
An unexpected unparsable file is one codeint rejects while `bash -n` (or `zsh -n` for Zsh files) accepts it.

The audit file set held 23,767 shell files, which includes files outside the 22,202 Production files of the panel.
These lexer audit numbers and the 100-line Flagged and Hazard samples below come from the earlier calibration build before the review fixes (`4f7ffad`, `3130b77`); the re-measurement confirmed identical Production line counts, Erosion, and Cognitive complexity across all 50 repositories, showing function and decision-point extraction did not change.
Before the fixes below, on those 23,767 files: function recall `0.9979` (81,811 of 81,987), decision-point recall `0.9946` (431,368 of 433,725), decision-point false positives `0.0041`, and 5 unexpected unparsable files.
After the fixes: function recall `0.9983` (82,485 of 82,626), decision-point recall `0.9980` (440,317 of 441,207), decision-point false positives `0.0014`, function false positives `0`, and 0 unexpected unparsable files.

A seeded sample of 100 Flagged lines and 100 Hazard lines from the panel's Production files had no line that contradicts its rule's documented match.
The `shfmt` tree confirmed 91 of the 100 Flagged lines, and the other 9 sit in files `shfmt` cannot parse and were checked by hand.
Ten of the 11 sampled `posix-sh-bashism` lines and 3 of the 7 sampled `missing-strict-mode` lines sit in shebang-less `.sh` files that their projects source from Bash.
They match the documented rule, because a `.sh` file without a shebang is POSIX `sh`, but a reader of those projects may see them as noise.

### Defects fixed during calibration

Each fix has an original synthetic regression test in `plugins/languages/shell/tests/oracle_regressions.rs`.

- The command after a `case` pattern's `)` did not start a command, so `if` and loops in case bodies were not counted and functions defined there were missed.
- Words inside an array assignment such as `list=(if then)` were read as reserved words.
- `for ((...))` was read as two subshells or a here-document.
- A bare `{` inside `${...}` nested the expansion and swallowed the rest of the file.
- In Zsh, a `}` right after a command ended the function body only after a separator.
- In Zsh, a `'` inside a double-quoted `${...}` is literal; Bash keeps its quoting meaning.
- Parentheses of `case` patterns closed a function body written as a subshell.
- A `}` right after `fi`, `done`, `esac`, or `}` did not end the enclosing function body.
- `$((` that is not arithmetic, such as `$((cmd) | other)`, is a command substitution.

### Known lexer limitations

- Functions defined inside `$(...)` command substitutions are not extracted.
- Backtick contents are not tokenized, so `&&` and `||` inside backticks are not decision points.
- The Zsh forms `{cmd` without a space and `name(){}` on one token are not recognized.

These cover the remaining 141 missed functions and 890 missed decision points out of 82,626 and 441,207.

## Measurements

### Per Metric, over the 50 repositories

| Metric | Mean | Median | Standard deviation | 10th percentile | 90th percentile | Lowest | Highest |
| ------ | ---- | ------ | ------------------ | --------------- | --------------- | ------ | ------- |
| Erosion | 0.695 | 0.768 | 0.222 | 0.416 | 0.927 | 0.002 | 0.956 |
| Verbosity | 0.008 | 0.004 | 0.011 | 0.000 | 0.025 | 0.000 | 0.044 |
| Duplication | 0.187 | 0.051 | 0.290 | 0.008 | 0.751 | 0.000 | 0.940 |

Cognitive complexity (10th to 90th percentile `0.277` to `0.807`) and Hazards (`0.004` to `0.133`) were measured but are not published, as for every other built-in language.
Import cycles is absent for shell because shell scripts have no imports.

### Per repository, in the Production Source set

| Repository | Erosion | Verbosity | Duplication |
| ---------- | ------- | --------- | ----------- |
| `ohmyzsh/ohmyzsh` | 0.588 | 0.004 | 0.052 |
| `msitarzewski/agency-agents` | 0.761 | 0.001 | 0.029 |
| `nvm-sh/nvm` | 0.932 | 0.001 | 0.007 |
| `tw93/Mole` | 0.892 | 0.004 | 0.028 |
| `pi-hole/pi-hole` | 0.559 | 0.013 | 0.009 |
| `romkatv/powerlevel10k` | 0.870 | 0.001 | 0.070 |
| `dockur/windows` | 0.846 | 0.003 | 0.023 |
| `acmesh-official/acme.sh` | 0.531 | 0.025 | 0.061 |
| `omacom/omarchy` | 0.413 | 0.002 | 0.010 |
| `Donchitos/Claude-Code-Game-Studios` | 0.956 | 0.000 | 0.034 |
| `community-scripts/ProxmoxVE` | 0.452 | 0.000 | 0.092 |
| `233boy/v2ray` | 0.926 | 0.005 | 0.000 |
| `hwdsl2/setup-ipsec-vpn` | 0.531 | 0.001 | 0.373 |
| `OthmanAdi/planning-with-files` | 0.887 | 0.001 | 0.712 |
| `mbadolato/iTerm2-Color-Schemes` | 0.002 | 0.000 | 0.613 |
| `yuaotian/go-cursor-help` | 0.786 | 0.016 | 0.058 |
| `zsh-users/zsh-syntax-highlighting` | 0.795 | 0.003 | 0.000 |
| `mack-a/v2ray-agent` | 0.801 | 0.002 | 0.749 |
| `termux/termux-packages` | 0.215 | 0.001 | 0.058 |
| `kaldi-asr/kaldi` | 0.934 | 0.026 | 0.599 |
| `Bash-it/bash-it` | 0.444 | 0.002 | 0.016 |
| `bin456789/reinstall` | 0.775 | 0.004 | 0.009 |
| `hq450/fancyss` | 0.660 | 0.006 | 0.057 |
| `juewuy/ShellCrash` | 0.796 | 0.021 | 0.083 |
| `Gogh-Co/Gogh` | 0.105 | 0.000 | 0.940 |
| `telekom-security/tpotce` | 0.723 | 0.016 | 0.011 |
| `HyDE-Project/HyDE` | 0.711 | 0.007 | 0.039 |
| `frankbria/ralph-claude-code` | 0.776 | 0.013 | 0.011 |
| `getsentry/self-hosted` | 0.486 | 0.003 | 0.032 |
| `ublue-os/bazzite` | 0.512 | 0.000 | 0.000 |
| `testssl/testssl.sh` | 0.948 | 0.019 | 0.051 |
| `kunchenguid/firstmate` | 0.861 | 0.001 | 0.025 |
| `RROrg/rr` | 0.866 | 0.041 | 0.138 |
| `xiaoyaDev/xiaoya-alist` | 0.688 | 0.027 | 0.183 |
| `prasanthrangan/hyprdots` | 0.741 | 0.025 | 0.025 |
| `HariSekhon/DevOps-Bash-tools` | 0.346 | 0.004 | 0.068 |
| `six2dez/reconftw` | 0.869 | 0.011 | 0.073 |
| `v1s1t0r1sh3r3/airgeddon` | 0.751 | 0.008 | 0.046 |
| `SuperManito/LinuxMirrors` | 0.816 | 0.004 | 0.484 |
| `ohmybash/oh-my-bash` | 0.601 | 0.004 | 0.068 |
| `akinomyoga/ble.sh` | 0.630 | 0.010 | 0.008 |
| `nyldn/claude-octopus` | 0.857 | 0.002 | 0.023 |
| `bitnami/containers` | 0.416 | 0.014 | 0.832 |
| `kejilion/sh` | 0.828 | 0.002 | 0.812 |
| `scop/bash-completion` | 0.707 | 0.001 | 0.033 |
| `void-linux/void-packages` | 0.651 | 0.002 | 0.018 |
| `deajan/osync` | 0.792 | 0.044 | 0.765 |
| `Danilop95/Proxmox-Enhanced-Configuration-Utility` | 0.841 | 0.015 | 0.056 |
| `woniuzfb/aios` | 0.914 | 0.007 | 0.874 |
| `shibco/ableton-linux` | 0.943 | 0.000 | 0.014 |

### Distribution and outliers

- Erosion: `mbadolato/iTerm2-Color-Schemes` and `Gogh-Co/Gogh` lie below 1.5 interquartile ranges, because thousands of small theme scripts hold almost no complex functions.
- Verbosity: `deajan/osync` and `RROrg/rr` lie above, mostly from `else-after-return` and `[ $? -ne 0 ]` checks.
- Duplication: 11 repositories lie above, mostly projects that ship near-identical copies of the same script per version, image, or theme (`bitnami/containers`, `Gogh-Co/Gogh`, `woniuzfb/aios`).
- ADR 0028 keeps every admitted repository, so no outlier was removed.
- Verbosity counts only Flagged lines and shell has 4 Flagged line rules, so shell Verbosity stays low and its range is narrow.

## Decision

**The Shell Language plugin shows, for Erosion, Verbosity, and Duplication, the 10th to 90th percentile of this panel as its Reference range.**
The source label reads `codeint panel of 50 Shell repositories, ADR 0031, codeint 0.6.0`.

| Metric | Metric version | Reference range |
| ------ | -------------- | --------------- |
| Erosion | 1 | 0.416 to 0.927 |
| Verbosity | 2 | 0.000 to 0.025 |
| Duplication | 1 | 0.008 to 0.751 |

- Shell publishes no Import cycles Reference range because shell scripts have no imports.

## Considered options

- **Walk each star band to the end before the next.**
  Rejected: the `10,000+` band alone would fill every bucket, and the panel would not represent smaller projects.
- **Classify shape from file extensions or directory names.**
  Rejected: shell projects use `.sh` for both sourced libraries and run scripts, so only the shebang separates them mechanically.
- **Calibrate with the lexer as first shipped.**
  Rejected: 5 panel files that `bash -n` or `zsh -n` accepts were unparsable, including the main script of `testssl/testssl.sh`, so their functions were missing from the measurement.

## Consequences

- Shell reports show a Reference range next to Erosion, Verbosity, and Duplication with the source label above, and leave Import cycles absent.
- No code, fixture, or text of a panel repository enters codeint; this record and its data file hold only names, commits, and numbers.
- Any future change to shell function or decision-point rules, shell Flagged line rules, or shell Source set rules requires re-measuring the panel and amending this record.
