# codeint Evaluation

Result: FAIL.
Corpus measured on 2026-11-25.
Binary: `codeint 0.6.0`.

## Machine profile

- Operating system: macos on aarch64
- CPU: Apple M5
- Logical CPUs: 10
- Memory: 32.0 GiB
- Runs per repository: 3

## Reference range panels

Each Reference range is the 10th to 90th percentile (linear interpolation) of the Production Source set ratios across the panel.
The standard deviation is the population standard deviation.

### dart

50 repositories: 15 Small, 20 Medium, 15 Large.

| Metric | Version | Mean | Median | Standard deviation | 10th percentile | 90th percentile | Lowest | Highest |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Erosion | 1 | 0.243 | 0.237 | 0.189 | 0.023 | 0.479 | 0.000 | 0.824 |
| Verbosity | 2 | 0.004 | 0.003 | 0.006 | 0.000 | 0.008 | 0.000 | 0.032 |
| Duplication | 1 | 0.156 | 0.124 | 0.132 | 0.024 | 0.262 | 0.000 | 0.692 |
| Import cycles | 1 | 0.259 | 0.260 | 0.201 | 0.013 | 0.485 | 0.000 | 0.780 |

### go

50 repositories: 15 Small, 20 Medium, 15 Large.

| Metric | Version | Mean | Median | Standard deviation | 10th percentile | 90th percentile | Lowest | Highest |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Erosion | 1 | 0.518 | 0.527 | 0.158 | 0.325 | 0.674 | 0.028 | 0.924 |
| Verbosity | 2 | 0.001 | 0.001 | 0.002 | 0.000 | 0.002 | 0.000 | 0.012 |
| Duplication | 1 | 0.102 | 0.068 | 0.113 | 0.025 | 0.181 | 0.008 | 0.767 |
| Import cycles | 1 | 0.000 | 0.000 | 0.000 | 0.000 | 0.000 | 0.000 | 0.000 |

### php

50 repositories: 15 Small, 20 Medium, 15 Large.

| Metric | Version | Mean | Median | Standard deviation | 10th percentile | 90th percentile | Lowest | Highest |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Erosion | 1 | 0.374 | 0.394 | 0.213 | 0.084 | 0.630 | 0.000 | 0.924 |
| Verbosity | 2 | 0.001 | 0.000 | 0.002 | 0.000 | 0.005 | 0.000 | 0.009 |
| Duplication | 1 | 0.093 | 0.072 | 0.068 | 0.017 | 0.189 | 0.000 | 0.266 |
| Import cycles | 1 | 0.072 | 0.040 | 0.090 | 0.000 | 0.145 | 0.000 | 0.451 |

### python

50 repositories: 15 Small, 20 Medium, 15 Large.

| Metric | Version | Mean | Median | Standard deviation | 10th percentile | 90th percentile | Lowest | Highest |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Erosion | 1 | 0.516 | 0.530 | 0.168 | 0.331 | 0.734 | 0.012 | 0.901 |
| Verbosity | 2 | 0.004 | 0.002 | 0.005 | 0.000 | 0.009 | 0.000 | 0.023 |
| Duplication | 1 | 0.090 | 0.058 | 0.125 | 0.008 | 0.172 | 0.001 | 0.740 |
| Import cycles | 1 | 0.138 | 0.055 | 0.183 | 0.000 | 0.437 | 0.000 | 0.724 |

### rust

50 repositories: 15 Small, 20 Medium, 15 Large.

| Metric | Version | Mean | Median | Standard deviation | 10th percentile | 90th percentile | Lowest | Highest |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Erosion | 1 | 0.496 | 0.542 | 0.188 | 0.212 | 0.716 | 0.049 | 0.820 |
| Verbosity | 2 | 0.002 | 0.001 | 0.003 | 0.000 | 0.005 | 0.000 | 0.021 |
| Duplication | 1 | 0.151 | 0.112 | 0.111 | 0.041 | 0.314 | 0.000 | 0.541 |
| Import cycles | 1 | 0.244 | 0.167 | 0.238 | 0.000 | 0.574 | 0.000 | 0.909 |

### sql

50 repositories: 15 Small, 20 Medium, 15 Large.

| Metric | Version | Mean | Median | Standard deviation | 10th percentile | 90th percentile | Lowest | Highest |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Erosion | 1 | 0.309 | 0.197 | 0.330 | 0.000 | 0.795 | 0.000 | 0.999 |
| Verbosity | 2 | 0.000 | 0.000 | 0.000 | 0.000 | 0.000 | 0.000 | 0.002 |
| Duplication | 1 | 0.315 | 0.142 | 0.330 | 0.001 | 0.909 | 0.000 | 0.960 |

### typescript-javascript

50 repositories: 15 Small, 20 Medium, 15 Large.

| Metric | Version | Mean | Median | Standard deviation | 10th percentile | 90th percentile | Lowest | Highest |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Erosion | 1 | 0.495 | 0.520 | 0.183 | 0.206 | 0.731 | 0.061 | 0.829 |
| Verbosity | 2 | 0.000 | 0.000 | 0.001 | 0.000 | 0.001 | 0.000 | 0.003 |
| Duplication | 1 | 0.094 | 0.054 | 0.129 | 0.006 | 0.194 | 0.000 | 0.703 |
| Import cycles | 1 | 0.070 | 0.017 | 0.127 | 0.000 | 0.184 | 0.000 | 0.588 |

### typescript-javascript (react Framework profile)

20 repositories: 6 Small, 8 Medium, 6 Large.

| Metric | Version | Mean | Median | Standard deviation | 10th percentile | 90th percentile | Lowest | Highest |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Erosion | 1 | 0.413 | 0.397 | 0.135 | 0.279 | 0.551 | 0.253 | 0.806 |
| Verbosity | 2 | 0.001 | 0.000 | 0.001 | 0.000 | 0.001 | 0.000 | 0.004 |
| Duplication | 1 | 0.198 | 0.112 | 0.218 | 0.029 | 0.456 | 0.018 | 0.826 |
| Import cycles | 1 | 0.047 | 0.015 | 0.092 | 0.000 | 0.083 | 0.000 | 0.422 |

### typescript-javascript (vue2 Framework profile)

20 repositories: 6 Small, 8 Medium, 6 Large.

| Metric | Version | Mean | Median | Standard deviation | 10th percentile | 90th percentile | Lowest | Highest |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Erosion | 1 | 0.277 | 0.297 | 0.184 | 0.099 | 0.576 | 0.000 | 0.692 |
| Verbosity | 2 | 0.001 | 0.001 | 0.001 | 0.000 | 0.003 | 0.000 | 0.005 |
| Duplication | 1 | 0.132 | 0.103 | 0.140 | 0.015 | 0.287 | 0.004 | 0.608 |
| Import cycles | 1 | 0.037 | 0.009 | 0.055 | 0.000 | 0.115 | 0.000 | 0.201 |

### typescript-javascript (vue3 Framework profile)

20 repositories: 6 Small, 8 Medium, 6 Large.

| Metric | Version | Mean | Median | Standard deviation | 10th percentile | 90th percentile | Lowest | Highest |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Erosion | 1 | 0.290 | 0.284 | 0.140 | 0.101 | 0.452 | 0.000 | 0.562 |
| Verbosity | 2 | 0.000 | 0.000 | 0.000 | 0.000 | 0.001 | 0.000 | 0.001 |
| Duplication | 1 | 0.092 | 0.063 | 0.088 | 0.012 | 0.194 | 0.000 | 0.315 |
| Import cycles | 1 | 0.024 | 0.005 | 0.037 | 0.000 | 0.074 | 0.000 | 0.141 |

## Native-parser recall

Each bar is 98% recall on functions, decision points, and static imports, with no unexpected unparsable files.
A file that the oracle parses and codeint did not measure counts as a miss on every function, decision point, and import in it.

| Language | Oracle | Functions | Decision points | Static imports | Oracle-unparsable files | Unexpected unparsable files | Unmeasured files | Result |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| dart | measured | 99.90% (683923/684583) | 99.31% (503333/506782) | 99.97% (201017/201062) | 65 | 0 | 5 | FAIL |
| go | measured | 99.98% (370149/370217) | 99.98% (1160847/1160999) | 99.98% (311314/311376) | 1 | 0 | 11 | PASS |
| php | measured | 99.73% (438110/439293) | 99.22% (373840/376774) | 99.99% (300174/300182) | 17 | 0 | 3 | FAIL |
| python | measured | 99.99% (923215/923221) | 99.83% (1394012/1396362) | 100.00% (479681/479681) | 0 | 0 | 1 | PASS |
| rust | no oracle | - | - | - | - | - | - | PASS |
| sql | no oracle | - | - | - | - | - | - | PASS |
| typescript-javascript | no oracle | - | - | - | - | - | - | PASS |

Files the dart oracle parses and codeint did not measure count as recall misses:

- `flutter/flutter engine/src/flutter/lib/web_ui/test/ui/paragraph_performance_test.dart`
- `OpenFlutter/fluwx packages/fluwx/example/lib/main.dart`
- `OpenFlutter/fluwx packages/fluwx/example/lib/utils.dart`
- `OpenFlutter/fluwx packages/fluwx_no_pay/example/lib/main.dart`
- `OpenFlutter/fluwx packages/fluwx_no_pay/example/lib/utils.dart`

Files the go oracle parses and codeint did not measure count as recall misses:

- `etcd-io/etcd client/v3/concurrency/example_election_test.go`
- `etcd-io/etcd client/v3/concurrency/example_mutex_test.go`
- `etcd-io/etcd client/v3/concurrency/example_stm_test.go`
- `etcd-io/etcd client/v3/example_auth_test.go`
- `etcd-io/etcd client/v3/example_cluster_test.go`
- `etcd-io/etcd client/v3/example_kv_test.go`
- `etcd-io/etcd client/v3/example_lease_test.go`
- `etcd-io/etcd client/v3/example_maintenance_test.go`
- `etcd-io/etcd client/v3/example_metrics_test.go`
- `etcd-io/etcd client/v3/example_test.go`
- `etcd-io/etcd client/v3/example_watch_test.go`

Files the php oracle parses and codeint did not measure count as recall misses:

- `symfony/symfony src/Symfony/Component/Cache/Traits/ValueWrapper.php`
- `symfony/symfony src/Symfony/Component/DependencyInjection/Tests/Fixtures/php/services8.php`
- `bcit-ci/CodeIgniter tests/codeigniter/core/Utf8_test.php`

Files the python oracle parses and codeint did not measure count as recall misses:

- `langchain-ai/langchain libs/langchain/tests/integration_tests/examples/non-utf8-encoding.py`

## Speed and memory

| Repository | Language | Declared bucket | Measured bucket | Production lines | Median wall time | Peak RSS | Production lines per second |
| --- | --- | --- | --- | --- | --- | --- | --- |
| morluto/rea | typescript-javascript | large | large | 844237 | 1.574 s | 1194.7 MiB | 536449 |
| affaan-m/ECC | typescript-javascript | large | large | 109036 | 0.576 s | 322.8 MiB | 189429 |
| deepseek-ai/deepseek-harness | typescript-javascript | large | large | 499150 | 2.325 s | 1433.0 MiB | 214667 |
| react/react | typescript-javascript | large | large | 137609 | 0.702 s | 303.6 MiB | 196151 |
| anomalyco/opencode | typescript-javascript | large | large | 855206 | 4.090 s | 1175.2 MiB | 209118 |
| vuejs/vue | typescript-javascript | medium | medium | 20865 | 0.036 s | 38.4 MiB | 572274 |
| n8n-io/n8n | typescript-javascript | large | large | 2022590 | 9.893 s | 2544.3 MiB | 204444 |
| firecrawl/firecrawl | typescript-javascript | large | large | 162753 | 0.451 s | 207.8 MiB | 361065 |
| DietrichGebert/ponytail | typescript-javascript | small | small | 3160 | 0.014 s | 21.5 MiB | 218430 |
| microsoft/vscode | typescript-javascript | large | large | 2339443 | 9.579 s | 3123.4 MiB | 244233 |
| clash-verge-rev/clash-verge-rev | typescript-javascript | medium | medium | 45752 | 0.094 s | 79.4 MiB | 489286 |
| anthropics/claude-code | typescript-javascript | medium | medium | 24633 | 0.112 s | 50.8 MiB | 219491 |
| iptv-org/iptv | typescript-javascript | small | small | 3762 | 0.013 s | 17.8 MiB | 300165 |
| vercel/next.js | typescript-javascript | large | large | 1138956 | 30.323 s | 1916.5 MiB | 37560 |
| garrytan/gstack | typescript-javascript | large | large | 176188 | 0.846 s | 625.4 MiB | 208349 |
| stablyai/orca | typescript-javascript | large | large | 3404433 | 5.753 s | 2094.7 MiB | 591747 |
| excalidraw/excalidraw | typescript-javascript | large | large | 129239 | 0.119 s | 88.6 MiB | 1083526 |
| shadcn-ui/ui | typescript-javascript | large | large | 415871 | 2.036 s | 395.4 MiB | 204240 |
| nodejs/node | typescript-javascript | large | large | 354099 | 5.438 s | 1482.0 MiB | 65112 |
| earendil-works/pi | typescript-javascript | large | large | 319865 | 1.151 s | 523.7 MiB | 277865 |
| TauricResearch/TradingAgents | typescript-javascript | medium | medium | 27453 | 0.052 s | 48.2 MiB | 527774 |
| Leonxlnx/taste-skill | typescript-javascript | medium | medium | 37977 | 0.159 s | 77.4 MiB | 238879 |
| addyosmani/agent-skills | typescript-javascript | small | small | 5321 | 0.021 s | 21.3 MiB | 256970 |
| axios/axios | typescript-javascript | small | small | 8576 | 0.044 s | 37.3 MiB | 196628 |
| OpenCut-app/OpenCut | typescript-javascript | small | small | 6539 | 0.017 s | 20.2 MiB | 392051 |
| tailwindlabs/tailwindcss | typescript-javascript | medium | medium | 32286 | 0.135 s | 83.7 MiB | 238810 |
| puppeteer/puppeteer | typescript-javascript | medium | medium | 45794 | 0.151 s | 76.9 MiB | 304261 |
| louislam/uptime-kuma | typescript-javascript | medium | medium | 39251 | 0.081 s | 59.5 MiB | 484242 |
| Egonex-AI/Understand-Anything | typescript-javascript | medium | medium | 33734 | 0.128 s | 67.5 MiB | 263111 |
| modelcontextprotocol/servers | typescript-javascript | medium | medium | 18539 | 0.100 s | 58.6 MiB | 185740 |
| ChatGPTNextWeb/NextChat | typescript-javascript | medium | medium | 37605 | 0.064 s | 46.9 MiB | 591208 |
| coder/code-server | typescript-javascript | small | small | 3704 | 0.023 s | 22.2 MiB | 159694 |
| anuraghazra/github-readme-stats | typescript-javascript | small | small | 6513 | 0.027 s | 21.2 MiB | 239985 |
| Fission-AI/OpenSpec | typescript-javascript | medium | medium | 36481 | 0.191 s | 107.2 MiB | 190962 |
| Eugeny/tabby | typescript-javascript | medium | medium | 26691 | 0.111 s | 42.7 MiB | 240644 |
| expressjs/express | typescript-javascript | small | small | 2369 | 0.029 s | 24.7 MiB | 81070 |
| chartjs/Chart.js | typescript-javascript | medium | medium | 15620 | 0.100 s | 59.0 MiB | 155729 |
| upstash/context7 | typescript-javascript | medium | medium | 17714 | 0.039 s | 29.8 MiB | 453766 |
| jgraph/drawio-desktop | typescript-javascript | small | small | 4710 | 0.014 s | 17.2 MiB | 325811 |
| socketio/socket.io | typescript-javascript | medium | medium | 23574 | 0.364 s | 83.0 MiB | 64731 |
| coreyhaines31/marketingskills | typescript-javascript | medium | medium | 21219 | 0.105 s | 46.4 MiB | 202412 |
| reduxjs/redux | typescript-javascript | small | small | 5010 | 0.018 s | 17.4 MiB | 276694 |
| jquery/jquery | typescript-javascript | small | small | 7203 | 0.048 s | 51.4 MiB | 149602 |
| pmndrs/zustand | typescript-javascript | small | small | 2316 | 0.020 s | 20.5 MiB | 114038 |
| scutan90/DeepLearning-500-questions | typescript-javascript | small | small | 3843 | 0.017 s | 14.0 MiB | 220875 |
| lyswhut/lx-music-desktop | typescript-javascript | medium | medium | 48661 | 0.081 s | 37.2 MiB | 598591 |
| upscayl/upscayl | typescript-javascript | small | small | 6863 | 0.024 s | 17.4 MiB | 284794 |
| prettier/prettier | typescript-javascript | medium | medium | 47273 | 1.243 s | 235.0 MiB | 38034 |
| KeygraphHQ/shannon | typescript-javascript | medium | medium | 40912 | 0.067 s | 45.4 MiB | 614842 |
| algorithm-visualizer/algorithm-visualizer | typescript-javascript | small | small | 2681 | 0.015 s | 14.0 MiB | 175519 |
| ultraworkers/claw-code | rust | large | large | 66820 | 0.229 s | 144.7 MiB | 291795 |
| farion1231/cc-switch | rust | large | large | 177711 | 0.712 s | 348.6 MiB | 249633 |
| openai/codex | rust | large | large | 921065 | 4.565 s | 1774.4 MiB | 201755 |
| rustdesk/rustdesk | rust | large | large | 153705 | 0.371 s | 250.3 MiB | 414768 |
| pbakaus/impeccable | rust | large | large | 174629 | 2.591 s | 836.6 MiB | 67397 |
| tauri-apps/tauri | rust | large | large | 71436 | 0.594 s | 149.9 MiB | 120309 |
| denoland/deno | rust | large | large | 410199 | 1.846 s | 727.4 MiB | 222239 |
| ruvnet/RuView | rust | large | large | 158139 | 0.891 s | 408.6 MiB | 177427 |
| zed-industries/zed | rust | large | large | 990328 | 4.861 s | 1353.2 MiB | 203731 |
| astral-sh/uv | rust | large | large | 202794 | 1.097 s | 453.6 MiB | 184839 |
| rtk-ai/rtk | rust | large | large | 85424 | 0.265 s | 175.6 MiB | 322735 |
| storytold/photocraft | rust | large | large | 340681 | 3.055 s | 1013.8 MiB | 111527 |
| unionlabs/union | rust | large | large | 181032 | 5.388 s | 1452.0 MiB | 33598 |
| dani-garcia/vaultwarden | rust | medium | medium | 31443 | 0.140 s | 92.5 MiB | 224219 |
| BurntSushi/ripgrep | rust | medium | medium | 25014 | 0.118 s | 85.9 MiB | 211366 |
| LadybirdBrowser/ladybird | rust | large | large | 822176 | 2.775 s | 1208.2 MiB | 296308 |
| openinterpreter/openinterpreter | rust | large | large | 752893 | 4.393 s | 1359.3 MiB | 171386 |
| alacritty/alacritty | rust | medium | medium | 20755 | 0.069 s | 55.6 MiB | 300800 |
| rust-lang/rustlings | rust | small | small | 6084 | 0.063 s | 23.9 MiB | 97067 |
| tw93/Pake | rust | small | small | 9861 | 0.048 s | 47.6 MiB | 205579 |
| starship/starship | rust | medium | medium | 20476 | 0.128 s | 57.2 MiB | 159472 |
| sharkdp/fd | rust | small | small | 3373 | 0.025 s | 23.5 MiB | 132530 |
| GyulyVGC/sniffnet | rust | medium | medium | 22399 | 0.065 s | 56.2 MiB | 344030 |
| FuelLabs/fuels-rs | rust | medium | medium | 16058 | 0.130 s | 61.2 MiB | 123508 |
| cjpais/Handy | rust | medium | medium | 23081 | 0.085 s | 69.1 MiB | 270690 |
| AlexsJones/llmfit | rust | medium | medium | 37372 | 0.138 s | 125.9 MiB | 271396 |
| casey/just | rust | medium | medium | 18848 | 0.147 s | 62.7 MiB | 128118 |
| TabbyML/tabby | rust | medium | medium | 33260 | 3.578 s | 413.2 MiB | 9295 |
| Zackriya-Solutions/meetily | rust | medium | medium | 35117 | 0.162 s | 89.1 MiB | 216698 |
| ankitects/anki | rust | medium | medium | 49762 | 0.188 s | 124.4 MiB | 264240 |
| yewstack/yew | rust | medium | medium | 21901 | 0.099 s | 70.5 MiB | 220884 |
| dandavison/delta | rust | medium | medium | 14652 | 0.045 s | 45.2 MiB | 323634 |
| googleworkspace/cli | rust | medium | medium | 14683 | 0.050 s | 52.0 MiB | 290937 |
| sharkdp/hyperfine | rust | small | small | 4901 | 0.020 s | 23.7 MiB | 242319 |
| cloudflare/pingora | rust | medium | medium | 48066 | 0.265 s | 126.6 MiB | 181414 |
| tokio-rs/axum | rust | medium | medium | 16103 | 0.145 s | 64.6 MiB | 110802 |
| tree-sitter/tree-sitter | rust | medium | medium | 35427 | 0.116 s | 89.2 MiB | 306624 |
| Schniz/fnm | rust | small | small | 3338 | 0.015 s | 21.3 MiB | 216393 |
| Wilfred/difftastic | rust | medium | medium | 14371 | 0.111 s | 53.5 MiB | 129198 |
| TheAlgorithms/Rust | rust | medium | medium | 21394 | 0.132 s | 44.4 MiB | 162127 |
| ogham/exa | rust | small | small | 5976 | 0.023 s | 27.0 MiB | 258154 |
| fathyb/carbonyl | rust | small | small | 2675 | 0.048 s | 19.4 MiB | 55707 |
| QuipNetwork/hashsigs-rs | rust | small | small | 7166 | 0.057 s | 44.8 MiB | 125781 |
| seanmonstar/warp | rust | small | small | 6593 | 0.026 s | 28.4 MiB | 256137 |
| actix/actix | rust | small | small | 5523 | 0.026 s | 27.0 MiB | 213584 |
| tokio-rs/mio | rust | small | small | 8214 | 0.094 s | 36.0 MiB | 86992 |
| dtolnay/anyhow | rust | small | small | 2378 | 0.055 s | 25.1 MiB | 43262 |
| emmett-framework/granian | rust | small | small | 8010 | 0.032 s | 31.4 MiB | 252195 |
| dtolnay/thiserror | rust | small | small | 2560 | 0.014 s | 20.5 MiB | 183064 |
| autumnai/leaf | rust | small | small | 3393 | 0.058 s | 35.3 MiB | 58897 |
| coollabsio/coolify | php | large | large | 227364 | 9.377 s | 592.9 MiB | 24247 |
| nextcloud/server | php | large | large | 310264 | 4.995 s | 727.9 MiB | 62119 |
| laravel/framework | php | large | large | 132915 | 0.792 s | 277.5 MiB | 167759 |
| filamentphp/filament | php | large | large | 270274 | 2.317 s | 352.0 MiB | 116638 |
| symfony/symfony | php | large | large | 1030081 | 2.923 s | 1094.9 MiB | 352371 |
| composer/composer | php | large | large | 50034 | 0.071 s | 46.4 MiB | 700052 |
| bagisto/bagisto | php | large | large | 303018 | 1.356 s | 314.5 MiB | 223390 |
| monicahq/monica | php | large | large | 164533 | 0.539 s | 236.6 MiB | 305230 |
| firefly-iii/firefly-iii | php | large | large | 109763 | 0.733 s | 167.8 MiB | 149782 |
| krayin/laravel-crm | php | large | large | 59939 | 0.922 s | 191.9 MiB | 64989 |
| livewire/livewire | php | medium | medium | 21413 | 0.108 s | 56.8 MiB | 198135 |
| guzzle/guzzle | php | medium | medium | 12931 | 0.068 s | 47.8 MiB | 191281 |
| PHPMailer/PHPMailer | php | small | small | 5717 | 0.024 s | 19.5 MiB | 239013 |
| DesignPatternsPHP/DesignPatternsPHP | php | small | small | 2018 | 0.025 s | 13.1 MiB | 79277 |
| matomo-org/matomo | php | large | large | 273171 | 4.628 s | 687.8 MiB | 59031 |
| Seldaek/monolog | php | small | small | 8691 | 0.019 s | 20.2 MiB | 459835 |
| sebastianbergmann/phpunit | php | large | large | 85993 | 0.428 s | 112.3 MiB | 200723 |
| fruitcake/laravel-debugbar | php | small | small | 5231 | 0.017 s | 19.5 MiB | 300682 |
| BookStackApp/BookStack | php | large | large | 116965 | 1.101 s | 171.1 MiB | 106228 |
| bcit-ci/CodeIgniter | php | medium | medium | 28489 | 0.042 s | 35.1 MiB | 675668 |
| nikic/PHP-Parser | php | medium | medium | 17251 | 0.106 s | 45.4 MiB | 162772 |
| koel/koel | php | medium | medium | 41312 | 0.199 s | 113.5 MiB | 207655 |
| briannesbitt/Carbon | php | medium | medium | 30625 | 0.192 s | 84.2 MiB | 159278 |
| getgrav/grav | php | large | large | 70378 | 0.263 s | 81.3 MiB | 267137 |
| grokability/snipe-it | php | large | large | 583155 | 5.493 s | 607.7 MiB | 106155 |
| barryvdh/laravel-ide-helper | php | small | small | 3677 | 0.032 s | 16.4 MiB | 115129 |
| Intervention/image | php | medium | medium | 16997 | 0.121 s | 50.4 MiB | 140968 |
| phpstan/phpstan | php | small | small | 2563 | 0.030 s | 24.2 MiB | 84947 |
| digininja/DVWA | php | small | small | 8614 | 0.028 s | 20.8 MiB | 304888 |
| thephpleague/flysystem | php | medium | medium | 10123 | 0.061 s | 22.8 MiB | 165820 |
| filp/whoops | php | small | small | 2157 | 0.019 s | 17.1 MiB | 116394 |
| wallabag/wallabag | php | medium | medium | 20434 | 0.122 s | 53.4 MiB | 166991 |
| spatie/laravel-permission | php | small | small | 2608 | 0.035 s | 22.8 MiB | 73755 |
| SpartnerNL/Laravel-Excel | php | small | small | 7326 | 0.030 s | 20.3 MiB | 242347 |
| ramsey/uuid | php | small | small | 4424 | 0.031 s | 23.2 MiB | 144882 |
| typecho/typecho | php | medium | medium | 23232 | 0.191 s | 60.3 MiB | 121480 |
| slimphp/Slim | php | small | small | 3080 | 0.024 s | 19.5 MiB | 127358 |
| pestphp/pest | php | medium | medium | 21028 | 0.078 s | 37.7 MiB | 268396 |
| egulias/EmailValidator | php | small | small | 2422 | 0.013 s | 12.5 MiB | 187560 |
| walkor/workerman | php | small | small | 6907 | 0.017 s | 16.7 MiB | 402453 |
| tymondesigns/jwt-auth | php | small | small | 2361 | 0.016 s | 14.2 MiB | 148514 |
| dompdf/dompdf | php | medium | medium | 22190 | 0.027 s | 27.0 MiB | 814180 |
| z-song/laravel-admin | php | medium | medium | 23640 | 0.186 s | 60.5 MiB | 127238 |
| deployphp/deployer | php | medium | medium | 11866 | 0.030 s | 20.8 MiB | 391783 |
| nextcloud/all-in-one | php | medium | medium | 12760 | 0.029 s | 25.0 MiB | 434673 |
| squizlabs/PHP_CodeSniffer | php | medium | medium | 40134 | 0.120 s | 51.7 MiB | 335528 |
| Anankke/SSPanel-UIM | php | medium | medium | 19727 | 0.032 s | 28.2 MiB | 611032 |
| doctrine/orm | php | medium | medium | 36744 | 0.297 s | 75.0 MiB | 123556 |
| bobthecow/psysh | php | medium | medium | 36911 | 0.196 s | 60.4 MiB | 188048 |
| symfony/console | php | medium | medium | 17886 | 0.076 s | 43.2 MiB | 236105 |
| flutter/flutter | dart | large | large | 806633 | 4.495 s | 1154.0 MiB | 179467 |
| AppFlowy-IO/AppFlowy | dart | large | large | 229243 | 0.464 s | 241.9 MiB | 493966 |
| chen08209/FlClash | dart | large | large | 125752 | 0.512 s | 224.1 MiB | 245783 |
| Predidit/Kazumi | dart | large | large | 65949 | 0.053 s | 34.6 MiB | 1253731 |
| hiddify/hiddify-app | dart | medium | medium | 42284 | 0.217 s | 86.0 MiB | 195171 |
| ccch1mneyyy/dsh-Go | dart | large | large | 537675 | 2.299 s | 1072.5 MiB | 233824 |
| ente/ente | dart | large | large | 352288 | 1.025 s | 605.8 MiB | 343625 |
| GopeedLab/gopeed | dart | medium | medium | 48337 | 0.181 s | 106.5 MiB | 266343 |
| alibaba/flutter-go | dart | medium | medium | 26521 | 0.038 s | 27.7 MiB | 697016 |
| mitesh77/Best-Flutter-UI-Templates | dart | small | small | 9524 | 0.013 s | 16.1 MiB | 711944 |
| ImranR98/Obtainium | dart | medium | medium | 33892 | 0.039 s | 42.0 MiB | 878685 |
| getlantern/lantern | dart | medium | medium | 46249 | 0.158 s | 81.0 MiB | 292059 |
| KaringX/karing | dart | large | large | 51672 | 1.667 s | 212.3 MiB | 30999 |
| xiaoyaocz/dart_simple_live | dart | medium | medium | 39907 | 0.078 s | 39.5 MiB | 512635 |
| bagisto/opensource-ecommerce-mobile-app | dart | large | large | 71308 | 0.183 s | 93.0 MiB | 390303 |
| CarGuo/gsy_github_app_flutter | dart | medium | medium | 36339 | 0.061 s | 41.7 MiB | 591473 |
| wanghongenpin/proxypin | dart | large | large | 67024 | 0.103 s | 70.4 MiB | 652434 |
| Notsfsssf/pixez-flutter | dart | large | large | 62354 | 0.245 s | 137.3 MiB | 254310 |
| cfug/dio | dart | small | small | 7698 | 0.027 s | 23.1 MiB | 286447 |
| felangel/bloc | dart | medium | medium | 11597 | 0.081 s | 42.5 MiB | 142781 |
| jonataslaw/getx | dart | medium | medium | 15922 | 0.031 s | 25.0 MiB | 521553 |
| flame-engine/flame | dart | large | large | 102657 | 0.287 s | 118.2 MiB | 357667 |
| KaringX/clashmi | dart | medium | medium | 35863 | 1.487 s | 194.5 MiB | 24121 |
| firebase/flutterfire | dart | large | large | 64761 | 0.379 s | 104.9 MiB | 171040 |
| lollipopkit/flutter_server_box | dart | large | large | 246009 | 2.001 s | 643.0 MiB | 122968 |
| brianegan/flutter_architecture_samples | dart | medium | medium | 12616 | 0.090 s | 26.3 MiB | 140791 |
| toly1994328/FlutterUnit | dart | large | large | 59372 | 0.087 s | 52.5 MiB | 681891 |
| simplezhli/flutter_deer | dart | medium | medium | 16839 | 0.023 s | 19.1 MiB | 744818 |
| Sle2p/AniCh | dart | medium | medium | 27488 | 0.055 s | 32.1 MiB | 500544 |
| ComicSparks/pikapika | dart | medium | medium | 31065 | 0.105 s | 43.0 MiB | 294697 |
| mornyeapp/Mornye | dart | large | large | 293436 | 0.880 s | 471.2 MiB | 333478 |
| alibaba/fish-redux | dart | small | small | 4484 | 0.020 s | 18.9 MiB | 226692 |
| OpenFlutter/Flutter-Notebook | dart | small | small | 8007 | 0.046 s | 17.0 MiB | 172574 |
| nisrulz/flutter-examples | dart | small | small | 8552 | 0.047 s | 19.1 MiB | 183807 |
| mylxsw/aidea | dart | medium | medium | 49709 | 0.084 s | 63.2 MiB | 594196 |
| vandadnp/flutter-tips-and-tricks | dart | medium | medium | 13755 | 0.027 s | 16.2 MiB | 510099 |
| 2d-inc/HistoryOfEverything | dart | small | small | 4833 | 0.012 s | 14.8 MiB | 406807 |
| iampawan/Flutter-UI-Kit | dart | small | small | 4684 | 0.009 s | 13.0 MiB | 537151 |
| samarthagarwal/FlutterScreens | dart | small | small | 3362 | 0.008 s | 13.5 MiB | 416474 |
| flutterchina/flukit | dart | small | small | 4586 | 0.020 s | 14.3 MiB | 233943 |
| miru-project/miru-app | dart | medium | medium | 19261 | 0.091 s | 34.7 MiB | 212535 |
| Sky24n/flutter_wanandroid | dart | small | small | 7314 | 0.040 s | 17.3 MiB | 180807 |
| ComicSparks/jasmine | dart | medium | medium | 18552 | 0.030 s | 25.9 MiB | 619057 |
| btwld/fvm | dart | small | small | 9465 | 0.049 s | 29.8 MiB | 193558 |
| ionicfirebaseapp/getwidget | dart | medium | medium | 11792 | 0.041 s | 24.0 MiB | 288719 |
| xuelongqy/flutter_easy_refresh | dart | medium | medium | 17664 | 0.054 s | 27.4 MiB | 326391 |
| best-flutter/flutter_swiper | dart | small | small | 2653 | 0.010 s | 13.8 MiB | 262139 |
| ReactiveX/rxdart | dart | small | small | 6582 | 0.062 s | 26.7 MiB | 105823 |
| rrousselGit/flutter_hooks | dart | small | small | 2855 | 0.033 s | 15.7 MiB | 87512 |
| OpenFlutter/fluwx | dart | small | small | 4102 | 0.039 s | 16.6 MiB | 105558 |
| NousResearch/hermes-agent | python | large | large | 947246 | 4.390 s | 1947.2 MiB | 215777 |
| yt-dlp/yt-dlp | python | large | large | 201346 | 0.196 s | 118.7 MiB | 1029862 |
| anthropics/skills | python | medium | medium | 17061 | 0.055 s | 32.8 MiB | 308899 |
| microsoft/markitdown | python | small | small | 7117 | 0.027 s | 18.2 MiB | 259167 |
| Significant-Gravitas/AutoGPT | python | large | large | 477476 | 1.327 s | 898.1 MiB | 359883 |
| huggingface/transformers | python | large | large | 752814 | 4.443 s | 868.2 MiB | 169420 |
| AUTOMATIC1111/stable-diffusion-webui | python | medium | medium | 33074 | 0.060 s | 48.1 MiB | 555021 |
| langgenius/dify | python | large | large | 480982 | 12.971 s | 1486.6 MiB | 37082 |
| langflow-ai/langflow | python | large | large | 370045 | 1.534 s | 637.3 MiB | 241163 |
| nextlevelbuilder/ui-ux-pro-max-skill | python | medium | medium | 24454 | 0.246 s | 64.6 MiB | 99483 |
| Graphify-Labs/graphify | python | large | large | 89067 | 0.296 s | 183.5 MiB | 301328 |
| github/spec-kit | python | large | large | 75044 | 0.271 s | 142.2 MiB | 277190 |
| langchain-ai/langchain | python | large | large | 205153 | 0.325 s | 168.6 MiB | 631497 |
| Comfy-Org/ComfyUI | python | large | large | 256452 | 0.528 s | 275.1 MiB | 485664 |
| Shubhamsaboo/awesome-llm-apps | python | large | large | 69835 | 4.049 s | 133.7 MiB | 17247 |
| harry0703/MoneyPrinterTurbo | python | medium | medium | 40254 | 0.090 s | 68.9 MiB | 445296 |
| ytdl-org/youtube-dl | python | large | large | 137578 | 0.134 s | 87.9 MiB | 1027610 |
| browser-use/browser-use | python | medium | medium | 14527 | 0.033 s | 32.5 MiB | 444026 |
| debpalash/VoiceStudio | python | large | large | 142694 | 0.397 s | 349.2 MiB | 359462 |
| Panniantong/Agent-Reach | python | small | small | 8000 | 0.019 s | 21.6 MiB | 424137 |
| openai/whisper | python | small | small | 3257 | 0.008 s | 13.0 MiB | 414412 |
| pytorch/pytorch | python | large | large | 1246987 | 4.364 s | 1457.0 MiB | 285751 |
| D4Vinci/Scrapling | python | medium | medium | 14643 | 0.041 s | 34.3 MiB | 354400 |
| fastapi/fastapi | python | medium | medium | 31370 | 0.171 s | 80.4 MiB | 183692 |
| hacksider/Deep-Live-Cam | python | small | small | 6485 | 0.010 s | 16.1 MiB | 648835 |
| nvbn/thefuck | python | small | small | 5066 | 0.017 s | 18.8 MiB | 306146 |
| vllm-project/vllm | python | large | large | 1293645 | 9.724 s | 1215.0 MiB | 133031 |
| 3b1b/manim | python | medium | medium | 21968 | 0.017 s | 21.5 MiB | 1311620 |
| 666ghj/MiroFish | python | medium | medium | 21160 | 0.035 s | 30.5 MiB | 612745 |
| Z4nzu/hackingtool | python | small | small | 8790 | 0.016 s | 18.8 MiB | 548575 |
| abi/screenshot-to-code | python | medium | medium | 11777 | 0.032 s | 32.1 MiB | 365336 |
| usestrix/strix | python | medium | medium | 39912 | 0.356 s | 99.4 MiB | 112136 |
| hiyouga/LlamaFactory | python | medium | medium | 42248 | 0.057 s | 45.4 MiB | 737239 |
| pallets/flask | python | small | small | 8142 | 0.017 s | 20.6 MiB | 484708 |
| ai-job-search-dev/ai-job-search | python | small | small | 5449 | 0.014 s | 19.3 MiB | 399977 |
| asgeirtj/system_prompts_leaks | python | medium | medium | 32579 | 0.097 s | 59.2 MiB | 334783 |
| NanmiCoder/MediaCrawler | python | medium | medium | 18289 | 0.035 s | 28.2 MiB | 527876 |
| binary-husky/gpt_academic | python | medium | medium | 49008 | 0.132 s | 61.9 MiB | 370979 |
| jingyaogong/minimind | python | small | small | 3994 | 0.011 s | 15.5 MiB | 379682 |
| scrapy/scrapy | python | medium | medium | 33076 | 0.062 s | 60.2 MiB | 531651 |
| virattt/ai-hedge-fund | python | medium | medium | 11397 | 0.015 s | 20.1 MiB | 781971 |
| bilibili/GPT-SoVITS | python | medium | medium | 40052 | 0.057 s | 50.3 MiB | 698125 |
| commaai/openpilot | python | medium | medium | 46200 | 0.064 s | 61.6 MiB | 726819 |
| jamiepine/voicebox | python | medium | medium | 18937 | 0.073 s | 54.1 MiB | 260922 |
| karpathy/nanochat | python | small | small | 5213 | 0.010 s | 16.2 MiB | 498047 |
| CorentinJ/Real-Time-Voice-Cloning | python | small | small | 4639 | 0.008 s | 14.2 MiB | 550498 |
| FoundationAgents/OpenManus | python | small | small | 9324 | 0.012 s | 18.0 MiB | 768506 |
| Zie619/n8n-workflows | python | small | small | 5971 | 0.020 s | 25.0 MiB | 298989 |
| psf/requests | python | small | small | 5095 | 0.014 s | 16.2 MiB | 364951 |
| hiroi-sora/Umi-OCR | python | small | small | 9983 | 0.030 s | 18.0 MiB | 330679 |
| ollama/ollama | go | large | large | 150942 | 0.466 s | 271.4 MiB | 324083 |
| JuliusBrussee/caveman | go | large | large | 86647 | 0.383 s | 252.8 MiB | 225975 |
| microsoft/TypeScript | go | large | large | 234260 | 2.913 s | 641.0 MiB | 80428 |
| fatedier/frp | go | medium | medium | 35256 | 0.063 s | 56.5 MiB | 559836 |
| infiniflow/ragflow | go | large | large | 405918 | 6.960 s | 847.6 MiB | 58321 |
| syncthing/syncthing | go | large | large | 52790 | 0.154 s | 105.6 MiB | 343548 |
| gin-gonic/gin | go | small | small | 5364 | 0.053 s | 24.7 MiB | 100754 |
| junegunn/fzf | go | medium | medium | 22956 | 0.077 s | 37.2 MiB | 297921 |
| netdata/netdata | go | large | large | 445710 | 2.956 s | 909.3 MiB | 150772 |
| caddyserver/caddy | go | large | large | 50918 | 0.129 s | 69.3 MiB | 393505 |
| grafana/grafana | go | large | large | 551632 | 16.288 s | 1962.0 MiB | 33867 |
| nektos/act | go | medium | medium | 15602 | 0.064 s | 39.8 MiB | 242062 |
| prometheus/prometheus | go | large | large | 120134 | 1.070 s | 292.3 MiB | 112257 |
| traefik/traefik | go | large | large | 56672 | 1.119 s | 259.0 MiB | 50649 |
| usememos/memos | go | medium | medium | 46670 | 0.575 s | 178.0 MiB | 81187 |
| rclone/rclone | go | large | large | 199107 | 0.573 s | 238.4 MiB | 347561 |
| router-for-me/CLIProxyAPI | go | large | large | 325665 | 2.371 s | 749.8 MiB | 137342 |
| go-gitea/gitea | go | large | large | 260214 | 0.677 s | 369.2 MiB | 384405 |
| multica-ai/multica | go | large | large | 289450 | 2.710 s | 1089.7 MiB | 106795 |
| alibaba/open-code-review | go | medium | medium | 37725 | 0.283 s | 131.9 MiB | 133497 |
| wagoodman/dive | go | small | small | 8289 | 0.027 s | 22.4 MiB | 302897 |
| etcd-io/etcd | go | large | large | 76188 | 0.297 s | 153.4 MiB | 256323 |
| v2ray/v2ray-core | go | medium | medium | 30798 | 0.174 s | 75.3 MiB | 176756 |
| charmbracelet/bubbletea | go | small | small | 9509 | 0.015 s | 16.7 MiB | 622932 |
| spf13/cobra | go | small | small | 5165 | 0.022 s | 19.1 MiB | 234164 |
| danielmiessler/Fabric | go | medium | medium | 21379 | 0.077 s | 43.3 MiB | 276913 |
| astaxie/build-web-application-with-golang | go | medium | medium | 12472 | 0.062 s | 29.0 MiB | 200970 |
| schollz/croc | go | medium | medium | 28527 | 0.090 s | 57.8 MiB | 316545 |
| gofiber/fiber | go | medium | medium | 38295 | 0.133 s | 82.4 MiB | 288982 |
| go-gorm/gorm | go | medium | medium | 12497 | 0.069 s | 44.8 MiB | 180281 |
| docker/compose | go | medium | medium | 31268 | 0.145 s | 77.9 MiB | 215220 |
| glanceapp/glance | go | small | small | 8168 | 0.020 s | 18.0 MiB | 401038 |
| IceWhaleTech/CasaOS | go | small | small | 9580 | 0.024 s | 20.7 MiB | 394008 |
| restic/restic | go | medium | medium | 38397 | 0.096 s | 68.2 MiB | 399710 |
| binwiederhier/ntfy | go | medium | medium | 30648 | 0.176 s | 85.8 MiB | 174476 |
| derailed/k9s | go | medium | medium | 39963 | 0.070 s | 38.8 MiB | 572213 |
| k3s-io/k3s | go | medium | medium | 30357 | 0.105 s | 54.7 MiB | 288630 |
| ehang-io/nps | go | small | small | 8545 | 0.301 s | 60.5 MiB | 28423 |
| labstack/echo | go | small | small | 8486 | 0.054 s | 32.6 MiB | 158257 |
| beego/beego | go | medium | medium | 35989 | 0.094 s | 49.1 MiB | 382636 |
| dokku/dokku | go | medium | medium | 40888 | 0.105 s | 54.8 MiB | 388962 |
| iawia002/lux | go | small | small | 8571 | 0.031 s | 20.2 MiB | 280207 |
| abiosoft/colima | go | medium | medium | 10091 | 0.020 s | 18.8 MiB | 492937 |
| gitleaks/gitleaks | go | medium | medium | 12156 | 0.035 s | 23.8 MiB | 344763 |
| chubin/wttr.in | go | small | small | 8733 | 0.021 s | 18.4 MiB | 407948 |
| spf13/viper | go | small | small | 2280 | 0.014 s | 14.4 MiB | 162232 |
| charmbracelet/glow | go | small | small | 3200 | 0.012 s | 12.5 MiB | 275572 |
| go-kit/kit | go | small | small | 8996 | 0.033 s | 21.7 MiB | 272305 |
| sirupsen/logrus | go | small | small | 2190 | 0.015 s | 14.3 MiB | 147092 |
| gorilla/websocket | go | small | small | 2971 | 0.011 s | 14.4 MiB | 278643 |
| pgmq/pgmq | sql | medium | medium | 10854 | 0.069 s | 77.7 MiB | 156352 |
| datacharmer/test_db | sql | medium | medium | 47574 | 0.317 s | 217.0 MiB | 150306 |
| BrentOzarULTD/SQL-Server-First-Responder-Kit | sql | large | large | 137263 | 0.891 s | 568.2 MiB | 153985 |
| olahallengren/sql-server-maintenance-solution | sql | medium | medium | 20808 | 0.135 s | 103.8 MiB | 154636 |
| openmaptiles/openmaptiles | sql | medium | medium | 10411 | 0.039 s | 51.0 MiB | 270221 |
| pgpartman/pg_partman | sql | large | large | 156326 | 1.503 s | 578.5 MiB | 104018 |
| lerocha/chinook-database | sql | large | large | 173602 | 0.954 s | 850.5 MiB | 181942 |
| darold/pgFormatter | sql | large | large | 310653 | 1.757 s | 1851.4 MiB | 176822 |
| NikolayS/PgQue | sql | medium | medium | 34095 | 0.239 s | 82.4 MiB | 142639 |
| NixOS/hydra | sql | medium | medium | 38904 | 0.593 s | 273.0 MiB | 65567 |
| amachanic/sp_whoisactive | sql | medium | medium | 14923 | 0.059 s | 94.0 MiB | 254180 |
| microsoft/fhir-server | sql | large | large | 335344 | 2.428 s | 1265.2 MiB | 138134 |
| cahyadsn/wilayah | sql | large | large | 509905 | 1.461 s | 1510.8 MiB | 349088 |
| NikolayS/postgres_dba | sql | small | small | 2900 | 0.015 s | 27.3 MiB | 194386 |
| theory/pgtap | sql | small | small | 4990 | 0.091 s | 136.8 MiB | 54634 |
| aquametalabs/aquameta | sql | small | small | 4742 | 0.021 s | 36.4 MiB | 224373 |
| mqttsnet/thinglinks | sql | medium | medium | 11692 | 2.250 s | 482.0 MiB | 5195 |
| oracle-samples/db-sample-schemas | sql | large | large | 110419 | 0.374 s | 363.9 MiB | 295302 |
| smpetersgithub/AdvancedSQLPuzzles | sql | medium | medium | 11192 | 0.025 s | 40.6 MiB | 442618 |
| chinnonsantos/sql-paises-estados-cidades | sql | medium | medium | 18097 | 0.164 s | 216.0 MiB | 110653 |
| anthonydb/practical-sql-2 | sql | small | small | 3059 | 0.015 s | 34.3 MiB | 197377 |
| rin-nas/postgresql-patterns-library | sql | medium | medium | 12411 | 0.031 s | 39.3 MiB | 400080 |
| tanelpoder/tpt-oracle | sql | large | large | 100830 | 0.880 s | 812.1 MiB | 114538 |
| erikdarlingdata/DarlingData | sql | large | large | 145386 | 0.450 s | 479.8 MiB | 323020 |
| anthonydb/practical-sql | sql | small | small | 2558 | 0.012 s | 30.7 MiB | 214334 |
| Ysurac/FlightAirMap | sql | large | large | 137049 | 1.949 s | 1099.0 MiB | 70325 |
| utPLSQL/utPLSQL | sql | small | small | 2769 | 0.012 s | 20.9 MiB | 223185 |
| trickerer/Trinity-Bots | sql | medium | medium | 28859 | 0.236 s | 258.2 MiB | 122366 |
| morenoh149/postgresDBSamples | sql | large | large | 59597 | 0.278 s | 471.5 MiB | 214592 |
| jOOQ/sakila | sql | large | large | 928489 | 3.310 s | 7766.1 MiB | 280498 |
| microsoft/Windows-appsample-customers-orders-database | sql | medium | medium | 16236 | 0.072 s | 81.5 MiB | 224183 |
| dbkangaroo/kangaroo | sql | large | large | 141672 | 1.820 s | 2466.2 MiB | 77843 |
| marcingminski/sqlwatch | sql | medium | medium | 14828 | 0.024 s | 31.4 MiB | 615835 |
| lorint/AdventureWorks-for-Postgres | sql | small | small | 2732 | 0.021 s | 30.8 MiB | 132710 |
| 3dcitydb/3dcitydb | sql | small | small | 2883 | 0.015 s | 25.4 MiB | 193408 |
| geocompass/pg-coordtransform | sql | small | small | 2117 | 0.010 s | 15.6 MiB | 214360 |
| point-source/supabase-tenant-rbac | sql | medium | medium | 12138 | 0.057 s | 70.0 MiB | 213206 |
| tSQLt-org/tSQLt | sql | medium | medium | 24792 | 0.055 s | 106.8 MiB | 449231 |
| pljson/pljson | sql | small | small | 8422 | 0.020 s | 35.6 MiB | 419420 |
| andrelugomes/digital-innovation-one | sql | medium | medium | 11873 | 0.012 s | 23.7 MiB | 961941 |
| Abstrct/Schemaverse | sql | small | small | 9141 | 0.161 s | 76.8 MiB | 56754 |
| basedosdados/sdk | sql | medium | medium | 19773 | 0.086 s | 53.6 MiB | 229424 |
| pgpointcloud/pointcloud | sql | medium | medium | 40779 | 0.746 s | 1166.8 MiB | 54659 |
| pgq/pgq | sql | small | small | 3223 | 0.012 s | 24.1 MiB | 271861 |
| pgMemento/pgMemento | sql | small | small | 6727 | 0.019 s | 28.5 MiB | 346047 |
| siqichen-usc/LeetCode-SQL-Summary | sql | small | small | 4664 | 0.018 s | 45.4 MiB | 260946 |
| bnagy/cgasm | sql | large | large | 53520 | 0.027 s | 50.4 MiB | 1998749 |
| postgis/h3-pg | sql | small | small | 5558 | 0.023 s | 46.9 MiB | 245842 |
| Hameds/IranCountryDivisions | sql | large | large | 1047837 | 4.901 s | 6032.2 MiB | 213816 |
| czwbig/Tmall_JavaEE | sql | medium | medium | 29551 | 0.129 s | 249.4 MiB | 229567 |
| vercel/next.js | typescript-javascript | large | large | 1138956 | 22.089 s | 1810.5 MiB | 51561 |
| shadcn-ui/ui | typescript-javascript | large | large | 415871 | 1.167 s | 398.9 MiB | 356479 |
| react/create-react-app | typescript-javascript | large | large | 56349 | 0.295 s | 111.5 MiB | 190864 |
| mui/material-ui | typescript-javascript | large | large | 577616 | 1.235 s | 428.0 MiB | 467606 |
| storybookjs/storybook | typescript-javascript | large | large | 359426 | 1.494 s | 452.2 MiB | 240612 |
| ChatGPTNextWeb/NextChat | typescript-javascript | medium | medium | 37605 | 0.120 s | 45.8 MiB | 313159 |
| remotion-dev/remotion | typescript-javascript | large | large | 1223808 | 2.621 s | 890.6 MiB | 466873 |
| AykutSarac/jsoncrack.com | typescript-javascript | small | small | 8237 | 0.027 s | 19.7 MiB | 307116 |
| xyflow/xyflow | typescript-javascript | medium | medium | 45226 | 0.082 s | 36.2 MiB | 552582 |
| jaredpalmer/formik | typescript-javascript | small | small | 8487 | 0.043 s | 21.7 MiB | 199430 |
| pmndrs/react-three-fiber | typescript-javascript | small | small | 6962 | 0.016 s | 22.4 MiB | 440891 |
| gethomepage/homepage | typescript-javascript | medium | medium | 29325 | 0.115 s | 74.5 MiB | 254587 |
| vercel/swr | typescript-javascript | small | small | 4940 | 0.023 s | 25.5 MiB | 211528 |
| pmndrs/react-spring | typescript-javascript | medium | medium | 21487 | 0.035 s | 33.8 MiB | 608433 |
| slymnoyann/hey.xyz | typescript-javascript | medium | medium | 44766 | 0.212 s | 156.4 MiB | 211536 |
| tailwindlabs/headlessui | typescript-javascript | medium | medium | 33429 | 0.161 s | 79.8 MiB | 207983 |
| maotoumao/MusicFree | typescript-javascript | medium | medium | 27546 | 0.027 s | 25.8 MiB | 1005495 |
| nextauthjs/next-auth | typescript-javascript | medium | medium | 29698 | 0.052 s | 39.9 MiB | 573854 |
| rahuldkjain/github-profile-readme-generator | typescript-javascript | small | small | 5392 | 0.010 s | 15.5 MiB | 542225 |
| tabler/tabler-icons | typescript-javascript | small | small | 5141 | 0.033 s | 21.0 MiB | 153559 |
| PanJiaChen/vue-element-admin | typescript-javascript | medium | medium | 10502 | 0.013 s | 17.8 MiB | 787763 |
| bailicangdu/vue2-elm | typescript-javascript | small | small | 4236 | 0.037 s | 27.8 MiB | 114356 |
| vuejs/vuepress | typescript-javascript | small | small | 8059 | 0.034 s | 17.8 MiB | 233792 |
| zxwk1998/vue-admin-better | typescript-javascript | small | small | 8928 | 0.028 s | 27.6 MiB | 321105 |
| iview/iview-admin | typescript-javascript | small | small | 5994 | 0.011 s | 15.7 MiB | 567452 |
| codesandbox/codesandbox-client | typescript-javascript | large | large | 906913 | 117.778 s | 2702.7 MiB | 7700 |
| bailicangdu/vue2-manage | typescript-javascript | small | small | 2170 | 0.619 s | 101.5 MiB | 3505 |
| vueComponent/ant-design-vue-pro | typescript-javascript | medium | medium | 11135 | 0.014 s | 18.4 MiB | 809156 |
| getgridea/gridea | typescript-javascript | small | small | 6670 | 0.011 s | 17.3 MiB | 624693 |
| tsparticles/tsparticles | typescript-javascript | large | large | 142624 | 0.554 s | 149.0 MiB | 257418 |
| OnsenUI/OnsenUI | typescript-javascript | medium | medium | 36420 | 0.065 s | 53.9 MiB | 557899 |
| x-extends/vxe-table | typescript-javascript | large | large | 60659 | 0.041 s | 49.3 MiB | 1470350 |
| gridsome/gridsome | typescript-javascript | medium | medium | 13188 | 0.023 s | 25.0 MiB | 577866 |
| emqx/MQTTX | typescript-javascript | large | large | 56309 | 0.054 s | 44.9 MiB | 1052115 |
| Tencent/wujie | typescript-javascript | medium | medium | 15475 | 0.037 s | 32.0 MiB | 416239 |
| elunez/eladmin-web | typescript-javascript | medium | medium | 10603 | 0.016 s | 18.9 MiB | 664902 |
| herozhou/vue-framework-wz | typescript-javascript | medium | medium | 14092 | 0.071 s | 54.6 MiB | 197993 |
| umicro/uView | typescript-javascript | medium | medium | 23352 | 0.026 s | 28.9 MiB | 893148 |
| algolia/instantsearch | typescript-javascript | large | large | 94534 | 1.063 s | 279.9 MiB | 88932 |
| Simon-He95/markstream-vue | typescript-javascript | large | large | 169970 | 0.220 s | 136.0 MiB | 771143 |
| storybookjs/storybook | typescript-javascript | large | large | 359496 | 1.389 s | 476.0 MiB | 258801 |
| hoppscotch/hoppscotch | typescript-javascript | large | large | 157256 | 0.307 s | 170.7 MiB | 512969 |
| lyswhut/lx-music-desktop | typescript-javascript | medium | medium | 48661 | 0.054 s | 38.0 MiB | 902628 |
| TanStack/query | typescript-javascript | large | large | 82110 | 0.277 s | 132.5 MiB | 296647 |
| slidevjs/slidev | typescript-javascript | medium | medium | 25212 | 0.026 s | 28.0 MiB | 958067 |
| vuetifyjs/vuetify | typescript-javascript | large | large | 107760 | 0.118 s | 75.1 MiB | 913902 |
| xyflow/xyflow | typescript-javascript | medium | medium | 45226 | 0.047 s | 37.0 MiB | 958184 |
| ueberdosis/tiptap | typescript-javascript | large | large | 89064 | 0.198 s | 113.8 MiB | 449727 |
| directus/directus | typescript-javascript | large | large | 189245 | 0.312 s | 175.8 MiB | 607019 |
| tailwindlabs/headlessui | typescript-javascript | medium | medium | 33429 | 0.146 s | 79.2 MiB | 228557 |
| Molunerfinn/PicGo | typescript-javascript | medium | medium | 34336 | 0.036 s | 37.4 MiB | 941724 |
| lucide-icons/lucide | typescript-javascript | medium | medium | 18541 | 0.065 s | 26.3 MiB | 284288 |
| youzan/vant | typescript-javascript | medium | medium | 41040 | 0.045 s | 37.4 MiB | 903639 |
| vueuse/vueuse | typescript-javascript | medium | medium | 26071 | 0.062 s | 47.2 MiB | 419289 |
| lin-xin/vue-manage-system | typescript-javascript | small | small | 2946 | 0.008 s | 14.7 MiB | 353418 |
| darkroomengineering/lenis | typescript-javascript | small | small | 2821 | 0.008 s | 15.5 MiB | 338140 |
| soybeanjs/soybean-admin | typescript-javascript | small | small | 9454 | 0.016 s | 20.0 MiB | 609416 |
| vuejs/pinia | typescript-javascript | small | small | 6272 | 0.018 s | 22.0 MiB | 353378 |
| formkit/auto-animate | typescript-javascript | small | small | 2822 | 0.010 s | 16.5 MiB | 276130 |
| logaretm/vee-validate | typescript-javascript | small | small | 7510 | 0.028 s | 27.9 MiB | 269054 |

Largest repository: stablyai/orca with 3404433 Production lines, 5.753 s median wall time, and 2094.7 MiB peak RSS.

## Before-after case studies

Each study measures the Production Source set at two pinned commits with the binary above.
The report holds commit references and Metric ratios only, never source excerpts.

### nektos/act 2bb3e74

feat: split job steps into its own files/structs (#1004)

Repository: <https://github.com/nektos/act>

Before: `5d7027dc3fc6a16e2023ffa1ebac93f1c667786e`; after: `2bb3e74616f86ece6a11cd854ad54701542410fd`.

| Metric | Before | After |
| --- | --- | --- |
| Production lines | 6968 | 7219 |
| Erosion | 0.4324 | 0.4118 |
| Verbosity | 0.0019 | 0.0018 |
| Duplication | 0.0412 | 0.0400 |
| Import cycles | 0.0000 | 0.0000 |

### restic/restic 4b70bba

Split Scanner from Archiver, refactor Progress

Repository: <https://github.com/restic/restic>

Before: `a93bc3c9917a12a1051af83911a726c79f309b34`; after: `4b70bba588304ee254a5c580443ada7dd277e1e1`.

| Metric | Before | After |
| --- | --- | --- |
| Production lines | 3837 | 3917 |
| Erosion | 0.4583 | 0.4417 |
| Verbosity | 0.0044 | 0.0038 |
| Duplication | 0.0774 | 0.0758 |
| Import cycles | 0.0000 | 0.0000 |

### avidianity/codeint Workstream 1

Workstream 1 maintainability foundation: single Metric registry, built-in plugins behind the host, and five 1,000-line core files split

Repository: <https://github.com/avidianity/codeint>

Before: `8e3f49d5024ba64c5f93e8f71013a8079a495d80`; after: `9c532f069caa54107675551652c265060fa7e156`.

| Metric | Before | After |
| --- | --- | --- |
| Production lines | 58764 | 59605 |
| Erosion | 0.0733 | 0.0730 |
| Verbosity | 0.0004 | 0.0004 |
| Duplication | 0.0089 | 0.0083 |
| Import cycles | 0.0138 | 0.0000 |

## Repositories

| Repository | Commit | Result |
| --- | --- | --- |
| [morluto/rea](https://github.com/morluto/rea) | `ca7fdb32454c` | PASS (exit code 0) |
| [affaan-m/ECC](https://github.com/affaan-m/ECC) | `4adeee6732a1` | PASS (exit code 0) |
| [deepseek-ai/deepseek-harness](https://github.com/deepseek-ai/deepseek-harness) | `4b399c39a6d1` | PASS (exit code 0) |
| [react/react](https://github.com/react/react) | `92bb0678c521` | PASS (exit code 0) |
| [anomalyco/opencode](https://github.com/anomalyco/opencode) | `5050225a097a` | PASS (exit code 0) |
| [vuejs/vue](https://github.com/vuejs/vue) | `9e8870794008` | PASS (exit code 0) |
| [n8n-io/n8n](https://github.com/n8n-io/n8n) | `87575809a0ca` | PASS (exit code 0) |
| [firecrawl/firecrawl](https://github.com/firecrawl/firecrawl) | `0519b26a2b73` | PASS (exit code 0) |
| [DietrichGebert/ponytail](https://github.com/DietrichGebert/ponytail) | `ebd8654c2c62` | PASS (exit code 0) |
| [microsoft/vscode](https://github.com/microsoft/vscode) | `3e9189b30eda` | PASS (exit code 0) |
| [clash-verge-rev/clash-verge-rev](https://github.com/clash-verge-rev/clash-verge-rev) | `52bb25531c69` | PASS (exit code 0) |
| [anthropics/claude-code](https://github.com/anthropics/claude-code) | `4cc5e8b63a5c` | PASS (exit code 0) |
| [iptv-org/iptv](https://github.com/iptv-org/iptv) | `46c85ffcd6e0` | PASS (exit code 0) |
| [vercel/next.js](https://github.com/vercel/next.js) | `b8e68eb8f87d` | PASS (exit code 0) |
| [garrytan/gstack](https://github.com/garrytan/gstack) | `a5acd49d9e45` | PASS (exit code 0) |
| [stablyai/orca](https://github.com/stablyai/orca) | `cb49083b6506` | PASS (exit code 0) |
| [excalidraw/excalidraw](https://github.com/excalidraw/excalidraw) | `5681e7d8be31` | PASS (exit code 0) |
| [shadcn-ui/ui](https://github.com/shadcn-ui/ui) | `055aa6b7837b` | PASS (exit code 0) |
| [nodejs/node](https://github.com/nodejs/node) | `ffc57f007f60` | PASS (exit code 0) |
| [earendil-works/pi](https://github.com/earendil-works/pi) | `e2cb38ac149b` | PASS (exit code 0) |
| [TauricResearch/TradingAgents](https://github.com/TauricResearch/TradingAgents) | `121265d18d46` | PASS (exit code 0) |
| [Leonxlnx/taste-skill](https://github.com/Leonxlnx/taste-skill) | `d1a2bb41d927` | PASS (exit code 0) |
| [addyosmani/agent-skills](https://github.com/addyosmani/agent-skills) | `e997ac5b9a74` | PASS (exit code 0) |
| [axios/axios](https://github.com/axios/axios) | `a35e6f4ee55d` | PASS (exit code 0) |
| [OpenCut-app/OpenCut](https://github.com/OpenCut-app/OpenCut) | `e66801077856` | PASS (exit code 0) |
| [tailwindlabs/tailwindcss](https://github.com/tailwindlabs/tailwindcss) | `7a4ef9c7ce90` | PASS (exit code 0) |
| [puppeteer/puppeteer](https://github.com/puppeteer/puppeteer) | `f4bf4ac07a3c` | PASS (exit code 0) |
| [louislam/uptime-kuma](https://github.com/louislam/uptime-kuma) | `8c079b8d4fb2` | PASS (exit code 0) |
| [Egonex-AI/Understand-Anything](https://github.com/Egonex-AI/Understand-Anything) | `b232f3564462` | PASS (exit code 0) |
| [modelcontextprotocol/servers](https://github.com/modelcontextprotocol/servers) | `c799c8552b0f` | PASS (exit code 0) |
| [ChatGPTNextWeb/NextChat](https://github.com/ChatGPTNextWeb/NextChat) | `1d3dc3315129` | PASS (exit code 0) |
| [coder/code-server](https://github.com/coder/code-server) | `c094099c1516` | PASS (exit code 0) |
| [anuraghazra/github-readme-stats](https://github.com/anuraghazra/github-readme-stats) | `54a7985aeefd` | PASS (exit code 0) |
| [Fission-AI/OpenSpec](https://github.com/Fission-AI/OpenSpec) | `f1cd852f4edc` | PASS (exit code 0) |
| [Eugeny/tabby](https://github.com/Eugeny/tabby) | `dc72f059410a` | PASS (exit code 0) |
| [expressjs/express](https://github.com/expressjs/express) | `43fca52d28a4` | PASS (exit code 0) |
| [chartjs/Chart.js](https://github.com/chartjs/Chart.js) | `a020fd2018b5` | PASS (exit code 0) |
| [upstash/context7](https://github.com/upstash/context7) | `6335fd8458bb` | PASS (exit code 0) |
| [jgraph/drawio-desktop](https://github.com/jgraph/drawio-desktop) | `5e8406789216` | PASS (exit code 0) |
| [socketio/socket.io](https://github.com/socketio/socket.io) | `c04ce00d380a` | PASS (exit code 0) |
| [coreyhaines31/marketingskills](https://github.com/coreyhaines31/marketingskills) | `8fd2fcade2fd` | PASS (exit code 0) |
| [reduxjs/redux](https://github.com/reduxjs/redux) | `ce8d8362218b` | PASS (exit code 0) |
| [jquery/jquery](https://github.com/jquery/jquery) | `effd615f0009` | PASS (exit code 0) |
| [pmndrs/zustand](https://github.com/pmndrs/zustand) | `5d66d53be803` | PASS (exit code 0) |
| [scutan90/DeepLearning-500-questions](https://github.com/scutan90/DeepLearning-500-questions) | `6087a06b112c` | PASS (exit code 0) |
| [lyswhut/lx-music-desktop](https://github.com/lyswhut/lx-music-desktop) | `323e4bac505d` | PASS (exit code 0) |
| [upscayl/upscayl](https://github.com/upscayl/upscayl) | `a00d55fee90e` | PASS (exit code 0) |
| [prettier/prettier](https://github.com/prettier/prettier) | `4772dcfad899` | PASS (exit code 0) |
| [KeygraphHQ/shannon](https://github.com/KeygraphHQ/shannon) | `bade5321884f` | PASS (exit code 0) |
| [algorithm-visualizer/algorithm-visualizer](https://github.com/algorithm-visualizer/algorithm-visualizer) | `18de2edf6b62` | PASS (exit code 0) |
| [ultraworkers/claw-code](https://github.com/ultraworkers/claw-code) | `ca8752613b3b` | PASS (exit code 0) |
| [farion1231/cc-switch](https://github.com/farion1231/cc-switch) | `99f4bae58531` | PASS (exit code 0) |
| [openai/codex](https://github.com/openai/codex) | `9f3cb259535c` | PASS (exit code 0) |
| [rustdesk/rustdesk](https://github.com/rustdesk/rustdesk) | `a0c0963c483f` | PASS (exit code 0) |
| [pbakaus/impeccable](https://github.com/pbakaus/impeccable) | `9e460a87ef33` | PASS (exit code 0) |
| [tauri-apps/tauri](https://github.com/tauri-apps/tauri) | `3376e61fa85c` | PASS (exit code 0) |
| [denoland/deno](https://github.com/denoland/deno) | `d8c551b5a5b4` | PASS (exit code 0) |
| [ruvnet/RuView](https://github.com/ruvnet/RuView) | `8139c7e6258a` | PASS (exit code 0) |
| [zed-industries/zed](https://github.com/zed-industries/zed) | `e84c6c61adb5` | PASS (exit code 0) |
| [astral-sh/uv](https://github.com/astral-sh/uv) | `e3e7c1eacb98` | PASS (exit code 0) |
| [rtk-ai/rtk](https://github.com/rtk-ai/rtk) | `97e68d5358aa` | PASS (exit code 0) |
| [storytold/photocraft](https://github.com/storytold/photocraft) | `e22facf45b0e` | PASS (exit code 0) |
| [unionlabs/union](https://github.com/unionlabs/union) | `031785bb6dc6` | PASS (exit code 0) |
| [dani-garcia/vaultwarden](https://github.com/dani-garcia/vaultwarden) | `dbd7829ed140` | PASS (exit code 0) |
| [BurntSushi/ripgrep](https://github.com/BurntSushi/ripgrep) | `3e85c6c148fd` | PASS (exit code 0) |
| [LadybirdBrowser/ladybird](https://github.com/LadybirdBrowser/ladybird) | `b1e03741225d` | PASS (exit code 0) |
| [openinterpreter/openinterpreter](https://github.com/openinterpreter/openinterpreter) | `f1701ae772bf` | PASS (exit code 0) |
| [alacritty/alacritty](https://github.com/alacritty/alacritty) | `791efff6d74c` | PASS (exit code 0) |
| [rust-lang/rustlings](https://github.com/rust-lang/rustlings) | `f7b9b3c5882a` | PASS (exit code 0) |
| [tw93/Pake](https://github.com/tw93/Pake) | `17b7b7418d06` | PASS (exit code 0) |
| [starship/starship](https://github.com/starship/starship) | `1b5c2f257ef2` | PASS (exit code 0) |
| [sharkdp/fd](https://github.com/sharkdp/fd) | `2e8648f10fe5` | PASS (exit code 0) |
| [GyulyVGC/sniffnet](https://github.com/GyulyVGC/sniffnet) | `247b75b4f82e` | PASS (exit code 0) |
| [FuelLabs/fuels-rs](https://github.com/FuelLabs/fuels-rs) | `f5191b6aab3d` | PASS (exit code 0) |
| [cjpais/Handy](https://github.com/cjpais/Handy) | `bdb68c85730a` | PASS (exit code 0) |
| [AlexsJones/llmfit](https://github.com/AlexsJones/llmfit) | `6bc8f67890c1` | PASS (exit code 0) |
| [casey/just](https://github.com/casey/just) | `62784c2a6e16` | PASS (exit code 0) |
| [TabbyML/tabby](https://github.com/TabbyML/tabby) | `21b29048d7bc` | PASS (exit code 0) |
| [Zackriya-Solutions/meetily](https://github.com/Zackriya-Solutions/meetily) | `2166bc6fe3ba` | PASS (exit code 0) |
| [ankitects/anki](https://github.com/ankitects/anki) | `149b19014212` | PASS (exit code 0) |
| [yewstack/yew](https://github.com/yewstack/yew) | `5ee55894001d` | PASS (exit code 0) |
| [dandavison/delta](https://github.com/dandavison/delta) | `ad9986cf9ddd` | PASS (exit code 0) |
| [googleworkspace/cli](https://github.com/googleworkspace/cli) | `8e945eed02a8` | PASS (exit code 0) |
| [sharkdp/hyperfine](https://github.com/sharkdp/hyperfine) | `c51615249805` | PASS (exit code 0) |
| [cloudflare/pingora](https://github.com/cloudflare/pingora) | `4aa2e5ab612c` | PASS (exit code 0) |
| [tokio-rs/axum](https://github.com/tokio-rs/axum) | `8d8944e6db1a` | PASS (exit code 0) |
| [tree-sitter/tree-sitter](https://github.com/tree-sitter/tree-sitter) | `127254c1d6f8` | PASS (exit code 0) |
| [Schniz/fnm](https://github.com/Schniz/fnm) | `07f13237ae20` | PASS (exit code 0) |
| [Wilfred/difftastic](https://github.com/Wilfred/difftastic) | `4b834ef576fa` | PASS (exit code 0) |
| [TheAlgorithms/Rust](https://github.com/TheAlgorithms/Rust) | `fed66cb0c74c` | PASS (exit code 0) |
| [ogham/exa](https://github.com/ogham/exa) | `3d1edbb47052` | PASS (exit code 0) |
| [fathyb/carbonyl](https://github.com/fathyb/carbonyl) | `ab80a276b1bd` | PASS (exit code 0) |
| [QuipNetwork/hashsigs-rs](https://github.com/QuipNetwork/hashsigs-rs) | `2aee878ffbe5` | PASS (exit code 0) |
| [seanmonstar/warp](https://github.com/seanmonstar/warp) | `1fdc5eebfc6f` | PASS (exit code 0) |
| [actix/actix](https://github.com/actix/actix) | `4fed9792ae78` | PASS (exit code 0) |
| [tokio-rs/mio](https://github.com/tokio-rs/mio) | `101e5b7c92b1` | PASS (exit code 0) |
| [dtolnay/anyhow](https://github.com/dtolnay/anyhow) | `c89c9b82b155` | PASS (exit code 0) |
| [emmett-framework/granian](https://github.com/emmett-framework/granian) | `0aa7202723ba` | PASS (exit code 0) |
| [dtolnay/thiserror](https://github.com/dtolnay/thiserror) | `aa7e8161d55a` | PASS (exit code 0) |
| [autumnai/leaf](https://github.com/autumnai/leaf) | `858527fa6e2a` | PASS (exit code 0) |
| [coollabsio/coolify](https://github.com/coollabsio/coolify) | `b164dfb9d668` | PASS (exit code 0) |
| [nextcloud/server](https://github.com/nextcloud/server) | `c6ce5a965ee6` | PASS (exit code 0) |
| [laravel/framework](https://github.com/laravel/framework) | `64de808fe0f4` | PASS (exit code 0) |
| [filamentphp/filament](https://github.com/filamentphp/filament) | `a14f2fb477ae` | PASS (exit code 0) |
| [symfony/symfony](https://github.com/symfony/symfony) | `4be5be214758` | PASS (exit code 0) |
| [composer/composer](https://github.com/composer/composer) | `c4899ab08f9e` | PASS (exit code 0) |
| [bagisto/bagisto](https://github.com/bagisto/bagisto) | `708eee8aa206` | PASS (exit code 0) |
| [monicahq/monica](https://github.com/monicahq/monica) | `2b34248b6192` | PASS (exit code 0) |
| [firefly-iii/firefly-iii](https://github.com/firefly-iii/firefly-iii) | `3bba58800a08` | PASS (exit code 0) |
| [krayin/laravel-crm](https://github.com/krayin/laravel-crm) | `64ef70505c5d` | PASS (exit code 0) |
| [livewire/livewire](https://github.com/livewire/livewire) | `0b8f03c9b7f5` | FAIL (exit code 0) |
| [guzzle/guzzle](https://github.com/guzzle/guzzle) | `ba6574fd975a` | PASS (exit code 0) |
| [PHPMailer/PHPMailer](https://github.com/PHPMailer/PHPMailer) | `a3689ab9ab48` | PASS (exit code 0) |
| [DesignPatternsPHP/DesignPatternsPHP](https://github.com/DesignPatternsPHP/DesignPatternsPHP) | `54254e0f2a59` | PASS (exit code 0) |
| [matomo-org/matomo](https://github.com/matomo-org/matomo) | `d9d53e00f858` | PASS (exit code 0) |
| [Seldaek/monolog](https://github.com/Seldaek/monolog) | `6d4cf3a1199b` | PASS (exit code 0) |
| [sebastianbergmann/phpunit](https://github.com/sebastianbergmann/phpunit) | `a816bd15cfda` | PASS (exit code 0) |
| [fruitcake/laravel-debugbar](https://github.com/fruitcake/laravel-debugbar) | `033b34a64489` | FAIL (exit code 0) |
| [BookStackApp/BookStack](https://github.com/BookStackApp/BookStack) | `2513cd852af0` | PASS (exit code 0) |
| [bcit-ci/CodeIgniter](https://github.com/bcit-ci/CodeIgniter) | `3658d731eaab` | PASS (exit code 0) |
| [nikic/PHP-Parser](https://github.com/nikic/PHP-Parser) | `24c72b4d8413` | PASS (exit code 0) |
| [koel/koel](https://github.com/koel/koel) | `f0d5bc79d308` | PASS (exit code 0) |
| [briannesbitt/Carbon](https://github.com/briannesbitt/Carbon) | `4c5056ed90f6` | PASS (exit code 0) |
| [getgrav/grav](https://github.com/getgrav/grav) | `190ad5899be4` | PASS (exit code 0) |
| [grokability/snipe-it](https://github.com/grokability/snipe-it) | `5a7b60cfaad9` | PASS (exit code 0) |
| [barryvdh/laravel-ide-helper](https://github.com/barryvdh/laravel-ide-helper) | `66f7ec11122b` | PASS (exit code 0) |
| [Intervention/image](https://github.com/Intervention/image) | `4cb9b7008d67` | PASS (exit code 0) |
| [phpstan/phpstan](https://github.com/phpstan/phpstan) | `6eb49ccb9a53` | PASS (exit code 0) |
| [digininja/DVWA](https://github.com/digininja/DVWA) | `502ac150ce37` | PASS (exit code 0) |
| [thephpleague/flysystem](https://github.com/thephpleague/flysystem) | `12930254888f` | PASS (exit code 0) |
| [filp/whoops](https://github.com/filp/whoops) | `d5480d53f100` | PASS (exit code 0) |
| [wallabag/wallabag](https://github.com/wallabag/wallabag) | `066328363ff3` | PASS (exit code 0) |
| [spatie/laravel-permission](https://github.com/spatie/laravel-permission) | `0bd8ab977e53` | PASS (exit code 0) |
| [SpartnerNL/Laravel-Excel](https://github.com/SpartnerNL/Laravel-Excel) | `ba07e2d82e38` | PASS (exit code 0) |
| [ramsey/uuid](https://github.com/ramsey/uuid) | `7a16331da5bb` | PASS (exit code 0) |
| [typecho/typecho](https://github.com/typecho/typecho) | `1e53c04e1ef9` | PASS (exit code 0) |
| [slimphp/Slim](https://github.com/slimphp/Slim) | `e257f61f94a4` | PASS (exit code 0) |
| [pestphp/pest](https://github.com/pestphp/pest) | `0fe7c2440cd4` | PASS (exit code 0) |
| [egulias/EmailValidator](https://github.com/egulias/EmailValidator) | `9bef884e1227` | PASS (exit code 0) |
| [walkor/workerman](https://github.com/walkor/workerman) | `26b96338cf3f` | PASS (exit code 0) |
| [tymondesigns/jwt-auth](https://github.com/tymondesigns/jwt-auth) | `99e8f24cf96b` | PASS (exit code 0) |
| [dompdf/dompdf](https://github.com/dompdf/dompdf) | `c41f2f4871f1` | FAIL (exit code 0) |
| [z-song/laravel-admin](https://github.com/z-song/laravel-admin) | `67c441eb78ec` | FAIL (exit code 0) |
| [deployphp/deployer](https://github.com/deployphp/deployer) | `c08d1367d8f1` | FAIL (exit code 0) |
| [nextcloud/all-in-one](https://github.com/nextcloud/all-in-one) | `92d021d521f3` | PASS (exit code 0) |
| [squizlabs/PHP_CodeSniffer](https://github.com/squizlabs/PHP_CodeSniffer) | `c6c65ca0dc86` | FAIL (exit code 0) |
| [Anankke/SSPanel-UIM](https://github.com/Anankke/SSPanel-UIM) | `e90b9a5fed1a` | PASS (exit code 0) |
| [doctrine/orm](https://github.com/doctrine/orm) | `73bb4882e13d` | PASS (exit code 0) |
| [bobthecow/psysh](https://github.com/bobthecow/psysh) | `64b18aa0d5f8` | PASS (exit code 0) |
| [symfony/console](https://github.com/symfony/console) | `ea1860277087` | FAIL (exit code 0) |
| [flutter/flutter](https://github.com/flutter/flutter) | `06f1c24345a2` | PASS (exit code 0) |
| [AppFlowy-IO/AppFlowy](https://github.com/AppFlowy-IO/AppFlowy) | `8fcee27d9bd9` | PASS (exit code 0) |
| [chen08209/FlClash](https://github.com/chen08209/FlClash) | `afe713600227` | PASS (exit code 0) |
| [Predidit/Kazumi](https://github.com/Predidit/Kazumi) | `3ae13bfba2cc` | PASS (exit code 0) |
| [hiddify/hiddify-app](https://github.com/hiddify/hiddify-app) | `66c17f83a552` | PASS (exit code 0) |
| [ccch1mneyyy/dsh-Go](https://github.com/ccch1mneyyy/dsh-Go) | `f2d589c4a7f4` | PASS (exit code 0) |
| [ente/ente](https://github.com/ente/ente) | `6a7fa5ffa793` | PASS (exit code 0) |
| [GopeedLab/gopeed](https://github.com/GopeedLab/gopeed) | `bd0ac209bddb` | PASS (exit code 0) |
| [alibaba/flutter-go](https://github.com/alibaba/flutter-go) | `a41b2124278e` | PASS (exit code 0) |
| [mitesh77/Best-Flutter-UI-Templates](https://github.com/mitesh77/Best-Flutter-UI-Templates) | `c85450cb4367` | PASS (exit code 0) |
| [ImranR98/Obtainium](https://github.com/ImranR98/Obtainium) | `c570dccb097e` | PASS (exit code 0) |
| [getlantern/lantern](https://github.com/getlantern/lantern) | `5fafbcaa9e8d` | PASS (exit code 0) |
| [KaringX/karing](https://github.com/KaringX/karing) | `73b7cfa59222` | PASS (exit code 0) |
| [xiaoyaocz/dart_simple_live](https://github.com/xiaoyaocz/dart_simple_live) | `ba828e6783b1` | PASS (exit code 0) |
| [bagisto/opensource-ecommerce-mobile-app](https://github.com/bagisto/opensource-ecommerce-mobile-app) | `500bd7e28792` | PASS (exit code 0) |
| [CarGuo/gsy_github_app_flutter](https://github.com/CarGuo/gsy_github_app_flutter) | `f5e6565a60be` | PASS (exit code 0) |
| [wanghongenpin/proxypin](https://github.com/wanghongenpin/proxypin) | `ff0e63b272ac` | PASS (exit code 0) |
| [Notsfsssf/pixez-flutter](https://github.com/Notsfsssf/pixez-flutter) | `7db05ce25f09` | PASS (exit code 0) |
| [cfug/dio](https://github.com/cfug/dio) | `f1bef462289b` | PASS (exit code 0) |
| [felangel/bloc](https://github.com/felangel/bloc) | `654f15a566fe` | PASS (exit code 0) |
| [jonataslaw/getx](https://github.com/jonataslaw/getx) | `a294d2fbfc65` | PASS (exit code 0) |
| [flame-engine/flame](https://github.com/flame-engine/flame) | `d185bf9aa161` | PASS (exit code 0) |
| [KaringX/clashmi](https://github.com/KaringX/clashmi) | `62c15a2fb34f` | PASS (exit code 0) |
| [firebase/flutterfire](https://github.com/firebase/flutterfire) | `1e500b3a39ae` | PASS (exit code 0) |
| [lollipopkit/flutter_server_box](https://github.com/lollipopkit/flutter_server_box) | `cca102a0a82e` | PASS (exit code 0) |
| [brianegan/flutter_architecture_samples](https://github.com/brianegan/flutter_architecture_samples) | `d898d1329e04` | PASS (exit code 0) |
| [toly1994328/FlutterUnit](https://github.com/toly1994328/FlutterUnit) | `ec1effa777f0` | PASS (exit code 0) |
| [simplezhli/flutter_deer](https://github.com/simplezhli/flutter_deer) | `7b86fd00560c` | PASS (exit code 0) |
| [Sle2p/AniCh](https://github.com/Sle2p/AniCh) | `c5729a28af23` | PASS (exit code 0) |
| [ComicSparks/pikapika](https://github.com/ComicSparks/pikapika) | `a9d98a37249b` | PASS (exit code 0) |
| [mornyeapp/Mornye](https://github.com/mornyeapp/Mornye) | `af9a4d642275` | PASS (exit code 0) |
| [alibaba/fish-redux](https://github.com/alibaba/fish-redux) | `40b853bfa084` | PASS (exit code 0) |
| [OpenFlutter/Flutter-Notebook](https://github.com/OpenFlutter/Flutter-Notebook) | `1eab75edb34e` | PASS (exit code 0) |
| [nisrulz/flutter-examples](https://github.com/nisrulz/flutter-examples) | `9d67d31c72ce` | PASS (exit code 0) |
| [mylxsw/aidea](https://github.com/mylxsw/aidea) | `cea454f7b48a` | PASS (exit code 0) |
| [vandadnp/flutter-tips-and-tricks](https://github.com/vandadnp/flutter-tips-and-tricks) | `cdc7891b229f` | PASS (exit code 0) |
| [2d-inc/HistoryOfEverything](https://github.com/2d-inc/HistoryOfEverything) | `1864cfb7f2e6` | PASS (exit code 0) |
| [iampawan/Flutter-UI-Kit](https://github.com/iampawan/Flutter-UI-Kit) | `04dc321f7a6e` | PASS (exit code 0) |
| [samarthagarwal/FlutterScreens](https://github.com/samarthagarwal/FlutterScreens) | `8afc1abba2be` | PASS (exit code 0) |
| [flutterchina/flukit](https://github.com/flutterchina/flukit) | `862e033c33f1` | PASS (exit code 0) |
| [miru-project/miru-app](https://github.com/miru-project/miru-app) | `8599d0dbfbb1` | PASS (exit code 0) |
| [Sky24n/flutter_wanandroid](https://github.com/Sky24n/flutter_wanandroid) | `51061b34a492` | PASS (exit code 0) |
| [ComicSparks/jasmine](https://github.com/ComicSparks/jasmine) | `8060551d3441` | PASS (exit code 0) |
| [btwld/fvm](https://github.com/btwld/fvm) | `6d8104b334aa` | PASS (exit code 0) |
| [ionicfirebaseapp/getwidget](https://github.com/ionicfirebaseapp/getwidget) | `2bcf1e05d258` | PASS (exit code 0) |
| [xuelongqy/flutter_easy_refresh](https://github.com/xuelongqy/flutter_easy_refresh) | `dcdbb8143e12` | PASS (exit code 0) |
| [best-flutter/flutter_swiper](https://github.com/best-flutter/flutter_swiper) | `71eb23671d46` | PASS (exit code 0) |
| [ReactiveX/rxdart](https://github.com/ReactiveX/rxdart) | `d1f3ba64f716` | PASS (exit code 0) |
| [rrousselGit/flutter_hooks](https://github.com/rrousselGit/flutter_hooks) | `1c47821832bd` | FAIL (exit code 0) |
| [OpenFlutter/fluwx](https://github.com/OpenFlutter/fluwx) | `8334f3f2e8f4` | FAIL (exit code 0) |
| [NousResearch/hermes-agent](https://github.com/NousResearch/hermes-agent) | `f681186ea35e` | PASS (exit code 0) |
| [yt-dlp/yt-dlp](https://github.com/yt-dlp/yt-dlp) | `da452a569fb3` | PASS (exit code 0) |
| [anthropics/skills](https://github.com/anthropics/skills) | `e4bb5d960d87` | PASS (exit code 0) |
| [microsoft/markitdown](https://github.com/microsoft/markitdown) | `ed178952a667` | PASS (exit code 0) |
| [Significant-Gravitas/AutoGPT](https://github.com/Significant-Gravitas/AutoGPT) | `dd0b5f73266e` | PASS (exit code 0) |
| [huggingface/transformers](https://github.com/huggingface/transformers) | `9d5d0ee866d7` | PASS (exit code 0) |
| [AUTOMATIC1111/stable-diffusion-webui](https://github.com/AUTOMATIC1111/stable-diffusion-webui) | `82a973c04367` | PASS (exit code 0) |
| [langgenius/dify](https://github.com/langgenius/dify) | `9d920ce8bb1f` | PASS (exit code 0) |
| [langflow-ai/langflow](https://github.com/langflow-ai/langflow) | `6c9aea6f5d5f` | PASS (exit code 0) |
| [nextlevelbuilder/ui-ux-pro-max-skill](https://github.com/nextlevelbuilder/ui-ux-pro-max-skill) | `8db36338810d` | PASS (exit code 0) |
| [Graphify-Labs/graphify](https://github.com/Graphify-Labs/graphify) | `14f35e381cab` | PASS (exit code 0) |
| [github/spec-kit](https://github.com/github/spec-kit) | `f61648db96b5` | PASS (exit code 0) |
| [langchain-ai/langchain](https://github.com/langchain-ai/langchain) | `87ad2d71e135` | PASS (exit code 0) |
| [Comfy-Org/ComfyUI](https://github.com/Comfy-Org/ComfyUI) | `b27006af3d75` | PASS (exit code 0) |
| [Shubhamsaboo/awesome-llm-apps](https://github.com/Shubhamsaboo/awesome-llm-apps) | `14c5f8ff0094` | PASS (exit code 0) |
| [harry0703/MoneyPrinterTurbo](https://github.com/harry0703/MoneyPrinterTurbo) | `2dc10e850d78` | PASS (exit code 0) |
| [ytdl-org/youtube-dl](https://github.com/ytdl-org/youtube-dl) | `bc069d2bb40b` | PASS (exit code 0) |
| [browser-use/browser-use](https://github.com/browser-use/browser-use) | `94784c1f7ecb` | PASS (exit code 0) |
| [debpalash/VoiceStudio](https://github.com/debpalash/VoiceStudio) | `6f981a68ecd0` | PASS (exit code 0) |
| [Panniantong/Agent-Reach](https://github.com/Panniantong/Agent-Reach) | `8d9ecc1f5521` | PASS (exit code 0) |
| [openai/whisper](https://github.com/openai/whisper) | `86098128c0b4` | PASS (exit code 0) |
| [pytorch/pytorch](https://github.com/pytorch/pytorch) | `c6e0b2ad094d` | PASS (exit code 0) |
| [D4Vinci/Scrapling](https://github.com/D4Vinci/Scrapling) | `938867232230` | PASS (exit code 0) |
| [fastapi/fastapi](https://github.com/fastapi/fastapi) | `05e6df7e906c` | PASS (exit code 0) |
| [hacksider/Deep-Live-Cam](https://github.com/hacksider/Deep-Live-Cam) | `08896a51c9a6` | PASS (exit code 0) |
| [nvbn/thefuck](https://github.com/nvbn/thefuck) | `c7e7e1d884d3` | PASS (exit code 0) |
| [vllm-project/vllm](https://github.com/vllm-project/vllm) | `689b84c36976` | PASS (exit code 0) |
| [3b1b/manim](https://github.com/3b1b/manim) | `fafa083a4fb2` | PASS (exit code 0) |
| [666ghj/MiroFish](https://github.com/666ghj/MiroFish) | `7657031ac011` | PASS (exit code 0) |
| [Z4nzu/hackingtool](https://github.com/Z4nzu/hackingtool) | `c3a858990f1f` | PASS (exit code 0) |
| [abi/screenshot-to-code](https://github.com/abi/screenshot-to-code) | `f2e5bbdd1dd5` | PASS (exit code 0) |
| [usestrix/strix](https://github.com/usestrix/strix) | `935f82350782` | PASS (exit code 0) |
| [hiyouga/LlamaFactory](https://github.com/hiyouga/LlamaFactory) | `aef9c06ff4e2` | PASS (exit code 0) |
| [pallets/flask](https://github.com/pallets/flask) | `49943b5a9441` | PASS (exit code 0) |
| [ai-job-search-dev/ai-job-search](https://github.com/ai-job-search-dev/ai-job-search) | `4e021e52f7e3` | PASS (exit code 0) |
| [asgeirtj/system_prompts_leaks](https://github.com/asgeirtj/system_prompts_leaks) | `74761cc521a4` | PASS (exit code 0) |
| [NanmiCoder/MediaCrawler](https://github.com/NanmiCoder/MediaCrawler) | `f871fb42739d` | PASS (exit code 0) |
| [binary-husky/gpt_academic](https://github.com/binary-husky/gpt_academic) | `d6bde0fa5437` | PASS (exit code 0) |
| [jingyaogong/minimind](https://github.com/jingyaogong/minimind) | `858008eb98da` | PASS (exit code 0) |
| [scrapy/scrapy](https://github.com/scrapy/scrapy) | `531bc3299847` | PASS (exit code 0) |
| [virattt/ai-hedge-fund](https://github.com/virattt/ai-hedge-fund) | `5c553b522204` | PASS (exit code 0) |
| [bilibili/GPT-SoVITS](https://github.com/bilibili/GPT-SoVITS) | `6d51ffceafbf` | PASS (exit code 0) |
| [commaai/openpilot](https://github.com/commaai/openpilot) | `e5f93ab0aa51` | PASS (exit code 0) |
| [jamiepine/voicebox](https://github.com/jamiepine/voicebox) | `8dc2a5bfe0f7` | PASS (exit code 0) |
| [karpathy/nanochat](https://github.com/karpathy/nanochat) | `92d63d4e8bb4` | PASS (exit code 0) |
| [CorentinJ/Real-Time-Voice-Cloning](https://github.com/CorentinJ/Real-Time-Voice-Cloning) | `890f3a031871` | PASS (exit code 0) |
| [FoundationAgents/OpenManus](https://github.com/FoundationAgents/OpenManus) | `3309bf4e416f` | PASS (exit code 0) |
| [Zie619/n8n-workflows](https://github.com/Zie619/n8n-workflows) | `94007c1445d9` | PASS (exit code 0) |
| [psf/requests](https://github.com/psf/requests) | `3c7baaa05ab4` | PASS (exit code 0) |
| [hiroi-sora/Umi-OCR](https://github.com/hiroi-sora/Umi-OCR) | `83173efc4f44` | PASS (exit code 0) |
| [ollama/ollama](https://github.com/ollama/ollama) | `924cc286cc59` | PASS (exit code 0) |
| [JuliusBrussee/caveman](https://github.com/JuliusBrussee/caveman) | `47c61be2d10a` | PASS (exit code 0) |
| [microsoft/TypeScript](https://github.com/microsoft/TypeScript) | `502a981024b7` | PASS (exit code 0) |
| [fatedier/frp](https://github.com/fatedier/frp) | `a2c975efb5e5` | PASS (exit code 0) |
| [infiniflow/ragflow](https://github.com/infiniflow/ragflow) | `bb7b3c28888c` | PASS (exit code 0) |
| [syncthing/syncthing](https://github.com/syncthing/syncthing) | `134b625bcb0f` | PASS (exit code 0) |
| [gin-gonic/gin](https://github.com/gin-gonic/gin) | `0f09c3a9b462` | PASS (exit code 0) |
| [junegunn/fzf](https://github.com/junegunn/fzf) | `ed173e71e44b` | PASS (exit code 0) |
| [netdata/netdata](https://github.com/netdata/netdata) | `fd8102ea144f` | PASS (exit code 0) |
| [caddyserver/caddy](https://github.com/caddyserver/caddy) | `679806f92d49` | PASS (exit code 0) |
| [grafana/grafana](https://github.com/grafana/grafana) | `c5f86d0464a1` | PASS (exit code 0) |
| [nektos/act](https://github.com/nektos/act) | `ac4c09993f47` | PASS (exit code 0) |
| [prometheus/prometheus](https://github.com/prometheus/prometheus) | `e56aabdcbec6` | PASS (exit code 0) |
| [traefik/traefik](https://github.com/traefik/traefik) | `8fcaa7cda47c` | PASS (exit code 0) |
| [usememos/memos](https://github.com/usememos/memos) | `44d6c4f38b59` | PASS (exit code 0) |
| [rclone/rclone](https://github.com/rclone/rclone) | `554ddb5de24f` | PASS (exit code 0) |
| [router-for-me/CLIProxyAPI](https://github.com/router-for-me/CLIProxyAPI) | `dd543d090765` | PASS (exit code 0) |
| [go-gitea/gitea](https://github.com/go-gitea/gitea) | `a46dea66514b` | PASS (exit code 0) |
| [multica-ai/multica](https://github.com/multica-ai/multica) | `f9b3b4b18e52` | PASS (exit code 0) |
| [alibaba/open-code-review](https://github.com/alibaba/open-code-review) | `7ea9c5280655` | PASS (exit code 0) |
| [wagoodman/dive](https://github.com/wagoodman/dive) | `75dea9077a4f` | PASS (exit code 0) |
| [etcd-io/etcd](https://github.com/etcd-io/etcd) | `4e090c033abc` | PASS (exit code 0) |
| [v2ray/v2ray-core](https://github.com/v2ray/v2ray-core) | `d80440f3d57b` | PASS (exit code 0) |
| [charmbracelet/bubbletea](https://github.com/charmbracelet/bubbletea) | `4d611720dd2b` | PASS (exit code 0) |
| [spf13/cobra](https://github.com/spf13/cobra) | `adbc8813901b` | PASS (exit code 0) |
| [danielmiessler/Fabric](https://github.com/danielmiessler/Fabric) | `30b83e996cd1` | PASS (exit code 0) |
| [astaxie/build-web-application-with-golang](https://github.com/astaxie/build-web-application-with-golang) | `c294b087b96d` | PASS (exit code 0) |
| [schollz/croc](https://github.com/schollz/croc) | `d77d59a8931d` | PASS (exit code 0) |
| [gofiber/fiber](https://github.com/gofiber/fiber) | `042f553c55b1` | PASS (exit code 0) |
| [go-gorm/gorm](https://github.com/go-gorm/gorm) | `ead4e04ba7c6` | PASS (exit code 0) |
| [docker/compose](https://github.com/docker/compose) | `13471d2d5e11` | PASS (exit code 0) |
| [glanceapp/glance](https://github.com/glanceapp/glance) | `53aec7f392ca` | PASS (exit code 0) |
| [IceWhaleTech/CasaOS](https://github.com/IceWhaleTech/CasaOS) | `e099dd6b4982` | PASS (exit code 0) |
| [restic/restic](https://github.com/restic/restic) | `f25ec9a5ab18` | PASS (exit code 0) |
| [binwiederhier/ntfy](https://github.com/binwiederhier/ntfy) | `aa97efef7e35` | PASS (exit code 0) |
| [derailed/k9s](https://github.com/derailed/k9s) | `c732946a6da3` | PASS (exit code 0) |
| [k3s-io/k3s](https://github.com/k3s-io/k3s) | `a5327bede24a` | PASS (exit code 0) |
| [ehang-io/nps](https://github.com/ehang-io/nps) | `ab648d6f0c61` | PASS (exit code 0) |
| [labstack/echo](https://github.com/labstack/echo) | `59aacddc4a13` | PASS (exit code 0) |
| [beego/beego](https://github.com/beego/beego) | `939cfde380bb` | PASS (exit code 0) |
| [dokku/dokku](https://github.com/dokku/dokku) | `a31ba251c1d4` | PASS (exit code 0) |
| [iawia002/lux](https://github.com/iawia002/lux) | `dd00f6d258d8` | PASS (exit code 0) |
| [abiosoft/colima](https://github.com/abiosoft/colima) | `49dfe87aecc1` | PASS (exit code 0) |
| [gitleaks/gitleaks](https://github.com/gitleaks/gitleaks) | `5f6ec56cfa6f` | PASS (exit code 0) |
| [chubin/wttr.in](https://github.com/chubin/wttr.in) | `8ff2ef6053ef` | PASS (exit code 0) |
| [spf13/viper](https://github.com/spf13/viper) | `528f7416c4b5` | PASS (exit code 0) |
| [charmbracelet/glow](https://github.com/charmbracelet/glow) | `8b0600fa4455` | PASS (exit code 0) |
| [go-kit/kit](https://github.com/go-kit/kit) | `78fbbceece7b` | PASS (exit code 0) |
| [sirupsen/logrus](https://github.com/sirupsen/logrus) | `954b418844d9` | PASS (exit code 0) |
| [gorilla/websocket](https://github.com/gorilla/websocket) | `e064f32e3674` | PASS (exit code 0) |
| [pgmq/pgmq](https://github.com/pgmq/pgmq) | `51468f6ba2af` | PASS (exit code 0) |
| [datacharmer/test_db](https://github.com/datacharmer/test_db) | `e324b56193ca` | PASS (exit code 0) |
| [BrentOzarULTD/SQL-Server-First-Responder-Kit](https://github.com/BrentOzarULTD/SQL-Server-First-Responder-Kit) | `5d3a6811a44e` | PASS (exit code 0) |
| [olahallengren/sql-server-maintenance-solution](https://github.com/olahallengren/sql-server-maintenance-solution) | `41617578643e` | PASS (exit code 0) |
| [openmaptiles/openmaptiles](https://github.com/openmaptiles/openmaptiles) | `6c11838d3803` | PASS (exit code 0) |
| [pgpartman/pg_partman](https://github.com/pgpartman/pg_partman) | `e0a6c384a6c7` | PASS (exit code 0) |
| [lerocha/chinook-database](https://github.com/lerocha/chinook-database) | `7f67772503d7` | PASS (exit code 0) |
| [darold/pgFormatter](https://github.com/darold/pgFormatter) | `3aab449352e0` | PASS (exit code 0) |
| [NikolayS/PgQue](https://github.com/NikolayS/PgQue) | `798d6ebe84fb` | PASS (exit code 0) |
| [NixOS/hydra](https://github.com/NixOS/hydra) | `e181b587af32` | PASS (exit code 0) |
| [amachanic/sp_whoisactive](https://github.com/amachanic/sp_whoisactive) | `40ac17b8e61a` | PASS (exit code 0) |
| [microsoft/fhir-server](https://github.com/microsoft/fhir-server) | `8077ac83283a` | PASS (exit code 0) |
| [cahyadsn/wilayah](https://github.com/cahyadsn/wilayah) | `c2eff0881e3c` | PASS (exit code 0) |
| [NikolayS/postgres_dba](https://github.com/NikolayS/postgres_dba) | `d5c63fd11784` | PASS (exit code 0) |
| [theory/pgtap](https://github.com/theory/pgtap) | `21ceed193bf3` | PASS (exit code 0) |
| [aquametalabs/aquameta](https://github.com/aquametalabs/aquameta) | `ec5dbe6b84ea` | PASS (exit code 0) |
| [mqttsnet/thinglinks](https://github.com/mqttsnet/thinglinks) | `3705ee1564fa` | PASS (exit code 0) |
| [oracle-samples/db-sample-schemas](https://github.com/oracle-samples/db-sample-schemas) | `6660bad68c07` | PASS (exit code 0) |
| [smpetersgithub/AdvancedSQLPuzzles](https://github.com/smpetersgithub/AdvancedSQLPuzzles) | `4fe0cac6e23f` | PASS (exit code 0) |
| [chinnonsantos/sql-paises-estados-cidades](https://github.com/chinnonsantos/sql-paises-estados-cidades) | `a705674ebf1e` | PASS (exit code 0) |
| [anthonydb/practical-sql-2](https://github.com/anthonydb/practical-sql-2) | `e4e826fa03e7` | PASS (exit code 0) |
| [rin-nas/postgresql-patterns-library](https://github.com/rin-nas/postgresql-patterns-library) | `657ba2af7b08` | PASS (exit code 0) |
| [tanelpoder/tpt-oracle](https://github.com/tanelpoder/tpt-oracle) | `45f0a4e15502` | PASS (exit code 0) |
| [erikdarlingdata/DarlingData](https://github.com/erikdarlingdata/DarlingData) | `bf5a0666c2b5` | PASS (exit code 0) |
| [anthonydb/practical-sql](https://github.com/anthonydb/practical-sql) | `5ba4cf652522` | PASS (exit code 0) |
| [Ysurac/FlightAirMap](https://github.com/Ysurac/FlightAirMap) | `7083d7a036ba` | PASS (exit code 0) |
| [utPLSQL/utPLSQL](https://github.com/utPLSQL/utPLSQL) | `01a89eae1b35` | PASS (exit code 0) |
| [trickerer/Trinity-Bots](https://github.com/trickerer/Trinity-Bots) | `a2a13b24d47b` | PASS (exit code 0) |
| [morenoh149/postgresDBSamples](https://github.com/morenoh149/postgresDBSamples) | `2bdc95397620` | PASS (exit code 0) |
| [jOOQ/sakila](https://github.com/jOOQ/sakila) | `e089a5b1ec9a` | PASS (exit code 0) |
| [microsoft/Windows-appsample-customers-orders-database](https://github.com/microsoft/Windows-appsample-customers-orders-database) | `247226c93728` | PASS (exit code 0) |
| [dbkangaroo/kangaroo](https://github.com/dbkangaroo/kangaroo) | `79018b4200ff` | PASS (exit code 0) |
| [marcingminski/sqlwatch](https://github.com/marcingminski/sqlwatch) | `7273c6cba78c` | PASS (exit code 0) |
| [lorint/AdventureWorks-for-Postgres](https://github.com/lorint/AdventureWorks-for-Postgres) | `b474991f0df1` | PASS (exit code 0) |
| [3dcitydb/3dcitydb](https://github.com/3dcitydb/3dcitydb) | `208ad07c9f0f` | PASS (exit code 0) |
| [geocompass/pg-coordtransform](https://github.com/geocompass/pg-coordtransform) | `e5cf4ab0aea0` | PASS (exit code 0) |
| [point-source/supabase-tenant-rbac](https://github.com/point-source/supabase-tenant-rbac) | `2400ad76a74e` | PASS (exit code 0) |
| [tSQLt-org/tSQLt](https://github.com/tSQLt-org/tSQLt) | `4a921d0dacfb` | PASS (exit code 0) |
| [pljson/pljson](https://github.com/pljson/pljson) | `09a54e4e9318` | PASS (exit code 0) |
| [andrelugomes/digital-innovation-one](https://github.com/andrelugomes/digital-innovation-one) | `31982544848f` | PASS (exit code 0) |
| [Abstrct/Schemaverse](https://github.com/Abstrct/Schemaverse) | `47c01f886b21` | PASS (exit code 0) |
| [basedosdados/sdk](https://github.com/basedosdados/sdk) | `df859a6e71e2` | PASS (exit code 0) |
| [pgpointcloud/pointcloud](https://github.com/pgpointcloud/pointcloud) | `1770ff343183` | PASS (exit code 0) |
| [pgq/pgq](https://github.com/pgq/pgq) | `be3d2cee3fc7` | PASS (exit code 0) |
| [pgMemento/pgMemento](https://github.com/pgMemento/pgMemento) | `f09c44c96c55` | PASS (exit code 0) |
| [siqichen-usc/LeetCode-SQL-Summary](https://github.com/siqichen-usc/LeetCode-SQL-Summary) | `90114213c3ba` | PASS (exit code 0) |
| [bnagy/cgasm](https://github.com/bnagy/cgasm) | `6c54b38020a0` | PASS (exit code 0) |
| [postgis/h3-pg](https://github.com/postgis/h3-pg) | `f960a574eab6` | PASS (exit code 0) |
| [Hameds/IranCountryDivisions](https://github.com/Hameds/IranCountryDivisions) | `37c01f5e776b` | PASS (exit code 0) |
| [czwbig/Tmall_JavaEE](https://github.com/czwbig/Tmall_JavaEE) | `0a96527e8402` | PASS (exit code 0) |
| [vercel/next.js](https://github.com/vercel/next.js) | `b8e68eb8f87d` | PASS (exit code 0) |
| [shadcn-ui/ui](https://github.com/shadcn-ui/ui) | `055aa6b7837b` | PASS (exit code 0) |
| [react/create-react-app](https://github.com/react/create-react-app) | `6254386531d2` | PASS (exit code 0) |
| [mui/material-ui](https://github.com/mui/material-ui) | `9769f52feec5` | PASS (exit code 0) |
| [storybookjs/storybook](https://github.com/storybookjs/storybook) | `44cd55f152cc` | PASS (exit code 0) |
| [ChatGPTNextWeb/NextChat](https://github.com/ChatGPTNextWeb/NextChat) | `1d3dc3315129` | PASS (exit code 0) |
| [remotion-dev/remotion](https://github.com/remotion-dev/remotion) | `0e28fd7c83df` | PASS (exit code 0) |
| [AykutSarac/jsoncrack.com](https://github.com/AykutSarac/jsoncrack.com) | `c30e379df2c3` | PASS (exit code 0) |
| [xyflow/xyflow](https://github.com/xyflow/xyflow) | `9635a064f9d4` | PASS (exit code 0) |
| [jaredpalmer/formik](https://github.com/jaredpalmer/formik) | `91475adbf335` | PASS (exit code 0) |
| [pmndrs/react-three-fiber](https://github.com/pmndrs/react-three-fiber) | `d74622ea8ed7` | PASS (exit code 0) |
| [gethomepage/homepage](https://github.com/gethomepage/homepage) | `a1e3a5fd26d6` | PASS (exit code 0) |
| [vercel/swr](https://github.com/vercel/swr) | `8148ef012fd5` | PASS (exit code 0) |
| [pmndrs/react-spring](https://github.com/pmndrs/react-spring) | `13aacc5a88af` | PASS (exit code 0) |
| [slymnoyann/hey.xyz](https://github.com/slymnoyann/hey.xyz) | `44dee0c1223b` | PASS (exit code 0) |
| [tailwindlabs/headlessui](https://github.com/tailwindlabs/headlessui) | `eea57cf46fd6` | PASS (exit code 0) |
| [maotoumao/MusicFree](https://github.com/maotoumao/MusicFree) | `d118b18b3d0c` | PASS (exit code 0) |
| [nextauthjs/next-auth](https://github.com/nextauthjs/next-auth) | `a1a16a5a7780` | PASS (exit code 0) |
| [rahuldkjain/github-profile-readme-generator](https://github.com/rahuldkjain/github-profile-readme-generator) | `5ae90bfcbd7a` | PASS (exit code 0) |
| [tabler/tabler-icons](https://github.com/tabler/tabler-icons) | `a1422751964a` | PASS (exit code 0) |
| [PanJiaChen/vue-element-admin](https://github.com/PanJiaChen/vue-element-admin) | `6858a9ad6748` | PASS (exit code 0) |
| [bailicangdu/vue2-elm](https://github.com/bailicangdu/vue2-elm) | `b85ff0220366` | PASS (exit code 0) |
| [vuejs/vuepress](https://github.com/vuejs/vuepress) | `9fb4bb00925d` | PASS (exit code 0) |
| [zxwk1998/vue-admin-better](https://github.com/zxwk1998/vue-admin-better) | `b02834b2dd91` | PASS (exit code 0) |
| [iview/iview-admin](https://github.com/iview/iview-admin) | `dd86e386a854` | PASS (exit code 0) |
| [codesandbox/codesandbox-client](https://github.com/codesandbox/codesandbox-client) | `dcba8ebc49cd` | PASS (exit code 0) |
| [bailicangdu/vue2-manage](https://github.com/bailicangdu/vue2-manage) | `dca263e3a841` | PASS (exit code 0) |
| [vueComponent/ant-design-vue-pro](https://github.com/vueComponent/ant-design-vue-pro) | `616df9e063a4` | PASS (exit code 0) |
| [getgridea/gridea](https://github.com/getgridea/gridea) | `48e4ab573bfa` | PASS (exit code 0) |
| [tsparticles/tsparticles](https://github.com/tsparticles/tsparticles) | `814efd3b661d` | PASS (exit code 0) |
| [OnsenUI/OnsenUI](https://github.com/OnsenUI/OnsenUI) | `82c5ff8ce1b8` | PASS (exit code 0) |
| [x-extends/vxe-table](https://github.com/x-extends/vxe-table) | `6a3e68836856` | PASS (exit code 0) |
| [gridsome/gridsome](https://github.com/gridsome/gridsome) | `e396e60ddb94` | PASS (exit code 0) |
| [emqx/MQTTX](https://github.com/emqx/MQTTX) | `a3c542981675` | PASS (exit code 0) |
| [Tencent/wujie](https://github.com/Tencent/wujie) | `775fb9041c72` | PASS (exit code 0) |
| [elunez/eladmin-web](https://github.com/elunez/eladmin-web) | `6e20207bbaf9` | PASS (exit code 0) |
| [herozhou/vue-framework-wz](https://github.com/herozhou/vue-framework-wz) | `c500d578ef81` | PASS (exit code 0) |
| [umicro/uView](https://github.com/umicro/uView) | `1c73d473e15d` | PASS (exit code 0) |
| [algolia/instantsearch](https://github.com/algolia/instantsearch) | `0a50786ebf4c` | PASS (exit code 0) |
| [Simon-He95/markstream-vue](https://github.com/Simon-He95/markstream-vue) | `ee954b81971e` | PASS (exit code 0) |
| [storybookjs/storybook](https://github.com/storybookjs/storybook) | `1aa6a0e28f83` | PASS (exit code 0) |
| [hoppscotch/hoppscotch](https://github.com/hoppscotch/hoppscotch) | `6177eae1875e` | PASS (exit code 0) |
| [lyswhut/lx-music-desktop](https://github.com/lyswhut/lx-music-desktop) | `323e4bac505d` | PASS (exit code 0) |
| [TanStack/query](https://github.com/TanStack/query) | `18b1aded3290` | PASS (exit code 0) |
| [slidevjs/slidev](https://github.com/slidevjs/slidev) | `437f139ccd4c` | PASS (exit code 0) |
| [vuetifyjs/vuetify](https://github.com/vuetifyjs/vuetify) | `b506d3ff13bb` | PASS (exit code 0) |
| [xyflow/xyflow](https://github.com/xyflow/xyflow) | `9635a064f9d4` | PASS (exit code 0) |
| [ueberdosis/tiptap](https://github.com/ueberdosis/tiptap) | `643792f65e8e` | PASS (exit code 0) |
| [directus/directus](https://github.com/directus/directus) | `4856582eec1e` | PASS (exit code 0) |
| [tailwindlabs/headlessui](https://github.com/tailwindlabs/headlessui) | `eea57cf46fd6` | PASS (exit code 0) |
| [Molunerfinn/PicGo](https://github.com/Molunerfinn/PicGo) | `bf4069bd3bdb` | PASS (exit code 0) |
| [lucide-icons/lucide](https://github.com/lucide-icons/lucide) | `7ff66ea5a6d3` | PASS (exit code 0) |
| [youzan/vant](https://github.com/youzan/vant) | `7cc499279866` | PASS (exit code 0) |
| [vueuse/vueuse](https://github.com/vueuse/vueuse) | `6680e578f149` | PASS (exit code 0) |
| [lin-xin/vue-manage-system](https://github.com/lin-xin/vue-manage-system) | `6a7019ec1a74` | PASS (exit code 0) |
| [darkroomengineering/lenis](https://github.com/darkroomengineering/lenis) | `b82992bba894` | PASS (exit code 0) |
| [soybeanjs/soybean-admin](https://github.com/soybeanjs/soybean-admin) | `c0e8a0d1814c` | PASS (exit code 0) |
| [vuejs/pinia](https://github.com/vuejs/pinia) | `23e12f4e7863` | PASS (exit code 0) |
| [formkit/auto-animate](https://github.com/formkit/auto-animate) | `cdaef5660220` | PASS (exit code 0) |
| [logaretm/vee-validate](https://github.com/logaretm/vee-validate) | `7d8cc524f920` | PASS (exit code 0) |

## Audit sample

Each sample is drawn with a seed derived from the pinned commits, so the same corpus gives the same sample.

Reviewer: agent. These are agent-reviewed false positive rates; a person can recheck every verdict in the verdicts file.

| Category | Sampled | Reviewed | False positives | False positive rate |
| --- | --- | --- | --- | --- |
| Source sets | 100 | 100 | 3 | 3.00% |
| Flagged lines | 100 | 100 | 3 | 3.00% |
| Orphan files | 100 | 100 | 99 | 99.00% |

### Source sets

Seed `2668856fc08081a5`; 100 of 434483 Findings.

- `doctrine/orm tests/Tests/ORM/Cache/FileLockRegionTest.php test`
- `morluto/rea src/browser/CdpCaptureValues.test.ts test`
- `stablyai/orca src/renderer/src/components/settings/accounts/scope/system-default-lanes.ts production`
- `prettier/prettier tests/format/markdown/list/huge-first-number/format.test.js test`
- `vercel/next.js test/e2e/app-dir/global-error/basic/app/ssr/page.js test`
- `garrytan/gstack make-pdf/src/image-size.ts production`
- `sebastianbergmann/phpunit src/TextUI/ParallelTestRunner.php production`
- `Significant-Gravitas/AutoGPT autogpt_platform/backend/backend/api/ws_outbox.py production`
- `huggingface/transformers src/transformers/models/sam_hq/__init__.py generated`
- `filamentphp/filament packages/tables/src/Concerns/HasEmptyState.php production`
- `nextcloud/server lib/public/Security/IHasher.php production`
- `openinterpreter/openinterpreter codex-rs/network-proxy/src/environment_policy.rs production`
- `monicahq/monica app/Enums/RelationshipStatusEnum.php production`
- `astral-sh/uv crates/uv-configuration/src/exclude_newer.rs production`
- `matomo-org/matomo core/Updates/0.4.2.php production`
- `anomalyco/opencode packages/tui/src/component/prompt/index.tsx production`
- `nodejs/node test/parallel/test-urlpattern-invalidthis.js test`
- `deepseek-ai/deepseek-harness apps/desktop/tests/macos-signing-walk.spec.ts test`
- `n8n-io/n8n packages/nodes-base/nodes/Microsoft/ExcelSharePoint/test/readRows.harness.test.ts test`
- `symfony/symfony src/Symfony/Component/Security/Core/User/OidcUser.php production`
- `livewire/livewire src/Features/SupportSlots/PlaceholderSlot.php production`
- `anomalyco/opencode packages/desktop/src/renderer/html.test.ts test`
- `ramsey/uuid src/Rfc4122/NilUuid.php production`
- `anomalyco/opencode packages/app/src/new-session/mcp.ts production`
- `grokability/snipe-it tests/Feature/Components/Ui/BulkFulfillComponentTest.php test`
- `microsoft/vscode extensions/copilot/src/extension/completions-core/vscode-node/lib/src/test/snapshot.ts test`
- `stablyai/orca src/main/opencode/status-hooks/plugin-source/status-plugin-post-source.ts production`
- `microsoft/vscode src/vs/editor/common/tokens/contiguousMultilineTokens.ts production`
- `coollabsio/coolify tests/Unit/DatabaseReadWriteHostConfigTest.php test`
- `ccch1mneyyy/dsh-Go packages/dsh_tavern/test/regex_engine_test.dart test`
- `vercel/next.js crates/next-custom-transforms/tests/errors/use-cache-not-allowed/2/input.js test`
- `huggingface/transformers src/transformers/models/glpn/modeling_glpn.py production`
- `microsoft/vscode src/vs/workbench/services/agentHost/test/browser/codexContinuationService.test.ts test`
- `ytdl-org/youtube-dl youtube_dl/extractor/freesound.py production`
- `ComicSparks/jasmine lib/screens/downloads_exporting_screen.dart production`
- `NousResearch/hermes-agent tests/plugins/config_backends/test_remote_unavailable_consumers.py test`
- `morluto/rea scripts/ci/cache-retention.mjs production`
- `grokability/snipe-it resources/lang/fil-PH/auth.php production`
- `nodejs/node test/fixtures/snapshot/ts-example.ts test`
- `router-for-me/CLIProxyAPI sdk/cliproxy/auth/conductor_session_affinity_alias_test.go test`
- `nodejs/node test/fixtures/source-map/enclosing-call-site-min.js test`
- `pgMemento/pgMemento ctl/STOP.sql production`
- `garrytan/gstack lib/dx-error-capture.ts production`
- `mornyeapp/Mornye test/autoplay_taste_inheritance_test.dart test`
- `react/react compiler/packages/babel-plugin-react-compiler/src/__tests__/fixtures/compiler/dce-loop.js test`
- `firecrawl/firecrawl apps/api/src/services/worker/nuq-reconciler-worker.ts production`
- `stablyai/orca src/main/bots/bot-home.test.ts test`
- `ccch1mneyyy/dsh-Go app/test/update/ohos_notice_test.dart test`
- `nextcloud/server apps/dav/lib/Listener/ActivityUpdaterListener.php production`
- `derailed/k9s internal/config/data/config.go production`
- `stablyai/orca mobile/src/components/list/DisclosureChevron.tsx production`
- `vllm-project/vllm vllm/models/kimi_k3/amd/specialized_decode/mono/common/sync.py production`
- `openinterpreter/openinterpreter codex-rs/windows-sandbox-rs/src/provisioning_client_tests.rs test`
- `flutter/flutter dev/benchmarks/microbenchmarks/lib/foundation/platform_asset_bundle.dart production`
- `grokability/snipe-it resources/lang/fil-PH/admin/locations/message.php production`
- `earendil-works/pi packages/chord/src/remote/internal.ts production`
- `microsoft/vscode src/vs/platform/agentHost/node/codex/codexForkPlan.ts production`
- `n8n-io/n8n packages/cli/test/integration/middlewares/body-parser.test.ts test`
- `langflow-ai/langflow src/backend/base/langflow/agentic/api/router.py production`
- `NanmiCoder/MediaCrawler database/db_session.py production`
- `microsoft/vscode src/vs/sessions/contrib/chat/browser/requestOriginProvider.contribution.ts production`
- `ehang-io/nps web/controllers/base.go production`
- `go-gitea/gitea routers/api/v1/repo/migrate.go production`
- `n8n-io/n8n packages/workflow/test/expressions/expression-helpers.test.ts test`
- `n8n-io/n8n packages/nodes-base/nodes/Raindrop/__tests__/Raindrop.workflow.test.ts test`
- `openai/codex codex-rs/tui/src/bottom_pane/experimental_features_view_tests.rs test`
- `monicahq/monica app/Helpers/PossessionFieldOptions.php production`
- `NousResearch/hermes-agent tests/tools/test_search_error_guard.py test`
- `n8n-io/n8n packages/@n8n/workflow-engine-check/src/trigger-identity-rule.ts production`
- `n8n-io/n8n packages/frontend/editor-ui/src/features/settings/communityNodes/composables/useInstalledCommunityPackage.test.ts test`
- `monicahq/monica app/Helpers/FieldSettingsCatalogue.php production`
- `rclone/rclone cmd/serve/nfs/cache_test.go test`
- `vercel/next.js test/e2e/app-dir/instant-validation/app/suspense-in-root/static/route-group-shared-boundary/(outer)/(inner)/layout.tsx test`
- `vercel/next.js test/e2e/conflicting-public-file-page/pages/hello.js test`
- `smpetersgithub/AdvancedSQLPuzzles Advanced SQL Puzzles/Recursion Examples/Double the Number Recursion.sql production`
- `symfony/symfony src/Symfony/Bundle/FrameworkBundle/Tests/DependencyInjection/Fixtures/php/legacy_translator.php production`
- `NousResearch/hermes-agent tests/agent/test_conversation_loop_interpreter_shutdown.py test`
- `ccch1mneyyy/dsh-Go app/lib/ui/common/sheet_title_bar.dart production`
- `SpartnerNL/Laravel-Excel src/Concerns/WithSheetProtection.php production`
- `nodejs/node deps/v8/test/inspector/debugger/regression-419663.js test`
- `symfony/symfony src/Symfony/Component/Cache/CacheWarmer/AbstractPhpFileCacheWarmer.php production`
- `nodejs/node test/parallel/test-common-wpt-script-modifier.js test`
- `openai/codex codex-rs/cloud-tasks/src/lib.rs production`
- `Shubhamsaboo/awesome-llm-apps ai_agent_framework_crash_course/openai_sdk_crash_course/4_running_agents/4_2_conversation_management/__init__.py production`
- `n8n-io/n8n .github/scripts/link-check/check-report.mjs production`
- `vllm-project/vllm tests/models/quantization/conftest.py test`
- `hiddify/hiddify-app third_party/flutter_animate/lib/src/animate_list.dart vendored`
- `nodejs/node test/fixtures/snapshot/console.js test`
- `pytorch/pytorch torch/_native/ops/norm/rmsnorm_kernels.py production`
- `NousResearch/hermes-agent agent/bot_chain.py production`
- `netdata/netdata src/go/pkg/web/doc_test.go test`
- `microsoft/fhir-server src/Microsoft.Health.Fhir.SqlServer/Features/Schema/Sql/Sprocs/UnitTestHelper.sql production`
- `router-for-me/CLIProxyAPI internal/httpfetch/httpfetch_test.go test`
- `NousResearch/hermes-agent tests/fakes/main_era_venv.py test`
- `nodejs/node test/js-native-api/test_error/test.js test`
- `microsoft/vscode src/vs/sessions/contrib/agentFeedback/browser/agentFeedbackWorkbenchComments.ts production`
- `ente/ente mobile/apps/photos/lib/db/ml/clip_vector_db.dart production`
- `garrytan/gstack browse/src/cdp-allowlist.ts production`
- `n8n-io/n8n packages/nodes-base/nodes/InvoiceNinja/invoiceInterface.ts production`
- `Shubhamsaboo/awesome-llm-apps ai_agent_framework_crash_course/openai_sdk_crash_course/3_tool_using_agent/3_1_function_tools/__init__.py production`

### Flagged lines

Seed `2668856fc08081a5`; 100 of 73257 Findings.

- `ente/ente mobile/apps/photos/lib/models/gallery/gallery_groups.dart:495 else-after-return`
- `nikic/PHP-Parser lib/PhpParser/PrettyPrinter/Standard.php:774 else-after-return`
- `grafana/grafana packages/grafana-data/src/datetime/luxon_moment_compat/luxon.js:569 else-after-return`
- `nodejs/node deps/v8/tools/testrunner/local/utils.py:160 else-after-return`
- `nextcloud/server build/integration/features/bootstrap/BasicStructure.php:115 else-after-return`
- `rustdesk/rustdesk flutter/lib/models/web_model.dart:88 else-after-return`
- `langgenius/dify api/core/plugin/backwards_invocation/model.py:169 else-after-return`
- `Ysurac/FlightAirMap require/class.TrackerArchive.php:1040 else-after-return`
- `Ysurac/FlightAirMap require/class.TrackerArchive.php:1042 else-after-return`
- `pytorch/pytorch torch/utils/data/datapipes/datapipe.py:150 else-after-return`
- `pytorch/pytorch torch/_dynamo/variables/builtin.py:3037 else-after-return`
- `vandadnp/flutter-tips-and-tricks tipsandtricks/null-aware-infix-operators-in-dart/null-aware-infix-operators-in-dart.dart:23 else-after-return`
- `vercel/next.js turbopack/crates/turbopack-ecmascript/src/chunk/placeable.rs:137 else-after-return`
- `ytdl-org/youtube-dl youtube_dl/extractor/leeco.py:329 else-after-return`
- `flutterchina/flukit package_src/lib/src/scale_view.dart:115 else-after-return`
- `nodejs/node deps/v8/test/mjsunit/regress/regress-crbug-1302527.js:318 constructor-only-forwards`
- `ente/ente mobile/apps/auth/lib/ui/home_page.dart:1971 else-after-return`
- `unionlabs/union voyager/plugins/transaction/cosmos/src/main.rs:393 else-after-return`
- `matomo-org/matomo tests/javascript/frameworks/ext/ext-all-2.3.0.js:1684 else-after-return`
- `nextcloud/server apps/files_sharing/lib/External/Storage.php:365 else-after-return`
- `squizlabs/PHP_CodeSniffer src/Standards/Generic/Sniffs/Formatting/MultipleStatementAlignmentSniff.php:312 else-after-return`
- `squizlabs/PHP_CodeSniffer src/Tokenizers/CSS.php:123 else-after-return`
- `typecho/typecho var/Typecho/I18n/GetText.php:296 else-after-return`
- `lollipopkit/flutter_server_box lib/src/rust/frb_generated.dart:31563 else-after-return`
- `ente/ente mobile/apps/photos/lib/ui/viewer/hierarchical_search/chip_widgets/only_them_filter_chip.dart:166 else-after-return`
- `n8n-io/n8n packages/@n8n/nodes-langchain/nodes/vendors/OpenAi/v1/actions/text/classify.operation.ts:76 else-after-return`
- `Ysurac/FlightAirMap require/class.AIS.php:473 else-after-return`
- `pytorch/pytorch torch/_inductor/ir.py:10287 else-after-return`
- `prometheus/prometheus web/ui/react-app/src/pages/graph/HistogramHelpers.ts:100 else-after-return`
- `best-flutter/flutter_swiper lib/src/swiper_pagination.dart:57 else-after-return`
- `pytorch/pytorch torch/_numpy/testing/utils.py:250 else-after-return`
- `pytorch/pytorch torch/_subclasses/fake_tensor.py:3456 else-after-return`
- `pytorch/pytorch torch/_inductor/fx_passes/mkldnn_fusion.py:526 else-after-return`
- `syncthing/syncthing gui/default/vendor/angular/angular-dirPagination.js:523 else-after-return`
- `tree-sitter/tree-sitter lib/lldb_pretty_printers/table_entry.py:26 else-after-return`
- `lollipopkit/flutter_server_box crates/sbm_ffi/src/frb_generated.rs:9466 trailing-return`
- `tauri-apps/tauri crates/tauri-cli/src/plugin/init.rs:249 else-after-return`
- `rustdesk/rustdesk libs/clipboard/src/platform/unix/macos/paste_task.rs:449 else-after-return`
- `huggingface/transformers src/transformers/models/seamless_m4t_v2/modeling_seamless_m4t_v2.py:3196 else-after-return`
- `denoland/deno libs/core/modules/recursive_load.rs:712 else-after-return`
- `nodejs/node deps/cares/config/ltmain.sh:9599 legacy-backticks`
- `pytorch/pytorch test/dynamo/test_repros.py:830 else-after-return`
- `langgenius/dify api/providers/vdb/vdb-tidb-on-qdrant/src/dify_vdb_tidb_on_qdrant/tidb_on_qdrant_vector.py:77 else-after-return`
- `astral-sh/uv crates/uv-project-commands/src/run.rs:1521 else-after-return`
- `Ysurac/FlightAirMap require/class.Spotter.php:2423 else-after-return`
- `vllm-project/vllm vllm/v1/core/single_type_kv_cache_manager.py:2276 else-after-return`
- `microsoft/vscode src/vs/sessions/contrib/chat/browser/runScriptAction.ts:383 else-after-return`
- `Ysurac/FlightAirMap require/class.MarineArchive.php:805 else-after-return`
- `zed-industries/zed crates/editor/src/display_map/inlay_map.rs:960 else-after-return`
- `pytorch/pytorch torch/_higher_order_ops/triton_kernel_wrap.py:653 else-after-return`
- `bagisto/bagisto packages/Webkul/Admin/tests/e2e-pw/pages/shop/OmnibusShopPage.ts:5 constructor-only-forwards`
- `pytorch/pytorch torch/distributions/transforms.py:1197 else-after-return`
- `microsoft/vscode src/vs/base/common/date.ts:225 else-after-return`
- `vllm-project/vllm tests/v1/attention/test_mla_backends.py:664 else-after-return`
- `rustdesk/rustdesk src/core_main.rs:716 else-after-return`
- `Significant-Gravitas/AutoGPT autogpt_platform/backend/backend/blocks/todoist/labels.py:506 except-only-reraises`
- `rustdesk/rustdesk flutter/lib/desktop/pages/remote_tab_page.dart:409 else-after-return`
- `Significant-Gravitas/AutoGPT classic/forge/forge/components/data_processor/data_processor.py:475 else-after-return`
- `Significant-Gravitas/AutoGPT autogpt_platform/backend/backend/util/type.py:190 else-after-return`
- `huggingface/transformers src/transformers/utils/import_utils.py:251 else-after-return`
- `nodejs/node tools/gyp/pylib/gyp/xcodeproj_file.py:1363 else-after-return`
- `pytorch/pytorch test/test_proxy_tensor.py:2457 else-after-return`
- `rustdesk/rustdesk flutter/lib/mobile/widgets/floating_mouse.dart:702 else-after-return`
- `langgenius/dify api/providers/vdb/vdb-oracle/src/dify_vdb_oracle/oraclevector.py:139 else-after-return`
- `matomo-org/matomo plugins/Actions/Columns/Metrics/AveragePageGenerationTime.php:70 else-after-return`
- `binary-husky/gpt_academic crazy_functions/live_audio/aliyunASR.py:103 else-after-return`
- `grokability/snipe-it public/js/dist/all.js:29906 else-after-return`
- `flutter/flutter packages/flutter_driver/lib/src/driver/vmservice_driver.dart:166 else-after-return`
- `nextcloud/server lib/private/Files/Node/NonExistingFolder.php:199 else-after-return`
- `netdata/netdata packaging/installer/netdata-uninstaller.sh:668 else-after-return`
- `pytorch/pytorch torch/distributed/flight_recorder/components/types.py:350 else-after-return`
- `pytorch/pytorch torch/_inductor/fx_passes/decompose_mem_bound_mm.py:181 else-after-return`
- `pytorch/pytorch tools/testing/test_run.py:212 else-after-return`
- `CarGuo/gsy_github_app_flutter lib/common/repositories/repos_repository.dart:664 else-after-return`
- `anthropics/skills skills/docx/scripts/office/validators/base.py:669 else-after-return`
- `pytorch/pytorch torch/_inductor/codegen/cpp_micro_gemm.py:41 else-after-return`
- `pytorch/pytorch test/functorch/test_leaf_function.py:1020 else-after-return`
- `vllm-project/vllm vllm/distributed/kv_transfer/kv_connector/v1/lmcache_mp_connector.py:124 else-after-return`
- `lollipopkit/flutter_server_box crates/sbm_ffi/src/frb_generated.rs:11411 trailing-return`
- `router-for-me/CLIProxyAPI sdk/cliproxy/auth/conductor_cooldown.go:3673 if-else-returns-boolean`
- `ruvnet/RuView v2/vendor/security/sqlx-sqlite-0.8.6/src/connection/explain.rs:789 else-after-return`
- `ente/ente mobile/apps/photos/lib/utils/misc_util.dart:20 else-after-return`
- `denoland/deno libs/node_resolver/resolution.rs:1678 else-after-return`
- `grokability/snipe-it app/Presenters/AccessoryPresenter.php:461 else-after-return`
- `flutter/flutter packages/flutter_tools/lib/src/android/gradle_utils.dart:1335 else-after-return`
- `ruvnet/RuView v2/vendor/security/sqlx-sqlite-0.8.6/src/connection/explain.rs:703 else-after-return`
- `nextcloud/server apps/files_external/lib/Lib/Storage/SFTP.php:459 else-after-return`
- `rustdesk/rustdesk src/core_main.rs:715 else-after-return`
- `bagisto/opensource-ecommerce-mobile-app lib/core/currency/product_price_display.dart:173 else-after-return`
- `bagisto/bagisto packages/Webkul/Admin/tests/e2e-pw/pages/admin/sales/RmaCustomFieldsPage.ts:24 constructor-only-forwards`
- `prometheus/prometheus prompb/remote.pb.go:327 else-after-return`
- `grafana/grafana pkg/middleware/request_tracing.go:83 if-else-returns-boolean`
- `nextcloud/server apps/dav/tests/testsuits/carddav/script.sh:6 legacy-backticks`
- `etcd-io/etcd tools/local-tester/faults.sh:50 legacy-backticks`
- `vllm-project/vllm vllm/entrypoints/pooling/offline.py:186 else-after-return`
- `pbakaus/impeccable crates/context/src/catalog.rs:472 redundant-closure`
- `flutter/flutter packages/flutter/lib/src/painting/rounded_rectangle_border.dart:524 else-after-return`
- `garrytan/gstack lib/cso/state.ts:2149 catch-only-rethrows`
- `openai/codex codex-rs/tui/src/chatwidget/tests/mcp_startup.rs:783 redundant-closure`
- `pytorch/pytorch torch/_inductor/fx_passes/quantization.py:369 else-after-return`

### Orphan files

Seed `2668856fc08081a5`; 100 of 81387 Findings.

- `stablyai/orca src/renderer/src/components/skills/installation/skill-install-progress-state.ts unreferenced`
- `morluto/rea src/domain/javascript/javascriptBuiltinResourceMethods.ts test-only`
- `morluto/rea src/composition/firmware.ts test-only`
- `flame-engine/flame packages/flame_test/lib/src/expect_color.dart test-only`
- `stablyai/orca mobile/src/transport/rpc/rpc-client-socket-factory.ts test-only`
- `stablyai/orca src/main/runtime/terminal/wait/reattached-ready-screen.ts test-only`
- `n8n-io/n8n packages/frontend/editor-ui/src/features/ndv/parameters/components/ResourceLocator/ResourceLocator.test.constants.ts test-only`
- `stablyai/orca src/renderer/src/lib/simulator/search/simulator-palette-active-tab.ts unreferenced`
- `langchain-ai/langchain libs/langchain/langchain_classic/chat_loaders/telegram.py unreferenced`
- `bagisto/bagisto packages/Webkul/Admin/src/Resources/views/appearance/sections/index.blade.php unreferenced`
- `n8n-io/n8n packages/@n8n/api-types/src/dto/ai/ai-gateway-wallet-response.dto.ts unreferenced`
- `vercel/next.js packages/next/src/server/mcp/tools/get-compilation-issues.ts unreferenced`
- `stablyai/orca src/main/acp/acp-dialects/goose-dialect.ts test-only`
- `stablyai/orca mobile/src/worktree/project-sections/project-group-tree-sections.ts test-only`
- `ente/ente web/packages/gallery/services/download-core.ts unreferenced`
- `flutter/flutter dev/devicelab/bin/tasks/complex_layout_android__impeller_gles_start_up.dart unreferenced`
- `langgenius/dify api/libs/custom_inputs.py test-only`
- `coollabsio/coolify resources/views/livewire/security/cloud-provider-tokens.blade.php unreferenced`
- `stablyai/orca src/main/computer/desktop-script/snapshots/desktop-script-snapshot-store.ts unreferenced`
- `grokability/snipe-it resources/lang/zh-HK/admin/departments/table.php unreferenced`
- `binary-husky/gpt_academic crazy_functions/doc_fns/read_fns/unstructured_all/paper_metadata_extractor.py unreferenced`
- `stablyai/orca src/renderer/src/components/sidebar/workspace-card/controller/use-worktree-card-workspace-actions.ts test-only`
- `grokability/snipe-it resources/views/livewire/clone-fieldset.blade.php unreferenced`
- `coollabsio/coolify resources/views/components/terminal/theme-selector.blade.php unreferenced`
- `stablyai/orca src/main/managed-server/activation/orcad-candidate-preflight.ts test-only`
- `firefly-iii/firefly-iii resources/views/budgets/delete.blade.php unreferenced`
- `stablyai/orca src/renderer/src/components/right-sidebar/source-control/listing/split-open.ts test-only`
- `firefly-iii/firefly-iii resources/views/transactions/edit.blade.php unreferenced`
- `NousResearch/hermes-agent plugins/platforms/telegram/update_admission.py test-only`
- `stablyai/orca src/renderer/src/components/ui/sonner.fixture.tsx unreferenced`
- `ente/ente web/packages/gallery/services/save-progress.ts unreferenced`
- `n8n-io/n8n packages/nodes-base/credentials/JiraSoftwareServerApi.credentials.ts unreferenced`
- `stablyai/orca src/main/hang-watchdog/renderer-hang-stack-policy.ts unreferenced`
- `BookStackApp/BookStack app/Activity/Notifications/Messages/PageCreationNotification.php test-only`
- `grokability/snipe-it resources/lang/en-GB/admin/categories/table.php unreferenced`
- `stablyai/orca src/renderer/src/components/terminal-pane/layout/split/terminal-pane-split-completion.ts unreferenced`
- `langchain-ai/langchain libs/langchain/langchain_classic/tools/gmail/create_draft.py unreferenced`
- `ruvnet/RuView v2/crates/homecore-server/ui/js/panels/fleet.js test-only`
- `infiniflow/ragflow internal/dao/chat.go unreferenced`
- `firefly-iii/firefly-iii resources/views/partials/form/transaction/transaction-type.blade.php unreferenced`
- `bagisto/bagisto packages/Webkul/Rule/src/Providers/ModuleServiceProvider.php unreferenced`
- `stablyai/orca src/main/runtime/workspaces/removal/worktree-pty-stop-verdict.ts test-only`
- `krayin/laravel-crm packages/Webkul/Admin/src/DataGrids/Contact/PersonDataGrid.php unreferenced`
- `BookStackApp/BookStack lang/th/validation.php unreferenced`
- `stablyai/orca src/renderer/src/components/bots/bot-connectors-host-fixture.tsx unreferenced`
- `langchain-ai/langchain libs/standard-tests/langchain_tests/base.py test-only`
- `stablyai/orca src/renderer/src/lib/editor/monaco/monaco-delayer-cancellation-guard.ts unreferenced`
- `Significant-Gravitas/AutoGPT autogpt_platform/scripted_model/scripted_model/streaming.py unreferenced`
- `stablyai/orca src/renderer/src/components/plugins/catalog/PluginInUseAction.tsx unreferenced`
- `stablyai/orca src/renderer/src/components/terminal-pane/pty-connection/pane-connection-context.ts unreferenced`
- `anomalyco/opencode packages/app/perf/diff.bench.ts unreferenced`
- `anomalyco/opencode packages/core/perf/snapshot.ts unreferenced`
- `stablyai/orca src/main/daemon/providers/adapter/daemon-pty-runtime-state.ts test-only`
- `grokability/snipe-it app/Http/Transformers/AccessoriesTransformer.php unreferenced`
- `stablyai/orca src/renderer/src/lib/agent-session/launch/prompt/launch-agent-paste-timeout-notice.ts unreferenced`
- `n8n-io/n8n packages/nodes-base/nodes/Grist/Grist.node.ts test-only`
- `stablyai/orca src/renderer/src/components/new-workspace/source-search/smart-workspace-github-direct-link.ts unreferenced`
- `n8n-io/n8n packages/frontend/editor-ui/src/features/agents/components/AgentNewTestRunButton.vue test-only`
- `stablyai/orca src/main/runtime/rpc/server/pairing/network/pairing-endpoint.ts test-only`
- `n8n-io/n8n packages/nodes-base/nodes/Google/Sheet/v2/GoogleSheetsV2.node.ts unreferenced`
- `anomalyco/opencode packages/cli/src/acp/permission.ts test-only`
- `NousResearch/hermes-agent apps/desktop/electron/ui-preferences-ipc.ts unreferenced`
- `stablyai/orca src/renderer/src/components/comment-markdown/chat-markdown-math-typeset.ts test-only`
- `grokability/snipe-it resources/lang/it-IT/admin/manufacturers/message.php unreferenced`
- `stablyai/orca mobile/src/tasks/mobile-tasks-project-workspace-types.ts unreferenced`
- `morluto/rea src/ghidra/GhidraRiscvProfile.ts test-only`
- `stablyai/orca src/renderer/src/components/native-chat/host-access/native-chat-agy-sign-in.ts test-only`
- `chartjs/Chart.js src/core/core.typedRegistry.js test-only`
- `stablyai/orca src/shared/bots/bot-chat-identity.ts unreferenced`
- `n8n-io/n8n packages/@n8n/db/src/migrations/common/1795100170853-AllowApiSourceOnAiPreference.ts unreferenced`
- `stablyai/orca src/main/window/activation/macos-desktop-handoff.ts unreferenced`
- `stablyai/orca src/shared/agent-session/conversation-context/agent-session-compaction.ts test-only`
- `abi/screenshot-to-code backend/agent/providers/anthropic/__init__.py test-only`
- `stablyai/orca src/shared/ripgrep/ripgrep-match-offsets.ts test-only`
- `grokability/snipe-it resources/lang/cy-GB/admin/asset_maintenances/table.php unreferenced`
- `stablyai/orca src/main/persistence/loading-store/terminal-bindings/pty-binding-generations.ts test-only`
- `Shubhamsaboo/awesome-llm-apps advanced_llm_apps/chat_with_X_tutorials/chat_with_youtube_videos/chat_youtube.py unreferenced`
- `stablyai/orca src/shared/agent-status/store/agent-status-store-status-codec.ts test-only`
- `anomalyco/opencode packages/core/script/migration.ts unreferenced`
- `morluto/rea src/application/AnalysisOperationFacts.ts test-only`
- `n8n-io/n8n packages/nodes-base/nodes/Adalo/types.ts unreferenced`
- `n8n-io/n8n packages/nodes-base/nodes/InvoiceNinja/InvoiceNinja.node.ts test-only`
- `stablyai/orca src/main/managed-server/install/remote-install-lock-owner-commands.ts test-only`
- `stablyai/orca src/renderer/src/components/editor/rich-markdown/review/rich-markdown-review-annotations.ts unreferenced`
- `stablyai/orca src/main/runtime/rpc/server/pairing/declared-desktop-clients.ts test-only`
- `vllm-project/vllm vllm/models/mistral/mistral_large4/amd/specialized_decode/mono/op.py unreferenced`
- `stablyai/orca src/renderer/src/components/editor/rich-markdown/review/RichMarkdownAnnotationOverlay.tsx unreferenced`
- `n8n-io/n8n packages/nodes-base/nodes/Databricks/actions/genie/getMessage.operation.ts test-only`
- `stablyai/orca src/shared/agent-journal-orca-card-items.ts unreferenced`
- `pytorch/pytorch torch/testing/_internal/distributed/rpc/examples/__init__.py unreferenced`
- `stablyai/orca src/shared/speech/native-recognizer/speech-recognizer-package.ts test-only`
- `morluto/rea src/application/environment/environmentToolAnswer.ts test-only`
- `coollabsio/coolify app/Livewire/Storage/Concerns/SelectsSftpLogin.php test-only`
- `vllm-project/vllm vllm/transformers_utils/processors/internvl.py test-only`
- `grokability/snipe-it resources/lang/de-DE/admin/settings/mcp.php unreferenced`
- `NousResearch/hermes-agent ui-tui/packages/hermes-ink/src/utils/debug.ts test-only`
- `Comfy-Org/ComfyUI comfy/latent_formats.py test-only`
- `IceWhaleTech/CasaOS internal/conf/var.go unreferenced`
- `grafana/grafana packages/grafana-api-clients/src/generator/variants.ts test-only`
- `nodejs/node lib/internal/crypto/webcrypto_util.js test-only`
