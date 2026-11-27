# codeint Evaluation

Result: FAIL.
Corpus measured on 2026-11-27.
Binary: `codeint 0.8.0`.

## Machine profile

- Operating system: macos on aarch64
- CPU: Apple M5
- Logical CPUs: 10
- Memory: 32.0 GiB
- Runs per repository: 1

## Reference range panels

Each Reference range is the 10th to 90th percentile (linear interpolation) of the Production Source set ratios across the panel.
The standard deviation is the population standard deviation.

### dart

100 repositories: 30 Small, 40 Medium, 30 Large.

| Metric | Version | Mean | Median | Standard deviation | 10th percentile | 90th percentile | Lowest | Highest |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Erosion | 1 | 0.262 | 0.249 | 0.189 | 0.039 | 0.497 | 0.000 | 0.824 |
| Cognitive complexity | 1 | 0.192 | 0.180 | 0.154 | 0.000 | 0.377 | 0.000 | 0.823 |
| Verbosity | 2 | 0.004 | 0.002 | 0.006 | 0.000 | 0.011 | 0.000 | 0.033 |
| Duplication | 1 | 0.128 | 0.102 | 0.104 | 0.027 | 0.256 | 0.000 | 0.558 |
| Import cycles | 1 | 0.291 | 0.264 | 0.207 | 0.061 | 0.600 | 0.000 | 0.964 |

### go

100 repositories: 30 Small, 40 Medium, 30 Large.

| Metric | Version | Mean | Median | Standard deviation | 10th percentile | 90th percentile | Lowest | Highest |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Erosion | 1 | 0.518 | 0.520 | 0.170 | 0.304 | 0.746 | 0.149 | 0.924 |
| Cognitive complexity | 1 | 0.473 | 0.462 | 0.136 | 0.306 | 0.671 | 0.142 | 0.826 |
| Verbosity | 2 | 0.002 | 0.001 | 0.005 | 0.000 | 0.004 | 0.000 | 0.030 |
| Duplication | 1 | 0.117 | 0.093 | 0.098 | 0.034 | 0.243 | 0.000 | 0.581 |
| Import cycles | 1 | 0.000 | 0.000 | 0.000 | 0.000 | 0.000 | 0.000 | 0.000 |

### php

100 repositories: 30 Small, 40 Medium, 30 Large.

| Metric | Version | Mean | Median | Standard deviation | 10th percentile | 90th percentile | Lowest | Highest |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Erosion | 1 | 0.337 | 0.359 | 0.204 | 0.064 | 0.591 | 0.000 | 0.769 |
| Cognitive complexity | 1 | 0.291 | 0.309 | 0.187 | 0.044 | 0.542 | 0.000 | 0.707 |
| Verbosity | 2 | 0.002 | 0.000 | 0.003 | 0.000 | 0.004 | 0.000 | 0.024 |
| Duplication | 1 | 0.116 | 0.079 | 0.099 | 0.025 | 0.209 | 0.000 | 0.676 |
| Import cycles | 1 | 0.048 | 0.011 | 0.075 | 0.000 | 0.133 | 0.000 | 0.451 |

### python

100 repositories: 30 Small, 40 Medium, 30 Large.

| Metric | Version | Mean | Median | Standard deviation | 10th percentile | 90th percentile | Lowest | Highest |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Erosion | 1 | 0.497 | 0.523 | 0.184 | 0.239 | 0.717 | 0.000 | 0.918 |
| Cognitive complexity | 1 | 0.461 | 0.480 | 0.154 | 0.249 | 0.637 | 0.000 | 0.812 |
| Verbosity | 2 | 0.005 | 0.003 | 0.007 | 0.000 | 0.011 | 0.000 | 0.051 |
| Duplication | 1 | 0.101 | 0.061 | 0.125 | 0.011 | 0.220 | 0.000 | 0.739 |
| Import cycles | 1 | 0.134 | 0.053 | 0.183 | 0.000 | 0.446 | 0.000 | 0.739 |

### rust

100 repositories: 30 Small, 40 Medium, 30 Large.

| Metric | Version | Mean | Median | Standard deviation | 10th percentile | 90th percentile | Lowest | Highest |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Erosion | 1 | 0.506 | 0.539 | 0.178 | 0.245 | 0.704 | 0.000 | 0.815 |
| Cognitive complexity | 1 | 0.378 | 0.379 | 0.163 | 0.176 | 0.585 | 0.000 | 0.766 |
| Verbosity | 2 | 0.002 | 0.001 | 0.003 | 0.000 | 0.005 | 0.000 | 0.025 |
| Duplication | 1 | 0.195 | 0.138 | 0.185 | 0.047 | 0.377 | 0.000 | 0.920 |
| Import cycles | 1 | 0.260 | 0.221 | 0.234 | 0.000 | 0.611 | 0.000 | 0.943 |

### shell

100 repositories: 30 Small, 48 Medium, 22 Large.

| Metric | Version | Mean | Median | Standard deviation | 10th percentile | 90th percentile | Lowest | Highest |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Erosion | 1 | 0.677 | 0.768 | 0.237 | 0.364 | 0.930 | 0.002 | 0.993 |
| Cognitive complexity | 1 | 0.544 | 0.615 | 0.237 | 0.218 | 0.828 | 0.002 | 0.903 |
| Verbosity | 2 | 0.013 | 0.004 | 0.026 | 0.000 | 0.031 | 0.000 | 0.158 |
| Duplication | 1 | 0.194 | 0.058 | 0.272 | 0.003 | 0.713 | 0.000 | 0.940 |

### sql

77 repositories: 30 Small, 26 Medium, 21 Large.

| Metric | Version | Mean | Median | Standard deviation | 10th percentile | 90th percentile | Lowest | Highest |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Erosion | 1 | 0.365 | 0.264 | 0.360 | 0.000 | 0.918 | 0.000 | 0.999 |
| Cognitive complexity | 1 | 0.427 | 0.437 | 0.359 | 0.000 | 0.902 | 0.000 | 1.000 |
| Verbosity | 2 | 0.000 | 0.000 | 0.000 | 0.000 | 0.000 | 0.000 | 0.002 |
| Duplication | 1 | 0.353 | 0.224 | 0.344 | 0.000 | 0.909 | 0.000 | 0.969 |

### typescript-javascript

100 repositories: 30 Small, 40 Medium, 30 Large.

| Metric | Version | Mean | Median | Standard deviation | 10th percentile | 90th percentile | Lowest | Highest |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Erosion | 1 | 0.450 | 0.452 | 0.186 | 0.187 | 0.671 | 0.000 | 0.954 |
| Cognitive complexity | 1 | 0.326 | 0.326 | 0.160 | 0.121 | 0.532 | 0.000 | 0.888 |
| Verbosity | 2 | 0.001 | 0.000 | 0.001 | 0.000 | 0.002 | 0.000 | 0.005 |
| Duplication | 1 | 0.130 | 0.075 | 0.173 | 0.018 | 0.252 | 0.000 | 0.845 |
| Import cycles | 1 | 0.059 | 0.003 | 0.105 | 0.000 | 0.182 | 0.000 | 0.462 |

### typescript-javascript (react Framework profile)

50 repositories: 15 Small, 20 Medium, 15 Large.

| Metric | Version | Mean | Median | Standard deviation | 10th percentile | 90th percentile | Lowest | Highest |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Erosion | 1 | 0.313 | 0.291 | 0.191 | 0.054 | 0.566 | 0.000 | 0.806 |
| Cognitive complexity | 1 | 0.221 | 0.219 | 0.149 | 0.038 | 0.399 | 0.000 | 0.667 |
| Verbosity | 2 | 0.000 | 0.000 | 0.001 | 0.000 | 0.001 | 0.000 | 0.004 |
| Duplication | 1 | 0.142 | 0.092 | 0.172 | 0.026 | 0.315 | 0.007 | 0.826 |
| Import cycles | 1 | 0.058 | 0.012 | 0.098 | 0.000 | 0.202 | 0.000 | 0.422 |

### typescript-javascript (vue2 Framework profile)

50 repositories: 15 Small, 29 Medium, 6 Large.

| Metric | Version | Mean | Median | Standard deviation | 10th percentile | 90th percentile | Lowest | Highest |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Erosion | 1 | 0.227 | 0.197 | 0.166 | 0.025 | 0.479 | 0.000 | 0.595 |
| Cognitive complexity | 1 | 0.199 | 0.192 | 0.140 | 0.033 | 0.406 | 0.000 | 0.511 |
| Verbosity | 2 | 0.001 | 0.001 | 0.001 | 0.000 | 0.003 | 0.000 | 0.005 |
| Duplication | 1 | 0.130 | 0.106 | 0.125 | 0.023 | 0.267 | 0.000 | 0.753 |
| Import cycles | 1 | 0.042 | 0.001 | 0.074 | 0.000 | 0.128 | 0.000 | 0.306 |

### typescript-javascript (vue3 Framework profile)

50 repositories: 15 Small, 20 Medium, 15 Large.

| Metric | Version | Mean | Median | Standard deviation | 10th percentile | 90th percentile | Lowest | Highest |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Erosion | 1 | 0.230 | 0.203 | 0.156 | 0.042 | 0.438 | 0.000 | 0.631 |
| Cognitive complexity | 1 | 0.146 | 0.120 | 0.105 | 0.022 | 0.297 | 0.000 | 0.392 |
| Verbosity | 2 | 0.001 | 0.001 | 0.001 | 0.000 | 0.002 | 0.000 | 0.004 |
| Duplication | 1 | 0.135 | 0.093 | 0.150 | 0.021 | 0.297 | 0.003 | 0.871 |
| Import cycles | 1 | 0.110 | 0.031 | 0.159 | 0.000 | 0.325 | 0.000 | 0.676 |

## Native-parser recall

Each bar is 98% recall on functions, decision points, and static imports, with no unexpected unparsable files.
A file that the oracle parses and codeint did not measure counts as a miss on every function, decision point, and import in it.

| Language | Oracle | Functions | Decision points | Static imports | Oracle-unparsable files | Unexpected unparsable files | Unmeasured files | Result |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| dart | measured | 99.86% (968597/969939) | 99.23% (727030/732643) | 99.99% (276153/276160) | 297 | 0 | 1 | FAIL |
| go | measured | 99.96% (418050/418197) | 99.96% (1311407/1311877) | 99.98% (362279/362341) | 5 | 0 | 11 | FAIL |
| php | measured | 99.70% (679958/681966) | 99.29% (640860/645401) | 99.99% (463261/463269) | 102 | 0 | 6 | FAIL |
| python | measured | 99.98% (448532/448615) | 99.89% (814551/815429) | 99.98% (234737/234771) | 33 | 0 | 12 | PASS |
| rust | no oracle | - | - | - | - | - | - | PASS |
| shell | no oracle | - | - | - | - | - | - | PASS |
| sql | no oracle | - | - | - | - | - | - | PASS |
| typescript-javascript | no oracle | - | - | - | - | - | - | PASS |

Files the dart oracle parses and codeint did not measure count as recall misses:

- `flutter/flutter engine/src/flutter/lib/web_ui/test/ui/paragraph_performance_test.dart`

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
- `phacility/phabricator scripts/manage_bulk.php`
- `open-telemetry/opentelemetry-php proto/otel/GPBMetadata/Opentelemetry/Proto/Metrics/Experimental/MetricsConfigService.php`
- `open-telemetry/opentelemetry-php proto/otel/GPBMetadata/Opentelemetry/Proto/Trace/V1/TraceConfig.php`

Files the python oracle parses and codeint did not measure count as recall misses:

- `langchain-ai/langchain libs/langchain/tests/integration_tests/examples/non-utf8-encoding.py`
- `saulpw/visidata visidata/apps/galcon/__init__.py`
- `ansible/ansible lib/ansible/module_utils/ansible_release.py`
- `ansible/ansible lib/ansible/modules/systemd.py`
- `ansible/ansible test/lib/ansible_test/_internal/junit_xml.py`
- `ansible/ansible test/support/windows-integration/collections/ansible_collections/ansible/windows/plugins/action/win_copy.py`
- `ansible/ansible test/support/windows-integration/collections/ansible_collections/ansible/windows/plugins/modules/win_acl.py`
- `ansible/ansible test/support/windows-integration/collections/ansible_collections/ansible/windows/plugins/modules/win_copy.py`
- `ansible/ansible test/support/windows-integration/collections/ansible_collections/ansible/windows/plugins/modules/win_file.py`
- `ansible/ansible test/support/windows-integration/collections/ansible_collections/ansible/windows/plugins/modules/win_ping.py`
- `ansible/ansible test/support/windows-integration/collections/ansible_collections/ansible/windows/plugins/modules/win_shell.py`
- `ansible/ansible test/support/windows-integration/collections/ansible_collections/ansible/windows/plugins/modules/win_stat.py`

## Speed and memory

| Repository | Language | Declared bucket | Measured bucket | Production lines | Median wall time | Peak RSS | Production lines per second |
| --- | --- | --- | --- | --- | --- | --- | --- |
| morluto/rea | typescript-javascript | large | large | 878227 | 1.699 s | 1243.7 MiB | 516983 |
| keystonejs/keystone | typescript-javascript | large | large | 64285 | 0.154 s | 76.4 MiB | 417459 |
| sc-forks/solidity-coverage | typescript-javascript | small | small | 2366 | 0.045 s | 19.7 MiB | 52536 |
| aurora-opensource/streetscape.gl | typescript-javascript | medium | medium | 15481 | 0.039 s | 25.5 MiB | 399056 |
| gitkraken/vscode-gitlens | typescript-javascript | large | large | 455567 | 0.833 s | 413.3 MiB | 547192 |
| affaan-m/ECC | typescript-javascript | large | large | 109036 | 0.714 s | 326.9 MiB | 152755 |
| deepseek-ai/deepseek-harness | typescript-javascript | large | large | 499150 | 2.669 s | 1430.2 MiB | 187001 |
| cloudflare/computer | typescript-javascript | large | large | 60576 | 0.170 s | 111.3 MiB | 357071 |
| swamp-club/swamp | typescript-javascript | large | large | 1672532 | 2.176 s | 906.9 MiB | 768743 |
| react/react | typescript-javascript | large | large | 137633 | 0.815 s | 304.2 MiB | 168782 |
| cloudflare/moltworker | typescript-javascript | small | small | 3988 | 0.015 s | 16.7 MiB | 266046 |
| haydenull/logseq-plugin-agenda | typescript-javascript | medium | medium | 16684 | 0.021 s | 21.2 MiB | 807351 |
| anomalyco/opencode | typescript-javascript | large | large | 866321 | 24.197 s | 1190.1 MiB | 35803 |
| guopenghui/obsidian-language-learner | typescript-javascript | small | small | 5513 | 0.010 s | 16.6 MiB | 579074 |
| vuejs/vue | typescript-javascript | medium | medium | 20865 | 0.055 s | 39.5 MiB | 382221 |
| open-slide/open-slide | typescript-javascript | large | large | 125745 | 0.107 s | 92.0 MiB | 1173270 |
| joBr99/nspanel-lovelace-ui | typescript-javascript | medium | medium | 40043 | 0.178 s | 91.0 MiB | 225224 |
| n8n-io/n8n | typescript-javascript | large | large | 2038940 | 12.733 s | 2552.3 MiB | 160127 |
| evolus/pencil | typescript-javascript | large | large | 73989 | 0.129 s | 79.0 MiB | 572580 |
| lobehub/lobe-vidol | typescript-javascript | large | large | 79736 | 0.091 s | 64.0 MiB | 879419 |
| firecrawl/firecrawl | typescript-javascript | large | large | 76503 | 0.262 s | 138.2 MiB | 292261 |
| oblador/react-native-animatable | typescript-javascript | small | small | 2053 | 0.012 s | 13.7 MiB | 175303 |
| appandflow/react-native-ease | typescript-javascript | small | small | 5639 | 1.539 s | 150.2 MiB | 3664 |
| DietrichGebert/ponytail | typescript-javascript | small | small | 3160 | 0.016 s | 20.4 MiB | 203794 |
| PierreBeucher/cloudypad | typescript-javascript | medium | medium | 14291 | 0.048 s | 32.1 MiB | 297148 |
| modem-dev/hunk | typescript-javascript | large | large | 132887 | 0.270 s | 222.2 MiB | 492106 |
| microsoft/vscode | typescript-javascript | large | large | 2346633 | 12.129 s | 2939.2 MiB | 193467 |
| usekaneo/kaneo | typescript-javascript | large | large | 252983 | 0.412 s | 227.3 MiB | 614076 |
| blocktronics/moebius | typescript-javascript | medium | medium | 21458 | 0.039 s | 38.9 MiB | 556832 |
| elie222/rakazo | typescript-javascript | large | large | 497908 | 1.913 s | 887.7 MiB | 260267 |
| clash-verge-rev/clash-verge-rev | typescript-javascript | medium | medium | 45752 | 0.119 s | 78.5 MiB | 385468 |
| DevExpress/testcafe | typescript-javascript | medium | medium | 41378 | 0.196 s | 166.9 MiB | 211264 |
| webdriverio/webdriverio | typescript-javascript | large | large | 105673 | 0.406 s | 201.9 MiB | 260263 |
| anthropics/claude-code | typescript-javascript | medium | medium | 25348 | 0.120 s | 50.8 MiB | 211132 |
| mattdelacdev/WOW | typescript-javascript | small | small | 7609 | 0.013 s | 17.6 MiB | 580426 |
| PicGo/PicGo-Core | typescript-javascript | medium | medium | 12133 | 0.025 s | 27.7 MiB | 483753 |
| hiteshchoudhary/apihub | typescript-javascript | medium | medium | 23655 | 0.028 s | 24.2 MiB | 841761 |
| swimlane/ngx-graph | typescript-javascript | small | small | 7429 | 0.015 s | 19.9 MiB | 493339 |
| iptv-org/iptv | typescript-javascript | small | small | 3762 | 0.012 s | 17.4 MiB | 313100 |
| kody-hq/kody | typescript-javascript | large | large | 862054 | 2.171 s | 696.6 MiB | 397022 |
| vercel/next.js | typescript-javascript | large | large | 1148860 | 56.490 s | 1690.1 MiB | 20337 |
| sghall/resonance | typescript-javascript | small | small | 4507 | 0.014 s | 17.1 MiB | 325220 |
| jae-jae/Camtd | typescript-javascript | medium | medium | 17893 | 1.440 s | 192.4 MiB | 12428 |
| garrytan/gstack | typescript-javascript | large | large | 150277 | 1.852 s | 612.6 MiB | 81151 |
| stablyai/orca | typescript-javascript | large | large | 3455327 | 7.357 s | 2169.4 MiB | 469652 |
| reactjs/zh-hans.react.dev | typescript-javascript | medium | medium | 46030 | 0.066 s | 42.0 MiB | 696674 |
| excalidraw/excalidraw | typescript-javascript | large | large | 129323 | 0.203 s | 89.5 MiB | 638441 |
| anvaka/city-roads | typescript-javascript | small | small | 2711 | 0.020 s | 14.8 MiB | 138981 |
| LING71671/Open-ClaudeCode | typescript-javascript | large | large | 420498 | 15.872 s | 786.1 MiB | 26494 |
| shadcn-ui/ui | typescript-javascript | large | large | 415871 | 2.148 s | 398.7 MiB | 193597 |
| Denrox/nestjs-microservices-example | typescript-javascript | small | small | 2308 | 0.021 s | 15.2 MiB | 108050 |
| earendil-works/pi | typescript-javascript | large | large | 338013 | 1.250 s | 561.5 MiB | 270352 |
| nodejs/node | typescript-javascript | large | large | 354321 | 8.269 s | 1277.1 MiB | 42850 |
| purifycss/purifycss | typescript-javascript | small | small | 2022 | 0.013 s | 14.7 MiB | 159740 |
| seo-skills/seo-audit-skill | typescript-javascript | large | large | 59183 | 0.095 s | 47.8 MiB | 622986 |
| TauricResearch/TradingAgents | typescript-javascript | medium | medium | 27453 | 0.065 s | 44.9 MiB | 419816 |
| a-ghorbani/pocketpal-ai | typescript-javascript | large | large | 100676 | 2.030 s | 154.6 MiB | 49604 |
| Leonxlnx/taste-skill | typescript-javascript | medium | medium | 37977 | 0.121 s | 75.2 MiB | 314238 |
| ChrisAbdo/MotionVariants | typescript-javascript | small | small | 4167 | 0.017 s | 14.4 MiB | 244680 |
| addyosmani/agent-skills | typescript-javascript | small | small | 5321 | 0.050 s | 20.4 MiB | 107290 |
| codebymitch/TitanBot | typescript-javascript | medium | medium | 44312 | 0.070 s | 37.4 MiB | 637134 |
| DataV-Team/DataV-React | typescript-javascript | small | small | 4511 | 0.023 s | 16.0 MiB | 192290 |
| yang991178/fluent-reader | typescript-javascript | medium | medium | 16354 | 1.007 s | 44.9 MiB | 16242 |
| mulaRahul/keyviz | typescript-javascript | small | small | 8592 | 0.052 s | 27.1 MiB | 164453 |
| shahabyazdi/react-multi-date-picker | typescript-javascript | medium | medium | 17903 | 0.024 s | 20.9 MiB | 739439 |
| Netflix-Skunkworks/sketchy | typescript-javascript | small | small | 2703 | 0.016 s | 15.4 MiB | 167069 |
| star-history/star-history | typescript-javascript | small | small | 7060 | 0.014 s | 18.0 MiB | 502822 |
| bevacqua/react-dragula | typescript-javascript | medium | medium | 11927 | 0.028 s | 26.4 MiB | 427785 |
| axios/axios | typescript-javascript | small | small | 8576 | 0.057 s | 37.2 MiB | 149516 |
| sindresorhus/ora | typescript-javascript | small | small | 4447 | 0.017 s | 16.0 MiB | 264466 |
| modelcontextprotocol/modelcontextprotocol | typescript-javascript | small | small | 6393 | 0.024 s | 22.2 MiB | 262578 |
| jenissimo/unfake.js | typescript-javascript | small | small | 4207 | 0.060 s | 32.3 MiB | 70305 |
| zuiidea/antd-admin | typescript-javascript | medium | medium | 10417 | 0.051 s | 23.7 MiB | 205518 |
| dreamhunter2333/chatgpt-tarot-divination | typescript-javascript | small | small | 2989 | 0.016 s | 17.1 MiB | 184274 |
| GoogleCloudPlatform/knowledge-catalog | typescript-javascript | medium | medium | 36146 | 0.147 s | 95.2 MiB | 245122 |
| runtipi/runtipi | typescript-javascript | medium | medium | 25843 | 0.130 s | 56.5 MiB | 199034 |
| OpenCut-app/OpenCut | typescript-javascript | small | small | 6539 | 0.021 s | 20.5 MiB | 313704 |
| bpmn-io/bpmn-js | typescript-javascript | medium | medium | 17481 | 0.107 s | 54.7 MiB | 162652 |
| Raathigesh/fabulous | typescript-javascript | small | small | 3136 | 0.015 s | 15.0 MiB | 214912 |
| sinonjs/sinon | typescript-javascript | small | small | 4942 | 0.049 s | 31.8 MiB | 100334 |
| tailwindlabs/tailwindcss | typescript-javascript | medium | medium | 32288 | 0.143 s | 79.2 MiB | 225324 |
| githubdulong/Script | typescript-javascript | medium | medium | 17260 | 0.103 s | 51.4 MiB | 167877 |
| voidcosmos/npkill | typescript-javascript | small | small | 8582 | 0.023 s | 22.2 MiB | 380467 |
| puppeteer/puppeteer | typescript-javascript | medium | medium | 45921 | 0.158 s | 72.4 MiB | 290438 |
| louislam/uptime-kuma | typescript-javascript | medium | medium | 39388 | 0.073 s | 58.3 MiB | 535961 |
| microsoft/fast | typescript-javascript | medium | medium | 25172 | 0.267 s | 88.7 MiB | 94314 |
| nexu-io/codex-slides | typescript-javascript | medium | medium | 32212 | 0.036 s | 36.4 MiB | 894038 |
| zulip/zulip-desktop | typescript-javascript | small | small | 6618 | 0.030 s | 18.4 MiB | 219109 |
| Egonex-AI/Understand-Anything | typescript-javascript | medium | medium | 33734 | 0.126 s | 68.1 MiB | 268425 |
| adamwdraper/Numeral-js | typescript-javascript | small | small | 4554 | 0.048 s | 19.4 MiB | 95733 |
| modelcontextprotocol/servers | typescript-javascript | medium | medium | 18539 | 0.075 s | 57.9 MiB | 247994 |
| margelo/react-native-vision-camera | typescript-javascript | medium | medium | 12773 | 0.044 s | 28.4 MiB | 292197 |
| ChatGPTNextWeb/NextChat | typescript-javascript | medium | medium | 37605 | 0.056 s | 45.6 MiB | 668370 |
| taskforcesh/bullmq | typescript-javascript | medium | medium | 20633 | 0.475 s | 168.6 MiB | 43471 |
| pastelsky/bundlephobia | typescript-javascript | medium | medium | 30877 | 0.045 s | 39.4 MiB | 679888 |
| unicity-sphere/sphere | typescript-javascript | medium | medium | 40348 | 0.108 s | 60.5 MiB | 374882 |
| ntegrals/openbrowser | typescript-javascript | medium | medium | 13870 | 0.023 s | 23.5 MiB | 609793 |
| ruanyf/webpack-demos | typescript-javascript | medium | medium | 48622 | 0.327 s | 91.1 MiB | 148722 |
| Fission-AI/OpenSpec | typescript-javascript | medium | medium | 36481 | 0.205 s | 105.5 MiB | 178228 |
| Eugeny/tabby | typescript-javascript | medium | medium | 26691 | 0.058 s | 41.5 MiB | 460875 |
| ultraworkers/claw-code | rust | large | large | 66932 | 0.279 s | 136.1 MiB | 239758 |
| SeaQL/sea-orm | rust | large | large | 71106 | 0.686 s | 192.6 MiB | 103663 |
| farion1231/cc-switch | rust | large | large | 180756 | 1.025 s | 346.5 MiB | 176417 |
| chenxiaolong/avbroot | rust | medium | medium | 21705 | 0.064 s | 52.5 MiB | 339509 |
| openai/codex | rust | large | large | 923524 | 7.996 s | 1615.1 MiB | 115500 |
| pkgxdev/pkgx | rust | small | small | 2566 | 0.021 s | 18.1 MiB | 123981 |
| rustdesk/rustdesk | rust | large | large | 153705 | 0.484 s | 247.6 MiB | 317513 |
| funstory-ai/BabelDOC | rust | medium | medium | 40655 | 0.309 s | 133.1 MiB | 131508 |
| lanyeeee/copymanga-downloader | rust | small | small | 5410 | 0.023 s | 25.1 MiB | 233383 |
| ki-editor/ki-editor | rust | large | large | 57827 | 0.194 s | 107.3 MiB | 297381 |
| pbakaus/impeccable | rust | large | large | 174633 | 5.006 s | 746.5 MiB | 34885 |
| H-M-H/Weylus | rust | small | small | 5631 | 0.027 s | 27.8 MiB | 205341 |
| estuary/flow | rust | large | large | 218496 | 4.077 s | 676.0 MiB | 53590 |
| tauri-apps/tauri | rust | large | large | 72342 | 0.577 s | 140.2 MiB | 125335 |
| messense/jieba-rs | rust | small | small | 2721 | 0.024 s | 23.8 MiB | 114667 |
| jely2002/youtube-dl-gui | rust | small | small | 8544 | 0.137 s | 32.7 MiB | 62350 |
| matthiaskrgr/cargo-cache | rust | small | small | 6072 | 0.071 s | 24.4 MiB | 85197 |
| lune-org/lune | rust | medium | medium | 16834 | 0.111 s | 39.7 MiB | 151147 |
| FyroxEngine/Fyrox | rust | large | large | 208676 | 0.816 s | 243.0 MiB | 255803 |
| zed-industries/zed | rust | large | large | 990700 | 6.142 s | 1354.6 MiB | 161291 |
| bottlerocket-os/bottlerocket | rust | small | small | 9590 | 0.141 s | 30.8 MiB | 67797 |
| astral-sh/uv | rust | large | large | 202794 | 2.950 s | 432.3 MiB | 68754 |
| linebender/druid | rust | medium | medium | 39982 | 0.210 s | 80.5 MiB | 190794 |
| GaoSSR/best-claude-hud | rust | small | small | 8948 | 0.049 s | 30.7 MiB | 183341 |
| rtk-ai/rtk | rust | large | large | 85424 | 0.476 s | 182.5 MiB | 179348 |
| loco-rs/loco | rust | medium | medium | 28842 | 0.296 s | 82.7 MiB | 97352 |
| unionlabs/union | rust | large | large | 181032 | 5.920 s | 1357.7 MiB | 30580 |
| bbqsrc/cargo-ndk | rust | small | small | 2131 | 0.018 s | 17.8 MiB | 115235 |
| dani-garcia/vaultwarden | rust | medium | medium | 31443 | 0.142 s | 92.1 MiB | 222021 |
| skymanbp/autoshade | rust | large | large | 101641 | 0.660 s | 324.3 MiB | 153983 |
| BurntSushi/ripgrep | rust | medium | medium | 25014 | 0.132 s | 80.3 MiB | 189693 |
| humblepenguinn/envio | rust | small | small | 5089 | 0.035 s | 22.2 MiB | 143481 |
| ccfddl/ccf-deadlines | rust | small | small | 8344 | 0.120 s | 39.2 MiB | 69644 |
| image-rs/imageproc | rust | small | small | 9145 | 0.055 s | 39.8 MiB | 166668 |
| openinterpreter/openinterpreter | rust | large | large | 752904 | 6.155 s | 1347.5 MiB | 122317 |
| qarmin/szyszka | rust | small | small | 3070 | 0.042 s | 20.7 MiB | 72409 |
| alacritty/alacritty | rust | medium | medium | 20755 | 0.080 s | 55.5 MiB | 258229 |
| paradedb/paradedb | rust | large | large | 155080 | 0.703 s | 420.9 MiB | 220586 |
| winnow-rs/winnow | rust | medium | medium | 11538 | 0.097 s | 56.4 MiB | 118674 |
| jpochyla/psst | rust | medium | medium | 19116 | 0.070 s | 39.7 MiB | 273733 |
| k06a/macpow | rust | small | small | 7311 | 0.039 s | 28.6 MiB | 185398 |
| databendlabs/databend | rust | large | large | 673349 | 3.117 s | 900.8 MiB | 216016 |
| apollographql/router | rust | large | large | 228867 | 1.132 s | 360.2 MiB | 202208 |
| tw93/Pake | rust | small | small | 9868 | 0.059 s | 47.9 MiB | 166104 |
| 1Password/arboard | rust | small | small | 2592 | 0.021 s | 21.0 MiB | 125081 |
| sharkdp/bat | rust | small | small | 8634 | 0.182 s | 62.6 MiB | 47311 |
| omerbenamram/evtx | rust | medium | medium | 16971 | 0.126 s | 65.4 MiB | 134774 |
| FuelLabs/sway | rust | large | large | 177626 | 1.103 s | 243.2 MiB | 160985 |
| actix/actix | rust | small | small | 5523 | 0.042 s | 27.9 MiB | 130321 |
| torrentplayer/torrentplayer | rust | medium | medium | 37094 | 0.359 s | 96.2 MiB | 103373 |
| starship/starship | rust | medium | medium | 20476 | 0.116 s | 56.2 MiB | 175912 |
| storytold/effectcraft | rust | large | large | 296619 | 2.882 s | 705.1 MiB | 102907 |
| superradcompany/microsandbox | rust | large | large | 302529 | 2.869 s | 497.6 MiB | 105461 |
| wasm-bindgen/wasm-bindgen | rust | large | large | 295349 | 5.987 s | 682.2 MiB | 49330 |
| LLukas22/Jellyswarrm | rust | medium | medium | 44746 | 0.185 s | 89.3 MiB | 241789 |
| typst/typst | rust | large | large | 95614 | 0.732 s | 166.6 MiB | 130693 |
| kata-containers/kata-containers | rust | large | large | 139677 | 1.494 s | 266.0 MiB | 93507 |
| EpicGames/lore | rust | large | large | 331667 | 5.327 s | 902.9 MiB | 62262 |
| crocuda/pipelight | rust | small | small | 6469 | 0.043 s | 26.2 MiB | 150017 |
| FuelLabs/fuel-core | rust | large | large | 98333 | 0.949 s | 156.3 MiB | 103609 |
| spacejam/sled | rust | small | small | 6000 | 0.057 s | 30.6 MiB | 105812 |
| bytestring-net/bevy_lunex | rust | small | small | 4635 | 0.056 s | 34.1 MiB | 83450 |
| Morganamilo/paru | rust | medium | medium | 10753 | 0.071 s | 40.5 MiB | 152469 |
| yuxino/Mimi | rust | large | large | 77176 | 0.897 s | 208.1 MiB | 86003 |
| vercel-labs/agent-browser | rust | large | large | 93180 | 2.469 s | 283.1 MiB | 37737 |
| pydantic/monty | rust | large | large | 142931 | 1.051 s | 356.3 MiB | 136009 |
| mfontanini/presenterm | rust | medium | medium | 16815 | 0.062 s | 41.5 MiB | 272173 |
| j178/prek | rust | medium | medium | 34936 | 0.177 s | 80.0 MiB | 197229 |
| rust-num/num-traits | rust | small | small | 3784 | 0.045 s | 27.8 MiB | 84277 |
| clojure-rs/ClojureRS | rust | small | small | 4995 | 0.030 s | 25.0 MiB | 167724 |
| vi/websocat | rust | small | small | 9867 | 0.076 s | 28.0 MiB | 129144 |
| sharkdp/fd | rust | small | small | 3373 | 0.057 s | 22.2 MiB | 58923 |
| crossbeam-rs/crossbeam | rust | medium | medium | 13785 | 0.197 s | 69.1 MiB | 69916 |
| tower-rs/tower-http | rust | medium | medium | 13371 | 0.079 s | 42.6 MiB | 168544 |
| RustCrypto/AEADs | rust | small | small | 4486 | 0.102 s | 37.6 MiB | 43907 |
| Leafwing-Studios/leafwing-input-manager | rust | small | small | 7882 | 0.107 s | 38.4 MiB | 73803 |
| FuelLabs/fuels-rs | rust | medium | medium | 16058 | 0.128 s | 52.7 MiB | 125134 |
| Nazariglez/notan | rust | medium | medium | 18640 | 0.088 s | 44.5 MiB | 211212 |
| unicity-aos/aos-ce | rust | medium | medium | 37509 | 0.193 s | 82.9 MiB | 194632 |
| anza-xyz/pinocchio | rust | medium | medium | 17271 | 0.148 s | 55.4 MiB | 116701 |
| pop-os/libcosmic | rust | medium | medium | 45933 | 0.163 s | 63.5 MiB | 281400 |
| neon-bindings/neon | rust | medium | medium | 13868 | 0.059 s | 47.6 MiB | 236739 |
| RoaringBitmap/roaring-rs | rust | small | small | 8686 | 0.049 s | 39.5 MiB | 175922 |
| Ruddle/Fomos | rust | small | small | 6856 | 0.019 s | 25.1 MiB | 357231 |
| fwdai/rowel | rust | medium | medium | 45674 | 1.190 s | 143.4 MiB | 38389 |
| cjpais/Handy | rust | medium | medium | 23081 | 0.960 s | 61.0 MiB | 24048 |
| cross-rs/cross | rust | medium | medium | 11551 | 0.035 s | 36.7 MiB | 327509 |
| pls-rs/pls | rust | small | small | 3981 | 0.026 s | 24.5 MiB | 152592 |
| AlexsJones/llmfit | rust | medium | medium | 37486 | 0.249 s | 113.9 MiB | 150716 |
| zerobrewhq/zerobrew | rust | medium | medium | 14252 | 0.393 s | 46.0 MiB | 36301 |
| paperclip-rs/paperclip | rust | medium | medium | 13889 | 0.238 s | 39.7 MiB | 58351 |
| ck-zhang/reddix | rust | medium | medium | 16789 | 0.826 s | 46.3 MiB | 20338 |
| fujiapple852/trippy | rust | medium | medium | 17119 | 0.166 s | 53.3 MiB | 103001 |
| alloy-rs/core | rust | medium | medium | 32530 | 0.878 s | 78.2 MiB | 37051 |
| napi-rs/napi-rs | rust | medium | medium | 48962 | 2.300 s | 237.8 MiB | 21289 |
| jeremychone/rust-genai | rust | medium | medium | 18076 | 0.123 s | 49.6 MiB | 146438 |
| tokio-rs/axum | rust | medium | medium | 16103 | 0.156 s | 59.4 MiB | 103290 |
| rust-lang/rustup | rust | medium | medium | 15159 | 0.115 s | 62.8 MiB | 131666 |
| actix/actix-web | rust | medium | medium | 36952 | 0.246 s | 86.5 MiB | 149985 |
| tokio-rs/tracing | rust | medium | medium | 23328 | 0.234 s | 78.5 MiB | 99785 |
| coollabsio/coolify | php | large | large | 227555 | 17.455 s | 537.2 MiB | 13037 |
| kanboard/kanboard | php | large | large | 117429 | 1.766 s | 197.5 MiB | 66491 |
| bobthecow/psysh | php | medium | medium | 36911 | 0.108 s | 56.6 MiB | 341165 |
| laravel/framework | php | large | large | 132960 | 1.486 s | 276.3 MiB | 89462 |
| googleapis/google-api-php-client | php | small | small | 3169 | 0.013 s | 15.6 MiB | 235061 |
| symfony/console | php | medium | medium | 17886 | 0.076 s | 40.7 MiB | 236030 |
| nuxsmin/sysPass | php | large | large | 61038 | 0.762 s | 183.4 MiB | 80068 |
| symfony/symfony | php | large | large | 1030120 | 4.233 s | 1024.5 MiB | 243361 |
| doctrine/dbal | php | medium | medium | 31726 | 0.101 s | 36.3 MiB | 313421 |
| composer/composer | php | large | large | 50034 | 0.125 s | 46.4 MiB | 401542 |
| pterodactyl/panel | php | medium | medium | 26410 | 1.204 s | 169.1 MiB | 21940 |
| SolidInvoice/SolidInvoice | php | large | large | 68748 | 0.400 s | 107.0 MiB | 171993 |
| monicahq/monica | php | large | large | 171658 | 1.060 s | 252.3 MiB | 162003 |
| firefly-iii/firefly-iii | php | large | large | 109763 | 0.924 s | 166.6 MiB | 118802 |
| krayin/laravel-crm | php | large | large | 59939 | 1.864 s | 188.5 MiB | 32149 |
| dingo/api | php | small | small | 4250 | 0.025 s | 15.0 MiB | 166862 |
| livewire/livewire | php | medium | medium | 21413 | 0.190 s | 55.7 MiB | 112656 |
| RSS-Bridge/rss-bridge | php | large | large | 74110 | 0.090 s | 54.8 MiB | 823321 |
| guzzle/guzzle | php | medium | medium | 12931 | 0.114 s | 47.3 MiB | 113470 |
| phpDocumentor/ReflectionDocBlock | php | small | small | 3222 | 0.023 s | 15.2 MiB | 137159 |
| PHPMailer/PHPMailer | php | small | small | 5717 | 0.059 s | 20.4 MiB | 96313 |
| PrestaShop/PrestaShop | php | large | large | 385016 | 7.081 s | 804.8 MiB | 54370 |
| DesignPatternsPHP/DesignPatternsPHP | php | small | small | 2018 | 0.022 s | 13.3 MiB | 91405 |
| matomo-org/matomo | php | large | large | 273264 | 8.009 s | 638.5 MiB | 34118 |
| solidtime-io/solidtime | php | medium | medium | 25908 | 6.047 s | 134.6 MiB | 4284 |
| phpDocumentor/TypeResolver | php | small | small | 2654 | 0.193 s | 48.1 MiB | 13763 |
| TasmoAdmin/TasmoAdmin | php | small | small | 6382 | 0.038 s | 25.2 MiB | 169309 |
| Seldaek/monolog | php | small | small | 8691 | 0.032 s | 19.5 MiB | 273480 |
| sebastianbergmann/phpunit | php | large | large | 85993 | 0.421 s | 110.8 MiB | 204498 |
| fruitcake/laravel-debugbar | php | small | small | 5231 | 0.044 s | 19.0 MiB | 120071 |
| sebastianbergmann/php-code-coverage | php | medium | medium | 16042 | 0.139 s | 44.9 MiB | 115611 |
| ellite/Wallos | php | medium | medium | 46835 | 1.984 s | 81.7 MiB | 23610 |
| FTXInfotech/laravel-adminpanel | php | medium | medium | 12612 | 2.037 s | 332.7 MiB | 6192 |
| bcit-ci/CodeIgniter | php | medium | medium | 28489 | 0.074 s | 35.1 MiB | 383383 |
| nikic/PHP-Parser | php | medium | medium | 17251 | 0.167 s | 45.8 MiB | 103249 |
| owncloud/core | php | large | large | 116050 | 0.694 s | 283.4 MiB | 167101 |
| commerceguys/addressing | php | small | small | 5217 | 0.026 s | 16.0 MiB | 203737 |
| koel/koel | php | medium | medium | 41428 | 0.282 s | 107.9 MiB | 146725 |
| cakephp/cakephp | php | large | large | 77300 | 0.669 s | 155.6 MiB | 115497 |
| aimeos/aimeos-laravel | php | small | small | 2500 | 0.014 s | 13.7 MiB | 182843 |
| briannesbitt/Carbon | php | medium | medium | 30625 | 0.258 s | 82.5 MiB | 118515 |
| symfony/http-foundation | php | small | small | 9108 | 0.047 s | 22.2 MiB | 194737 |
| Sylius/Sylius | php | large | large | 172192 | 0.997 s | 217.9 MiB | 172748 |
| getgrav/grav | php | large | large | 70378 | 0.407 s | 78.3 MiB | 172850 |
| swlib/saber | php | small | small | 2334 | 0.011 s | 12.4 MiB | 207809 |
| phpspec/prophecy | php | small | small | 9821 | 0.028 s | 16.2 MiB | 356923 |
| grokability/snipe-it | php | large | large | 583444 | 6.602 s | 595.7 MiB | 88368 |
| barryvdh/laravel-ide-helper | php | small | small | 3677 | 0.023 s | 16.7 MiB | 158324 |
| crater-invoice-inc/crater | php | medium | medium | 29109 | 19.565 s | 553.3 MiB | 1488 |
| osgochina/swoole-crontab | php | small | small | 4970 | 3.091 s | 250.0 MiB | 1608 |
| opencart/opencart | php | large | large | 92137 | 21.759 s | 1265.5 MiB | 4234 |
| zdhxiong/mdclub | php | medium | medium | 11555 | 0.035 s | 23.2 MiB | 327362 |
| Intervention/image | php | medium | medium | 17000 | 0.192 s | 52.2 MiB | 88616 |
| symfony/http-kernel | php | medium | medium | 11930 | 0.065 s | 31.4 MiB | 182288 |
| Las-Fuerzas-Del-Cielo/Sistema-Anti-Fraude-Electoral | php | large | large | 102997 | 0.106 s | 62.0 MiB | 970207 |
| phpstan/phpstan | php | small | small | 2563 | 0.035 s | 24.7 MiB | 72553 |
| fightbulc/moment.php | php | small | small | 3033 | 0.018 s | 14.0 MiB | 166028 |
| digininja/DVWA | php | small | small | 8614 | 0.020 s | 21.6 MiB | 422959 |
| tedious/Stash | php | small | small | 2406 | 0.011 s | 14.5 MiB | 225036 |
| PHPOffice/PhpSpreadsheet | php | large | large | 106653 | 0.596 s | 161.2 MiB | 178838 |
| phpmyadmin/phpmyadmin | php | large | large | 96506 | 0.372 s | 136.2 MiB | 259544 |
| pdepend/pdepend | php | medium | medium | 19580 | 0.254 s | 66.5 MiB | 77084 |
| vrana/adminer | php | medium | medium | 38034 | 0.115 s | 46.8 MiB | 331486 |
| thephpleague/flysystem | php | medium | medium | 10123 | 0.026 s | 20.9 MiB | 391635 |
| PHP-CS-Fixer/PHP-CS-Fixer | php | large | large | 63682 | 0.681 s | 211.2 MiB | 93563 |
| web-token/jwt-framework | php | medium | medium | 28687 | 0.163 s | 56.5 MiB | 176482 |
| filp/whoops | php | small | small | 2157 | 0.026 s | 16.8 MiB | 82156 |
| symfony/polyfill-mbstring | php | small | small | 4626 | 0.017 s | 14.0 MiB | 272046 |
| CodepadME/laravel-tricks | php | small | small | 3615 | 0.088 s | 30.4 MiB | 41017 |
| wallabag/wallabag | php | medium | medium | 20514 | 0.134 s | 50.2 MiB | 152532 |
| predis/predis | php | medium | medium | 22594 | 0.202 s | 64.2 MiB | 111905 |
| mlazarov/supervisord-monitor | php | medium | medium | 24568 | 0.180 s | 32.0 MiB | 136506 |
| spatie/laravel-permission | php | small | small | 2608 | 0.026 s | 25.2 MiB | 100364 |
| pgrimaud/instagram-user-feed | php | small | small | 6126 | 0.018 s | 16.5 MiB | 348306 |
| HaschekSolutions/pictshare | php | small | small | 4669 | 0.113 s | 31.3 MiB | 41365 |
| SpartnerNL/Laravel-Excel | php | small | small | 7326 | 0.034 s | 20.1 MiB | 216341 |
| phparkitect/arkitect | php | small | small | 4848 | 0.023 s | 18.7 MiB | 214035 |
| ramsey/uuid | php | small | small | 4424 | 0.029 s | 22.8 MiB | 153532 |
| typecho/typecho | php | medium | medium | 23232 | 0.149 s | 57.0 MiB | 155440 |
| atymic/twitter | php | small | small | 2533 | 0.012 s | 13.1 MiB | 206665 |
| aureuserp/aureuserp | php | large | large | 429338 | 5.678 s | 514.0 MiB | 75618 |
| phacility/phabricator | php | large | large | 482175 | 1.401 s | 294.5 MiB | 344087 |
| PHPOffice/PHPWord | php | medium | medium | 27074 | 0.179 s | 45.0 MiB | 151264 |
| AdvancedCustomFields/acf | php | medium | medium | 35689 | 0.529 s | 135.7 MiB | 67450 |
| Leantime/leantime | php | large | large | 79468 | 0.668 s | 173.2 MiB | 118896 |
| pestphp/pest | php | medium | medium | 21066 | 0.078 s | 37.3 MiB | 269104 |
| WINBIGFOX/TimeScribe | php | medium | medium | 15695 | 0.045 s | 27.4 MiB | 345122 |
| investbrainapp/investbrain | php | medium | medium | 11589 | 2.111 s | 143.9 MiB | 5489 |
| dompdf/dompdf | php | medium | medium | 22191 | 0.054 s | 26.6 MiB | 410703 |
| octobercms/october | php | large | large | 119795 | 27.432 s | 730.6 MiB | 4367 |
| roundcube/roundcubemail | php | large | large | 63416 | 0.397 s | 110.6 MiB | 159586 |
| libretime/libretime | php | large | large | 90706 | 4.872 s | 210.9 MiB | 18618 |
| z-song/laravel-admin | php | medium | medium | 23640 | 0.360 s | 60.1 MiB | 65650 |
| laramint/laravel-brain | php | medium | medium | 23558 | 1.642 s | 177.8 MiB | 14346 |
| nextcloud/all-in-one | php | medium | medium | 12857 | 0.038 s | 24.0 MiB | 334236 |
| open-telemetry/opentelemetry-php | php | medium | medium | 27889 | 0.143 s | 56.7 MiB | 195550 |
| anonaddy/anonaddy | php | medium | medium | 36453 | 0.133 s | 43.3 MiB | 273484 |
| vanilophp/framework | php | medium | medium | 20014 | 0.183 s | 36.5 MiB | 109290 |
| Codeception/Codeception | php | medium | medium | 15208 | 0.053 s | 26.5 MiB | 285632 |
| webonyx/graphql-php | php | medium | medium | 21389 | 0.074 s | 35.2 MiB | 289563 |
| flutter/flutter | dart | large | large | 807242 | 10.023 s | 1174.2 MiB | 80541 |
| firebase/flutterfire | dart | large | large | 64761 | 0.443 s | 105.6 MiB | 146339 |
| AppFlowy-IO/AppFlowy | dart | large | large | 229243 | 0.600 s | 236.9 MiB | 381909 |
| ABausG/home_widget | dart | medium | medium | 31117 | 0.088 s | 45.2 MiB | 353626 |
| chen08209/FlClash | dart | large | large | 135119 | 0.829 s | 226.5 MiB | 162904 |
| lollipopkit/flutter_server_box | dart | large | large | 246268 | 3.047 s | 650.5 MiB | 80836 |
| namanshergill/diohub | dart | large | large | 57772 | 0.090 s | 47.1 MiB | 638530 |
| brianegan/flutter_architecture_samples | dart | medium | medium | 12616 | 0.117 s | 27.4 MiB | 107745 |
| Predidit/Kazumi | dart | large | large | 65967 | 0.069 s | 34.5 MiB | 956865 |
| simplezhli/flutter_deer | dart | medium | medium | 16839 | 0.031 s | 19.2 MiB | 550813 |
| Davies-K/Taskez | dart | small | small | 8239 | 0.028 s | 14.9 MiB | 298627 |
| Sle2p/AniCh | dart | medium | medium | 27488 | 0.087 s | 32.2 MiB | 317280 |
| OpenStrap/edge | dart | large | large | 153065 | 3.302 s | 227.6 MiB | 46359 |
| ComicSparks/pikapika | dart | medium | medium | 31946 | 0.122 s | 40.3 MiB | 261980 |
| alibaba/flutter-go | dart | medium | medium | 26521 | 0.074 s | 30.1 MiB | 356474 |
| nightmare-space/any_share | dart | small | small | 9122 | 0.026 s | 18.7 MiB | 350451 |
| ImranR98/Obtainium | dart | medium | medium | 33892 | 0.050 s | 42.1 MiB | 682212 |
| alibaba/fish-redux | dart | small | small | 4484 | 0.029 s | 18.9 MiB | 153354 |
| yako-dev/flutter-settings-ui | dart | medium | medium | 16908 | 0.037 s | 24.0 MiB | 462046 |
| getlantern/lantern | dart | medium | medium | 46251 | 0.435 s | 72.3 MiB | 106212 |
| sunarya-thito/shadcn_flutter | dart | large | large | 132538 | 0.412 s | 108.0 MiB | 321714 |
| xiaoyaocz/dart_simple_live | dart | medium | medium | 39907 | 1.060 s | 37.8 MiB | 37648 |
| UnboundTechCo/defyxVPN | dart | medium | medium | 21628 | 0.062 s | 29.6 MiB | 346088 |
| bagisto/opensource-ecommerce-mobile-app | dart | large | large | 71308 | 0.108 s | 45.6 MiB | 661734 |
| dart-lang/pub-dev | dart | large | large | 66434 | 0.306 s | 96.7 MiB | 217172 |
| CarGuo/gsy_github_app_flutter | dart | medium | medium | 36339 | 0.083 s | 39.1 MiB | 437900 |
| wanghongenpin/proxypin | dart | large | large | 67319 | 0.141 s | 72.2 MiB | 476587 |
| 2d-inc/HistoryOfEverything | dart | small | small | 4833 | 0.016 s | 15.5 MiB | 307359 |
| Notsfsssf/pixez-flutter | dart | large | large | 62354 | 0.332 s | 135.8 MiB | 187629 |
| cfug/dio | dart | small | small | 7698 | 0.034 s | 23.0 MiB | 229335 |
| samarthagarwal/FlutterScreens | dart | small | small | 3362 | 0.010 s | 13.5 MiB | 338928 |
| ubuntu/app-center | dart | medium | medium | 19309 | 0.055 s | 34.8 MiB | 352837 |
| felangel/bloc | dart | medium | medium | 11606 | 0.072 s | 40.0 MiB | 161225 |
| Canardoux/flutter_sound | dart | small | small | 6988 | 0.027 s | 30.0 MiB | 260856 |
| flutterchina/flukit | dart | small | small | 4586 | 0.012 s | 14.0 MiB | 378603 |
| jonataslaw/getx | dart | medium | medium | 15922 | 0.037 s | 24.7 MiB | 433804 |
| jiangtian616/JHenTai | dart | large | large | 56631 | 0.169 s | 57.5 MiB | 335624 |
| flame-engine/flame | dart | large | large | 102657 | 0.288 s | 117.8 MiB | 356870 |
| miru-project/miru-app | dart | medium | medium | 19261 | 0.096 s | 36.3 MiB | 200806 |
| KaringX/clashmi | dart | medium | medium | 35882 | 2.287 s | 194.2 MiB | 15690 |
| Sky24n/flutter_wanandroid | dart | small | small | 7314 | 0.018 s | 17.6 MiB | 400142 |
| StacDev/stac | dart | medium | medium | 46713 | 0.128 s | 66.1 MiB | 363701 |
| ComicSparks/jasmine | dart | medium | medium | 19537 | 0.043 s | 26.0 MiB | 458748 |
| mirarr-app/network-checker | dart | medium | medium | 47487 | 0.060 s | 34.9 MiB | 790828 |
| btwld/fvm | dart | small | small | 9465 | 0.053 s | 29.6 MiB | 177283 |
| RivaanRanawat/instagram-flutter-clone | dart | small | small | 2253 | 0.008 s | 12.4 MiB | 274935 |
| liuchuancong/pure_live_TV | dart | large | large | 101277 | 0.417 s | 127.4 MiB | 242832 |
| 4seer/openflutterecommerceapp | dart | medium | medium | 14298 | 0.021 s | 18.8 MiB | 668088 |
| jesusrp98/spacex-go | dart | small | small | 6157 | 0.018 s | 16.0 MiB | 334595 |
| rrousselGit/provider | dart | small | small | 4209 | 0.036 s | 22.0 MiB | 117928 |
| appshubcc/Bettbox | dart | large | large | 116100 | 0.979 s | 260.4 MiB | 118612 |
| appditto/natrium_wallet_flutter | dart | medium | medium | 26770 | 0.071 s | 40.7 MiB | 377781 |
| Regncreative/PulseDiagnostics | dart | medium | medium | 41735 | 0.107 s | 44.7 MiB | 389665 |
| echo-loop/Echo-Loop | dart | large | large | 182413 | 0.773 s | 212.1 MiB | 236086 |
| pawlik92/flutter_whirlpool | dart | small | small | 2303 | 0.008 s | 12.5 MiB | 292928 |
| bepass-org/oblivion | dart | medium | medium | 12297 | 0.035 s | 34.5 MiB | 348115 |
| Tencent/mxflutter | dart | large | large | 86909 | 0.573 s | 82.2 MiB | 151651 |
| harmonoid/harmonoid | dart | medium | medium | 25140 | 0.066 s | 33.8 MiB | 379776 |
| ionicfirebaseapp/getwidget | dart | medium | medium | 11792 | 0.048 s | 24.3 MiB | 244266 |
| aaronoe/FlutterCinematic | dart | small | small | 2000 | 0.008 s | 12.0 MiB | 243251 |
| Chevey339/kelivo | dart | large | large | 660589 | 10.658 s | 720.3 MiB | 61982 |
| hacktons/convex_bottom_bar | dart | small | small | 2492 | 0.013 s | 12.6 MiB | 188073 |
| sdegenaar/liquid_glass_widgets | dart | large | large | 96131 | 0.222 s | 107.1 MiB | 433071 |
| mhmzdev/the-holy-quran-app | dart | small | small | 4017 | 0.027 s | 16.5 MiB | 148957 |
| hamaluik/timecop | dart | small | small | 8733 | 0.020 s | 18.3 MiB | 436732 |
| gskinnerTeam/flutter-wonderous-app | dart | medium | medium | 23407 | 0.040 s | 28.3 MiB | 585587 |
| bp74/StageXL | dart | medium | medium | 17524 | 0.034 s | 22.7 MiB | 522143 |
| gokadzev/Musify | dart | medium | medium | 39949 | 0.058 s | 39.2 MiB | 692733 |
| mirarr-app/mirarr | dart | medium | medium | 28302 | 0.053 s | 27.0 MiB | 535849 |
| guilherme-v/flutter-clean-architecture-example | dart | small | small | 3020 | 0.013 s | 14.2 MiB | 225357 |
| GitJournal/GitJournal | dart | medium | medium | 28316 | 0.093 s | 45.3 MiB | 304402 |
| YuanDevTeam/OneXray | dart | large | large | 67241 | 0.150 s | 103.3 MiB | 446874 |
| sass/dart-sass | dart | medium | medium | 40673 | 0.202 s | 79.8 MiB | 201188 |
| avdept/JellyBoxPlayer | dart | large | large | 86644 | 0.220 s | 108.9 MiB | 394177 |
| TheAlphamerc/flutter_twitter_clone | dart | medium | medium | 13582 | 0.021 s | 17.2 MiB | 637387 |
| ryanheise/audio_service | dart | small | small | 5891 | 0.022 s | 17.5 MiB | 264286 |
| bostrot/wslmanager | dart | large | large | 56761 | 0.127 s | 72.6 MiB | 446729 |
| xuelongqy/flutter_easy_refresh | dart | medium | medium | 17664 | 0.067 s | 27.8 MiB | 264704 |
| Sky24n/GreenTravel | dart | small | small | 2339 | 0.009 s | 13.4 MiB | 265136 |
| isar/isar | dart | medium | medium | 10399 | 0.080 s | 43.3 MiB | 130119 |
| dart-lang/build | dart | medium | medium | 40436 | 0.328 s | 94.0 MiB | 123350 |
| akaMrNagar/Mindful | dart | large | large | 50358 | 0.098 s | 49.5 MiB | 515772 |
| gskinnerTeam/flutter-folio | dart | medium | medium | 11083 | 0.019 s | 17.2 MiB | 569125 |
| tekartik/sembast.dart | dart | medium | medium | 11395 | 0.052 s | 30.9 MiB | 218443 |
| pichillilorenzo/flutter_inappwebview | dart | large | large | 109308 | 0.558 s | 158.3 MiB | 195969 |
| cddqssc/Caption-Trans | dart | small | small | 8250 | 0.019 s | 17.7 MiB | 423685 |
| HTWMedia/HTV | dart | small | small | 5686 | 0.012 s | 15.4 MiB | 465728 |
| roughike/inKino | dart | small | small | 5042 | 0.014 s | 15.8 MiB | 349412 |
| Tarikul711/flutter-ecommerce | dart | small | small | 2290 | 0.012 s | 12.6 MiB | 184873 |
| best-flutter/flutter_swiper | dart | small | small | 2653 | 0.015 s | 13.7 MiB | 175905 |
| Tkko/Flutter_Pinput | dart | small | small | 3634 | 0.017 s | 17.7 MiB | 214392 |
| GhostenEditor/Ghosten-Player | dart | medium | medium | 31040 | 0.043 s | 29.4 MiB | 723085 |
| ReactiveX/rxdart | dart | small | small | 6582 | 0.044 s | 27.2 MiB | 149163 |
| kangpeiqin/bilivideo_down | dart | small | small | 2947 | 0.010 s | 12.6 MiB | 307868 |
| Hyper-Beast/BiliTV | dart | medium | medium | 17090 | 0.018 s | 18.9 MiB | 960128 |
| GimelStudio/GimelStudio | dart | small | small | 8142 | 0.019 s | 16.9 MiB | 417654 |
| memex-lab/memex | dart | large | large | 132941 | 0.285 s | 129.5 MiB | 466116 |
| simolus3/drift | dart | large | large | 93158 | 0.491 s | 141.0 MiB | 189557 |
| iebb/mithka | dart | large | large | 301912 | 0.369 s | 206.6 MiB | 818428 |
| breeze-project-org/Breeze | dart | large | large | 84508 | 0.418 s | 146.8 MiB | 202236 |
| open-mmlab/mmsegmentation | python | large | large | 62867 | 0.165 s | 56.8 MiB | 381734 |
| google-deepmind/sonnet | python | small | small | 9251 | 0.032 s | 22.9 MiB | 289228 |
| ruotianluo/self-critical.pytorch | python | small | small | 6130 | 0.097 s | 23.5 MiB | 62984 |
| huggingface/accelerate | python | medium | medium | 34480 | 0.054 s | 40.3 MiB | 643725 |
| Ekultek/Zeus-Scanner | python | small | small | 5714 | 0.011 s | 15.7 MiB | 513755 |
| awslabs/mcp | python | large | large | 272427 | 1.475 s | 361.5 MiB | 184678 |
| baidu/DDParser | python | small | small | 8689 | 0.016 s | 18.6 MiB | 552494 |
| yt-dlp/yt-dlp | python | large | large | 201346 | 0.185 s | 121.5 MiB | 1088731 |
| anthropics/skills | python | medium | medium | 17061 | 0.050 s | 30.4 MiB | 338611 |
| scofield7419/Video-of-Thought | python | small | small | 7538 | 0.016 s | 18.8 MiB | 477731 |
| microsoft/markitdown | python | small | small | 7117 | 0.017 s | 18.1 MiB | 415535 |
| keras-team/keras-hub | python | large | large | 213794 | 0.369 s | 143.6 MiB | 580074 |
| boto/boto3 | python | small | small | 6820 | 0.020 s | 20.8 MiB | 340715 |
| MichalGeyer/plug-and-play | python | small | small | 8194 | 0.016 s | 17.8 MiB | 517273 |
| paramiko/paramiko | python | medium | medium | 15278 | 0.016 s | 19.1 MiB | 962704 |
| AUTOMATIC1111/stable-diffusion-webui | python | medium | medium | 33074 | 0.096 s | 47.7 MiB | 343804 |
| dflook/terraform-github-actions | python | small | small | 6828 | 0.027 s | 17.7 MiB | 249434 |
| nextlevelbuilder/ui-ux-pro-max-skill | python | medium | medium | 24510 | 0.293 s | 64.5 MiB | 83744 |
| simpleai-team/simpleai | python | small | small | 3118 | 0.103 s | 24.2 MiB | 30152 |
| Graphify-Labs/graphify | python | large | large | 91932 | 0.317 s | 162.3 MiB | 289912 |
| github/spec-kit | python | large | large | 75095 | 0.336 s | 139.2 MiB | 223774 |
| darknessomi/musicbox | python | small | small | 6772 | 0.016 s | 16.7 MiB | 415196 |
| otto-torino/django-baton | python | small | small | 3093 | 0.525 s | 39.1 MiB | 5887 |
| langchain-ai/langchain | python | large | large | 205996 | 0.556 s | 166.9 MiB | 370797 |
| piglei/ai-vocabulary-builder | python | small | small | 2172 | 0.014 s | 15.2 MiB | 160672 |
| Comfy-Org/ComfyUI | python | large | large | 256581 | 0.541 s | 255.8 MiB | 474393 |
| pymc-devs/pymc | python | large | large | 51954 | 0.076 s | 60.4 MiB | 679817 |
| ADLab-AutoDrive/BEVFusion | python | large | large | 109345 | 0.186 s | 86.9 MiB | 586866 |
| HKUDS/AutoAgent | python | medium | medium | 13675 | 0.034 s | 23.2 MiB | 399124 |
| sehmaschine/django-filebrowser | python | small | small | 2586 | 0.014 s | 15.8 MiB | 187578 |
| ytdl-org/youtube-dl | python | large | large | 137578 | 0.155 s | 85.0 MiB | 887131 |
| falconry/falcon | python | medium | medium | 26799 | 0.057 s | 35.1 MiB | 473738 |
| browser-use/browser-use | python | medium | medium | 14734 | 0.134 s | 31.7 MiB | 109658 |
| tobymao/sqlglot | python | large | large | 72710 | 0.564 s | 168.2 MiB | 128990 |
| JesusFreke/lalboard | python | small | small | 4108 | 0.020 s | 14.2 MiB | 203021 |
| concept-graphs/concept-graphs | python | small | small | 8183 | 0.031 s | 15.6 MiB | 264602 |
| jiaaro/pydub | python | small | small | 2555 | 0.009 s | 13.0 MiB | 271496 |
| Panniantong/Agent-Reach | python | small | small | 8000 | 0.039 s | 21.3 MiB | 204771 |
| poljar/weechat-matrix | python | small | small | 8270 | 0.015 s | 16.2 MiB | 543360 |
| openai/whisper | python | small | small | 3257 | 0.010 s | 13.0 MiB | 341672 |
| harlanhong/CVPR2022-DaGAN | python | small | small | 6376 | 0.012 s | 17.1 MiB | 530930 |
| D4Vinci/Scrapling | python | medium | medium | 14643 | 0.038 s | 34.5 MiB | 387986 |
| Jakeschincariol/day-trader-skill | python | small | small | 7520 | 0.017 s | 18.6 MiB | 441771 |
| fastapi/fastapi | python | medium | medium | 31370 | 0.184 s | 80.8 MiB | 170477 |
| meituan-longcat/LongCat-Video | python | medium | medium | 10988 | 0.021 s | 20.1 MiB | 523416 |
| gcovr/gcovr | python | medium | medium | 15485 | 0.635 s | 219.4 MiB | 24404 |
| py2exe/py2exe | python | small | small | 3360 | 0.012 s | 13.6 MiB | 287204 |
| dataabc/weiboSpider | python | small | small | 2038 | 0.008 s | 11.8 MiB | 255809 |
| 51bitquant/binance_grid_trader | python | small | small | 6135 | 0.011 s | 14.8 MiB | 577240 |
| nvbn/thefuck | python | small | small | 5066 | 0.016 s | 18.4 MiB | 316783 |
| lovit/soynlp | python | small | small | 5991 | 0.012 s | 14.5 MiB | 503588 |
| xonsh/xonsh | python | large | large | 56432 | 0.306 s | 74.9 MiB | 184455 |
| 3b1b/manim | python | medium | medium | 21968 | 0.016 s | 21.4 MiB | 1331465 |
| odysseus-dev/odysseus | python | large | large | 244931 | 5.085 s | 631.2 MiB | 48168 |
| domlysz/BlenderGIS | python | medium | medium | 27046 | 0.025 s | 27.1 MiB | 1089364 |
| VoltaML/voltaML-fast-stable-diffusion | python | medium | medium | 24640 | 0.143 s | 90.1 MiB | 172650 |
| taehoonlee/tensornets | python | medium | medium | 12161 | 0.031 s | 23.1 MiB | 393257 |
| aliyun/aliyun-oss-python-sdk | python | medium | medium | 14582 | 0.024 s | 26.6 MiB | 603475 |
| unclecode/crawl4ai | python | large | large | 67679 | 0.171 s | 89.6 MiB | 394852 |
| 666ghj/MiroFish | python | medium | medium | 21160 | 0.035 s | 30.8 MiB | 608612 |
| kartograph/kartograph.py | python | small | small | 3909 | 0.009 s | 13.5 MiB | 437004 |
| PeterL1n/RobustVideoMatting | python | small | small | 2784 | 0.009 s | 13.3 MiB | 310333 |
| fportantier/habu | python | small | small | 5945 | 0.010 s | 15.2 MiB | 573459 |
| platformio/platformio-core | python | medium | medium | 22152 | 0.020 s | 21.4 MiB | 1119000 |
| superdesigndev/treg | python | large | large | 133225 | 0.413 s | 238.9 MiB | 322512 |
| pwr-Solaar/Solaar | python | medium | medium | 31887 | 0.044 s | 43.6 MiB | 722953 |
| kennbroorg/iKy | python | medium | medium | 13801 | 0.039 s | 27.0 MiB | 357048 |
| abi/screenshot-to-code | python | medium | medium | 11777 | 0.044 s | 30.9 MiB | 266784 |
| conan-io/conan | python | large | large | 51375 | 0.126 s | 61.4 MiB | 406659 |
| runesleo/x-reader | python | medium | medium | 11368 | 0.032 s | 21.2 MiB | 360141 |
| OpenBMB/VisRAG | python | large | large | 87412 | 0.199 s | 79.5 MiB | 438202 |
| autonomousvision/stylegan-xl | python | medium | medium | 12009 | 0.021 s | 19.0 MiB | 569318 |
| usestrix/strix | python | medium | medium | 39946 | 0.672 s | 98.2 MiB | 59432 |
| RecursiveMAS/RecursiveMAS | python | medium | medium | 14723 | 0.031 s | 22.2 MiB | 471009 |
| saulpw/visidata | python | medium | medium | 33135 | 0.044 s | 34.1 MiB | 760357 |
| hiyouga/LlamaFactory | python | medium | medium | 42248 | 0.080 s | 45.8 MiB | 529664 |
| tensorflow/transform | python | medium | medium | 23648 | 0.056 s | 34.9 MiB | 425687 |
| ScottSloan/Bili23-Downloader | python | large | large | 53373 | 0.042 s | 28.8 MiB | 1266236 |
| oumi-ai/oumi | python | large | large | 79380 | 0.128 s | 79.5 MiB | 619082 |
| cv-cat/XHS_ALL_IN_ONE | python | medium | medium | 17832 | 0.089 s | 36.3 MiB | 201278 |
| Alishahryar1/free-claude-code | python | large | large | 79409 | 0.219 s | 163.6 MiB | 362647 |
| asgeirtj/system_prompts_leaks | python | medium | medium | 32579 | 0.149 s | 57.7 MiB | 218409 |
| angr/angr | python | large | large | 278534 | 0.653 s | 338.8 MiB | 426469 |
| binary-husky/gpt_academic | python | medium | medium | 49008 | 0.277 s | 62.3 MiB | 176714 |
| spyder-ide/spyder | python | large | large | 160228 | 9.218 s | 1065.2 MiB | 17381 |
| ansible/ansible | python | large | large | 105271 | 0.239 s | 112.2 MiB | 441043 |
| tox-dev/filelock | python | medium | medium | 14098 | 0.041 s | 30.9 MiB | 344436 |
| FoundationAgents/MetaGPT | python | large | large | 54195 | 0.067 s | 52.1 MiB | 814591 |
| mem0ai/mem0 | python | large | large | 66570 | 0.408 s | 129.2 MiB | 163244 |
| HypothesisWorks/hypothesis | python | medium | medium | 37505 | 0.098 s | 71.5 MiB | 384658 |
| Kuaishou-OneRec/OpenOneRec | python | large | large | 141326 | 0.652 s | 187.1 MiB | 216853 |
| scikit-hep/awkward | python | large | large | 101342 | 0.877 s | 213.4 MiB | 115565 |
| MemPalace/mempalace | python | large | large | 84308 | 0.285 s | 131.3 MiB | 295751 |
| kivy/python-for-android | python | medium | medium | 14924 | 0.067 s | 22.2 MiB | 224338 |
| exo-explore/exo | python | large | large | 53116 | 0.169 s | 89.8 MiB | 313951 |
| ashawkey/stable-dreamfusion | python | medium | medium | 16497 | 0.060 s | 30.1 MiB | 276987 |
| fal-ai/fal | python | medium | medium | 37140 | 0.416 s | 35.9 MiB | 89224 |
| wshobson/agents | python | medium | medium | 10398 | 0.056 s | 24.7 MiB | 186011 |
| apple-aiml-research/ml-ferret | python | medium | medium | 12013 | 0.060 s | 26.6 MiB | 199676 |
| LAION-AI/Open-Assistant | python | medium | medium | 36814 | 0.090 s | 57.5 MiB | 407688 |
| miniflux/v2 | go | medium | medium | 36633 | 0.185 s | 63.9 MiB | 197613 |
| NyaaPantsu/nyaa | go | medium | medium | 12014 | 0.779 s | 50.2 MiB | 15419 |
| ollama/ollama | go | large | large | 150969 | 0.509 s | 269.8 MiB | 296428 |
| anchore/syft | go | large | large | 73875 | 0.307 s | 141.8 MiB | 240390 |
| michaelsauter/crane | go | small | small | 3591 | 0.016 s | 14.7 MiB | 222563 |
| lib/pq | go | small | small | 7596 | 0.030 s | 22.8 MiB | 253219 |
| piotrnar/gocoin | go | large | large | 52453 | 1.586 s | 191.0 MiB | 33068 |
| ThreeDotsLabs/watermill | go | medium | medium | 14471 | 0.081 s | 32.1 MiB | 178600 |
| JuliusBrussee/caveman | go | large | large | 86647 | 0.592 s | 258.2 MiB | 146396 |
| gruntwork-io/terragrunt | go | large | large | 127399 | 0.825 s | 269.0 MiB | 154434 |
| faceair/clash-speedtest | go | small | small | 2703 | 0.013 s | 13.2 MiB | 209256 |
| google/go-cloud | go | medium | medium | 32581 | 0.175 s | 52.4 MiB | 185688 |
| safebucket/safebucket | go | medium | medium | 21214 | 0.107 s | 57.4 MiB | 198793 |
| fatedier/frp | go | medium | medium | 35256 | 0.089 s | 51.5 MiB | 397219 |
| gorse-io/gorse | go | medium | medium | 29787 | 0.157 s | 59.9 MiB | 189666 |
| samber/oops | go | small | small | 2233 | 0.017 s | 15.0 MiB | 128028 |
| syncthing/syncthing | go | large | large | 52846 | 0.172 s | 102.6 MiB | 306947 |
| etcd-io/bbolt | go | small | small | 8020 | 0.037 s | 26.2 MiB | 219471 |
| aarzilli/nucular | go | medium | medium | 15257 | 0.021 s | 23.6 MiB | 734174 |
| gohugoio/hugo | go | large | large | 92681 | 0.357 s | 115.7 MiB | 259373 |
| SpecterOps/AzureHound | go | medium | medium | 16036 | 0.056 s | 29.5 MiB | 285111 |
| gin-gonic/gin | go | small | small | 5364 | 0.026 s | 25.3 MiB | 207078 |
| gomodule/redigo | go | small | small | 2840 | 0.011 s | 14.6 MiB | 262906 |
| junegunn/fzf | go | medium | medium | 22956 | 0.039 s | 33.3 MiB | 582596 |
| tmc/langchaingo | go | medium | medium | 44487 | 0.152 s | 68.0 MiB | 293465 |
| reviewdog/reviewdog | go | small | small | 8072 | 0.053 s | 25.7 MiB | 152032 |
| element-hq/dendrite | go | large | large | 86596 | 0.287 s | 102.8 MiB | 301410 |
| netdata/netdata | go | large | large | 448018 | 3.088 s | 887.2 MiB | 145062 |
| tidwall/tile38 | go | medium | medium | 22106 | 0.052 s | 28.1 MiB | 424493 |
| caddyserver/caddy | go | large | large | 50918 | 0.137 s | 69.0 MiB | 372069 |
| cloudnative-pg/cloudnative-pg | go | large | large | 73516 | 0.241 s | 135.0 MiB | 305126 |
| failsafe-security/artex | go | large | large | 53230 | 0.148 s | 119.5 MiB | 360538 |
| nektos/act | go | medium | medium | 15602 | 0.053 s | 35.6 MiB | 294483 |
| real-web-world/hh-lol-prophet | go | small | small | 3995 | 0.009 s | 13.0 MiB | 449162 |
| prometheus/prometheus | go | large | large | 120405 | 0.734 s | 246.2 MiB | 164045 |
| gokcehan/lf | go | medium | medium | 10456 | 0.020 s | 20.2 MiB | 536157 |
| gotomicro/ego | go | medium | medium | 11757 | 0.036 s | 24.2 MiB | 327336 |
| traefik/traefik | go | large | large | 56672 | 0.869 s | 244.9 MiB | 65178 |
| goksan/statusnook | go | medium | medium | 17108 | 0.035 s | 26.2 MiB | 493882 |
| usememos/memos | go | medium | medium | 47702 | 0.471 s | 178.1 MiB | 101337 |
| flannel-io/flannel | go | medium | medium | 10364 | 0.023 s | 21.3 MiB | 458635 |
| v2fly/geoip | go | small | small | 2468 | 0.008 s | 12.8 MiB | 295401 |
| rclone/rclone | go | large | large | 199125 | 0.485 s | 222.2 MiB | 410600 |
| sosedoff/pgweb | go | small | small | 3504 | 0.607 s | 43.0 MiB | 5770 |
| qifengzhang007/GinSkeleton | go | small | small | 4914 | 0.034 s | 24.2 MiB | 143400 |
| router-for-me/CLIProxyAPI | go | large | large | 332082 | 4.420 s | 732.5 MiB | 75126 |
| go-gitea/gitea | go | large | large | 262121 | 0.570 s | 320.0 MiB | 460234 |
| hybridgroup/gobot | go | large | large | 54354 | 0.116 s | 66.2 MiB | 470048 |
| multica-ai/multica | go | large | large | 290550 | 4.513 s | 1013.0 MiB | 64379 |
| alibaba/open-code-review | go | medium | medium | 37725 | 0.316 s | 126.8 MiB | 119266 |
| goplus/xgo | go | medium | medium | 37665 | 0.059 s | 45.4 MiB | 642646 |
| netxfly/sec-dev-in-action-src | go | medium | medium | 13151 | 3.130 s | 166.3 MiB | 4201 |
| wagoodman/dive | go | small | small | 8289 | 0.044 s | 21.0 MiB | 186373 |
| certimate-go/certimate | go | large | large | 88718 | 1.965 s | 144.4 MiB | 45159 |
| etcd-io/etcd | go | large | large | 76188 | 0.571 s | 135.0 MiB | 133462 |
| QuantumNous/new-api | go | large | large | 162009 | 1.267 s | 322.6 MiB | 127877 |
| ethereum/go-ethereum | go | large | large | 184759 | 5.372 s | 280.7 MiB | 34395 |
| kubernetes-sigs/kubebuilder | go | medium | medium | 34794 | 0.395 s | 85.8 MiB | 87982 |
| prometheus/statsd_exporter | go | small | small | 2987 | 0.035 s | 17.0 MiB | 85722 |
| MHSanaei/3x-ui | go | large | large | 96257 | 0.531 s | 236.6 MiB | 181391 |
| runatlantis/atlantis | go | medium | medium | 37686 | 0.828 s | 134.7 MiB | 45496 |
| mudler/LocalRecall | go | small | small | 7873 | 0.029 s | 23.8 MiB | 275887 |
| bitnami/sealed-secrets | go | small | small | 2947 | 0.021 s | 19.7 MiB | 139273 |
| AlistGo/alist | go | large | large | 100905 | 0.152 s | 94.4 MiB | 663875 |
| google/pprof | go | medium | medium | 12913 | 0.043 s | 31.8 MiB | 299935 |
| sijms/go-ora | go | large | large | 242193 | 3.997 s | 1286.4 MiB | 60601 |
| hashicorp/terraform | go | large | large | 182287 | 2.665 s | 431.9 MiB | 68411 |
| gtsteffaniak/filebrowser | go | large | large | 72016 | 6.710 s | 202.6 MiB | 10733 |
| bqlpfy/flux-panel | go | large | large | 52230 | 0.230 s | 69.1 MiB | 226616 |
| gogs/gogs | go | large | large | 60526 | 0.429 s | 144.8 MiB | 141005 |
| quexten/goldwarden | go | small | small | 8899 | 0.037 s | 18.6 MiB | 238583 |
| v2ray/v2ray-core | go | medium | medium | 30798 | 0.193 s | 71.1 MiB | 159708 |
| mark3labs/mcp-go | go | medium | medium | 26116 | 0.182 s | 73.8 MiB | 143300 |
| twpayne/go-geom | go | small | small | 7119 | 0.033 s | 22.3 MiB | 217612 |
| mvdan/sh | go | medium | medium | 19616 | 0.056 s | 34.0 MiB | 348183 |
| charmbracelet/bubbletea | go | small | small | 9634 | 0.017 s | 16.5 MiB | 579563 |
| spf13/cobra | go | small | small | 5165 | 0.019 s | 18.3 MiB | 275742 |
| hashicorp/raft | go | small | small | 8243 | 0.036 s | 22.0 MiB | 232167 |
| rcrowley/go-tigertonic | go | small | small | 2117 | 0.013 s | 14.4 MiB | 169009 |
| danielmiessler/Fabric | go | medium | medium | 21384 | 0.091 s | 43.1 MiB | 236019 |
| kubernetes-sigs/external-dns | go | medium | medium | 29680 | 0.501 s | 99.8 MiB | 59249 |
| redcode-labs/Coldfire | go | small | small | 2059 | 0.010 s | 11.6 MiB | 211517 |
| yliu7949/KouShare-dl | go | small | small | 3562 | 0.013 s | 13.3 MiB | 282477 |
| smallstep/certificates | go | medium | medium | 39381 | 0.577 s | 130.0 MiB | 68235 |
| schollz/croc | go | medium | medium | 28555 | 0.102 s | 62.6 MiB | 279381 |
| dinedal/textql | go | small | small | 4194 | 0.018 s | 13.6 MiB | 235129 |
| onsi/ginkgo | go | medium | medium | 18114 | 0.110 s | 45.9 MiB | 164209 |
| itzg/mc-router | go | small | small | 6165 | 0.018 s | 17.4 MiB | 348433 |
| charmbracelet/bubbles | go | small | small | 7333 | 0.014 s | 18.5 MiB | 520941 |
| vishen/go-chromecast | go | small | small | 5089 | 0.015 s | 15.5 MiB | 343926 |
| teamhanko/hanko | go | medium | medium | 31304 | 0.105 s | 60.6 MiB | 298592 |
| zombiezen/go-sqlite | go | small | small | 6623 | 0.014 s | 17.3 MiB | 482796 |
| gofiber/fiber | go | medium | medium | 40395 | 0.116 s | 85.6 MiB | 349332 |
| evilmartians/lefthook | go | small | small | 7957 | 0.022 s | 22.8 MiB | 358527 |
| go-gorm/gorm | go | medium | medium | 12497 | 0.063 s | 38.8 MiB | 197024 |
| google/badwolf | go | medium | medium | 12683 | 0.050 s | 25.2 MiB | 253121 |
| securego/gosec | go | medium | medium | 37606 | 0.077 s | 38.5 MiB | 490865 |
| HoudiniGraphql/houdini | go | medium | medium | 29044 | 0.177 s | 73.0 MiB | 164198 |
| GoAdminGroup/go-admin | go | medium | medium | 37153 | 0.793 s | 92.7 MiB | 46848 |
| docker/compose | go | medium | medium | 31304 | 0.170 s | 67.3 MiB | 184573 |
| pgmq/pgmq | sql | medium | medium | 10854 | 0.071 s | 74.0 MiB | 152976 |
| oracle-samples/db-sample-schemas | sql | large | large | 110419 | 0.500 s | 358.6 MiB | 220884 |
| BrentOzarULTD/SQL-Server-First-Responder-Kit | sql | large | large | 137263 | 0.759 s | 557.9 MiB | 180921 |
| olahallengren/sql-server-maintenance-solution | sql | medium | medium | 20808 | 0.115 s | 109.4 MiB | 181166 |
| smpetersgithub/AdvancedSQLPuzzles | sql | medium | medium | 11192 | 0.045 s | 41.0 MiB | 247675 |
| chinnonsantos/sql-paises-estados-cidades | sql | medium | medium | 18097 | 0.191 s | 204.0 MiB | 94750 |
| openmaptiles/openmaptiles | sql | medium | medium | 10411 | 0.054 s | 50.5 MiB | 194546 |
| pgpartman/pg_partman | sql | large | large | 156326 | 1.982 s | 559.9 MiB | 78874 |
| lerocha/chinook-database | sql | large | large | 173602 | 1.008 s | 845.0 MiB | 172278 |
| rin-nas/postgresql-patterns-library | sql | medium | medium | 12411 | 0.036 s | 38.7 MiB | 347683 |
| postgis/postgis | sql | medium | medium | 29736 | 0.517 s | 536.0 MiB | 57476 |
| darold/pgFormatter | sql | large | large | 310653 | 1.793 s | 1842.6 MiB | 173279 |
| NikolayS/PgQue | sql | medium | medium | 34478 | 0.172 s | 80.7 MiB | 200904 |
| tanelpoder/tpt-oracle | sql | large | large | 100830 | 1.199 s | 791.6 MiB | 84123 |
| erikdarlingdata/DarlingData | sql | large | large | 145386 | 0.778 s | 420.0 MiB | 186847 |
| NixOS/hydra | sql | medium | medium | 38904 | 0.701 s | 272.8 MiB | 55509 |
| amachanic/sp_whoisactive | sql | medium | medium | 14923 | 0.053 s | 95.4 MiB | 280357 |
| microsoft/fhir-server | sql | large | large | 335344 | 3.401 s | 1245.5 MiB | 98594 |
| cahyadsn/wilayah | sql | large | large | 509905 | 2.824 s | 1316.2 MiB | 180576 |
| utPLSQL/utPLSQL | sql | small | small | 2769 | 0.028 s | 20.2 MiB | 98709 |
| NikolayS/postgres_dba | sql | small | small | 2900 | 0.022 s | 26.9 MiB | 133375 |
| trickerer/Trinity-Bots | sql | medium | medium | 28859 | 0.351 s | 241.5 MiB | 82116 |
| theory/pgtap | sql | small | small | 4990 | 0.101 s | 141.2 MiB | 49240 |
| starkfell/100DaysOfIaC | sql | small | small | 5460 | 0.104 s | 53.0 MiB | 52258 |
| aquametalabs/aquameta | sql | small | small | 4742 | 0.062 s | 35.5 MiB | 76149 |
| jOOQ/sakila | sql | large | large | 928489 | 8.918 s | 5012.2 MiB | 104109 |
| microsoft/Windows-appsample-customers-orders-database | sql | medium | medium | 16236 | 0.087 s | 80.5 MiB | 186602 |
| dbkangaroo/kangaroo | sql | large | large | 141672 | 5.915 s | 1801.5 MiB | 23951 |
| mqttsnet/thinglinks | sql | medium | medium | 11692 | 3.785 s | 471.5 MiB | 3089 |
| marcingminski/sqlwatch | sql | medium | medium | 14828 | 0.049 s | 29.6 MiB | 301913 |
| lorint/AdventureWorks-for-Postgres | sql | small | small | 2732 | 0.160 s | 30.5 MiB | 17113 |
| 3dcitydb/3dcitydb | sql | small | small | 2899 | 0.042 s | 23.9 MiB | 68901 |
| geocompass/pg-coordtransform | sql | small | small | 2117 | 0.020 s | 15.9 MiB | 105045 |
| tSQLt-org/tSQLt | sql | medium | medium | 24792 | 0.096 s | 107.6 MiB | 257821 |
| pljson/pljson | sql | small | small | 8422 | 0.038 s | 33.6 MiB | 221206 |
| andrelugomes/digital-innovation-one | sql | medium | medium | 11873 | 0.022 s | 23.2 MiB | 534902 |
| Abstrct/Schemaverse | sql | small | small | 9141 | 0.561 s | 75.6 MiB | 16292 |
| basedosdados/sdk | sql | medium | medium | 19773 | 0.104 s | 54.6 MiB | 190919 |
| pgpointcloud/pointcloud | sql | medium | medium | 40779 | 1.228 s | 1167.9 MiB | 33205 |
| pgq/pgq | sql | small | small | 3223 | 0.025 s | 25.1 MiB | 127193 |
| pgMemento/pgMemento | sql | small | small | 6727 | 0.033 s | 28.9 MiB | 206022 |
| siqichen-usc/LeetCode-SQL-Summary | sql | small | small | 4664 | 0.044 s | 45.1 MiB | 106969 |
| bnagy/cgasm | sql | large | large | 53520 | 0.045 s | 50.8 MiB | 1192713 |
| postgis/h3-pg | sql | small | small | 5558 | 0.050 s | 45.5 MiB | 112195 |
| czwbig/Tmall_JavaEE | sql | medium | medium | 29551 | 0.165 s | 250.9 MiB | 178822 |
| Hameds/IranCountryDivisions | sql | large | large | 1047837 | 7.345 s | 4843.6 MiB | 142663 |
| powa-team/pg_qualstats | sql | small | small | 3342 | 0.015 s | 24.8 MiB | 228987 |
| MontageD/nuxt-maopingshou | sql | small | small | 6565 | 0.129 s | 80.3 MiB | 51052 |
| babelfish-for-postgresql/babelfish_extensions | sql | large | large | 178071 | 2.878 s | 1117.2 MiB | 61878 |
| xocolatl/periods | sql | small | small | 9680 | 0.047 s | 45.7 MiB | 204902 |
| DavidSchanzer/Sql-Server-DBA-Toolbox | sql | medium | medium | 21541 | 0.082 s | 88.5 MiB | 261550 |
| oraclebase/dba | sql | small | small | 7940 | 0.050 s | 22.3 MiB | 157738 |
| Dogkann/sql-calisma-ornekleri | sql | small | small | 9401 | 0.050 s | 58.7 MiB | 186167 |
| pgflow-dev/pgflow | sql | medium | medium | 20392 | 0.232 s | 235.2 MiB | 87998 |
| microsoft/Azure_Synapse_Toolbox | sql | small | small | 3543 | 0.014 s | 25.8 MiB | 254726 |
| supabase/splinter | sql | small | small | 4200 | 0.095 s | 51.0 MiB | 44229 |
| OHDSI/Vocabulary-v5.0 | sql | large | large | 157440 | 1.423 s | 659.5 MiB | 110618 |
| hasura/graphql-bench | sql | medium | medium | 39402 | 0.804 s | 754.0 MiB | 49034 |
| pgspider/sqlite_fdw | sql | large | large | 75081 | 1.061 s | 1072.2 MiB | 70749 |
| drobcosta/digital_innovation_one | sql | small | small | 4057 | 0.059 s | 97.8 MiB | 68496 |
| vibhorkum/pg_background | sql | small | small | 6064 | 0.043 s | 33.0 MiB | 140819 |
| crtsh/certwatch_db | sql | small | small | 4476 | 0.046 s | 38.3 MiB | 98293 |
| NikolayS/pg_ash | sql | medium | medium | 22488 | 0.164 s | 71.8 MiB | 137497 |
| ether-camp/ethereum-harmony | sql | large | large | 143336 | 0.450 s | 457.6 MiB | 318237 |
| supabase-community/supabase-graphql-example | sql | small | small | 2805 | 0.027 s | 27.0 MiB | 102146 |
| wuchong/flink-sql-demo | sql | small | small | 5017 | 0.020 s | 27.7 MiB | 257171 |
| TexanInParis/Effective-SQL | sql | large | large | 110136 | 4.099 s | 3883.0 MiB | 26868 |
| pgspider/parquet_s3_fdw | sql | medium | medium | 27272 | 0.456 s | 374.4 MiB | 59800 |
| sivers/sivers | sql | small | small | 5372 | 0.057 s | 91.6 MiB | 93548 |
| jeesun/oauthserver | sql | large | large | 242903 | 5.019 s | 2883.8 MiB | 48400 |
| rrg92/sqlserver-lib | sql | small | small | 8326 | 0.027 s | 27.4 MiB | 309392 |
| cseagle/fREedom | sql | small | small | 3477 | 0.024 s | 27.5 MiB | 142917 |
| stac-utils/pgstac | sql | large | large | 235203 | 2.393 s | 566.5 MiB | 98294 |
| yudaocode/yudao-boot-mini | sql | large | large | 58668 | 4.433 s | 2904.1 MiB | 13233 |
| orbisgis/h2gis | sql | small | small | 8342 | 0.273 s | 273.6 MiB | 30583 |
| journeyapps/node-sqlcipher | sql | medium | medium | 10003 | 0.186 s | 166.4 MiB | 53902 |
| czwbig/Tmall_SSM | sql | medium | medium | 14694 | 0.228 s | 252.5 MiB | 64439 |
| deajan/osync | shell | medium | medium | 18782 | 0.078 s | 36.1 MiB | 240524 |
| ohmyzsh/ohmyzsh | shell | medium | medium | 26488 | 0.061 s | 31.7 MiB | 430845 |
| msitarzewski/agency-agents | shell | small | small | 4429 | 0.023 s | 17.5 MiB | 191309 |
| telekom-security/tpotce | shell | small | small | 3001 | 0.033 s | 21.2 MiB | 91719 |
| nvm-sh/nvm | shell | small | small | 7190 | 0.027 s | 21.0 MiB | 269315 |
| tw93/Mole | shell | large | large | 50452 | 0.096 s | 69.6 MiB | 523660 |
| frankbria/ralph-claude-code | shell | medium | medium | 10174 | 0.025 s | 21.8 MiB | 403529 |
| pi-hole/pi-hole | shell | small | small | 4867 | 0.055 s | 25.5 MiB | 87996 |
| getsentry/self-hosted | shell | small | small | 2308 | 0.021 s | 15.0 MiB | 108720 |
| romkatv/powerlevel10k | shell | medium | medium | 14978 | 0.052 s | 30.3 MiB | 288722 |
| dockur/windows | shell | medium | medium | 12602 | 0.024 s | 25.8 MiB | 517709 |
| Danilop95/Proxmox-Enhanced-Configuration-Utility | shell | small | small | 5681 | 0.025 s | 19.3 MiB | 222858 |
| ublue-os/bazzite | shell | small | small | 2461 | 0.027 s | 15.4 MiB | 92723 |
| acmesh-official/acme.sh | shell | large | large | 52037 | 0.072 s | 33.0 MiB | 722433 |
| omacom/omarchy | shell | medium | medium | 10372 | 0.383 s | 79.7 MiB | 27115 |
| testssl/testssl.sh | shell | medium | medium | 23746 | 0.091 s | 43.0 MiB | 260383 |
| kunchenguid/firstmate | shell | large | large | 103158 | 0.370 s | 173.8 MiB | 278735 |
| RROrg/rr | shell | small | small | 6394 | 0.028 s | 24.7 MiB | 229224 |
| community-scripts/ProxmoxVE | shell | large | large | 110490 | 0.256 s | 41.1 MiB | 431820 |
| conversun/fnos-apps | shell | medium | medium | 15578 | 0.100 s | 19.3 MiB | 155785 |
| 233boy/v2ray | shell | small | small | 2757 | 0.017 s | 12.9 MiB | 164431 |
| hwdsl2/setup-ipsec-vpn | shell | small | small | 6430 | 0.015 s | 18.1 MiB | 424878 |
| xiaoyaDev/xiaoya-alist | shell | small | small | 9319 | 0.506 s | 84.6 MiB | 18415 |
| OthmanAdi/planning-with-files | shell | large | large | 56626 | 0.286 s | 107.1 MiB | 198019 |
| mbadolato/iTerm2-Color-Schemes | shell | large | large | 83337 | 0.347 s | 89.5 MiB | 240091 |
| prasanthrangan/hyprdots | shell | small | small | 3832 | 0.013 s | 15.8 MiB | 297215 |
| yuaotian/go-cursor-help | shell | small | small | 4186 | 0.013 s | 19.2 MiB | 332385 |
| HariSekhon/DevOps-Bash-tools | shell | large | large | 65117 | 0.058 s | 33.4 MiB | 1121243 |
| six2dez/reconftw | shell | medium | medium | 14298 | 0.028 s | 28.4 MiB | 507458 |
| zsh-users/zsh-syntax-highlighting | shell | small | small | 4313 | 0.014 s | 13.6 MiB | 298452 |
| mack-a/v2ray-agent | shell | medium | medium | 26985 | 0.054 s | 54.8 MiB | 496366 |
| teddysun/shadowsocks_install | shell | small | small | 3512 | 0.011 s | 14.8 MiB | 331420 |
| huifer/WellAlly-health | shell | small | small | 5098 | 0.021 s | 19.8 MiB | 241800 |
| v1s1t0r1sh3r3/airgeddon | shell | medium | medium | 30345 | 0.047 s | 46.9 MiB | 641952 |
| spaceship-prompt/spaceship-prompt | shell | small | small | 3047 | 0.014 s | 15.3 MiB | 216011 |
| grahampugh/erase-install | shell | small | small | 3946 | 0.043 s | 15.9 MiB | 92061 |
| pivpn/pivpn | shell | small | small | 5800 | 0.014 s | 17.4 MiB | 414491 |
| docker-mailserver/docker-mailserver | shell | small | small | 3805 | 0.015 s | 14.4 MiB | 245813 |
| hex/claude-council | shell | small | small | 5611 | 0.027 s | 25.0 MiB | 206271 |
| linux-surface/linux-surface | shell | small | small | 2488 | 0.033 s | 13.8 MiB | 74642 |
| kossakovsky/selfhost-ai | shell | small | small | 4950 | 0.025 s | 16.6 MiB | 197604 |
| SuperManito/LinuxMirrors | shell | medium | medium | 20653 | 0.048 s | 29.3 MiB | 432310 |
| eylenburg/linoffice | shell | small | small | 4336 | 0.015 s | 16.4 MiB | 289244 |
| termux/termux-packages | shell | large | large | 104500 | 0.533 s | 70.5 MiB | 196190 |
| attogram/bash-screensavers | shell | small | small | 2517 | 0.008 s | 12.5 MiB | 311191 |
| ohmybash/oh-my-bash | shell | medium | medium | 22736 | 0.022 s | 24.5 MiB | 1037532 |
| z-shell/zi | shell | small | small | 9723 | 0.035 s | 27.9 MiB | 278421 |
| woniuzfb/aios | shell | large | large | 129386 | 0.458 s | 190.4 MiB | 282467 |
| zplug/zplug | shell | small | small | 4121 | 0.013 s | 13.4 MiB | 306757 |
| mishakorzik/AllHackingTools | shell | small | small | 7720 | 0.029 s | 23.9 MiB | 267671 |
| 1N3/Sn1per | shell | small | small | 7782 | 0.038 s | 29.5 MiB | 203510 |
| sorin-ionescu/prezto | shell | small | small | 2019 | 0.026 s | 12.5 MiB | 76552 |
| kaldi-asr/kaldi | shell | large | large | 298400 | 0.960 s | 368.3 MiB | 310695 |
| Bash-it/bash-it | shell | medium | medium | 16534 | 0.024 s | 20.1 MiB | 697255 |
| shibco/ableton-linux | shell | medium | medium | 35056 | 0.057 s | 39.6 MiB | 610078 |
| bin456789/reinstall | shell | medium | medium | 11253 | 0.026 s | 31.8 MiB | 438271 |
| juewuy/ShellCrash | shell | medium | medium | 12514 | 0.049 s | 18.7 MiB | 253397 |
| google/oss-fuzz | shell | medium | medium | 29786 | 0.147 s | 81.0 MiB | 202313 |
| MariaDB/mariadb-docker | shell | medium | medium | 14149 | 0.043 s | 25.0 MiB | 332523 |
| Dicklesworthstone/claude_code_agent_farm | shell | medium | medium | 10035 | 0.014 s | 18.2 MiB | 719202 |
| wnlen/clash-for-linux | shell | medium | medium | 18472 | 0.031 s | 30.1 MiB | 594209 |
| RetroPie/RetroPie-Setup | shell | medium | medium | 25726 | 0.106 s | 34.0 MiB | 242396 |
| Gogh-Co/Gogh | shell | large | large | 52137 | 0.075 s | 37.0 MiB | 694946 |
| armbian/build | shell | medium | medium | 26503 | 0.258 s | 45.7 MiB | 102599 |
| ChrisTitusTech/linutil | shell | medium | medium | 13361 | 0.038 s | 34.7 MiB | 353522 |
| Eric86777/vps-tcp-tune | shell | medium | medium | 23893 | 0.054 s | 38.5 MiB | 439800 |
| Aegrah/PANIX | shell | medium | medium | 15461 | 0.034 s | 23.3 MiB | 454191 |
| PeonPing/peon-ping | shell | medium | medium | 14796 | 0.045 s | 32.8 MiB | 331472 |
| akinomyoga/ble.sh | shell | large | large | 63530 | 0.075 s | 69.3 MiB | 850487 |
| GameServerManagers/LinuxGSM | shell | medium | medium | 20397 | 0.038 s | 24.5 MiB | 541653 |
| ToyoDAdoubi/doubi | shell | medium | medium | 20546 | 0.037 s | 26.2 MiB | 550978 |
| hestiacp/hestiacp | shell | medium | medium | 16485 | 0.079 s | 55.4 MiB | 209310 |
| AryanVBW/LinuxDroid | shell | medium | medium | 17958 | 0.038 s | 16.4 MiB | 471076 |
| nyldn/claude-octopus | shell | large | large | 76529 | 0.341 s | 107.7 MiB | 224416 |
| sno-ai/sno-station-skills | shell | medium | medium | 10149 | 0.036 s | 34.4 MiB | 278222 |
| bitnami/containers | shell | large | large | 230403 | 0.832 s | 145.3 MiB | 276942 |
| fluffypony/dothething | shell | medium | medium | 11317 | 0.012 s | 14.7 MiB | 958337 |
| jedi4ever/veewee | shell | medium | medium | 16531 | 0.059 s | 28.4 MiB | 281812 |
| leebaird/discover | shell | medium | medium | 25427 | 0.250 s | 49.3 MiB | 101750 |
| BrowserBox/BrowserBox | shell | medium | medium | 10110 | 0.079 s | 25.1 MiB | 128291 |
| e-m-b-a/emba | shell | medium | medium | 38773 | 0.047 s | 30.8 MiB | 830102 |
| pexoai/pexo-skills | shell | medium | medium | 16182 | 0.035 s | 24.6 MiB | 458544 |
| kejilion/sh | shell | large | large | 217024 | 0.907 s | 266.9 MiB | 239175 |
| SamNet-dev/MTProxyMax | shell | medium | medium | 20039 | 0.059 s | 35.5 MiB | 337787 |
| scop/bash-completion | shell | medium | medium | 25796 | 0.048 s | 28.5 MiB | 535148 |
| replicatedhq/kURL | shell | large | large | 208436 | 1.833 s | 249.0 MiB | 113702 |
| StressOzz/Zapret-Manager | shell | large | large | 156213 | 0.048 s | 57.1 MiB | 3224327 |
| bioconda/bioconda-recipes | shell | large | large | 95323 | 0.676 s | 120.1 MiB | 141112 |
| centminmod/centminmod | shell | large | large | 117059 | 0.520 s | 143.9 MiB | 225109 |
| lavabit/robox | shell | large | large | 67914 | 0.169 s | 53.9 MiB | 401036 |
| Cloud-Wala-Banda/Labs-Solutions | shell | large | large | 55577 | 0.097 s | 51.7 MiB | 572272 |
| Dicklesworthstone/agentic_coding_flywheel_setup | shell | large | large | 123502 | 0.602 s | 244.5 MiB | 205214 |
| zoddDev/dotfiles | shell | medium | medium | 18213 | 0.067 s | 22.7 MiB | 272947 |
| Gentleman-Programming/Gentleman.Dots | shell | medium | medium | 21610 | 0.117 s | 42.5 MiB | 185203 |
| dragoonDorise/EmuDeck | shell | medium | medium | 23171 | 0.059 s | 37.8 MiB | 395595 |
| Chachamaru127/claude-code-harness | shell | medium | medium | 38205 | 0.621 s | 98.7 MiB | 61512 |
| linuxeye/lnmp | shell | medium | medium | 14935 | 0.042 s | 21.1 MiB | 357220 |
| alexbelgium/hassio-addons | shell | medium | medium | 16048 | 0.076 s | 23.9 MiB | 212018 |
| matryer/xbar-plugins | shell | medium | medium | 18045 | 0.089 s | 40.4 MiB | 203894 |
| oneinstack/oneinstack | shell | medium | medium | 19691 | 0.051 s | 26.5 MiB | 388002 |
| keystonejs/keystone | typescript-javascript | large | large | 64285 | 0.344 s | 78.4 MiB | 186755 |
| open-slide/open-slide | typescript-javascript | large | large | 125745 | 0.193 s | 91.0 MiB | 651126 |
| usekaneo/kaneo | typescript-javascript | large | large | 252983 | 1.872 s | 226.7 MiB | 135168 |
| vercel/next.js | typescript-javascript | large | large | 1148860 | 63.932 s | 1380.0 MiB | 17970 |
| shadcn-ui/ui | typescript-javascript | large | large | 415871 | 2.703 s | 405.7 MiB | 153868 |
| zuiidea/antd-admin | typescript-javascript | medium | medium | 10417 | 0.033 s | 23.7 MiB | 313624 |
| react/create-react-app | typescript-javascript | large | large | 56349 | 0.635 s | 110.6 MiB | 88676 |
| mui/material-ui | typescript-javascript | large | large | 577674 | 1.881 s | 424.2 MiB | 307097 |
| margelo/react-native-vision-camera | typescript-javascript | medium | medium | 12773 | 0.149 s | 27.9 MiB | 85468 |
| ChatGPTNextWeb/NextChat | typescript-javascript | medium | medium | 37605 | 0.115 s | 45.0 MiB | 327919 |
| pmndrs/use-gesture | typescript-javascript | small | small | 5588 | 0.027 s | 19.1 MiB | 205404 |
| Ovyerus/prismaliser | typescript-javascript | small | small | 4203 | 1.933 s | 135.3 MiB | 2175 |
| meta/docusaurus | typescript-javascript | large | large | 55436 | 0.235 s | 65.0 MiB | 236274 |
| aws-amplify/amplify-js | typescript-javascript | large | large | 60232 | 0.439 s | 114.0 MiB | 137275 |
| makeplane/plane | typescript-javascript | large | large | 212939 | 0.849 s | 230.8 MiB | 250870 |
| projectstorm/react-diagrams | typescript-javascript | small | small | 8098 | 0.035 s | 18.6 MiB | 231765 |
| SyntaxUI/syntaxui | typescript-javascript | medium | medium | 10674 | 0.023 s | 18.3 MiB | 473451 |
| miurla/morphic | typescript-javascript | medium | medium | 41880 | 0.081 s | 49.0 MiB | 517036 |
| tldraw/signia | typescript-javascript | small | small | 2556 | 0.028 s | 17.8 MiB | 91447 |
| tldraw/tldraw | typescript-javascript | large | large | 271441 | 1.520 s | 264.0 MiB | 178560 |
| maotoumao/MusicFreeDesktop | typescript-javascript | medium | medium | 21206 | 0.048 s | 23.3 MiB | 437414 |
| ayastreb/money-tracker | typescript-javascript | small | small | 6968 | 0.020 s | 17.1 MiB | 345223 |
| TanStack/query | typescript-javascript | large | large | 82121 | 0.648 s | 129.1 MiB | 126652 |
| webstudio-is/webstudio | typescript-javascript | large | large | 423435 | 2.238 s | 443.7 MiB | 189180 |
| victoralvesf/aonsoku | typescript-javascript | medium | medium | 33933 | 0.181 s | 35.1 MiB | 187335 |
| fireproof-storage/fireproof | typescript-javascript | medium | medium | 33763 | 0.157 s | 56.0 MiB | 215456 |
| benawad/dogehouse | typescript-javascript | medium | medium | 34873 | 8.963 s | 153.4 MiB | 3891 |
| reactive-resume/reactive-resume | typescript-javascript | large | large | 219098 | 0.825 s | 243.3 MiB | 265494 |
| xitanggg/open-resume | typescript-javascript | small | small | 6116 | 0.023 s | 15.7 MiB | 260912 |
| payloadcms/payload | typescript-javascript | large | large | 415874 | 11.721 s | 549.9 MiB | 35482 |
| radix-ui/themes | typescript-javascript | medium | medium | 15714 | 0.053 s | 26.7 MiB | 297767 |
| OnsenUI/OnsenUI | typescript-javascript | medium | medium | 36420 | 0.115 s | 53.6 MiB | 316166 |
| AykutSarac/jsoncrack.com | typescript-javascript | small | small | 8237 | 0.033 s | 19.8 MiB | 248001 |
| uber/react-vis | typescript-javascript | medium | medium | 18648 | 0.069 s | 26.5 MiB | 271441 |
| code-forge-io/react-router-devtools | typescript-javascript | medium | medium | 18600 | 0.050 s | 24.8 MiB | 370077 |
| prevwong/craft.js | typescript-javascript | medium | medium | 10958 | 1.026 s | 103.9 MiB | 10679 |
| chvin/react-tetris | typescript-javascript | small | small | 3010 | 0.283 s | 30.2 MiB | 10619 |
| react-cosmos/react-cosmos | typescript-javascript | medium | medium | 19848 | 0.079 s | 32.3 MiB | 252267 |
| stackworx/formik-mui | typescript-javascript | small | small | 2978 | 0.019 s | 15.5 MiB | 158938 |
| xyflow/xyflow | typescript-javascript | medium | medium | 45226 | 0.082 s | 37.4 MiB | 550577 |
| Gamote/lottie-react | typescript-javascript | small | small | 5230 | 0.024 s | 20.7 MiB | 220296 |
| omeralpi/shadcn-phone-input | typescript-javascript | small | small | 2583 | 0.015 s | 14.3 MiB | 171541 |
| react-toolbox/react-toolbox | typescript-javascript | medium | medium | 19470 | 0.052 s | 31.5 MiB | 374624 |
| garmeeh/next-seo | typescript-javascript | medium | medium | 22815 | 0.083 s | 33.1 MiB | 274455 |
| mat-sz/filedrop | typescript-javascript | small | small | 3939 | 0.036 s | 16.1 MiB | 110134 |
| markdoc/markdoc | typescript-javascript | small | small | 8194 | 0.037 s | 21.5 MiB | 223716 |
| LottieFiles/dotlottie-web | typescript-javascript | medium | medium | 17093 | 0.068 s | 36.7 MiB | 250861 |
| appbaseio/dejavu | typescript-javascript | small | small | 6905 | 0.036 s | 17.1 MiB | 189191 |
| stoatchat/for-legacy-web | typescript-javascript | medium | medium | 28511 | 2.246 s | 183.9 MiB | 12696 |
| jaredpalmer/formik | typescript-javascript | small | small | 8487 | 0.029 s | 22.1 MiB | 290643 |
| PanJiaChen/vue-element-admin | typescript-javascript | medium | medium | 10502 | 0.029 s | 17.4 MiB | 365015 |
| kiwiirc/kiwiirc | typescript-javascript | medium | medium | 15135 | 0.020 s | 21.3 MiB | 753881 |
| bailicangdu/vue2-elm | typescript-javascript | small | small | 4236 | 0.114 s | 27.6 MiB | 37160 |
| docmirror/dev-sidecar | typescript-javascript | small | small | 8324 | 0.022 s | 20.5 MiB | 383866 |
| JakHuang/form-generator | typescript-javascript | small | small | 4247 | 15.283 s | 361.2 MiB | 278 |
| zxwk1998/vue-admin-better | typescript-javascript | small | small | 8928 | 0.044 s | 26.1 MiB | 203294 |
| Moonvy/OpenPromptStudio | typescript-javascript | small | small | 2383 | 0.009 s | 15.3 MiB | 260593 |
| vueComponent/ant-design-vue-pro | typescript-javascript | medium | medium | 11135 | 0.017 s | 18.5 MiB | 667198 |
| getgridea/gridea | typescript-javascript | small | small | 6670 | 0.012 s | 17.5 MiB | 537768 |
| woai3c/visual-drag-demo | typescript-javascript | small | small | 4345 | 0.010 s | 15.7 MiB | 425475 |
| GavinZhuLei/vue-form-making | typescript-javascript | small | small | 3404 | 5.361 s | 439.8 MiB | 635 |
| HoshinoSuzumi/CTFever | typescript-javascript | small | small | 6228 | 0.039 s | 20.8 MiB | 161276 |
| lensh/vue-qq | typescript-javascript | small | small | 4027 | 0.039 s | 20.7 MiB | 102350 |
| elunez/eladmin-web | typescript-javascript | medium | medium | 10603 | 0.023 s | 18.9 MiB | 461451 |
| pearmini/colorfu | typescript-javascript | small | small | 3522 | 0.017 s | 14.3 MiB | 206607 |
| ymm-tech/gods-pen | typescript-javascript | medium | medium | 13203 | 2.061 s | 102.1 MiB | 6406 |
| zhengguorong/h5maker | typescript-javascript | small | small | 3131 | 0.020 s | 16.9 MiB | 158353 |
| iamdarcy/hioshop-admin | typescript-javascript | small | small | 6463 | 0.017 s | 17.2 MiB | 391011 |
| herozhou/vue-framework-wz | typescript-javascript | medium | medium | 14092 | 0.125 s | 53.1 MiB | 112733 |
| myliang/fish-ui | typescript-javascript | small | small | 5665 | 0.287 s | 54.3 MiB | 19710 |
| umicro/uView | typescript-javascript | medium | medium | 23352 | 0.028 s | 28.7 MiB | 845602 |
| wdlhao/vue2-element-touzi-admin | typescript-javascript | small | small | 4409 | 0.010 s | 15.8 MiB | 429222 |
| alaboshu/alaweb | typescript-javascript | medium | medium | 14009 | 0.023 s | 23.5 MiB | 602275 |
| Simon-He95/markstream-vue | typescript-javascript | large | large | 170014 | 0.339 s | 132.4 MiB | 502213 |
| chiflix/splayerx | typescript-javascript | medium | medium | 44152 | 0.059 s | 45.0 MiB | 744244 |
| KlausSchaefers/quant-ux | typescript-javascript | large | large | 128113 | 0.110 s | 85.3 MiB | 1162943 |
| xiaozhu188/electron-vue-cloud-music | typescript-javascript | medium | medium | 13150 | 0.018 s | 19.2 MiB | 717855 |
| mainsail-crew/mainsail | typescript-javascript | medium | medium | 38999 | 0.072 s | 40.4 MiB | 540658 |
| tuandm/laravue | typescript-javascript | medium | medium | 11048 | 2.054 s | 61.5 MiB | 5380 |
| hua1995116/musiccloudWebapp | typescript-javascript | medium | medium | 17066 | 0.056 s | 36.3 MiB | 305163 |
| vform666/variant-form | typescript-javascript | medium | medium | 12380 | 0.025 s | 18.9 MiB | 488634 |
| Exrick/xboot-front | typescript-javascript | medium | medium | 21661 | 0.029 s | 21.9 MiB | 753040 |
| webmobiledev/vue-tradingview | typescript-javascript | medium | medium | 35288 | 1.594 s | 522.0 MiB | 22135 |
| fluidd-core/fluidd | typescript-javascript | medium | medium | 39279 | 0.046 s | 31.9 MiB | 859612 |
| a54552239/pearProject | typescript-javascript | medium | medium | 12308 | 0.024 s | 20.0 MiB | 504706 |
| umicro/uView2.0 | typescript-javascript | medium | medium | 20293 | 0.023 s | 24.2 MiB | 866221 |
| stavyan/TinyShop-UniApp | typescript-javascript | medium | medium | 22036 | 1.901 s | 109.5 MiB | 11594 |
| N3-components/N3-components | typescript-javascript | large | large | 179301 | 1.830 s | 303.1 MiB | 97973 |
| gongfuxiang/shopxo-uniapp | typescript-javascript | large | large | 180390 | 1.920 s | 235.8 MiB | 93975 |
| gooking/uni-app-mall | typescript-javascript | large | large | 154319 | 0.752 s | 180.4 MiB | 205255 |
| dcloudio/uni-admin | typescript-javascript | large | large | 67902 | 2.789 s | 182.1 MiB | 24345 |
| HolodexNet/Holodex | typescript-javascript | medium | medium | 21867 | 0.047 s | 30.1 MiB | 462004 |
| FXLP/MarkTool | typescript-javascript | medium | medium | 17996 | 0.056 s | 23.3 MiB | 319698 |
| ITmonkey-cn/shopro-uniapp | typescript-javascript | medium | medium | 26214 | 0.040 s | 27.7 MiB | 655119 |
| auto-i18n/auto-i18n-translation-plugins | typescript-javascript | medium | medium | 11593 | 0.024 s | 19.9 MiB | 492166 |
| MegEngine/MegSpot | typescript-javascript | medium | medium | 15026 | 0.183 s | 53.0 MiB | 82212 |
| Akryum/guijs | typescript-javascript | medium | medium | 22392 | 0.054 s | 28.8 MiB | 412171 |
| SNFocus/approvalFlow | typescript-javascript | medium | medium | 20168 | 0.027 s | 24.0 MiB | 747171 |
| ClickerMonkey/dayspan-vuetify | typescript-javascript | medium | medium | 10090 | 0.043 s | 25.2 MiB | 234561 |
| Tucsky/aggr | typescript-javascript | medium | medium | 33619 | 0.035 s | 32.4 MiB | 950261 |
| galaxy-s10/billd-desk | typescript-javascript | small | small | 9987 | 0.023 s | 19.1 MiB | 436313 |
| zouzhibin/zb-admin | typescript-javascript | medium | medium | 12797 | 0.031 s | 20.2 MiB | 416427 |
| CorentinTh/it-tools | typescript-javascript | small | small | 9618 | 0.038 s | 18.8 MiB | 251338 |
| readpage/undraw-ui | typescript-javascript | small | small | 8180 | 0.041 s | 17.6 MiB | 199203 |
| jiangrui1994/CloudSaver | typescript-javascript | small | small | 4181 | 0.050 s | 17.1 MiB | 84332 |
| dromara/go-view | typescript-javascript | medium | medium | 22485 | 0.058 s | 23.7 MiB | 388470 |
| Hiram-Wong/zyfun | typescript-javascript | large | large | 144839 | 0.590 s | 176.5 MiB | 245315 |
| quasarframework/quasar | typescript-javascript | large | large | 127459 | 0.692 s | 206.7 MiB | 184076 |
| snapshot-labs/snapshot-v1 | typescript-javascript | medium | medium | 28785 | 0.038 s | 25.3 MiB | 766518 |
| HalseySpicy/Geeker-Admin | typescript-javascript | small | small | 8691 | 0.022 s | 18.5 MiB | 392216 |
| youzan/vant | typescript-javascript | medium | medium | 41040 | 0.134 s | 35.7 MiB | 306546 |
| thinkgem/jeesite | typescript-javascript | medium | medium | 49070 | 0.429 s | 49.8 MiB | 114377 |
| FreeTubeApp/FreeTube | typescript-javascript | medium | medium | 34861 | 0.060 s | 32.5 MiB | 578543 |
| romainsimon/uisfx | typescript-javascript | small | small | 4665 | 0.017 s | 18.9 MiB | 273212 |
| vueuse/vueuse | typescript-javascript | medium | medium | 26071 | 0.124 s | 45.8 MiB | 209651 |
| HuLaSpark/HuLa | typescript-javascript | large | large | 51321 | 52.830 s | 269.5 MiB | 971 |
| antfu-collective/icones | typescript-javascript | small | small | 6920 | 0.012 s | 17.9 MiB | 555071 |
| AutomaApp/automa | typescript-javascript | medium | medium | 38088 | 0.036 s | 29.3 MiB | 1067165 |
| BaudDance/SerialAssistant | typescript-javascript | medium | medium | 10174 | 0.019 s | 21.9 MiB | 536078 |
| MoeKoeMusic/MoeKoeMusic | typescript-javascript | medium | medium | 18113 | 0.023 s | 22.8 MiB | 777157 |
| lin-xin/vue-manage-system | typescript-javascript | small | small | 2946 | 0.015 s | 14.9 MiB | 200420 |
| VueTorrent/VueTorrent | typescript-javascript | medium | medium | 16927 | 0.049 s | 23.7 MiB | 344327 |
| yangzongzhuan/RuoYi-Vue3 | typescript-javascript | medium | medium | 11407 | 0.018 s | 20.2 MiB | 617447 |
| lkw199711/smanga | typescript-javascript | medium | medium | 25185 | 0.097 s | 28.0 MiB | 260748 |
| Datlyfe/jira_clone | typescript-javascript | small | small | 4442 | 0.012 s | 17.2 MiB | 379676 |
| jd-opensource/nutui | typescript-javascript | medium | medium | 49371 | 0.234 s | 45.5 MiB | 211358 |
| newbee-ltd/newbee-mall-vue3-app | typescript-javascript | small | small | 5520 | 0.029 s | 19.1 MiB | 190841 |
| elk-zone/elk | typescript-javascript | medium | medium | 14317 | 0.085 s | 21.1 MiB | 169334 |
| doocs/md | typescript-javascript | medium | medium | 49236 | 0.260 s | 53.7 MiB | 189415 |
| x007xyz/flycut | typescript-javascript | small | small | 5857 | 0.029 s | 19.5 MiB | 201423 |
| jekip/naive-ui-admin | typescript-javascript | medium | medium | 11000 | 0.075 s | 18.1 MiB | 146092 |
| macrozheng/mall-admin-web | typescript-javascript | medium | medium | 11927 | 0.900 s | 98.5 MiB | 13255 |
| zyronon/TypeWords | typescript-javascript | medium | medium | 29319 | 0.988 s | 75.8 MiB | 29668 |
| EKKOLearnAI/ekko-studio | typescript-javascript | large | large | 313235 | 0.541 s | 334.0 MiB | 579413 |
| october-coder/api-check | typescript-javascript | small | small | 4317 | 0.015 s | 16.4 MiB | 293901 |
| homer-dashboard/homer | typescript-javascript | small | small | 5020 | 0.017 s | 16.3 MiB | 294850 |
| hslr-s/sun-panel | typescript-javascript | small | small | 8354 | 0.039 s | 23.1 MiB | 211599 |
| kaangiray26/geddit-app | typescript-javascript | small | small | 3543 | 0.025 s | 17.3 MiB | 140824 |
| varletjs/varlet | typescript-javascript | medium | medium | 41250 | 0.122 s | 42.1 MiB | 336836 |
| kinopio-club/kinopio-client | typescript-javascript | large | large | 54932 | 0.156 s | 53.5 MiB | 352967 |
| Hacker233/resume-design | typescript-javascript | large | large | 95396 | 0.466 s | 76.1 MiB | 204885 |
| JosephusPaye/Keen-UI | typescript-javascript | large | large | 85062 | 3.134 s | 188.6 MiB | 27144 |
| yudaocode/yudao-ui-admin-vue3 | typescript-javascript | large | large | 244434 | 0.781 s | 140.6 MiB | 312830 |
| itq5/OpenClaw-Admin | typescript-javascript | large | large | 55828 | 0.073 s | 48.6 MiB | 768173 |
| yenche123/liubai | typescript-javascript | large | large | 105213 | 0.109 s | 54.9 MiB | 965666 |
| epicmaxco/vuestic-ui | typescript-javascript | large | large | 65906 | 0.204 s | 84.6 MiB | 322333 |
| arco-design/arco-design-vue | typescript-javascript | large | large | 123587 | 0.171 s | 93.7 MiB | 723477 |
| Simon-He95/markstream-vue | typescript-javascript | large | large | 170014 | 0.319 s | 140.1 MiB | 533524 |
| KunMoe/kun-galgame-forum | typescript-javascript | large | large | 58586 | 0.577 s | 199.2 MiB | 101612 |
| staniel359/muffon | typescript-javascript | large | large | 93925 | 0.274 s | 48.7 MiB | 343152 |

Largest repository: stablyai/orca with 3455327 Production lines, 7.357 s median wall time, and 2169.4 MiB peak RSS.

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
| Cognitive complexity | 0.4203 | 0.4081 |
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
| Cognitive complexity | 0.4564 | 0.4359 |
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
| Cognitive complexity | 0.0558 | 0.0555 |
| Verbosity | 0.0004 | 0.0004 |
| Duplication | 0.0089 | 0.0083 |
| Import cycles | 0.0138 | 0.0000 |

## Repositories

| Repository | Commit | Result |
| --- | --- | --- |
| [morluto/rea](https://github.com/morluto/rea) | `412f92736e09` | PASS (exit code 0) |
| [keystonejs/keystone](https://github.com/keystonejs/keystone) | `c34ec685c3af` | PASS (exit code 0) |
| [sc-forks/solidity-coverage](https://github.com/sc-forks/solidity-coverage) | `8e52fd7eae73` | PASS (exit code 0) |
| [aurora-opensource/streetscape.gl](https://github.com/aurora-opensource/streetscape.gl) | `befae1354ca8` | PASS (exit code 0) |
| [gitkraken/vscode-gitlens](https://github.com/gitkraken/vscode-gitlens) | `195975d423d9` | PASS (exit code 0) |
| [affaan-m/ECC](https://github.com/affaan-m/ECC) | `4adeee6732a1` | PASS (exit code 0) |
| [deepseek-ai/deepseek-harness](https://github.com/deepseek-ai/deepseek-harness) | `4b399c39a6d1` | PASS (exit code 0) |
| [cloudflare/computer](https://github.com/cloudflare/computer) | `46f8d4af3a5a` | PASS (exit code 0) |
| [swamp-club/swamp](https://github.com/swamp-club/swamp) | `2d6f287adbd3` | PASS (exit code 0) |
| [react/react](https://github.com/react/react) | `edae846a9a17` | PASS (exit code 0) |
| [cloudflare/moltworker](https://github.com/cloudflare/moltworker) | `7b00c1d72921` | PASS (exit code 0) |
| [haydenull/logseq-plugin-agenda](https://github.com/haydenull/logseq-plugin-agenda) | `7cffbeeab9e9` | PASS (exit code 0) |
| [anomalyco/opencode](https://github.com/anomalyco/opencode) | `7bfda96047ae` | PASS (exit code 0) |
| [guopenghui/obsidian-language-learner](https://github.com/guopenghui/obsidian-language-learner) | `eea07a3f4715` | PASS (exit code 0) |
| [vuejs/vue](https://github.com/vuejs/vue) | `9e8870794008` | PASS (exit code 0) |
| [open-slide/open-slide](https://github.com/open-slide/open-slide) | `30b9f7f93fbb` | PASS (exit code 0) |
| [joBr99/nspanel-lovelace-ui](https://github.com/joBr99/nspanel-lovelace-ui) | `0006c6a9e72e` | PASS (exit code 0) |
| [n8n-io/n8n](https://github.com/n8n-io/n8n) | `467bcd35b9a5` | PASS (exit code 0) |
| [evolus/pencil](https://github.com/evolus/pencil) | `3fc5856b6041` | PASS (exit code 0) |
| [lobehub/lobe-vidol](https://github.com/lobehub/lobe-vidol) | `fa50051cfeaf` | PASS (exit code 0) |
| [firecrawl/firecrawl](https://github.com/firecrawl/firecrawl) | `cb8e1c09106e` | PASS (exit code 0) |
| [oblador/react-native-animatable](https://github.com/oblador/react-native-animatable) | `dfd7413734eb` | PASS (exit code 0) |
| [appandflow/react-native-ease](https://github.com/appandflow/react-native-ease) | `edbbccd9b6dd` | PASS (exit code 0) |
| [DietrichGebert/ponytail](https://github.com/DietrichGebert/ponytail) | `ebd8654c2c62` | PASS (exit code 0) |
| [PierreBeucher/cloudypad](https://github.com/PierreBeucher/cloudypad) | `b5925952099a` | PASS (exit code 0) |
| [modem-dev/hunk](https://github.com/modem-dev/hunk) | `015d39bf325c` | PASS (exit code 0) |
| [microsoft/vscode](https://github.com/microsoft/vscode) | `e8b6d8b54673` | PASS (exit code 0) |
| [usekaneo/kaneo](https://github.com/usekaneo/kaneo) | `e46680d28a00` | PASS (exit code 0) |
| [blocktronics/moebius](https://github.com/blocktronics/moebius) | `1f624d62654d` | PASS (exit code 0) |
| [elie222/rakazo](https://github.com/elie222/rakazo) | `d1a0eb6d058e` | PASS (exit code 0) |
| [clash-verge-rev/clash-verge-rev](https://github.com/clash-verge-rev/clash-verge-rev) | `1daee71a9e10` | PASS (exit code 0) |
| [DevExpress/testcafe](https://github.com/DevExpress/testcafe) | `3e933c682f24` | PASS (exit code 0) |
| [webdriverio/webdriverio](https://github.com/webdriverio/webdriverio) | `f50ab796927b` | PASS (exit code 0) |
| [anthropics/claude-code](https://github.com/anthropics/claude-code) | `2287e8ceed6e` | PASS (exit code 0) |
| [mattdelacdev/WOW](https://github.com/mattdelacdev/WOW) | `c248bd26104b` | PASS (exit code 0) |
| [PicGo/PicGo-Core](https://github.com/PicGo/PicGo-Core) | `9193ec072f07` | PASS (exit code 0) |
| [hiteshchoudhary/apihub](https://github.com/hiteshchoudhary/apihub) | `3163bf09d9ee` | PASS (exit code 0) |
| [swimlane/ngx-graph](https://github.com/swimlane/ngx-graph) | `39b2a8405d71` | PASS (exit code 0) |
| [iptv-org/iptv](https://github.com/iptv-org/iptv) | `8ff1c0aa6acc` | PASS (exit code 0) |
| [kody-hq/kody](https://github.com/kody-hq/kody) | `f6914a6f5a5d` | PASS (exit code 0) |
| [vercel/next.js](https://github.com/vercel/next.js) | `0173455ce606` | PASS (exit code 0) |
| [sghall/resonance](https://github.com/sghall/resonance) | `987eba30c99c` | PASS (exit code 0) |
| [jae-jae/Camtd](https://github.com/jae-jae/Camtd) | `23bb622a686d` | PASS (exit code 0) |
| [garrytan/gstack](https://github.com/garrytan/gstack) | `306ff23b9d2b` | PASS (exit code 0) |
| [stablyai/orca](https://github.com/stablyai/orca) | `d456987d232c` | PASS (exit code 0) |
| [reactjs/zh-hans.react.dev](https://github.com/reactjs/zh-hans.react.dev) | `4474b5d1e015` | PASS (exit code 0) |
| [excalidraw/excalidraw](https://github.com/excalidraw/excalidraw) | `dabd83c4aa6b` | PASS (exit code 0) |
| [anvaka/city-roads](https://github.com/anvaka/city-roads) | `a665d3977796` | PASS (exit code 0) |
| [LING71671/Open-ClaudeCode](https://github.com/LING71671/Open-ClaudeCode) | `1f0fab6b70c9` | PASS (exit code 0) |
| [shadcn-ui/ui](https://github.com/shadcn-ui/ui) | `0e5b2e621a9e` | PASS (exit code 0) |
| [Denrox/nestjs-microservices-example](https://github.com/Denrox/nestjs-microservices-example) | `112c1d352157` | PASS (exit code 0) |
| [earendil-works/pi](https://github.com/earendil-works/pi) | `129c326a4b63` | PASS (exit code 0) |
| [nodejs/node](https://github.com/nodejs/node) | `eaf46fc7a994` | PASS (exit code 0) |
| [purifycss/purifycss](https://github.com/purifycss/purifycss) | `be52c1c6b1b6` | PASS (exit code 0) |
| [seo-skills/seo-audit-skill](https://github.com/seo-skills/seo-audit-skill) | `f43ba12d6589` | PASS (exit code 0) |
| [TauricResearch/TradingAgents](https://github.com/TauricResearch/TradingAgents) | `121265d18d46` | PASS (exit code 0) |
| [a-ghorbani/pocketpal-ai](https://github.com/a-ghorbani/pocketpal-ai) | `25b1c1c495c8` | PASS (exit code 0) |
| [Leonxlnx/taste-skill](https://github.com/Leonxlnx/taste-skill) | `952435f63ace` | PASS (exit code 0) |
| [ChrisAbdo/MotionVariants](https://github.com/ChrisAbdo/MotionVariants) | `aca142d6027f` | PASS (exit code 0) |
| [addyosmani/agent-skills](https://github.com/addyosmani/agent-skills) | `e997ac5b9a74` | PASS (exit code 0) |
| [codebymitch/TitanBot](https://github.com/codebymitch/TitanBot) | `c59378a52a66` | PASS (exit code 0) |
| [DataV-Team/DataV-React](https://github.com/DataV-Team/DataV-React) | `423998d8ebe6` | PASS (exit code 0) |
| [yang991178/fluent-reader](https://github.com/yang991178/fluent-reader) | `591207030975` | PASS (exit code 0) |
| [mulaRahul/keyviz](https://github.com/mulaRahul/keyviz) | `a2be40778aa3` | PASS (exit code 0) |
| [shahabyazdi/react-multi-date-picker](https://github.com/shahabyazdi/react-multi-date-picker) | `ad739ff1a5e3` | PASS (exit code 0) |
| [Netflix-Skunkworks/sketchy](https://github.com/Netflix-Skunkworks/sketchy) | `24909d660955` | PASS (exit code 0) |
| [star-history/star-history](https://github.com/star-history/star-history) | `8f5608175d95` | PASS (exit code 0) |
| [bevacqua/react-dragula](https://github.com/bevacqua/react-dragula) | `25c363ce48f9` | PASS (exit code 0) |
| [axios/axios](https://github.com/axios/axios) | `71c87d506567` | PASS (exit code 0) |
| [sindresorhus/ora](https://github.com/sindresorhus/ora) | `413994cb92cc` | PASS (exit code 0) |
| [modelcontextprotocol/modelcontextprotocol](https://github.com/modelcontextprotocol/modelcontextprotocol) | `024d5255eb44` | PASS (exit code 0) |
| [jenissimo/unfake.js](https://github.com/jenissimo/unfake.js) | `663686899e83` | PASS (exit code 0) |
| [zuiidea/antd-admin](https://github.com/zuiidea/antd-admin) | `4dc473ba5563` | PASS (exit code 0) |
| [dreamhunter2333/chatgpt-tarot-divination](https://github.com/dreamhunter2333/chatgpt-tarot-divination) | `44d185187d1b` | PASS (exit code 0) |
| [GoogleCloudPlatform/knowledge-catalog](https://github.com/GoogleCloudPlatform/knowledge-catalog) | `c7947f60eb8a` | PASS (exit code 0) |
| [runtipi/runtipi](https://github.com/runtipi/runtipi) | `881559aebc74` | PASS (exit code 0) |
| [OpenCut-app/OpenCut](https://github.com/OpenCut-app/OpenCut) | `e66801077856` | PASS (exit code 0) |
| [bpmn-io/bpmn-js](https://github.com/bpmn-io/bpmn-js) | `3327bcf5c823` | PASS (exit code 0) |
| [Raathigesh/fabulous](https://github.com/Raathigesh/fabulous) | `10d55a87af42` | PASS (exit code 0) |
| [sinonjs/sinon](https://github.com/sinonjs/sinon) | `617dadd2004a` | PASS (exit code 0) |
| [tailwindlabs/tailwindcss](https://github.com/tailwindlabs/tailwindcss) | `f95406ebb8d9` | PASS (exit code 0) |
| [githubdulong/Script](https://github.com/githubdulong/Script) | `edce72c33d86` | PASS (exit code 0) |
| [voidcosmos/npkill](https://github.com/voidcosmos/npkill) | `ca908483869c` | PASS (exit code 0) |
| [puppeteer/puppeteer](https://github.com/puppeteer/puppeteer) | `d1ab44e96472` | PASS (exit code 0) |
| [louislam/uptime-kuma](https://github.com/louislam/uptime-kuma) | `dcc55dcbd7ed` | PASS (exit code 0) |
| [microsoft/fast](https://github.com/microsoft/fast) | `544a7306c5a5` | PASS (exit code 0) |
| [nexu-io/codex-slides](https://github.com/nexu-io/codex-slides) | `dbc2a5992e93` | PASS (exit code 0) |
| [zulip/zulip-desktop](https://github.com/zulip/zulip-desktop) | `4a5471e96704` | PASS (exit code 0) |
| [Egonex-AI/Understand-Anything](https://github.com/Egonex-AI/Understand-Anything) | `b232f3564462` | PASS (exit code 0) |
| [adamwdraper/Numeral-js](https://github.com/adamwdraper/Numeral-js) | `7de892ffb438` | PASS (exit code 0) |
| [modelcontextprotocol/servers](https://github.com/modelcontextprotocol/servers) | `c799c8552b0f` | PASS (exit code 0) |
| [margelo/react-native-vision-camera](https://github.com/margelo/react-native-vision-camera) | `d54fc486c3a9` | PASS (exit code 0) |
| [ChatGPTNextWeb/NextChat](https://github.com/ChatGPTNextWeb/NextChat) | `1d3dc3315129` | PASS (exit code 0) |
| [taskforcesh/bullmq](https://github.com/taskforcesh/bullmq) | `fd7bf342e852` | PASS (exit code 0) |
| [pastelsky/bundlephobia](https://github.com/pastelsky/bundlephobia) | `7dd02d00611b` | PASS (exit code 0) |
| [unicity-sphere/sphere](https://github.com/unicity-sphere/sphere) | `78e8589eb3ee` | PASS (exit code 0) |
| [ntegrals/openbrowser](https://github.com/ntegrals/openbrowser) | `067fc45d649b` | PASS (exit code 0) |
| [ruanyf/webpack-demos](https://github.com/ruanyf/webpack-demos) | `3a4da6852c82` | PASS (exit code 0) |
| [Fission-AI/OpenSpec](https://github.com/Fission-AI/OpenSpec) | `f1cd852f4edc` | PASS (exit code 0) |
| [Eugeny/tabby](https://github.com/Eugeny/tabby) | `dc72f059410a` | PASS (exit code 0) |
| [ultraworkers/claw-code](https://github.com/ultraworkers/claw-code) | `52bbfccb2a15` | PASS (exit code 0) |
| [SeaQL/sea-orm](https://github.com/SeaQL/sea-orm) | `c453c64eb0a5` | PASS (exit code 0) |
| [farion1231/cc-switch](https://github.com/farion1231/cc-switch) | `1963028ee162` | PASS (exit code 0) |
| [chenxiaolong/avbroot](https://github.com/chenxiaolong/avbroot) | `7dfb91140251` | PASS (exit code 0) |
| [openai/codex](https://github.com/openai/codex) | `c7005a3d84c3` | PASS (exit code 0) |
| [pkgxdev/pkgx](https://github.com/pkgxdev/pkgx) | `e927f6e0f81e` | PASS (exit code 0) |
| [rustdesk/rustdesk](https://github.com/rustdesk/rustdesk) | `45e34a69239c` | PASS (exit code 0) |
| [funstory-ai/BabelDOC](https://github.com/funstory-ai/BabelDOC) | `dd9adc352b95` | PASS (exit code 0) |
| [lanyeeee/copymanga-downloader](https://github.com/lanyeeee/copymanga-downloader) | `9de207a90f91` | PASS (exit code 0) |
| [ki-editor/ki-editor](https://github.com/ki-editor/ki-editor) | `9c898ed4b5b1` | PASS (exit code 0) |
| [pbakaus/impeccable](https://github.com/pbakaus/impeccable) | `2fc5e1459efb` | PASS (exit code 0) |
| [H-M-H/Weylus](https://github.com/H-M-H/Weylus) | `38a01a8f8e42` | PASS (exit code 0) |
| [estuary/flow](https://github.com/estuary/flow) | `02935502ee4d` | PASS (exit code 0) |
| [tauri-apps/tauri](https://github.com/tauri-apps/tauri) | `ffe0b2167c95` | PASS (exit code 0) |
| [messense/jieba-rs](https://github.com/messense/jieba-rs) | `bac49e65b86f` | PASS (exit code 0) |
| [jely2002/youtube-dl-gui](https://github.com/jely2002/youtube-dl-gui) | `c402ee39c0ea` | PASS (exit code 0) |
| [matthiaskrgr/cargo-cache](https://github.com/matthiaskrgr/cargo-cache) | `2f4497beeab9` | PASS (exit code 0) |
| [lune-org/lune](https://github.com/lune-org/lune) | `0ea48037a3aa` | PASS (exit code 0) |
| [FyroxEngine/Fyrox](https://github.com/FyroxEngine/Fyrox) | `92b214a13cb7` | PASS (exit code 0) |
| [zed-industries/zed](https://github.com/zed-industries/zed) | `150c2aaeca87` | PASS (exit code 0) |
| [bottlerocket-os/bottlerocket](https://github.com/bottlerocket-os/bottlerocket) | `59b0684a0407` | PASS (exit code 0) |
| [astral-sh/uv](https://github.com/astral-sh/uv) | `9bf01bef71c7` | PASS (exit code 0) |
| [linebender/druid](https://github.com/linebender/druid) | `0f8b1195e4e0` | PASS (exit code 0) |
| [GaoSSR/best-claude-hud](https://github.com/GaoSSR/best-claude-hud) | `0dd174e30eaa` | PASS (exit code 0) |
| [rtk-ai/rtk](https://github.com/rtk-ai/rtk) | `97e68d5358aa` | PASS (exit code 0) |
| [loco-rs/loco](https://github.com/loco-rs/loco) | `0fce092950d4` | PASS (exit code 0) |
| [unionlabs/union](https://github.com/unionlabs/union) | `031785bb6dc6` | PASS (exit code 0) |
| [bbqsrc/cargo-ndk](https://github.com/bbqsrc/cargo-ndk) | `8cffdefbc9fc` | PASS (exit code 0) |
| [dani-garcia/vaultwarden](https://github.com/dani-garcia/vaultwarden) | `dbd7829ed140` | PASS (exit code 0) |
| [skymanbp/autoshade](https://github.com/skymanbp/autoshade) | `3e200af5e88b` | PASS (exit code 0) |
| [BurntSushi/ripgrep](https://github.com/BurntSushi/ripgrep) | `3e85c6c148fd` | PASS (exit code 0) |
| [humblepenguinn/envio](https://github.com/humblepenguinn/envio) | `c5e3463644cb` | PASS (exit code 0) |
| [ccfddl/ccf-deadlines](https://github.com/ccfddl/ccf-deadlines) | `9bc33889e2f4` | PASS (exit code 0) |
| [image-rs/imageproc](https://github.com/image-rs/imageproc) | `e825693df992` | PASS (exit code 0) |
| [openinterpreter/openinterpreter](https://github.com/openinterpreter/openinterpreter) | `520887a24c38` | PASS (exit code 0) |
| [qarmin/szyszka](https://github.com/qarmin/szyszka) | `cd1ed2cb375a` | PASS (exit code 0) |
| [alacritty/alacritty](https://github.com/alacritty/alacritty) | `791efff6d74c` | PASS (exit code 0) |
| [paradedb/paradedb](https://github.com/paradedb/paradedb) | `da0ea4461040` | PASS (exit code 0) |
| [winnow-rs/winnow](https://github.com/winnow-rs/winnow) | `4c5941702d96` | PASS (exit code 0) |
| [jpochyla/psst](https://github.com/jpochyla/psst) | `40024e23b3df` | PASS (exit code 0) |
| [k06a/macpow](https://github.com/k06a/macpow) | `696de22d87ac` | PASS (exit code 0) |
| [databendlabs/databend](https://github.com/databendlabs/databend) | `85d49ed98003` | PASS (exit code 0) |
| [apollographql/router](https://github.com/apollographql/router) | `110f7694b7d5` | PASS (exit code 0) |
| [tw93/Pake](https://github.com/tw93/Pake) | `fbba237182ed` | PASS (exit code 0) |
| [1Password/arboard](https://github.com/1Password/arboard) | `538073486463` | PASS (exit code 0) |
| [sharkdp/bat](https://github.com/sharkdp/bat) | `72505e4555dc` | PASS (exit code 0) |
| [omerbenamram/evtx](https://github.com/omerbenamram/evtx) | `5d0a82b7b5a8` | PASS (exit code 0) |
| [FuelLabs/sway](https://github.com/FuelLabs/sway) | `2ce2b31118ef` | PASS (exit code 0) |
| [actix/actix](https://github.com/actix/actix) | `4fed9792ae78` | PASS (exit code 0) |
| [torrentplayer/torrentplayer](https://github.com/torrentplayer/torrentplayer) | `a4533bc6a5df` | PASS (exit code 0) |
| [starship/starship](https://github.com/starship/starship) | `064b6f20752a` | PASS (exit code 0) |
| [storytold/effectcraft](https://github.com/storytold/effectcraft) | `bfe6878d6535` | PASS (exit code 0) |
| [superradcompany/microsandbox](https://github.com/superradcompany/microsandbox) | `ab152d65e1c2` | PASS (exit code 0) |
| [wasm-bindgen/wasm-bindgen](https://github.com/wasm-bindgen/wasm-bindgen) | `c8ddc95c888c` | PASS (exit code 0) |
| [LLukas22/Jellyswarrm](https://github.com/LLukas22/Jellyswarrm) | `585f91637207` | PASS (exit code 0) |
| [typst/typst](https://github.com/typst/typst) | `1e62e68e9bdd` | PASS (exit code 0) |
| [kata-containers/kata-containers](https://github.com/kata-containers/kata-containers) | `620fcb59449b` | PASS (exit code 0) |
| [EpicGames/lore](https://github.com/EpicGames/lore) | `2219b7ef848c` | PASS (exit code 0) |
| [crocuda/pipelight](https://github.com/crocuda/pipelight) | `478d67baa8d1` | PASS (exit code 0) |
| [FuelLabs/fuel-core](https://github.com/FuelLabs/fuel-core) | `8bd82d4d1ff4` | PASS (exit code 0) |
| [spacejam/sled](https://github.com/spacejam/sled) | `e449d17111f4` | PASS (exit code 0) |
| [bytestring-net/bevy_lunex](https://github.com/bytestring-net/bevy_lunex) | `15fdc857d938` | PASS (exit code 0) |
| [Morganamilo/paru](https://github.com/Morganamilo/paru) | `df7d6d78318d` | PASS (exit code 0) |
| [yuxino/Mimi](https://github.com/yuxino/Mimi) | `f78b257f43c3` | PASS (exit code 0) |
| [vercel-labs/agent-browser](https://github.com/vercel-labs/agent-browser) | `3f43a2affd99` | PASS (exit code 0) |
| [pydantic/monty](https://github.com/pydantic/monty) | `a96e6bd72a0f` | PASS (exit code 0) |
| [mfontanini/presenterm](https://github.com/mfontanini/presenterm) | `aa667f91352e` | PASS (exit code 0) |
| [j178/prek](https://github.com/j178/prek) | `4ba8189c4c90` | PASS (exit code 0) |
| [rust-num/num-traits](https://github.com/rust-num/num-traits) | `1166177ec398` | PASS (exit code 0) |
| [clojure-rs/ClojureRS](https://github.com/clojure-rs/ClojureRS) | `cb64b4340954` | PASS (exit code 0) |
| [vi/websocat](https://github.com/vi/websocat) | `c8d9185087cf` | PASS (exit code 0) |
| [sharkdp/fd](https://github.com/sharkdp/fd) | `2e8648f10fe5` | PASS (exit code 0) |
| [crossbeam-rs/crossbeam](https://github.com/crossbeam-rs/crossbeam) | `81ff7fe69e8c` | PASS (exit code 0) |
| [tower-rs/tower-http](https://github.com/tower-rs/tower-http) | `8569ec8b99f6` | PASS (exit code 0) |
| [RustCrypto/AEADs](https://github.com/RustCrypto/AEADs) | `1ad331608610` | PASS (exit code 0) |
| [Leafwing-Studios/leafwing-input-manager](https://github.com/Leafwing-Studios/leafwing-input-manager) | `d07faab19380` | PASS (exit code 0) |
| [FuelLabs/fuels-rs](https://github.com/FuelLabs/fuels-rs) | `f5191b6aab3d` | PASS (exit code 0) |
| [Nazariglez/notan](https://github.com/Nazariglez/notan) | `385e22a730ad` | PASS (exit code 0) |
| [unicity-aos/aos-ce](https://github.com/unicity-aos/aos-ce) | `6e4e8ec147ea` | PASS (exit code 0) |
| [anza-xyz/pinocchio](https://github.com/anza-xyz/pinocchio) | `b45dd2a98e34` | PASS (exit code 0) |
| [pop-os/libcosmic](https://github.com/pop-os/libcosmic) | `009fb9d4138f` | PASS (exit code 0) |
| [neon-bindings/neon](https://github.com/neon-bindings/neon) | `452108e000b7` | PASS (exit code 0) |
| [RoaringBitmap/roaring-rs](https://github.com/RoaringBitmap/roaring-rs) | `fe8ac700d371` | PASS (exit code 0) |
| [Ruddle/Fomos](https://github.com/Ruddle/Fomos) | `336be2d5bb27` | PASS (exit code 0) |
| [fwdai/rowel](https://github.com/fwdai/rowel) | `f1363a8652fa` | PASS (exit code 0) |
| [cjpais/Handy](https://github.com/cjpais/Handy) | `792625e9c506` | PASS (exit code 0) |
| [cross-rs/cross](https://github.com/cross-rs/cross) | `a9301e91f72b` | PASS (exit code 0) |
| [pls-rs/pls](https://github.com/pls-rs/pls) | `26ce12ebbd8c` | PASS (exit code 0) |
| [AlexsJones/llmfit](https://github.com/AlexsJones/llmfit) | `3e7a8fc4215a` | PASS (exit code 0) |
| [zerobrewhq/zerobrew](https://github.com/zerobrewhq/zerobrew) | `cb288c08ccfd` | PASS (exit code 0) |
| [paperclip-rs/paperclip](https://github.com/paperclip-rs/paperclip) | `8d8f0c6b0772` | PASS (exit code 0) |
| [ck-zhang/reddix](https://github.com/ck-zhang/reddix) | `e2df2f48d583` | PASS (exit code 0) |
| [fujiapple852/trippy](https://github.com/fujiapple852/trippy) | `938605ba3462` | PASS (exit code 0) |
| [alloy-rs/core](https://github.com/alloy-rs/core) | `e81d93ccc17e` | PASS (exit code 0) |
| [napi-rs/napi-rs](https://github.com/napi-rs/napi-rs) | `fe23eb08a52e` | PASS (exit code 0) |
| [jeremychone/rust-genai](https://github.com/jeremychone/rust-genai) | `8faa1a4d5240` | PASS (exit code 0) |
| [tokio-rs/axum](https://github.com/tokio-rs/axum) | `8d8944e6db1a` | PASS (exit code 0) |
| [rust-lang/rustup](https://github.com/rust-lang/rustup) | `7747d0cd242b` | PASS (exit code 0) |
| [actix/actix-web](https://github.com/actix/actix-web) | `91254d42921c` | PASS (exit code 0) |
| [tokio-rs/tracing](https://github.com/tokio-rs/tracing) | `8d4d4d6ceaba` | PASS (exit code 0) |
| [coollabsio/coolify](https://github.com/coollabsio/coolify) | `9f2c812bd421` | PASS (exit code 0) |
| [kanboard/kanboard](https://github.com/kanboard/kanboard) | `c04dc0a2b5d6` | PASS (exit code 0) |
| [bobthecow/psysh](https://github.com/bobthecow/psysh) | `64b18aa0d5f8` | PASS (exit code 0) |
| [laravel/framework](https://github.com/laravel/framework) | `321caf77d6c0` | PASS (exit code 0) |
| [googleapis/google-api-php-client](https://github.com/googleapis/google-api-php-client) | `d063284a621f` | FAIL (exit code 0) |
| [symfony/console](https://github.com/symfony/console) | `ea1860277087` | FAIL (exit code 0) |
| [nuxsmin/sysPass](https://github.com/nuxsmin/sysPass) | `9f204f1150ae` | PASS (exit code 0) |
| [symfony/symfony](https://github.com/symfony/symfony) | `922264783668` | PASS (exit code 0) |
| [doctrine/dbal](https://github.com/doctrine/dbal) | `733584dee067` | PASS (exit code 0) |
| [composer/composer](https://github.com/composer/composer) | `a52d230b3a12` | PASS (exit code 0) |
| [pterodactyl/panel](https://github.com/pterodactyl/panel) | `60a100158158` | PASS (exit code 0) |
| [SolidInvoice/SolidInvoice](https://github.com/SolidInvoice/SolidInvoice) | `8d5227f52794` | PASS (exit code 0) |
| [monicahq/monica](https://github.com/monicahq/monica) | `189cd02eff4c` | PASS (exit code 0) |
| [firefly-iii/firefly-iii](https://github.com/firefly-iii/firefly-iii) | `3bba58800a08` | PASS (exit code 0) |
| [krayin/laravel-crm](https://github.com/krayin/laravel-crm) | `64ef70505c5d` | PASS (exit code 0) |
| [dingo/api](https://github.com/dingo/api) | `42b6afa6e20a` | PASS (exit code 0) |
| [livewire/livewire](https://github.com/livewire/livewire) | `0b8f03c9b7f5` | FAIL (exit code 0) |
| [RSS-Bridge/rss-bridge](https://github.com/RSS-Bridge/rss-bridge) | `beb20b0fd086` | PASS (exit code 0) |
| [guzzle/guzzle](https://github.com/guzzle/guzzle) | `ba6574fd975a` | PASS (exit code 0) |
| [phpDocumentor/ReflectionDocBlock](https://github.com/phpDocumentor/ReflectionDocBlock) | `4c7c7b1561dd` | PASS (exit code 0) |
| [PHPMailer/PHPMailer](https://github.com/PHPMailer/PHPMailer) | `a3689ab9ab48` | PASS (exit code 0) |
| [PrestaShop/PrestaShop](https://github.com/PrestaShop/PrestaShop) | `9248ef7e76fb` | PASS (exit code 0) |
| [DesignPatternsPHP/DesignPatternsPHP](https://github.com/DesignPatternsPHP/DesignPatternsPHP) | `54254e0f2a59` | PASS (exit code 0) |
| [matomo-org/matomo](https://github.com/matomo-org/matomo) | `bcc0f7b0a591` | PASS (exit code 0) |
| [solidtime-io/solidtime](https://github.com/solidtime-io/solidtime) | `ec74a1d841c7` | PASS (exit code 0) |
| [phpDocumentor/TypeResolver](https://github.com/phpDocumentor/TypeResolver) | `dbe2133bb23e` | PASS (exit code 0) |
| [TasmoAdmin/TasmoAdmin](https://github.com/TasmoAdmin/TasmoAdmin) | `b378d7f85b6d` | PASS (exit code 0) |
| [Seldaek/monolog](https://github.com/Seldaek/monolog) | `6d4cf3a1199b` | PASS (exit code 0) |
| [sebastianbergmann/phpunit](https://github.com/sebastianbergmann/phpunit) | `04eb84520c95` | PASS (exit code 0) |
| [fruitcake/laravel-debugbar](https://github.com/fruitcake/laravel-debugbar) | `033b34a64489` | FAIL (exit code 0) |
| [sebastianbergmann/php-code-coverage](https://github.com/sebastianbergmann/php-code-coverage) | `d1932d4a2759` | PASS (exit code 0) |
| [ellite/Wallos](https://github.com/ellite/Wallos) | `6e43a0d81997` | PASS (exit code 0) |
| [FTXInfotech/laravel-adminpanel](https://github.com/FTXInfotech/laravel-adminpanel) | `35b45f461dd3` | PASS (exit code 0) |
| [bcit-ci/CodeIgniter](https://github.com/bcit-ci/CodeIgniter) | `3658d731eaab` | PASS (exit code 0) |
| [nikic/PHP-Parser](https://github.com/nikic/PHP-Parser) | `24c72b4d8413` | PASS (exit code 0) |
| [owncloud/core](https://github.com/owncloud/core) | `cc614bf702cc` | PASS (exit code 0) |
| [commerceguys/addressing](https://github.com/commerceguys/addressing) | `390ebd12f93c` | PASS (exit code 0) |
| [koel/koel](https://github.com/koel/koel) | `eae22b1a9670` | PASS (exit code 0) |
| [cakephp/cakephp](https://github.com/cakephp/cakephp) | `0c1cc741b529` | PASS (exit code 0) |
| [aimeos/aimeos-laravel](https://github.com/aimeos/aimeos-laravel) | `2e0f31528212` | PASS (exit code 0) |
| [briannesbitt/Carbon](https://github.com/briannesbitt/Carbon) | `4c5056ed90f6` | PASS (exit code 0) |
| [symfony/http-foundation](https://github.com/symfony/http-foundation) | `92dbc703438a` | PASS (exit code 0) |
| [Sylius/Sylius](https://github.com/Sylius/Sylius) | `7a0a593e8da4` | PASS (exit code 0) |
| [getgrav/grav](https://github.com/getgrav/grav) | `190ad5899be4` | PASS (exit code 0) |
| [swlib/saber](https://github.com/swlib/saber) | `4bfa9d8d735a` | PASS (exit code 0) |
| [phpspec/prophecy](https://github.com/phpspec/prophecy) | `09c2e5949d67` | PASS (exit code 0) |
| [grokability/snipe-it](https://github.com/grokability/snipe-it) | `8986cbdbffb9` | PASS (exit code 0) |
| [barryvdh/laravel-ide-helper](https://github.com/barryvdh/laravel-ide-helper) | `66f7ec11122b` | PASS (exit code 0) |
| [crater-invoice-inc/crater](https://github.com/crater-invoice-inc/crater) | `05d5ce26fdd8` | PASS (exit code 0) |
| [osgochina/swoole-crontab](https://github.com/osgochina/swoole-crontab) | `c8a18887c215` | PASS (exit code 0) |
| [opencart/opencart](https://github.com/opencart/opencart) | `3ff81361a721` | PASS (exit code 0) |
| [zdhxiong/mdclub](https://github.com/zdhxiong/mdclub) | `f5b1fa905de8` | PASS (exit code 0) |
| [Intervention/image](https://github.com/Intervention/image) | `7d8ef63ee953` | PASS (exit code 0) |
| [symfony/http-kernel](https://github.com/symfony/http-kernel) | `c3830ca6154b` | PASS (exit code 0) |
| [Las-Fuerzas-Del-Cielo/Sistema-Anti-Fraude-Electoral](https://github.com/Las-Fuerzas-Del-Cielo/Sistema-Anti-Fraude-Electoral) | `051da907b8ae` | PASS (exit code 0) |
| [phpstan/phpstan](https://github.com/phpstan/phpstan) | `b3ba28f5d1de` | PASS (exit code 0) |
| [fightbulc/moment.php](https://github.com/fightbulc/moment.php) | `2aa67d3b0e3f` | PASS (exit code 0) |
| [digininja/DVWA](https://github.com/digininja/DVWA) | `502ac150ce37` | PASS (exit code 0) |
| [tedious/Stash](https://github.com/tedious/Stash) | `39c426ed180f` | PASS (exit code 0) |
| [PHPOffice/PhpSpreadsheet](https://github.com/PHPOffice/PhpSpreadsheet) | `d7fb3e9505e2` | FAIL (exit code 0) |
| [phpmyadmin/phpmyadmin](https://github.com/phpmyadmin/phpmyadmin) | `8d3a2d5ddef2` | PASS (exit code 0) |
| [pdepend/pdepend](https://github.com/pdepend/pdepend) | `934ebec1f1ed` | PASS (exit code 0) |
| [vrana/adminer](https://github.com/vrana/adminer) | `ffb07d20f08a` | FAIL (exit code 0) |
| [thephpleague/flysystem](https://github.com/thephpleague/flysystem) | `12930254888f` | PASS (exit code 0) |
| [PHP-CS-Fixer/PHP-CS-Fixer](https://github.com/PHP-CS-Fixer/PHP-CS-Fixer) | `511285399374` | PASS (exit code 0) |
| [web-token/jwt-framework](https://github.com/web-token/jwt-framework) | `024031b36eb1` | PASS (exit code 0) |
| [filp/whoops](https://github.com/filp/whoops) | `d5480d53f100` | PASS (exit code 0) |
| [symfony/polyfill-mbstring](https://github.com/symfony/polyfill-mbstring) | `122889ed24dc` | PASS (exit code 0) |
| [CodepadME/laravel-tricks](https://github.com/CodepadME/laravel-tricks) | `2f401473f199` | PASS (exit code 0) |
| [wallabag/wallabag](https://github.com/wallabag/wallabag) | `3db73d0ba254` | PASS (exit code 0) |
| [predis/predis](https://github.com/predis/predis) | `2fb877b77577` | PASS (exit code 0) |
| [mlazarov/supervisord-monitor](https://github.com/mlazarov/supervisord-monitor) | `6b0f3577fd86` | PASS (exit code 0) |
| [spatie/laravel-permission](https://github.com/spatie/laravel-permission) | `753b6fbda5ff` | PASS (exit code 0) |
| [pgrimaud/instagram-user-feed](https://github.com/pgrimaud/instagram-user-feed) | `8e39f0f3f6ba` | PASS (exit code 0) |
| [HaschekSolutions/pictshare](https://github.com/HaschekSolutions/pictshare) | `05426aab1f8b` | PASS (exit code 0) |
| [SpartnerNL/Laravel-Excel](https://github.com/SpartnerNL/Laravel-Excel) | `ba07e2d82e38` | PASS (exit code 0) |
| [phparkitect/arkitect](https://github.com/phparkitect/arkitect) | `ed8d635e4741` | PASS (exit code 0) |
| [ramsey/uuid](https://github.com/ramsey/uuid) | `7a16331da5bb` | PASS (exit code 0) |
| [typecho/typecho](https://github.com/typecho/typecho) | `1e53c04e1ef9` | PASS (exit code 0) |
| [atymic/twitter](https://github.com/atymic/twitter) | `bc0dd97f8c67` | PASS (exit code 0) |
| [aureuserp/aureuserp](https://github.com/aureuserp/aureuserp) | `34bf04cb4c46` | PASS (exit code 0) |
| [phacility/phabricator](https://github.com/phacility/phabricator) | `c64f9e461965` | PASS (exit code 0) |
| [PHPOffice/PHPWord](https://github.com/PHPOffice/PHPWord) | `63339365b9b0` | PASS (exit code 0) |
| [AdvancedCustomFields/acf](https://github.com/AdvancedCustomFields/acf) | `80cf0c9f0c4d` | PASS (exit code 0) |
| [Leantime/leantime](https://github.com/Leantime/leantime) | `2b749836c96b` | PASS (exit code 0) |
| [pestphp/pest](https://github.com/pestphp/pest) | `8d004b80d751` | PASS (exit code 0) |
| [WINBIGFOX/TimeScribe](https://github.com/WINBIGFOX/TimeScribe) | `dfa6ed507a35` | PASS (exit code 0) |
| [investbrainapp/investbrain](https://github.com/investbrainapp/investbrain) | `48ff78af4ef9` | PASS (exit code 0) |
| [dompdf/dompdf](https://github.com/dompdf/dompdf) | `0b4d9ef5c008` | FAIL (exit code 0) |
| [octobercms/october](https://github.com/octobercms/october) | `84ddc71413b5` | PASS (exit code 0) |
| [roundcube/roundcubemail](https://github.com/roundcube/roundcubemail) | `b8f1e741f983` | FAIL (exit code 0) |
| [libretime/libretime](https://github.com/libretime/libretime) | `97290cf3a340` | PASS (exit code 0) |
| [z-song/laravel-admin](https://github.com/z-song/laravel-admin) | `67c441eb78ec` | FAIL (exit code 0) |
| [laramint/laravel-brain](https://github.com/laramint/laravel-brain) | `07df09a40269` | PASS (exit code 0) |
| [nextcloud/all-in-one](https://github.com/nextcloud/all-in-one) | `5446eb3eb4f0` | PASS (exit code 0) |
| [open-telemetry/opentelemetry-php](https://github.com/open-telemetry/opentelemetry-php) | `241b4724a91d` | PASS (exit code 0) |
| [anonaddy/anonaddy](https://github.com/anonaddy/anonaddy) | `6a6fcc063ee6` | PASS (exit code 0) |
| [vanilophp/framework](https://github.com/vanilophp/framework) | `770ed17366eb` | PASS (exit code 0) |
| [Codeception/Codeception](https://github.com/Codeception/Codeception) | `87a32e4f3a97` | FAIL (exit code 0) |
| [webonyx/graphql-php](https://github.com/webonyx/graphql-php) | `f7e30ebc7157` | PASS (exit code 0) |
| [flutter/flutter](https://github.com/flutter/flutter) | `43150a886d38` | PASS (exit code 0) |
| [firebase/flutterfire](https://github.com/firebase/flutterfire) | `1e500b3a39ae` | PASS (exit code 0) |
| [AppFlowy-IO/AppFlowy](https://github.com/AppFlowy-IO/AppFlowy) | `8fcee27d9bd9` | PASS (exit code 0) |
| [ABausG/home_widget](https://github.com/ABausG/home_widget) | `eeced9098511` | PASS (exit code 0) |
| [chen08209/FlClash](https://github.com/chen08209/FlClash) | `deba70d80d2a` | PASS (exit code 0) |
| [lollipopkit/flutter_server_box](https://github.com/lollipopkit/flutter_server_box) | `fa7941a0b3d4` | PASS (exit code 0) |
| [namanshergill/diohub](https://github.com/namanshergill/diohub) | `246002f81e50` | PASS (exit code 0) |
| [brianegan/flutter_architecture_samples](https://github.com/brianegan/flutter_architecture_samples) | `d898d1329e04` | PASS (exit code 0) |
| [Predidit/Kazumi](https://github.com/Predidit/Kazumi) | `889555ad44a3` | PASS (exit code 0) |
| [simplezhli/flutter_deer](https://github.com/simplezhli/flutter_deer) | `7b86fd00560c` | PASS (exit code 0) |
| [Davies-K/Taskez](https://github.com/Davies-K/Taskez) | `0b9b7b468110` | PASS (exit code 0) |
| [Sle2p/AniCh](https://github.com/Sle2p/AniCh) | `c5729a28af23` | PASS (exit code 0) |
| [OpenStrap/edge](https://github.com/OpenStrap/edge) | `a2d76654b16d` | PASS (exit code 0) |
| [ComicSparks/pikapika](https://github.com/ComicSparks/pikapika) | `fa3605e3128a` | PASS (exit code 0) |
| [alibaba/flutter-go](https://github.com/alibaba/flutter-go) | `a41b2124278e` | PASS (exit code 0) |
| [nightmare-space/any_share](https://github.com/nightmare-space/any_share) | `b9c067b1668b` | PASS (exit code 0) |
| [ImranR98/Obtainium](https://github.com/ImranR98/Obtainium) | `c570dccb097e` | PASS (exit code 0) |
| [alibaba/fish-redux](https://github.com/alibaba/fish-redux) | `40b853bfa084` | PASS (exit code 0) |
| [yako-dev/flutter-settings-ui](https://github.com/yako-dev/flutter-settings-ui) | `6b1599310647` | PASS (exit code 0) |
| [getlantern/lantern](https://github.com/getlantern/lantern) | `cda7d8ebfcaf` | PASS (exit code 0) |
| [sunarya-thito/shadcn_flutter](https://github.com/sunarya-thito/shadcn_flutter) | `611caafcdfa0` | PASS (exit code 0) |
| [xiaoyaocz/dart_simple_live](https://github.com/xiaoyaocz/dart_simple_live) | `ba828e6783b1` | PASS (exit code 0) |
| [UnboundTechCo/defyxVPN](https://github.com/UnboundTechCo/defyxVPN) | `662fd3795642` | PASS (exit code 0) |
| [bagisto/opensource-ecommerce-mobile-app](https://github.com/bagisto/opensource-ecommerce-mobile-app) | `d2b758a777bc` | PASS (exit code 0) |
| [dart-lang/pub-dev](https://github.com/dart-lang/pub-dev) | `a6b664df3b74` | PASS (exit code 0) |
| [CarGuo/gsy_github_app_flutter](https://github.com/CarGuo/gsy_github_app_flutter) | `f5e6565a60be` | PASS (exit code 0) |
| [wanghongenpin/proxypin](https://github.com/wanghongenpin/proxypin) | `2d13a0a1a01a` | PASS (exit code 0) |
| [2d-inc/HistoryOfEverything](https://github.com/2d-inc/HistoryOfEverything) | `1864cfb7f2e6` | PASS (exit code 0) |
| [Notsfsssf/pixez-flutter](https://github.com/Notsfsssf/pixez-flutter) | `7db05ce25f09` | PASS (exit code 0) |
| [cfug/dio](https://github.com/cfug/dio) | `f1bef462289b` | PASS (exit code 0) |
| [samarthagarwal/FlutterScreens](https://github.com/samarthagarwal/FlutterScreens) | `8afc1abba2be` | PASS (exit code 0) |
| [ubuntu/app-center](https://github.com/ubuntu/app-center) | `b343e7338e18` | PASS (exit code 0) |
| [felangel/bloc](https://github.com/felangel/bloc) | `3a123e219e9e` | PASS (exit code 0) |
| [Canardoux/flutter_sound](https://github.com/Canardoux/flutter_sound) | `abb8aac12838` | PASS (exit code 0) |
| [flutterchina/flukit](https://github.com/flutterchina/flukit) | `862e033c33f1` | PASS (exit code 0) |
| [jonataslaw/getx](https://github.com/jonataslaw/getx) | `a294d2fbfc65` | PASS (exit code 0) |
| [jiangtian616/JHenTai](https://github.com/jiangtian616/JHenTai) | `f6996bb9ef6f` | PASS (exit code 0) |
| [flame-engine/flame](https://github.com/flame-engine/flame) | `28425cc29dc3` | PASS (exit code 0) |
| [miru-project/miru-app](https://github.com/miru-project/miru-app) | `8599d0dbfbb1` | PASS (exit code 0) |
| [KaringX/clashmi](https://github.com/KaringX/clashmi) | `d5dbd5090949` | PASS (exit code 0) |
| [Sky24n/flutter_wanandroid](https://github.com/Sky24n/flutter_wanandroid) | `51061b34a492` | PASS (exit code 0) |
| [StacDev/stac](https://github.com/StacDev/stac) | `45073378a971` | FAIL (exit code 0) |
| [ComicSparks/jasmine](https://github.com/ComicSparks/jasmine) | `3e2d53fd6bb9` | PASS (exit code 0) |
| [mirarr-app/network-checker](https://github.com/mirarr-app/network-checker) | `d450fdc5e355` | FAIL (exit code 0) |
| [btwld/fvm](https://github.com/btwld/fvm) | `6d8104b334aa` | PASS (exit code 0) |
| [RivaanRanawat/instagram-flutter-clone](https://github.com/RivaanRanawat/instagram-flutter-clone) | `b43f9c7871d4` | PASS (exit code 0) |
| [liuchuancong/pure_live_TV](https://github.com/liuchuancong/pure_live_TV) | `6a1186630b41` | PASS (exit code 0) |
| [4seer/openflutterecommerceapp](https://github.com/4seer/openflutterecommerceapp) | `92496f69f280` | PASS (exit code 0) |
| [jesusrp98/spacex-go](https://github.com/jesusrp98/spacex-go) | `e797faead941` | FAIL (exit code 0) |
| [rrousselGit/provider](https://github.com/rrousselGit/provider) | `3ed4c6d6b170` | PASS (exit code 0) |
| [appshubcc/Bettbox](https://github.com/appshubcc/Bettbox) | `2193159511eb` | PASS (exit code 0) |
| [appditto/natrium_wallet_flutter](https://github.com/appditto/natrium_wallet_flutter) | `814c26f6106a` | PASS (exit code 0) |
| [Regncreative/PulseDiagnostics](https://github.com/Regncreative/PulseDiagnostics) | `682a3487328b` | PASS (exit code 0) |
| [echo-loop/Echo-Loop](https://github.com/echo-loop/Echo-Loop) | `cb9414be7e8b` | PASS (exit code 0) |
| [pawlik92/flutter_whirlpool](https://github.com/pawlik92/flutter_whirlpool) | `fbf821caed38` | PASS (exit code 0) |
| [bepass-org/oblivion](https://github.com/bepass-org/oblivion) | `bd8d2ac0659a` | PASS (exit code 0) |
| [Tencent/mxflutter](https://github.com/Tencent/mxflutter) | `4ef8373a19ad` | PASS (exit code 0) |
| [harmonoid/harmonoid](https://github.com/harmonoid/harmonoid) | `ae3099f9da8e` | PASS (exit code 0) |
| [ionicfirebaseapp/getwidget](https://github.com/ionicfirebaseapp/getwidget) | `2bcf1e05d258` | PASS (exit code 0) |
| [aaronoe/FlutterCinematic](https://github.com/aaronoe/FlutterCinematic) | `3929f7052eba` | PASS (exit code 0) |
| [Chevey339/kelivo](https://github.com/Chevey339/kelivo) | `69144d443e53` | PASS (exit code 0) |
| [hacktons/convex_bottom_bar](https://github.com/hacktons/convex_bottom_bar) | `203d8e505152` | PASS (exit code 0) |
| [sdegenaar/liquid_glass_widgets](https://github.com/sdegenaar/liquid_glass_widgets) | `632dd2c1f02d` | PASS (exit code 0) |
| [mhmzdev/the-holy-quran-app](https://github.com/mhmzdev/the-holy-quran-app) | `33edb9039b24` | PASS (exit code 0) |
| [hamaluik/timecop](https://github.com/hamaluik/timecop) | `7f81590ff800` | PASS (exit code 0) |
| [gskinnerTeam/flutter-wonderous-app](https://github.com/gskinnerTeam/flutter-wonderous-app) | `30ccc36a2762` | PASS (exit code 0) |
| [bp74/StageXL](https://github.com/bp74/StageXL) | `1d1c4cbd8749` | PASS (exit code 0) |
| [gokadzev/Musify](https://github.com/gokadzev/Musify) | `9ef2891d43af` | PASS (exit code 0) |
| [mirarr-app/mirarr](https://github.com/mirarr-app/mirarr) | `b7f8ae0a916b` | PASS (exit code 0) |
| [guilherme-v/flutter-clean-architecture-example](https://github.com/guilherme-v/flutter-clean-architecture-example) | `9d506dcf58c7` | FAIL (exit code 0) |
| [GitJournal/GitJournal](https://github.com/GitJournal/GitJournal) | `c8a67e098db0` | PASS (exit code 0) |
| [YuanDevTeam/OneXray](https://github.com/YuanDevTeam/OneXray) | `9ecde06d61f5` | PASS (exit code 0) |
| [sass/dart-sass](https://github.com/sass/dart-sass) | `1bb305d1b424` | PASS (exit code 0) |
| [avdept/JellyBoxPlayer](https://github.com/avdept/JellyBoxPlayer) | `8cf5e8601504` | PASS (exit code 0) |
| [TheAlphamerc/flutter_twitter_clone](https://github.com/TheAlphamerc/flutter_twitter_clone) | `9e8defdebefa` | PASS (exit code 0) |
| [ryanheise/audio_service](https://github.com/ryanheise/audio_service) | `da405ddd0784` | PASS (exit code 0) |
| [bostrot/wslmanager](https://github.com/bostrot/wslmanager) | `77e5e6d83e2b` | PASS (exit code 0) |
| [xuelongqy/flutter_easy_refresh](https://github.com/xuelongqy/flutter_easy_refresh) | `dcdbb8143e12` | PASS (exit code 0) |
| [Sky24n/GreenTravel](https://github.com/Sky24n/GreenTravel) | `1fa8ef511dd9` | PASS (exit code 0) |
| [isar/isar](https://github.com/isar/isar) | `713001bf1615` | FAIL (exit code 0) |
| [dart-lang/build](https://github.com/dart-lang/build) | `0596d4da48f0` | PASS (exit code 0) |
| [akaMrNagar/Mindful](https://github.com/akaMrNagar/Mindful) | `b6eb68d50046` | PASS (exit code 0) |
| [gskinnerTeam/flutter-folio](https://github.com/gskinnerTeam/flutter-folio) | `be0cfa2fa54a` | PASS (exit code 0) |
| [tekartik/sembast.dart](https://github.com/tekartik/sembast.dart) | `013e3a7a0f2d` | PASS (exit code 0) |
| [pichillilorenzo/flutter_inappwebview](https://github.com/pichillilorenzo/flutter_inappwebview) | `17527cae5a33` | PASS (exit code 0) |
| [cddqssc/Caption-Trans](https://github.com/cddqssc/Caption-Trans) | `250391a81d73` | PASS (exit code 0) |
| [HTWMedia/HTV](https://github.com/HTWMedia/HTV) | `ba60d211274c` | PASS (exit code 0) |
| [roughike/inKino](https://github.com/roughike/inKino) | `d406a23c3b57` | PASS (exit code 0) |
| [Tarikul711/flutter-ecommerce](https://github.com/Tarikul711/flutter-ecommerce) | `99088b3fcc0c` | FAIL (exit code 0) |
| [best-flutter/flutter_swiper](https://github.com/best-flutter/flutter_swiper) | `71eb23671d46` | PASS (exit code 0) |
| [Tkko/Flutter_Pinput](https://github.com/Tkko/Flutter_Pinput) | `c1b4f881b167` | PASS (exit code 0) |
| [GhostenEditor/Ghosten-Player](https://github.com/GhostenEditor/Ghosten-Player) | `8c4fc325c2f8` | PASS (exit code 0) |
| [ReactiveX/rxdart](https://github.com/ReactiveX/rxdart) | `d1f3ba64f716` | PASS (exit code 0) |
| [kangpeiqin/bilivideo_down](https://github.com/kangpeiqin/bilivideo_down) | `15848a641229` | FAIL (exit code 0) |
| [Hyper-Beast/BiliTV](https://github.com/Hyper-Beast/BiliTV) | `01d324a717a7` | PASS (exit code 0) |
| [GimelStudio/GimelStudio](https://github.com/GimelStudio/GimelStudio) | `d2b27d6e0565` | PASS (exit code 0) |
| [memex-lab/memex](https://github.com/memex-lab/memex) | `38890fd68741` | PASS (exit code 0) |
| [simolus3/drift](https://github.com/simolus3/drift) | `ec871b612175` | PASS (exit code 0) |
| [iebb/mithka](https://github.com/iebb/mithka) | `8ef26bf3477d` | PASS (exit code 0) |
| [breeze-project-org/Breeze](https://github.com/breeze-project-org/Breeze) | `e08948f04c51` | PASS (exit code 0) |
| [open-mmlab/mmsegmentation](https://github.com/open-mmlab/mmsegmentation) | `b040e147adfa` | PASS (exit code 0) |
| [google-deepmind/sonnet](https://github.com/google-deepmind/sonnet) | `1c3b3e4dad22` | PASS (exit code 0) |
| [ruotianluo/self-critical.pytorch](https://github.com/ruotianluo/self-critical.pytorch) | `4c48a3304932` | PASS (exit code 0) |
| [huggingface/accelerate](https://github.com/huggingface/accelerate) | `bcc1fe6c8a53` | PASS (exit code 0) |
| [Ekultek/Zeus-Scanner](https://github.com/Ekultek/Zeus-Scanner) | `21b875630623` | PASS (exit code 0) |
| [awslabs/mcp](https://github.com/awslabs/mcp) | `a760620171c1` | PASS (exit code 0) |
| [baidu/DDParser](https://github.com/baidu/DDParser) | `144c09bd058c` | PASS (exit code 0) |
| [yt-dlp/yt-dlp](https://github.com/yt-dlp/yt-dlp) | `da452a569fb3` | PASS (exit code 0) |
| [anthropics/skills](https://github.com/anthropics/skills) | `e4bb5d960d87` | PASS (exit code 0) |
| [scofield7419/Video-of-Thought](https://github.com/scofield7419/Video-of-Thought) | `30646b91f3ff` | PASS (exit code 0) |
| [microsoft/markitdown](https://github.com/microsoft/markitdown) | `ed178952a667` | PASS (exit code 0) |
| [keras-team/keras-hub](https://github.com/keras-team/keras-hub) | `ccd21ed0bb8e` | PASS (exit code 0) |
| [boto/boto3](https://github.com/boto/boto3) | `52040c72bf13` | PASS (exit code 0) |
| [MichalGeyer/plug-and-play](https://github.com/MichalGeyer/plug-and-play) | `4e7f703df5f5` | PASS (exit code 0) |
| [paramiko/paramiko](https://github.com/paramiko/paramiko) | `0e9ccb91e0f9` | PASS (exit code 0) |
| [AUTOMATIC1111/stable-diffusion-webui](https://github.com/AUTOMATIC1111/stable-diffusion-webui) | `82a973c04367` | PASS (exit code 0) |
| [dflook/terraform-github-actions](https://github.com/dflook/terraform-github-actions) | `049f2bade585` | PASS (exit code 0) |
| [nextlevelbuilder/ui-ux-pro-max-skill](https://github.com/nextlevelbuilder/ui-ux-pro-max-skill) | `72e5fc76a4f3` | PASS (exit code 0) |
| [simpleai-team/simpleai](https://github.com/simpleai-team/simpleai) | `f231a4c08b4f` | PASS (exit code 0) |
| [Graphify-Labs/graphify](https://github.com/Graphify-Labs/graphify) | `0f22ae762e3e` | PASS (exit code 0) |
| [github/spec-kit](https://github.com/github/spec-kit) | `f2010547121b` | PASS (exit code 0) |
| [darknessomi/musicbox](https://github.com/darknessomi/musicbox) | `6c0debb3b931` | PASS (exit code 0) |
| [otto-torino/django-baton](https://github.com/otto-torino/django-baton) | `02062f42fe14` | PASS (exit code 0) |
| [langchain-ai/langchain](https://github.com/langchain-ai/langchain) | `30c4e38d71c2` | PASS (exit code 0) |
| [piglei/ai-vocabulary-builder](https://github.com/piglei/ai-vocabulary-builder) | `c7aa55b98801` | PASS (exit code 0) |
| [Comfy-Org/ComfyUI](https://github.com/Comfy-Org/ComfyUI) | `7c78b5641ab2` | PASS (exit code 0) |
| [pymc-devs/pymc](https://github.com/pymc-devs/pymc) | `92dbdb6cde6c` | PASS (exit code 0) |
| [ADLab-AutoDrive/BEVFusion](https://github.com/ADLab-AutoDrive/BEVFusion) | `fb4384ca05ca` | PASS (exit code 0) |
| [HKUDS/AutoAgent](https://github.com/HKUDS/AutoAgent) | `16c12b052ef2` | PASS (exit code 0) |
| [sehmaschine/django-filebrowser](https://github.com/sehmaschine/django-filebrowser) | `8efba882ff45` | PASS (exit code 0) |
| [ytdl-org/youtube-dl](https://github.com/ytdl-org/youtube-dl) | `bc069d2bb40b` | PASS (exit code 0) |
| [falconry/falcon](https://github.com/falconry/falcon) | `5fd985e8e468` | PASS (exit code 0) |
| [browser-use/browser-use](https://github.com/browser-use/browser-use) | `456ff150afb8` | PASS (exit code 0) |
| [tobymao/sqlglot](https://github.com/tobymao/sqlglot) | `ba0651085c69` | PASS (exit code 0) |
| [JesusFreke/lalboard](https://github.com/JesusFreke/lalboard) | `eddf521062c8` | PASS (exit code 0) |
| [concept-graphs/concept-graphs](https://github.com/concept-graphs/concept-graphs) | `93277a02bd89` | PASS (exit code 0) |
| [jiaaro/pydub](https://github.com/jiaaro/pydub) | `103e339d3bc7` | PASS (exit code 0) |
| [Panniantong/Agent-Reach](https://github.com/Panniantong/Agent-Reach) | `8d9ecc1f5521` | PASS (exit code 0) |
| [poljar/weechat-matrix](https://github.com/poljar/weechat-matrix) | `feae9fda26ea` | PASS (exit code 0) |
| [openai/whisper](https://github.com/openai/whisper) | `86098128c0b4` | PASS (exit code 0) |
| [harlanhong/CVPR2022-DaGAN](https://github.com/harlanhong/CVPR2022-DaGAN) | `052ad359a019` | PASS (exit code 0) |
| [D4Vinci/Scrapling](https://github.com/D4Vinci/Scrapling) | `938867232230` | PASS (exit code 0) |
| [Jakeschincariol/day-trader-skill](https://github.com/Jakeschincariol/day-trader-skill) | `3d0f4ef6986f` | PASS (exit code 0) |
| [fastapi/fastapi](https://github.com/fastapi/fastapi) | `05e6df7e906c` | PASS (exit code 0) |
| [meituan-longcat/LongCat-Video](https://github.com/meituan-longcat/LongCat-Video) | `58eb336edc5c` | PASS (exit code 0) |
| [gcovr/gcovr](https://github.com/gcovr/gcovr) | `b830bb081f51` | PASS (exit code 0) |
| [py2exe/py2exe](https://github.com/py2exe/py2exe) | `1be98bd71ac7` | PASS (exit code 0) |
| [dataabc/weiboSpider](https://github.com/dataabc/weiboSpider) | `720d52a58aef` | PASS (exit code 0) |
| [51bitquant/binance_grid_trader](https://github.com/51bitquant/binance_grid_trader) | `97a65982541b` | PASS (exit code 0) |
| [nvbn/thefuck](https://github.com/nvbn/thefuck) | `c7e7e1d884d3` | PASS (exit code 0) |
| [lovit/soynlp](https://github.com/lovit/soynlp) | `503eaee28799` | PASS (exit code 0) |
| [xonsh/xonsh](https://github.com/xonsh/xonsh) | `68f4a0c59f71` | PASS (exit code 0) |
| [3b1b/manim](https://github.com/3b1b/manim) | `fafa083a4fb2` | PASS (exit code 0) |
| [odysseus-dev/odysseus](https://github.com/odysseus-dev/odysseus) | `eb40e40ba424` | PASS (exit code 0) |
| [domlysz/BlenderGIS](https://github.com/domlysz/BlenderGIS) | `2add45ffec54` | PASS (exit code 0) |
| [VoltaML/voltaML-fast-stable-diffusion](https://github.com/VoltaML/voltaML-fast-stable-diffusion) | `1aea33bb7a37` | PASS (exit code 0) |
| [taehoonlee/tensornets](https://github.com/taehoonlee/tensornets) | `c9b1d78f8068` | PASS (exit code 0) |
| [aliyun/aliyun-oss-python-sdk](https://github.com/aliyun/aliyun-oss-python-sdk) | `eb329ec91d24` | PASS (exit code 0) |
| [unclecode/crawl4ai](https://github.com/unclecode/crawl4ai) | `4e4bab5d44a7` | PASS (exit code 0) |
| [666ghj/MiroFish](https://github.com/666ghj/MiroFish) | `7657031ac011` | PASS (exit code 0) |
| [kartograph/kartograph.py](https://github.com/kartograph/kartograph.py) | `815929e85e8b` | PASS (exit code 0) |
| [PeterL1n/RobustVideoMatting](https://github.com/PeterL1n/RobustVideoMatting) | `53d74c682673` | PASS (exit code 0) |
| [fportantier/habu](https://github.com/fportantier/habu) | `9868861bd728` | PASS (exit code 0) |
| [platformio/platformio-core](https://github.com/platformio/platformio-core) | `d8c21353ee2e` | PASS (exit code 0) |
| [superdesigndev/treg](https://github.com/superdesigndev/treg) | `2f7c0755c98b` | PASS (exit code 0) |
| [pwr-Solaar/Solaar](https://github.com/pwr-Solaar/Solaar) | `e7304c4c451c` | PASS (exit code 0) |
| [kennbroorg/iKy](https://github.com/kennbroorg/iKy) | `03ffb4ce99f0` | PASS (exit code 0) |
| [abi/screenshot-to-code](https://github.com/abi/screenshot-to-code) | `f2e5bbdd1dd5` | PASS (exit code 0) |
| [conan-io/conan](https://github.com/conan-io/conan) | `704b960a09ed` | PASS (exit code 0) |
| [runesleo/x-reader](https://github.com/runesleo/x-reader) | `92c7be64697b` | PASS (exit code 0) |
| [OpenBMB/VisRAG](https://github.com/OpenBMB/VisRAG) | `56f9785bf838` | PASS (exit code 0) |
| [autonomousvision/stylegan-xl](https://github.com/autonomousvision/stylegan-xl) | `4241ff9cfeb6` | PASS (exit code 0) |
| [usestrix/strix](https://github.com/usestrix/strix) | `9ed84f99c8f1` | PASS (exit code 0) |
| [RecursiveMAS/RecursiveMAS](https://github.com/RecursiveMAS/RecursiveMAS) | `cbfcaab56c9a` | PASS (exit code 0) |
| [saulpw/visidata](https://github.com/saulpw/visidata) | `68d8ac313f5f` | PASS (exit code 0) |
| [hiyouga/LlamaFactory](https://github.com/hiyouga/LlamaFactory) | `aef9c06ff4e2` | PASS (exit code 0) |
| [tensorflow/transform](https://github.com/tensorflow/transform) | `deefef17a969` | PASS (exit code 0) |
| [ScottSloan/Bili23-Downloader](https://github.com/ScottSloan/Bili23-Downloader) | `50594cd1f8ef` | PASS (exit code 0) |
| [oumi-ai/oumi](https://github.com/oumi-ai/oumi) | `c0bb5eb6ea3b` | PASS (exit code 0) |
| [cv-cat/XHS_ALL_IN_ONE](https://github.com/cv-cat/XHS_ALL_IN_ONE) | `e86f82bbf07e` | PASS (exit code 0) |
| [Alishahryar1/free-claude-code](https://github.com/Alishahryar1/free-claude-code) | `919a94f97607` | PASS (exit code 0) |
| [asgeirtj/system_prompts_leaks](https://github.com/asgeirtj/system_prompts_leaks) | `c607f91ed3e6` | PASS (exit code 0) |
| [angr/angr](https://github.com/angr/angr) | `48d4130c44d2` | PASS (exit code 0) |
| [binary-husky/gpt_academic](https://github.com/binary-husky/gpt_academic) | `d6bde0fa5437` | PASS (exit code 0) |
| [spyder-ide/spyder](https://github.com/spyder-ide/spyder) | `a1dd1ccbc85d` | PASS (exit code 0) |
| [ansible/ansible](https://github.com/ansible/ansible) | `cd313f0a8650` | PASS (exit code 0) |
| [tox-dev/filelock](https://github.com/tox-dev/filelock) | `be45d684b9ea` | PASS (exit code 0) |
| [FoundationAgents/MetaGPT](https://github.com/FoundationAgents/MetaGPT) | `11cdf466d042` | PASS (exit code 0) |
| [mem0ai/mem0](https://github.com/mem0ai/mem0) | `a330119bda24` | PASS (exit code 0) |
| [HypothesisWorks/hypothesis](https://github.com/HypothesisWorks/hypothesis) | `16dc3c921318` | PASS (exit code 0) |
| [Kuaishou-OneRec/OpenOneRec](https://github.com/Kuaishou-OneRec/OpenOneRec) | `a969edcadd57` | PASS (exit code 0) |
| [scikit-hep/awkward](https://github.com/scikit-hep/awkward) | `2b1e4ec35d33` | PASS (exit code 0) |
| [MemPalace/mempalace](https://github.com/MemPalace/mempalace) | `87ae5024d8ee` | PASS (exit code 0) |
| [kivy/python-for-android](https://github.com/kivy/python-for-android) | `9562d1b4f306` | PASS (exit code 0) |
| [exo-explore/exo](https://github.com/exo-explore/exo) | `a60b95698ca8` | PASS (exit code 0) |
| [ashawkey/stable-dreamfusion](https://github.com/ashawkey/stable-dreamfusion) | `5550b91862a3` | PASS (exit code 0) |
| [fal-ai/fal](https://github.com/fal-ai/fal) | `a2eea8d23a71` | PASS (exit code 0) |
| [wshobson/agents](https://github.com/wshobson/agents) | `c96b5f6552db` | PASS (exit code 0) |
| [apple-aiml-research/ml-ferret](https://github.com/apple-aiml-research/ml-ferret) | `25985c2c4f24` | PASS (exit code 0) |
| [LAION-AI/Open-Assistant](https://github.com/LAION-AI/Open-Assistant) | `f1e6ed9526f5` | PASS (exit code 0) |
| [miniflux/v2](https://github.com/miniflux/v2) | `f3b0e65c904b` | PASS (exit code 0) |
| [NyaaPantsu/nyaa](https://github.com/NyaaPantsu/nyaa) | `ee808dc7a18e` | PASS (exit code 0) |
| [ollama/ollama](https://github.com/ollama/ollama) | `8ae8acb81afb` | PASS (exit code 0) |
| [anchore/syft](https://github.com/anchore/syft) | `bbb56b7d5a55` | PASS (exit code 0) |
| [michaelsauter/crane](https://github.com/michaelsauter/crane) | `3b3319fea8f0` | PASS (exit code 0) |
| [lib/pq](https://github.com/lib/pq) | `5e0c05ba8427` | PASS (exit code 0) |
| [piotrnar/gocoin](https://github.com/piotrnar/gocoin) | `ca8789c57a30` | PASS (exit code 0) |
| [ThreeDotsLabs/watermill](https://github.com/ThreeDotsLabs/watermill) | `462225d7ef4f` | PASS (exit code 0) |
| [JuliusBrussee/caveman](https://github.com/JuliusBrussee/caveman) | `e74a11ccdb38` | PASS (exit code 0) |
| [gruntwork-io/terragrunt](https://github.com/gruntwork-io/terragrunt) | `9f78f47697cf` | PASS (exit code 0) |
| [faceair/clash-speedtest](https://github.com/faceair/clash-speedtest) | `1e1fb9e2bbb2` | PASS (exit code 0) |
| [google/go-cloud](https://github.com/google/go-cloud) | `24606b8ed40f` | PASS (exit code 0) |
| [safebucket/safebucket](https://github.com/safebucket/safebucket) | `02eff0b04f5a` | PASS (exit code 0) |
| [fatedier/frp](https://github.com/fatedier/frp) | `a2c975efb5e5` | PASS (exit code 0) |
| [gorse-io/gorse](https://github.com/gorse-io/gorse) | `ef05eb6f851e` | PASS (exit code 0) |
| [samber/oops](https://github.com/samber/oops) | `71de68243685` | PASS (exit code 0) |
| [syncthing/syncthing](https://github.com/syncthing/syncthing) | `1107b3cd11db` | PASS (exit code 0) |
| [etcd-io/bbolt](https://github.com/etcd-io/bbolt) | `710ad6990eba` | PASS (exit code 0) |
| [aarzilli/nucular](https://github.com/aarzilli/nucular) | `37de78906f86` | PASS (exit code 0) |
| [gohugoio/hugo](https://github.com/gohugoio/hugo) | `b3d0a6df018f` | PASS (exit code 0) |
| [SpecterOps/AzureHound](https://github.com/SpecterOps/AzureHound) | `ce5b989ffe6c` | PASS (exit code 0) |
| [gin-gonic/gin](https://github.com/gin-gonic/gin) | `0f09c3a9b462` | PASS (exit code 0) |
| [gomodule/redigo](https://github.com/gomodule/redigo) | `57c8b99ccb90` | PASS (exit code 0) |
| [junegunn/fzf](https://github.com/junegunn/fzf) | `ed173e71e44b` | PASS (exit code 0) |
| [tmc/langchaingo](https://github.com/tmc/langchaingo) | `3308821182c0` | PASS (exit code 0) |
| [reviewdog/reviewdog](https://github.com/reviewdog/reviewdog) | `056fdfbf68ce` | PASS (exit code 0) |
| [element-hq/dendrite](https://github.com/element-hq/dendrite) | `08cac1ccf0a4` | PASS (exit code 0) |
| [netdata/netdata](https://github.com/netdata/netdata) | `7d96f6573502` | PASS (exit code 0) |
| [tidwall/tile38](https://github.com/tidwall/tile38) | `3b354aee0785` | PASS (exit code 0) |
| [caddyserver/caddy](https://github.com/caddyserver/caddy) | `679806f92d49` | PASS (exit code 0) |
| [cloudnative-pg/cloudnative-pg](https://github.com/cloudnative-pg/cloudnative-pg) | `62303c5680b8` | PASS (exit code 0) |
| [failsafe-security/artex](https://github.com/failsafe-security/artex) | `b054a35f6a01` | PASS (exit code 0) |
| [nektos/act](https://github.com/nektos/act) | `ac4c09993f47` | PASS (exit code 0) |
| [real-web-world/hh-lol-prophet](https://github.com/real-web-world/hh-lol-prophet) | `e81d7b8eb974` | PASS (exit code 0) |
| [prometheus/prometheus](https://github.com/prometheus/prometheus) | `1e794b83ad0d` | PASS (exit code 0) |
| [gokcehan/lf](https://github.com/gokcehan/lf) | `8b8780299a83` | PASS (exit code 0) |
| [gotomicro/ego](https://github.com/gotomicro/ego) | `5eb2a4041b0d` | PASS (exit code 0) |
| [traefik/traefik](https://github.com/traefik/traefik) | `8fcaa7cda47c` | PASS (exit code 0) |
| [goksan/statusnook](https://github.com/goksan/statusnook) | `a24ddc0bfafe` | PASS (exit code 0) |
| [usememos/memos](https://github.com/usememos/memos) | `bf6cdfcf0638` | PASS (exit code 0) |
| [flannel-io/flannel](https://github.com/flannel-io/flannel) | `87f7078166ef` | PASS (exit code 0) |
| [v2fly/geoip](https://github.com/v2fly/geoip) | `dcb3001639b4` | PASS (exit code 0) |
| [rclone/rclone](https://github.com/rclone/rclone) | `cd9c861693c8` | PASS (exit code 0) |
| [sosedoff/pgweb](https://github.com/sosedoff/pgweb) | `e4858a16d8e0` | PASS (exit code 0) |
| [qifengzhang007/GinSkeleton](https://github.com/qifengzhang007/GinSkeleton) | `a0c2d094657e` | PASS (exit code 0) |
| [router-for-me/CLIProxyAPI](https://github.com/router-for-me/CLIProxyAPI) | `47f18e4d82bd` | PASS (exit code 0) |
| [go-gitea/gitea](https://github.com/go-gitea/gitea) | `3df804d7b8f1` | PASS (exit code 0) |
| [hybridgroup/gobot](https://github.com/hybridgroup/gobot) | `798be8ce070d` | PASS (exit code 0) |
| [multica-ai/multica](https://github.com/multica-ai/multica) | `392ca3d8463b` | PASS (exit code 0) |
| [alibaba/open-code-review](https://github.com/alibaba/open-code-review) | `d4f568f71d3a` | PASS (exit code 0) |
| [goplus/xgo](https://github.com/goplus/xgo) | `c70869bd8cc9` | FAIL (exit code 0) |
| [netxfly/sec-dev-in-action-src](https://github.com/netxfly/sec-dev-in-action-src) | `332d60421f58` | PASS (exit code 0) |
| [wagoodman/dive](https://github.com/wagoodman/dive) | `75dea9077a4f` | PASS (exit code 0) |
| [certimate-go/certimate](https://github.com/certimate-go/certimate) | `4734ac1d1d97` | PASS (exit code 0) |
| [etcd-io/etcd](https://github.com/etcd-io/etcd) | `4e090c033abc` | PASS (exit code 0) |
| [QuantumNous/new-api](https://github.com/QuantumNous/new-api) | `81c1fd20efad` | PASS (exit code 0) |
| [ethereum/go-ethereum](https://github.com/ethereum/go-ethereum) | `bbafd66b66f5` | PASS (exit code 0) |
| [kubernetes-sigs/kubebuilder](https://github.com/kubernetes-sigs/kubebuilder) | `eadb62de0521` | PASS (exit code 0) |
| [prometheus/statsd_exporter](https://github.com/prometheus/statsd_exporter) | `9410ef850538` | PASS (exit code 0) |
| [MHSanaei/3x-ui](https://github.com/MHSanaei/3x-ui) | `22ffae302fcb` | PASS (exit code 0) |
| [runatlantis/atlantis](https://github.com/runatlantis/atlantis) | `d04d61cd377d` | PASS (exit code 0) |
| [mudler/LocalRecall](https://github.com/mudler/LocalRecall) | `09926f78c8cf` | PASS (exit code 0) |
| [bitnami/sealed-secrets](https://github.com/bitnami/sealed-secrets) | `53625dfe02d8` | PASS (exit code 0) |
| [AlistGo/alist](https://github.com/AlistGo/alist) | `92e745360766` | PASS (exit code 0) |
| [google/pprof](https://github.com/google/pprof) | `084b33a85f63` | PASS (exit code 0) |
| [sijms/go-ora](https://github.com/sijms/go-ora) | `62f8a42fe821` | PASS (exit code 0) |
| [hashicorp/terraform](https://github.com/hashicorp/terraform) | `b07e3171bff4` | PASS (exit code 0) |
| [gtsteffaniak/filebrowser](https://github.com/gtsteffaniak/filebrowser) | `1672af6a6ffc` | PASS (exit code 0) |
| [bqlpfy/flux-panel](https://github.com/bqlpfy/flux-panel) | `9b5ccdb1f428` | PASS (exit code 0) |
| [gogs/gogs](https://github.com/gogs/gogs) | `43bf2190ee54` | PASS (exit code 0) |
| [quexten/goldwarden](https://github.com/quexten/goldwarden) | `7727d3ac291d` | PASS (exit code 0) |
| [v2ray/v2ray-core](https://github.com/v2ray/v2ray-core) | `d80440f3d57b` | PASS (exit code 0) |
| [mark3labs/mcp-go](https://github.com/mark3labs/mcp-go) | `bab30b2377e2` | PASS (exit code 0) |
| [twpayne/go-geom](https://github.com/twpayne/go-geom) | `c07c342b4159` | PASS (exit code 0) |
| [mvdan/sh](https://github.com/mvdan/sh) | `9780540a62e1` | PASS (exit code 0) |
| [charmbracelet/bubbletea](https://github.com/charmbracelet/bubbletea) | `f510168f1895` | PASS (exit code 0) |
| [spf13/cobra](https://github.com/spf13/cobra) | `adbc8813901b` | PASS (exit code 0) |
| [hashicorp/raft](https://github.com/hashicorp/raft) | `dd4346dbb127` | PASS (exit code 0) |
| [rcrowley/go-tigertonic](https://github.com/rcrowley/go-tigertonic) | `a26c5e8568b0` | PASS (exit code 0) |
| [danielmiessler/Fabric](https://github.com/danielmiessler/Fabric) | `d3ee2c3bfbc6` | PASS (exit code 0) |
| [kubernetes-sigs/external-dns](https://github.com/kubernetes-sigs/external-dns) | `aa9e2d9a85a2` | PASS (exit code 0) |
| [redcode-labs/Coldfire](https://github.com/redcode-labs/Coldfire) | `72e0ebf8537d` | PASS (exit code 0) |
| [yliu7949/KouShare-dl](https://github.com/yliu7949/KouShare-dl) | `05d9ce133df5` | PASS (exit code 0) |
| [smallstep/certificates](https://github.com/smallstep/certificates) | `41fe0e9aecd2` | PASS (exit code 0) |
| [schollz/croc](https://github.com/schollz/croc) | `91f07e7cecf0` | PASS (exit code 0) |
| [dinedal/textql](https://github.com/dinedal/textql) | `e6545d501ca9` | FAIL (exit code 0) |
| [onsi/ginkgo](https://github.com/onsi/ginkgo) | `7035669bb556` | PASS (exit code 0) |
| [itzg/mc-router](https://github.com/itzg/mc-router) | `d1930b47a03d` | PASS (exit code 0) |
| [charmbracelet/bubbles](https://github.com/charmbracelet/bubbles) | `0c1a4915f403` | PASS (exit code 0) |
| [vishen/go-chromecast](https://github.com/vishen/go-chromecast) | `17b10a3630ac` | PASS (exit code 0) |
| [teamhanko/hanko](https://github.com/teamhanko/hanko) | `686a57519873` | PASS (exit code 0) |
| [zombiezen/go-sqlite](https://github.com/zombiezen/go-sqlite) | `ca2041ed3dd2` | PASS (exit code 0) |
| [gofiber/fiber](https://github.com/gofiber/fiber) | `0d4d872a29e8` | PASS (exit code 0) |
| [evilmartians/lefthook](https://github.com/evilmartians/lefthook) | `179bc8c85f53` | PASS (exit code 0) |
| [go-gorm/gorm](https://github.com/go-gorm/gorm) | `ead4e04ba7c6` | PASS (exit code 0) |
| [google/badwolf](https://github.com/google/badwolf) | `6cde56dbc7db` | PASS (exit code 0) |
| [securego/gosec](https://github.com/securego/gosec) | `244f1f6f4d1d` | PASS (exit code 0) |
| [HoudiniGraphql/houdini](https://github.com/HoudiniGraphql/houdini) | `ed025d020fa4` | PASS (exit code 0) |
| [GoAdminGroup/go-admin](https://github.com/GoAdminGroup/go-admin) | `c47763c7bb63` | PASS (exit code 0) |
| [docker/compose](https://github.com/docker/compose) | `e68a65737b2f` | PASS (exit code 0) |
| [pgmq/pgmq](https://github.com/pgmq/pgmq) | `51468f6ba2af` | PASS (exit code 0) |
| [oracle-samples/db-sample-schemas](https://github.com/oracle-samples/db-sample-schemas) | `6660bad68c07` | PASS (exit code 0) |
| [BrentOzarULTD/SQL-Server-First-Responder-Kit](https://github.com/BrentOzarULTD/SQL-Server-First-Responder-Kit) | `5d3a6811a44e` | PASS (exit code 0) |
| [olahallengren/sql-server-maintenance-solution](https://github.com/olahallengren/sql-server-maintenance-solution) | `41617578643e` | PASS (exit code 0) |
| [smpetersgithub/AdvancedSQLPuzzles](https://github.com/smpetersgithub/AdvancedSQLPuzzles) | `4fe0cac6e23f` | PASS (exit code 0) |
| [chinnonsantos/sql-paises-estados-cidades](https://github.com/chinnonsantos/sql-paises-estados-cidades) | `a705674ebf1e` | PASS (exit code 0) |
| [openmaptiles/openmaptiles](https://github.com/openmaptiles/openmaptiles) | `6c11838d3803` | PASS (exit code 0) |
| [pgpartman/pg_partman](https://github.com/pgpartman/pg_partman) | `e0a6c384a6c7` | PASS (exit code 0) |
| [lerocha/chinook-database](https://github.com/lerocha/chinook-database) | `7f67772503d7` | PASS (exit code 0) |
| [rin-nas/postgresql-patterns-library](https://github.com/rin-nas/postgresql-patterns-library) | `657ba2af7b08` | PASS (exit code 0) |
| [postgis/postgis](https://github.com/postgis/postgis) | `75af069142a6` | PASS (exit code 0) |
| [darold/pgFormatter](https://github.com/darold/pgFormatter) | `3aab449352e0` | PASS (exit code 0) |
| [NikolayS/PgQue](https://github.com/NikolayS/PgQue) | `c75cd8da5005` | PASS (exit code 0) |
| [tanelpoder/tpt-oracle](https://github.com/tanelpoder/tpt-oracle) | `45f0a4e15502` | PASS (exit code 0) |
| [erikdarlingdata/DarlingData](https://github.com/erikdarlingdata/DarlingData) | `bf5a0666c2b5` | PASS (exit code 0) |
| [NixOS/hydra](https://github.com/NixOS/hydra) | `e181b587af32` | PASS (exit code 0) |
| [amachanic/sp_whoisactive](https://github.com/amachanic/sp_whoisactive) | `40ac17b8e61a` | PASS (exit code 0) |
| [microsoft/fhir-server](https://github.com/microsoft/fhir-server) | `12673f5b31ab` | PASS (exit code 0) |
| [cahyadsn/wilayah](https://github.com/cahyadsn/wilayah) | `c2eff0881e3c` | PASS (exit code 0) |
| [utPLSQL/utPLSQL](https://github.com/utPLSQL/utPLSQL) | `01a89eae1b35` | PASS (exit code 0) |
| [NikolayS/postgres_dba](https://github.com/NikolayS/postgres_dba) | `d5c63fd11784` | PASS (exit code 0) |
| [trickerer/Trinity-Bots](https://github.com/trickerer/Trinity-Bots) | `a2a13b24d47b` | PASS (exit code 0) |
| [theory/pgtap](https://github.com/theory/pgtap) | `21ceed193bf3` | PASS (exit code 0) |
| [starkfell/100DaysOfIaC](https://github.com/starkfell/100DaysOfIaC) | `3b986fca6d53` | PASS (exit code 0) |
| [aquametalabs/aquameta](https://github.com/aquametalabs/aquameta) | `ec5dbe6b84ea` | PASS (exit code 0) |
| [jOOQ/sakila](https://github.com/jOOQ/sakila) | `e089a5b1ec9a` | PASS (exit code 0) |
| [microsoft/Windows-appsample-customers-orders-database](https://github.com/microsoft/Windows-appsample-customers-orders-database) | `247226c93728` | PASS (exit code 0) |
| [dbkangaroo/kangaroo](https://github.com/dbkangaroo/kangaroo) | `79018b4200ff` | PASS (exit code 0) |
| [mqttsnet/thinglinks](https://github.com/mqttsnet/thinglinks) | `3705ee1564fa` | PASS (exit code 0) |
| [marcingminski/sqlwatch](https://github.com/marcingminski/sqlwatch) | `7273c6cba78c` | PASS (exit code 0) |
| [lorint/AdventureWorks-for-Postgres](https://github.com/lorint/AdventureWorks-for-Postgres) | `b474991f0df1` | PASS (exit code 0) |
| [3dcitydb/3dcitydb](https://github.com/3dcitydb/3dcitydb) | `c2c3ab6609a9` | PASS (exit code 0) |
| [geocompass/pg-coordtransform](https://github.com/geocompass/pg-coordtransform) | `e5cf4ab0aea0` | PASS (exit code 0) |
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
| [czwbig/Tmall_JavaEE](https://github.com/czwbig/Tmall_JavaEE) | `0a96527e8402` | PASS (exit code 0) |
| [Hameds/IranCountryDivisions](https://github.com/Hameds/IranCountryDivisions) | `37c01f5e776b` | PASS (exit code 0) |
| [powa-team/pg_qualstats](https://github.com/powa-team/pg_qualstats) | `ef98c7ec0613` | PASS (exit code 0) |
| [MontageD/nuxt-maopingshou](https://github.com/MontageD/nuxt-maopingshou) | `fe32517dc24c` | PASS (exit code 0) |
| [babelfish-for-postgresql/babelfish_extensions](https://github.com/babelfish-for-postgresql/babelfish_extensions) | `2a354b28e440` | PASS (exit code 0) |
| [xocolatl/periods](https://github.com/xocolatl/periods) | `b7bc4eac98d5` | PASS (exit code 0) |
| [DavidSchanzer/Sql-Server-DBA-Toolbox](https://github.com/DavidSchanzer/Sql-Server-DBA-Toolbox) | `64ff78e00dd8` | PASS (exit code 0) |
| [oraclebase/dba](https://github.com/oraclebase/dba) | `a3115408f679` | PASS (exit code 0) |
| [Dogkann/sql-calisma-ornekleri](https://github.com/Dogkann/sql-calisma-ornekleri) | `868734654821` | PASS (exit code 0) |
| [pgflow-dev/pgflow](https://github.com/pgflow-dev/pgflow) | `10b15f1641d1` | PASS (exit code 0) |
| [microsoft/Azure_Synapse_Toolbox](https://github.com/microsoft/Azure_Synapse_Toolbox) | `af39147d2449` | PASS (exit code 0) |
| [supabase/splinter](https://github.com/supabase/splinter) | `cc529aa9d3ee` | PASS (exit code 0) |
| [OHDSI/Vocabulary-v5.0](https://github.com/OHDSI/Vocabulary-v5.0) | `fd0ca46c1b41` | PASS (exit code 0) |
| [hasura/graphql-bench](https://github.com/hasura/graphql-bench) | `679efdc49e98` | PASS (exit code 0) |
| [pgspider/sqlite_fdw](https://github.com/pgspider/sqlite_fdw) | `ea52a24f63dd` | PASS (exit code 0) |
| [drobcosta/digital_innovation_one](https://github.com/drobcosta/digital_innovation_one) | `4692c08e41b6` | PASS (exit code 0) |
| [vibhorkum/pg_background](https://github.com/vibhorkum/pg_background) | `989b0b453a7c` | PASS (exit code 0) |
| [crtsh/certwatch_db](https://github.com/crtsh/certwatch_db) | `f4a6bc75510d` | PASS (exit code 0) |
| [NikolayS/pg_ash](https://github.com/NikolayS/pg_ash) | `fa121471316f` | PASS (exit code 0) |
| [ether-camp/ethereum-harmony](https://github.com/ether-camp/ethereum-harmony) | `3b9e8186f79d` | PASS (exit code 0) |
| [supabase-community/supabase-graphql-example](https://github.com/supabase-community/supabase-graphql-example) | `504560d3124a` | PASS (exit code 0) |
| [wuchong/flink-sql-demo](https://github.com/wuchong/flink-sql-demo) | `208f8902a10a` | PASS (exit code 0) |
| [TexanInParis/Effective-SQL](https://github.com/TexanInParis/Effective-SQL) | `8047973838c2` | PASS (exit code 0) |
| [pgspider/parquet_s3_fdw](https://github.com/pgspider/parquet_s3_fdw) | `cf4384dba7d1` | PASS (exit code 0) |
| [sivers/sivers](https://github.com/sivers/sivers) | `f7c6d2e0ec7a` | PASS (exit code 0) |
| [jeesun/oauthserver](https://github.com/jeesun/oauthserver) | `c4643ec3569f` | PASS (exit code 0) |
| [rrg92/sqlserver-lib](https://github.com/rrg92/sqlserver-lib) | `926096d87987` | PASS (exit code 0) |
| [cseagle/fREedom](https://github.com/cseagle/fREedom) | `49dda0d45799` | PASS (exit code 0) |
| [stac-utils/pgstac](https://github.com/stac-utils/pgstac) | `a2a214b64c82` | PASS (exit code 0) |
| [yudaocode/yudao-boot-mini](https://github.com/yudaocode/yudao-boot-mini) | `f03151e85edf` | PASS (exit code 0) |
| [orbisgis/h2gis](https://github.com/orbisgis/h2gis) | `c11ef1db28d7` | PASS (exit code 0) |
| [journeyapps/node-sqlcipher](https://github.com/journeyapps/node-sqlcipher) | `5240d730c9c9` | PASS (exit code 0) |
| [czwbig/Tmall_SSM](https://github.com/czwbig/Tmall_SSM) | `21e5b60451c4` | PASS (exit code 0) |
| [deajan/osync](https://github.com/deajan/osync) | `c3f77008b1ac` | PASS (exit code 0) |
| [ohmyzsh/ohmyzsh](https://github.com/ohmyzsh/ohmyzsh) | `abfce729ab56` | PASS (exit code 0) |
| [msitarzewski/agency-agents](https://github.com/msitarzewski/agency-agents) | `8ae6164602c9` | PASS (exit code 0) |
| [telekom-security/tpotce](https://github.com/telekom-security/tpotce) | `9c3ae4aea976` | PASS (exit code 0) |
| [nvm-sh/nvm](https://github.com/nvm-sh/nvm) | `3bf66860904e` | PASS (exit code 0) |
| [tw93/Mole](https://github.com/tw93/Mole) | `eaa95f99148e` | PASS (exit code 0) |
| [frankbria/ralph-claude-code](https://github.com/frankbria/ralph-claude-code) | `a487bc6136a3` | PASS (exit code 0) |
| [pi-hole/pi-hole](https://github.com/pi-hole/pi-hole) | `f7c0220e63e7` | PASS (exit code 0) |
| [getsentry/self-hosted](https://github.com/getsentry/self-hosted) | `835245951d60` | PASS (exit code 0) |
| [romkatv/powerlevel10k](https://github.com/romkatv/powerlevel10k) | `50a35cc8d9f1` | PASS (exit code 0) |
| [dockur/windows](https://github.com/dockur/windows) | `013ea27498e0` | PASS (exit code 0) |
| [Danilop95/Proxmox-Enhanced-Configuration-Utility](https://github.com/Danilop95/Proxmox-Enhanced-Configuration-Utility) | `d00fd0d1abae` | PASS (exit code 0) |
| [ublue-os/bazzite](https://github.com/ublue-os/bazzite) | `b42cdec64f91` | PASS (exit code 0) |
| [acmesh-official/acme.sh](https://github.com/acmesh-official/acme.sh) | `5567d7fe7298` | PASS (exit code 0) |
| [omacom/omarchy](https://github.com/omacom/omarchy) | `a11b6a601579` | PASS (exit code 0) |
| [testssl/testssl.sh](https://github.com/testssl/testssl.sh) | `f14f6610b86f` | PASS (exit code 0) |
| [kunchenguid/firstmate](https://github.com/kunchenguid/firstmate) | `b119e42294ce` | PASS (exit code 0) |
| [RROrg/rr](https://github.com/RROrg/rr) | `ccf322ff697f` | PASS (exit code 0) |
| [community-scripts/ProxmoxVE](https://github.com/community-scripts/ProxmoxVE) | `2a0ec4415f89` | PASS (exit code 0) |
| [conversun/fnos-apps](https://github.com/conversun/fnos-apps) | `071eaaee2f30` | PASS (exit code 0) |
| [233boy/v2ray](https://github.com/233boy/v2ray) | `707ecf7601ff` | PASS (exit code 0) |
| [hwdsl2/setup-ipsec-vpn](https://github.com/hwdsl2/setup-ipsec-vpn) | `3a5a4548b027` | PASS (exit code 0) |
| [xiaoyaDev/xiaoya-alist](https://github.com/xiaoyaDev/xiaoya-alist) | `dae54c1b2bcd` | PASS (exit code 0) |
| [OthmanAdi/planning-with-files](https://github.com/OthmanAdi/planning-with-files) | `189fd09fecb2` | PASS (exit code 0) |
| [mbadolato/iTerm2-Color-Schemes](https://github.com/mbadolato/iTerm2-Color-Schemes) | `39d0d4d40d14` | PASS (exit code 0) |
| [prasanthrangan/hyprdots](https://github.com/prasanthrangan/hyprdots) | `3c8b0dfb5e7f` | PASS (exit code 0) |
| [yuaotian/go-cursor-help](https://github.com/yuaotian/go-cursor-help) | `2d00ebc01eb9` | PASS (exit code 0) |
| [HariSekhon/DevOps-Bash-tools](https://github.com/HariSekhon/DevOps-Bash-tools) | `0ab20dad6852` | PASS (exit code 0) |
| [six2dez/reconftw](https://github.com/six2dez/reconftw) | `39252113a854` | PASS (exit code 0) |
| [zsh-users/zsh-syntax-highlighting](https://github.com/zsh-users/zsh-syntax-highlighting) | `e6683f20e5c3` | PASS (exit code 0) |
| [mack-a/v2ray-agent](https://github.com/mack-a/v2ray-agent) | `b5d76b921f82` | PASS (exit code 0) |
| [teddysun/shadowsocks_install](https://github.com/teddysun/shadowsocks_install) | `333637c337f3` | PASS (exit code 0) |
| [huifer/WellAlly-health](https://github.com/huifer/WellAlly-health) | `f604350c8448` | PASS (exit code 0) |
| [v1s1t0r1sh3r3/airgeddon](https://github.com/v1s1t0r1sh3r3/airgeddon) | `40c6beb3082a` | PASS (exit code 0) |
| [spaceship-prompt/spaceship-prompt](https://github.com/spaceship-prompt/spaceship-prompt) | `ef7f71da6307` | PASS (exit code 0) |
| [grahampugh/erase-install](https://github.com/grahampugh/erase-install) | `f5e2da3ba457` | PASS (exit code 0) |
| [pivpn/pivpn](https://github.com/pivpn/pivpn) | `aa96de7d27bf` | PASS (exit code 0) |
| [docker-mailserver/docker-mailserver](https://github.com/docker-mailserver/docker-mailserver) | `4f61137c1db0` | PASS (exit code 0) |
| [hex/claude-council](https://github.com/hex/claude-council) | `f2f7f42122b2` | PASS (exit code 0) |
| [linux-surface/linux-surface](https://github.com/linux-surface/linux-surface) | `3c4b467134a9` | PASS (exit code 0) |
| [kossakovsky/selfhost-ai](https://github.com/kossakovsky/selfhost-ai) | `c586bc70c841` | PASS (exit code 0) |
| [SuperManito/LinuxMirrors](https://github.com/SuperManito/LinuxMirrors) | `7f8989fd2373` | PASS (exit code 0) |
| [eylenburg/linoffice](https://github.com/eylenburg/linoffice) | `407cf61ef572` | PASS (exit code 0) |
| [termux/termux-packages](https://github.com/termux/termux-packages) | `0e3fd66f6eed` | PASS (exit code 0) |
| [attogram/bash-screensavers](https://github.com/attogram/bash-screensavers) | `b6bb74a24352` | PASS (exit code 0) |
| [ohmybash/oh-my-bash](https://github.com/ohmybash/oh-my-bash) | `94c2977eda06` | PASS (exit code 0) |
| [z-shell/zi](https://github.com/z-shell/zi) | `5d095f60bb45` | PASS (exit code 0) |
| [woniuzfb/aios](https://github.com/woniuzfb/aios) | `70b95081e851` | PASS (exit code 0) |
| [zplug/zplug](https://github.com/zplug/zplug) | `cc6906ea7ea1` | PASS (exit code 0) |
| [mishakorzik/AllHackingTools](https://github.com/mishakorzik/AllHackingTools) | `def693c66c49` | PASS (exit code 0) |
| [1N3/Sn1per](https://github.com/1N3/Sn1per) | `0e1cbf415566` | PASS (exit code 0) |
| [sorin-ionescu/prezto](https://github.com/sorin-ionescu/prezto) | `a7c61670c349` | PASS (exit code 0) |
| [kaldi-asr/kaldi](https://github.com/kaldi-asr/kaldi) | `e02e35f0254b` | PASS (exit code 0) |
| [Bash-it/bash-it](https://github.com/Bash-it/bash-it) | `0191fca5df3e` | PASS (exit code 0) |
| [shibco/ableton-linux](https://github.com/shibco/ableton-linux) | `1b1f596d17bb` | PASS (exit code 0) |
| [bin456789/reinstall](https://github.com/bin456789/reinstall) | `d9ede3db41dd` | PASS (exit code 0) |
| [juewuy/ShellCrash](https://github.com/juewuy/ShellCrash) | `34b768b5a5df` | PASS (exit code 0) |
| [google/oss-fuzz](https://github.com/google/oss-fuzz) | `3a14bf2b9776` | PASS (exit code 0) |
| [MariaDB/mariadb-docker](https://github.com/MariaDB/mariadb-docker) | `0d99c3d3d774` | PASS (exit code 0) |
| [Dicklesworthstone/claude_code_agent_farm](https://github.com/Dicklesworthstone/claude_code_agent_farm) | `d88ba78ea5e5` | PASS (exit code 0) |
| [wnlen/clash-for-linux](https://github.com/wnlen/clash-for-linux) | `42c26d8ad443` | PASS (exit code 0) |
| [RetroPie/RetroPie-Setup](https://github.com/RetroPie/RetroPie-Setup) | `427e76216633` | PASS (exit code 0) |
| [Gogh-Co/Gogh](https://github.com/Gogh-Co/Gogh) | `fd715c810bb0` | PASS (exit code 0) |
| [armbian/build](https://github.com/armbian/build) | `591920430bab` | PASS (exit code 0) |
| [ChrisTitusTech/linutil](https://github.com/ChrisTitusTech/linutil) | `c20db8be2c11` | PASS (exit code 0) |
| [Eric86777/vps-tcp-tune](https://github.com/Eric86777/vps-tcp-tune) | `6b5c9ae67420` | PASS (exit code 0) |
| [Aegrah/PANIX](https://github.com/Aegrah/PANIX) | `98e2e502e08b` | PASS (exit code 0) |
| [PeonPing/peon-ping](https://github.com/PeonPing/peon-ping) | `c9e8dadaede5` | PASS (exit code 0) |
| [akinomyoga/ble.sh](https://github.com/akinomyoga/ble.sh) | `152942758f56` | PASS (exit code 0) |
| [GameServerManagers/LinuxGSM](https://github.com/GameServerManagers/LinuxGSM) | `98be8a980846` | PASS (exit code 0) |
| [ToyoDAdoubi/doubi](https://github.com/ToyoDAdoubi/doubi) | `28184d72b855` | PASS (exit code 0) |
| [hestiacp/hestiacp](https://github.com/hestiacp/hestiacp) | `f1ae6b717d5f` | PASS (exit code 0) |
| [AryanVBW/LinuxDroid](https://github.com/AryanVBW/LinuxDroid) | `f7a10cfd69fc` | PASS (exit code 0) |
| [nyldn/claude-octopus](https://github.com/nyldn/claude-octopus) | `dfb3c173baa2` | PASS (exit code 0) |
| [sno-ai/sno-station-skills](https://github.com/sno-ai/sno-station-skills) | `531dad8d2619` | PASS (exit code 0) |
| [bitnami/containers](https://github.com/bitnami/containers) | `6a1ecf9482f8` | PASS (exit code 0) |
| [fluffypony/dothething](https://github.com/fluffypony/dothething) | `adfbcb51fa39` | PASS (exit code 0) |
| [jedi4ever/veewee](https://github.com/jedi4ever/veewee) | `0173803b6d9c` | PASS (exit code 0) |
| [leebaird/discover](https://github.com/leebaird/discover) | `82c20ee5072f` | PASS (exit code 0) |
| [BrowserBox/BrowserBox](https://github.com/BrowserBox/BrowserBox) | `bfe19bc86dcb` | PASS (exit code 0) |
| [e-m-b-a/emba](https://github.com/e-m-b-a/emba) | `c509f6651175` | PASS (exit code 0) |
| [pexoai/pexo-skills](https://github.com/pexoai/pexo-skills) | `f724267e45a0` | PASS (exit code 0) |
| [kejilion/sh](https://github.com/kejilion/sh) | `cd10a6be821a` | PASS (exit code 0) |
| [SamNet-dev/MTProxyMax](https://github.com/SamNet-dev/MTProxyMax) | `abb2a1db68fd` | PASS (exit code 0) |
| [scop/bash-completion](https://github.com/scop/bash-completion) | `db439a2c67ea` | PASS (exit code 0) |
| [replicatedhq/kURL](https://github.com/replicatedhq/kURL) | `757d1fb6d78f` | PASS (exit code 0) |
| [StressOzz/Zapret-Manager](https://github.com/StressOzz/Zapret-Manager) | `a0b855a2852b` | PASS (exit code 0) |
| [bioconda/bioconda-recipes](https://github.com/bioconda/bioconda-recipes) | `7a5d2bcb0c12` | PASS (exit code 0) |
| [centminmod/centminmod](https://github.com/centminmod/centminmod) | `4e7aec8585c6` | PASS (exit code 0) |
| [lavabit/robox](https://github.com/lavabit/robox) | `2b308999bda3` | PASS (exit code 0) |
| [Cloud-Wala-Banda/Labs-Solutions](https://github.com/Cloud-Wala-Banda/Labs-Solutions) | `74279e502b9f` | PASS (exit code 0) |
| [Dicklesworthstone/agentic_coding_flywheel_setup](https://github.com/Dicklesworthstone/agentic_coding_flywheel_setup) | `7c1967c83306` | PASS (exit code 0) |
| [zoddDev/dotfiles](https://github.com/zoddDev/dotfiles) | `01fa9923f5d8` | PASS (exit code 0) |
| [Gentleman-Programming/Gentleman.Dots](https://github.com/Gentleman-Programming/Gentleman.Dots) | `59aee785989c` | PASS (exit code 0) |
| [dragoonDorise/EmuDeck](https://github.com/dragoonDorise/EmuDeck) | `d181675eb9ab` | PASS (exit code 0) |
| [Chachamaru127/claude-code-harness](https://github.com/Chachamaru127/claude-code-harness) | `aeb665e8cc42` | PASS (exit code 0) |
| [linuxeye/lnmp](https://github.com/linuxeye/lnmp) | `3b4dff6ae362` | PASS (exit code 0) |
| [alexbelgium/hassio-addons](https://github.com/alexbelgium/hassio-addons) | `285495acb859` | PASS (exit code 0) |
| [matryer/xbar-plugins](https://github.com/matryer/xbar-plugins) | `44180ccde937` | PASS (exit code 0) |
| [oneinstack/oneinstack](https://github.com/oneinstack/oneinstack) | `fa0995bd466b` | PASS (exit code 0) |
| [keystonejs/keystone](https://github.com/keystonejs/keystone) | `c34ec685c3af` | PASS (exit code 0) |
| [open-slide/open-slide](https://github.com/open-slide/open-slide) | `30b9f7f93fbb` | PASS (exit code 0) |
| [usekaneo/kaneo](https://github.com/usekaneo/kaneo) | `e46680d28a00` | PASS (exit code 0) |
| [vercel/next.js](https://github.com/vercel/next.js) | `0173455ce606` | PASS (exit code 0) |
| [shadcn-ui/ui](https://github.com/shadcn-ui/ui) | `0e5b2e621a9e` | PASS (exit code 0) |
| [zuiidea/antd-admin](https://github.com/zuiidea/antd-admin) | `4dc473ba5563` | PASS (exit code 0) |
| [react/create-react-app](https://github.com/react/create-react-app) | `6254386531d2` | PASS (exit code 0) |
| [mui/material-ui](https://github.com/mui/material-ui) | `4ee439010d64` | PASS (exit code 0) |
| [margelo/react-native-vision-camera](https://github.com/margelo/react-native-vision-camera) | `d54fc486c3a9` | PASS (exit code 0) |
| [ChatGPTNextWeb/NextChat](https://github.com/ChatGPTNextWeb/NextChat) | `1d3dc3315129` | PASS (exit code 0) |
| [pmndrs/use-gesture](https://github.com/pmndrs/use-gesture) | `c779631aa059` | PASS (exit code 0) |
| [Ovyerus/prismaliser](https://github.com/Ovyerus/prismaliser) | `b08a906e6c19` | PASS (exit code 0) |
| [meta/docusaurus](https://github.com/meta/docusaurus) | `343097822317` | PASS (exit code 0) |
| [aws-amplify/amplify-js](https://github.com/aws-amplify/amplify-js) | `c08112f4b821` | PASS (exit code 0) |
| [makeplane/plane](https://github.com/makeplane/plane) | `723bb178f97b` | PASS (exit code 0) |
| [projectstorm/react-diagrams](https://github.com/projectstorm/react-diagrams) | `cd3bdd119376` | PASS (exit code 0) |
| [SyntaxUI/syntaxui](https://github.com/SyntaxUI/syntaxui) | `322491934c9e` | PASS (exit code 0) |
| [miurla/morphic](https://github.com/miurla/morphic) | `59e2be2445fe` | PASS (exit code 0) |
| [tldraw/signia](https://github.com/tldraw/signia) | `b55c1ec1349d` | PASS (exit code 0) |
| [tldraw/tldraw](https://github.com/tldraw/tldraw) | `9a831f6d9ebc` | PASS (exit code 0) |
| [maotoumao/MusicFreeDesktop](https://github.com/maotoumao/MusicFreeDesktop) | `f3b526a6c1ea` | PASS (exit code 0) |
| [ayastreb/money-tracker](https://github.com/ayastreb/money-tracker) | `4af70cff39f4` | PASS (exit code 0) |
| [TanStack/query](https://github.com/TanStack/query) | `635aad6a95b0` | PASS (exit code 0) |
| [webstudio-is/webstudio](https://github.com/webstudio-is/webstudio) | `726b288a3e20` | PASS (exit code 0) |
| [victoralvesf/aonsoku](https://github.com/victoralvesf/aonsoku) | `13343e35a7d5` | PASS (exit code 0) |
| [fireproof-storage/fireproof](https://github.com/fireproof-storage/fireproof) | `7de16b1da7d9` | PASS (exit code 0) |
| [benawad/dogehouse](https://github.com/benawad/dogehouse) | `584055ad407b` | PASS (exit code 0) |
| [reactive-resume/reactive-resume](https://github.com/reactive-resume/reactive-resume) | `88b337550eb5` | PASS (exit code 0) |
| [xitanggg/open-resume](https://github.com/xitanggg/open-resume) | `4f8255a2c763` | PASS (exit code 0) |
| [payloadcms/payload](https://github.com/payloadcms/payload) | `432a2541a4c1` | PASS (exit code 0) |
| [radix-ui/themes](https://github.com/radix-ui/themes) | `1faff10ac26a` | PASS (exit code 0) |
| [OnsenUI/OnsenUI](https://github.com/OnsenUI/OnsenUI) | `82c5ff8ce1b8` | PASS (exit code 0) |
| [AykutSarac/jsoncrack.com](https://github.com/AykutSarac/jsoncrack.com) | `c30e379df2c3` | PASS (exit code 0) |
| [uber/react-vis](https://github.com/uber/react-vis) | `92e2af2eac82` | PASS (exit code 0) |
| [code-forge-io/react-router-devtools](https://github.com/code-forge-io/react-router-devtools) | `025a4c24d545` | PASS (exit code 0) |
| [prevwong/craft.js](https://github.com/prevwong/craft.js) | `23f44f3208eb` | PASS (exit code 0) |
| [chvin/react-tetris](https://github.com/chvin/react-tetris) | `89435b72f127` | PASS (exit code 0) |
| [react-cosmos/react-cosmos](https://github.com/react-cosmos/react-cosmos) | `47332e1273a4` | PASS (exit code 0) |
| [stackworx/formik-mui](https://github.com/stackworx/formik-mui) | `d65e4627b30c` | PASS (exit code 0) |
| [xyflow/xyflow](https://github.com/xyflow/xyflow) | `9635a064f9d4` | PASS (exit code 0) |
| [Gamote/lottie-react](https://github.com/Gamote/lottie-react) | `4205a49678c9` | PASS (exit code 0) |
| [omeralpi/shadcn-phone-input](https://github.com/omeralpi/shadcn-phone-input) | `0ea744fe5285` | PASS (exit code 0) |
| [react-toolbox/react-toolbox](https://github.com/react-toolbox/react-toolbox) | `799da6783853` | PASS (exit code 0) |
| [garmeeh/next-seo](https://github.com/garmeeh/next-seo) | `738ef7e774ee` | PASS (exit code 0) |
| [mat-sz/filedrop](https://github.com/mat-sz/filedrop) | `257b00a842c5` | PASS (exit code 0) |
| [markdoc/markdoc](https://github.com/markdoc/markdoc) | `635fb7631b04` | PASS (exit code 0) |
| [LottieFiles/dotlottie-web](https://github.com/LottieFiles/dotlottie-web) | `db3f9b19e8e5` | PASS (exit code 0) |
| [appbaseio/dejavu](https://github.com/appbaseio/dejavu) | `8b8f11f40cd6` | PASS (exit code 0) |
| [stoatchat/for-legacy-web](https://github.com/stoatchat/for-legacy-web) | `8acff5232725` | PASS (exit code 0) |
| [jaredpalmer/formik](https://github.com/jaredpalmer/formik) | `91475adbf335` | PASS (exit code 0) |
| [PanJiaChen/vue-element-admin](https://github.com/PanJiaChen/vue-element-admin) | `6858a9ad6748` | PASS (exit code 0) |
| [kiwiirc/kiwiirc](https://github.com/kiwiirc/kiwiirc) | `7345960f0da2` | PASS (exit code 0) |
| [bailicangdu/vue2-elm](https://github.com/bailicangdu/vue2-elm) | `b85ff0220366` | PASS (exit code 0) |
| [docmirror/dev-sidecar](https://github.com/docmirror/dev-sidecar) | `bf1d8986ae18` | PASS (exit code 0) |
| [JakHuang/form-generator](https://github.com/JakHuang/form-generator) | `c4e20ad59a55` | PASS (exit code 0) |
| [zxwk1998/vue-admin-better](https://github.com/zxwk1998/vue-admin-better) | `b02834b2dd91` | PASS (exit code 0) |
| [Moonvy/OpenPromptStudio](https://github.com/Moonvy/OpenPromptStudio) | `707b6e5d3d02` | PASS (exit code 0) |
| [vueComponent/ant-design-vue-pro](https://github.com/vueComponent/ant-design-vue-pro) | `616df9e063a4` | PASS (exit code 0) |
| [getgridea/gridea](https://github.com/getgridea/gridea) | `48e4ab573bfa` | PASS (exit code 0) |
| [woai3c/visual-drag-demo](https://github.com/woai3c/visual-drag-demo) | `7b2f0b263527` | PASS (exit code 0) |
| [GavinZhuLei/vue-form-making](https://github.com/GavinZhuLei/vue-form-making) | `acec212b77b5` | PASS (exit code 0) |
| [HoshinoSuzumi/CTFever](https://github.com/HoshinoSuzumi/CTFever) | `3c1d26120a52` | PASS (exit code 0) |
| [lensh/vue-qq](https://github.com/lensh/vue-qq) | `5ad309829aaf` | PASS (exit code 0) |
| [elunez/eladmin-web](https://github.com/elunez/eladmin-web) | `6e20207bbaf9` | PASS (exit code 0) |
| [pearmini/colorfu](https://github.com/pearmini/colorfu) | `3ab8947e27c4` | PASS (exit code 0) |
| [ymm-tech/gods-pen](https://github.com/ymm-tech/gods-pen) | `c1c18ada8ff0` | PASS (exit code 0) |
| [zhengguorong/h5maker](https://github.com/zhengguorong/h5maker) | `d375d3a9862c` | PASS (exit code 0) |
| [iamdarcy/hioshop-admin](https://github.com/iamdarcy/hioshop-admin) | `4b181c5a6982` | PASS (exit code 0) |
| [herozhou/vue-framework-wz](https://github.com/herozhou/vue-framework-wz) | `c500d578ef81` | PASS (exit code 0) |
| [myliang/fish-ui](https://github.com/myliang/fish-ui) | `422559409192` | PASS (exit code 0) |
| [umicro/uView](https://github.com/umicro/uView) | `1c73d473e15d` | PASS (exit code 0) |
| [wdlhao/vue2-element-touzi-admin](https://github.com/wdlhao/vue2-element-touzi-admin) | `7a73cd93e5e3` | PASS (exit code 0) |
| [alaboshu/alaweb](https://github.com/alaboshu/alaweb) | `bbc6e616d538` | PASS (exit code 0) |
| [Simon-He95/markstream-vue](https://github.com/Simon-He95/markstream-vue) | `0d7d8cd446f3` | PASS (exit code 0) |
| [chiflix/splayerx](https://github.com/chiflix/splayerx) | `032efcc9924c` | PASS (exit code 0) |
| [KlausSchaefers/quant-ux](https://github.com/KlausSchaefers/quant-ux) | `661e29b216f2` | PASS (exit code 0) |
| [xiaozhu188/electron-vue-cloud-music](https://github.com/xiaozhu188/electron-vue-cloud-music) | `f911e765fd56` | PASS (exit code 0) |
| [mainsail-crew/mainsail](https://github.com/mainsail-crew/mainsail) | `ff70ceb3c49a` | PASS (exit code 0) |
| [tuandm/laravue](https://github.com/tuandm/laravue) | `b56ec7526e47` | PASS (exit code 0) |
| [hua1995116/musiccloudWebapp](https://github.com/hua1995116/musiccloudWebapp) | `3522f6c28610` | PASS (exit code 0) |
| [vform666/variant-form](https://github.com/vform666/variant-form) | `24e62c360bab` | PASS (exit code 0) |
| [Exrick/xboot-front](https://github.com/Exrick/xboot-front) | `fb933de4c592` | PASS (exit code 0) |
| [webmobiledev/vue-tradingview](https://github.com/webmobiledev/vue-tradingview) | `0c22802ad4b6` | PASS (exit code 0) |
| [fluidd-core/fluidd](https://github.com/fluidd-core/fluidd) | `0ffcd7f2ca66` | PASS (exit code 0) |
| [a54552239/pearProject](https://github.com/a54552239/pearProject) | `900c02bc8719` | PASS (exit code 0) |
| [umicro/uView2.0](https://github.com/umicro/uView2.0) | `e5649b708bb5` | PASS (exit code 0) |
| [stavyan/TinyShop-UniApp](https://github.com/stavyan/TinyShop-UniApp) | `c0ed622beb2b` | PASS (exit code 0) |
| [N3-components/N3-components](https://github.com/N3-components/N3-components) | `6c65c408a90c` | PASS (exit code 0) |
| [gongfuxiang/shopxo-uniapp](https://github.com/gongfuxiang/shopxo-uniapp) | `a3b04daeef9a` | PASS (exit code 0) |
| [gooking/uni-app-mall](https://github.com/gooking/uni-app-mall) | `56f3dcfda3c8` | PASS (exit code 0) |
| [dcloudio/uni-admin](https://github.com/dcloudio/uni-admin) | `2a865af27bce` | PASS (exit code 0) |
| [HolodexNet/Holodex](https://github.com/HolodexNet/Holodex) | `8c6d0dacbe93` | PASS (exit code 0) |
| [FXLP/MarkTool](https://github.com/FXLP/MarkTool) | `0d2f77de4d40` | PASS (exit code 0) |
| [ITmonkey-cn/shopro-uniapp](https://github.com/ITmonkey-cn/shopro-uniapp) | `cc2ed389b9d7` | PASS (exit code 0) |
| [auto-i18n/auto-i18n-translation-plugins](https://github.com/auto-i18n/auto-i18n-translation-plugins) | `09cd872214d3` | PASS (exit code 0) |
| [MegEngine/MegSpot](https://github.com/MegEngine/MegSpot) | `2d0e05e116ad` | PASS (exit code 0) |
| [Akryum/guijs](https://github.com/Akryum/guijs) | `a3b0f92c4c3a` | PASS (exit code 0) |
| [SNFocus/approvalFlow](https://github.com/SNFocus/approvalFlow) | `1913b41941c1` | PASS (exit code 0) |
| [ClickerMonkey/dayspan-vuetify](https://github.com/ClickerMonkey/dayspan-vuetify) | `842c66a7cc3f` | PASS (exit code 0) |
| [Tucsky/aggr](https://github.com/Tucsky/aggr) | `41c1609ea3ac` | PASS (exit code 0) |
| [galaxy-s10/billd-desk](https://github.com/galaxy-s10/billd-desk) | `9212de860f0d` | PASS (exit code 0) |
| [zouzhibin/zb-admin](https://github.com/zouzhibin/zb-admin) | `10ac76648902` | PASS (exit code 0) |
| [CorentinTh/it-tools](https://github.com/CorentinTh/it-tools) | `d505845f918e` | PASS (exit code 0) |
| [readpage/undraw-ui](https://github.com/readpage/undraw-ui) | `f7fe6b657cf6` | PASS (exit code 0) |
| [jiangrui1994/CloudSaver](https://github.com/jiangrui1994/CloudSaver) | `672cc48ab656` | PASS (exit code 0) |
| [dromara/go-view](https://github.com/dromara/go-view) | `7d88704d9c18` | PASS (exit code 0) |
| [Hiram-Wong/zyfun](https://github.com/Hiram-Wong/zyfun) | `ffba0e2c8b72` | PASS (exit code 0) |
| [quasarframework/quasar](https://github.com/quasarframework/quasar) | `7de7b9f8fd64` | PASS (exit code 0) |
| [snapshot-labs/snapshot-v1](https://github.com/snapshot-labs/snapshot-v1) | `15f515c4e36d` | PASS (exit code 0) |
| [HalseySpicy/Geeker-Admin](https://github.com/HalseySpicy/Geeker-Admin) | `ad08fdd1c08b` | PASS (exit code 0) |
| [youzan/vant](https://github.com/youzan/vant) | `7cc499279866` | PASS (exit code 0) |
| [thinkgem/jeesite](https://github.com/thinkgem/jeesite) | `ba594847585c` | PASS (exit code 0) |
| [FreeTubeApp/FreeTube](https://github.com/FreeTubeApp/FreeTube) | `efa0915df12e` | PASS (exit code 0) |
| [romainsimon/uisfx](https://github.com/romainsimon/uisfx) | `9950fe66f993` | PASS (exit code 0) |
| [vueuse/vueuse](https://github.com/vueuse/vueuse) | `6680e578f149` | PASS (exit code 0) |
| [HuLaSpark/HuLa](https://github.com/HuLaSpark/HuLa) | `0ce2a98ddf29` | PASS (exit code 0) |
| [antfu-collective/icones](https://github.com/antfu-collective/icones) | `02fd287a36d4` | PASS (exit code 0) |
| [AutomaApp/automa](https://github.com/AutomaApp/automa) | `a4cbe34a60c9` | PASS (exit code 0) |
| [BaudDance/SerialAssistant](https://github.com/BaudDance/SerialAssistant) | `23892f4bd0af` | PASS (exit code 0) |
| [MoeKoeMusic/MoeKoeMusic](https://github.com/MoeKoeMusic/MoeKoeMusic) | `6be8e5c201ee` | PASS (exit code 0) |
| [lin-xin/vue-manage-system](https://github.com/lin-xin/vue-manage-system) | `6a7019ec1a74` | PASS (exit code 0) |
| [VueTorrent/VueTorrent](https://github.com/VueTorrent/VueTorrent) | `766974a709d5` | PASS (exit code 0) |
| [yangzongzhuan/RuoYi-Vue3](https://github.com/yangzongzhuan/RuoYi-Vue3) | `3c2b70c30ff9` | PASS (exit code 0) |
| [lkw199711/smanga](https://github.com/lkw199711/smanga) | `73acb54d124c` | PASS (exit code 0) |
| [Datlyfe/jira_clone](https://github.com/Datlyfe/jira_clone) | `72c7a35b3679` | PASS (exit code 0) |
| [jd-opensource/nutui](https://github.com/jd-opensource/nutui) | `212c7c3af6d0` | PASS (exit code 0) |
| [newbee-ltd/newbee-mall-vue3-app](https://github.com/newbee-ltd/newbee-mall-vue3-app) | `07740cba11c2` | PASS (exit code 0) |
| [elk-zone/elk](https://github.com/elk-zone/elk) | `ff32c55e5f1f` | PASS (exit code 0) |
| [doocs/md](https://github.com/doocs/md) | `80b43d495bf5` | PASS (exit code 0) |
| [x007xyz/flycut](https://github.com/x007xyz/flycut) | `8071cf7c9969` | PASS (exit code 0) |
| [jekip/naive-ui-admin](https://github.com/jekip/naive-ui-admin) | `c9db4cde4472` | PASS (exit code 0) |
| [macrozheng/mall-admin-web](https://github.com/macrozheng/mall-admin-web) | `81fc17e5a19f` | PASS (exit code 0) |
| [zyronon/TypeWords](https://github.com/zyronon/TypeWords) | `1bcc2a2c0a16` | PASS (exit code 0) |
| [EKKOLearnAI/ekko-studio](https://github.com/EKKOLearnAI/ekko-studio) | `f57b784f7c52` | PASS (exit code 0) |
| [october-coder/api-check](https://github.com/october-coder/api-check) | `b075d293a640` | PASS (exit code 0) |
| [homer-dashboard/homer](https://github.com/homer-dashboard/homer) | `b3f7fd87813c` | PASS (exit code 0) |
| [hslr-s/sun-panel](https://github.com/hslr-s/sun-panel) | `59831cedac62` | PASS (exit code 0) |
| [kaangiray26/geddit-app](https://github.com/kaangiray26/geddit-app) | `7eb5cc2499b4` | PASS (exit code 0) |
| [varletjs/varlet](https://github.com/varletjs/varlet) | `4d85d1582917` | PASS (exit code 0) |
| [kinopio-club/kinopio-client](https://github.com/kinopio-club/kinopio-client) | `32f601038ad3` | PASS (exit code 0) |
| [Hacker233/resume-design](https://github.com/Hacker233/resume-design) | `b87a87f8d8e9` | PASS (exit code 0) |
| [JosephusPaye/Keen-UI](https://github.com/JosephusPaye/Keen-UI) | `3348200acb23` | PASS (exit code 0) |
| [yudaocode/yudao-ui-admin-vue3](https://github.com/yudaocode/yudao-ui-admin-vue3) | `af5d4870b70e` | PASS (exit code 0) |
| [itq5/OpenClaw-Admin](https://github.com/itq5/OpenClaw-Admin) | `d722891ec634` | PASS (exit code 0) |
| [yenche123/liubai](https://github.com/yenche123/liubai) | `3dab83a16d34` | PASS (exit code 0) |
| [epicmaxco/vuestic-ui](https://github.com/epicmaxco/vuestic-ui) | `c5337ed8e7e2` | PASS (exit code 0) |
| [arco-design/arco-design-vue](https://github.com/arco-design/arco-design-vue) | `f5f403eba18c` | PASS (exit code 0) |
| [Simon-He95/markstream-vue](https://github.com/Simon-He95/markstream-vue) | `0d7d8cd446f3` | PASS (exit code 0) |
| [KunMoe/kun-galgame-forum](https://github.com/KunMoe/kun-galgame-forum) | `80cb39157edc` | PASS (exit code 0) |
| [staniel359/muffon](https://github.com/staniel359/muffon) | `f4eebf9af69b` | PASS (exit code 0) |

## Audit sample

Each sample is drawn with a seed derived from the pinned commits, so the same corpus gives the same sample.

Reviewer: agent. These are agent-reviewed false positive rates; a person can recheck every verdict in the verdicts file.

| Category | Sampled | Reviewed | False positives | False positive rate |
| --- | --- | --- | --- | --- |
| Source sets | 100 | 100 | 1 | 1.00% |
| Flagged lines | 100 | 100 | 0 | 0.00% |
| Orphan files | 100 | 100 | 97 | 97.00% |

### Source sets

Seed `450845187fcf2345`; 100 of 602395 Findings.

- `CarGuo/gsy_github_app_flutter lib/db/provider/repos/repository_pulse_db_provider.dart production`
- `certimate-go/certimate internal/tools/mproc/sender.go production`
- `modem-dev/hunk packages/hunk/src/core/vcs/untracked.ts production`
- `failsafe-security/artex agent/blackboard_inheritance_test.go test`
- `image-rs/imageproc tests/lib.rs test`
- `hiyouga/LlamaFactory scripts/dcp2hf.py production`
- `ruotianluo/self-critical.pytorch captioning/data/__init__.py production`
- `deepseek-ai/deepseek-harness apps/web/tests/clock.e2e.ts test`
- `grokability/snipe-it resources/lang/nl-NL/admin/components/table.php production`
- `OpenBMB/VisRAG timm_modified/timm/utils/agc.py production`
- `wallabag/wallabag src/Doctrine/WallabagMigration.php production`
- `kody-hq/kody packages/worker/start/pages/package-settings.tsx production`
- `anomalyco/opencode services/site/src/component/site/logo-settings.tsx production`
- `rustdesk/rustdesk src/server/uinput/layout/wayland.rs production`
- `databendlabs/databend src/query/functions/tests/it/scalars/encoded.rs test`
- `alibaba/fish-redux lib/src/redux_adapter/static_flow_adapter.dart production`
- `clojure-rs/ClojureRS src/rust_core/special_form/var.rs production`
- `openai/codex codex-rs/exec-server/src/rpc_client_metrics_tests.rs test`
- `nodejs/node test/parallel/test-buffer-parent-property.js test`
- `pbakaus/impeccable crates/browser/tests/low_contrast_pixel_reads.rs test`
- `anomalyco/opencode packages/session-ui/component-tests/tool-status-title.spec.ts test`
- `chen08209/FlClash tool/src/package/steps.dart production`
- `microsoft/vscode src/vs/workbench/contrib/extensions/test/common/runtimeExtensionsInput.test.ts test`
- `router-for-me/CLIProxyAPI internal/client/codex/models/apply_patch_test.go test`
- `Bash-it/bash-it themes/demula/demula.theme.bash production`
- `anthropics/claude-code mods/easel/hooks/core/cells/read-time/time-from-parts/is-different-in/is-different-in.ts production`
- `ellite/Wallos libs/constant_time_encoding/Encoding.php production`
- `safebucket/safebucket internal/events/user_invitation.go production`
- `garrytan/gstack browse/test/server-sanitize-surrogates.test.ts test`
- `kanboard/kanboard vendor/psr/container/src/NotFoundExceptionInterface.php vendored`
- `apollographql/router apollo-router/tests/integration/metrics.rs test`
- `usekaneo/kaneo apps/api/src/admin/users/account-email-links.ts production`
- `mbadolato/iTerm2-Color-Schemes dynamic-colors/Vercel.sh production`
- `deepseek-ai/deepseek-harness scripts/persistence-developer.spec.ts test`
- `unionlabs/union cosmwasm/lst/src/tests/instantiate_tests.rs test`
- `GoAdminGroup/go-admin examples/gorilla/main.go production`
- `pdepend/pdepend tests/resources/files/Source/AST/ASTParentReference/testParentReferenceHasExpectedStartLine.php test`
- `garrytan/gstack test/helpers/plan-review-board-feedback.ts test`
- `termux/termux-packages packages/ratt/build.sh production`
- `oumi-ai/oumi src/oumi/launcher/clouds/perlmutter_cloud.py production`
- `nodejs/node test/parallel/test-runner-root-duration.js test`
- `swamp-club/swamp src/domain/workflows/run_save_horizon_test.ts production`
- `OHDSI/Vocabulary-v5.0 working/packages/vocabulary_pack/CutConceptName.sql production`
- `morluto/rea src/domain/analysisErrorCleanup.ts production`
- `Chevey339/kelivo test/features/workspace/environment/environment_test_fakes.dart test`
- `n8n-io/n8n packages/quality/policy/code-health/src/utils/pnpm-lock-parser.test.ts test`
- `flutter/flutter dev/benchmarks/macrobenchmarks/lib/src/path_tessellation.dart production`
- `alibaba/flutter-go lib/widgets/elements/Form/Slider/index.dart production`
- `react/react scripts/release/build-release-locally-commands/npm-pack-and-unpack.js production`
- `basedosdados/sdk bases/br_camara_atividade_legislativa/deputados/publish.sql production`
- `openai/codex codex-rs/http-client/src/retry_after.rs production`
- `PrestaShop/PrestaShop classes/stock/SupplyOrderDetail.php production`
- `flutter/flutter packages/flutter_tools/packages/flutter_tools_core/test/diagnostics_test.dart test`
- `openai/codex codex-rs/memories/read/src/metrics.rs production`
- `isar/isar packages/isar_inspector/lib/collection/button_sort.dart production`
- `n8n-io/n8n packages/modules/insights/backend/src/insights.settings.ts production`
- `n8n-io/n8n .github/scripts/quality/check-cubic-config.mjs production`
- `bagisto/opensource-ecommerce-mobile-app lib/core/utils/phone_validation.dart production`
- `n8n-io/n8n packages/cli/test/integration/database/repositories/agent.repository.test.ts test`
- `nodejs/node deps/v8/test/mjsunit/modules-skip-9.mjs test`
- `cfug/dio dio/lib/src/compute/compute_io.dart production`
- `PeonPing/peon-ping adapters/grok.sh production`
- `awslabs/mcp src/aws-dataprocessing-mcp-server/awslabs/aws_dataprocessing_mcp_server/server.py production`
- `wasm-bindgen/wasm-bindgen crates/web-sys/src/features/gen_SvgAnimateTransformElement.rs production`
- `krayin/laravel-crm packages/Webkul/User/src/Repositories/RoleRepository.php production`
- `a-ghorbani/pocketpal-ai e2e/helpers/iap-actions.ts production`
- `stablyai/orca mobile/src/bots/new-bot-host-reads.test.ts test`
- `nodejs/node deps/npm/tap-snapshots/test/lib/commands/diff.js.test.cjs generated`
- `anomalyco/opencode packages/gui-plugins/src/review/i18n/th.ts production`
- `matomo-org/matomo core/Plugin/Categories.php production`
- `microsoft/vscode src/vs/platform/extensions/common/extensions.ts production`
- `anomalyco/opencode packages/session-ui/src/components/markdown-phase-machine.test.ts test`
- `nodejs/node test/parallel/test-stream-destroy.js test`
- `react/react packages/react-devtools-shared/src/__tests__/useEditableValue-test.js test`
- `FuelLabs/fuels-rs packages/fuels-macros/tests/ui/setup_program_test/invalid_path.rs test`
- `matomo-org/matomo plugins/DevicesDetection/RecordBuilders/BrowserEngines.php production`
- `open-telemetry/opentelemetry-php src/Config/SDK/ComponentProvider/Metrics/AggregationResolverBase2ExponentialBucketHistogram.php production`
- `react/react compiler/packages/babel-plugin-react-compiler/src/__tests__/fixtures/compiler/jsx-string-attribute-non-ascii.js test`
- `earendil-works/pi packages/mcp/test/config.test.ts test`
- `FoundationAgents/MetaGPT tests/metagpt/actions/__init__.py test`
- `babelfish-for-postgresql/babelfish_extensions test/JDBC/upgrade/latest/verification_cleanup/13_5/sys_all_objects-dep-vu-cleanup.sql test`
- `scop/bash-completion completions-core/flake8.bash production`
- `n8n-io/n8n packages/frontend/editor-ui/src/features/ndv/agents/composables/__tests__/useInlineAgentConfig.test.ts test`
- `phacility/phabricator scripts/init/init-aphlict.php production`
- `open-mmlab/mmsegmentation mmseg/evaluation/metrics/depth_metric.py production`
- `go-gitea/gitea modules/session/redis.go production`
- `anomalyco/opencode packages/ai/src/protocols/zai-messages.ts production`
- `ollama/ollama decision/systemone.go production`
- `bioconda/bioconda-recipes recipes/adapterremoval/build.sh production`
- `bioconda/bioconda-recipes recipes/bioconductor-rwgcod.db/pre-unlink.sh production`
- `Dicklesworthstone/claude_code_agent_farm tool_setup_scripts/setup_sveltekit_remix_astro.sh production`
- `gitkraken/vscode-gitlens src/commands/git/worktree/create.ts production`
- `briannesbitt/Carbon src/Carbon/Lang/kk_KZ.php production`
- `morluto/rea src/cli/contractCliDocumentDescription.ts production`
- `awslabs/mcp src/aws-serverless-mcp-server/awslabs/aws_serverless_mcp_server/utils/const.py production`
- `HariSekhon/DevOps-Bash-tools checks/check_shellcheck.sh production`
- `bioconda/bioconda-recipes recipes/mutscan/build.sh production`
- `bioconda/bioconda-recipes recipes/aardvark/build.sh production`
- `AppFlowy-IO/AppFlowy frontend/appflowy_flutter/lib/plugins/document/presentation/editor_plugins/transaction_handler/mention_transaction_handler.dart production`
- `octobercms/october modules/backend/widgets/table/ServerEventDataSource.php production`

### Flagged lines

Seed `450845187fcf2345`; 100 of 143003 Findings.

- `awslabs/mcp src/oracle-mcp-server/awslabs/oracle_mcp_server/connection/oracledb_pool_connection.py:236 else-after-return`
- `jeremychone/rust-genai src/adapter/adapters/openai/streamer.rs:419 else-after-return`
- `clojure-rs/ClojureRS src/clojure_string/blank_qmark_.rs:22 else-after-return`
- `flutter/flutter engine/src/flutter/lib/ui/fixtures/ui_test.dart:1784 else-after-return`
- `centminmod/centminmod addons/nodejs.sh:672 else-after-return`
- `Notsfsssf/pixez-flutter plugins/rhttp/rhttp/rust/third_party/quinn/quinn-proto/src/connection/assembler.rs:53 else-after-return`
- `Cloud-Wala-Banda/Labs-Solutions Configuring IAM Permissions with gCloud - Azure/gsp1119-1.sh:8 legacy-backticks`
- `certimate-go/certimate pkg/sdk3rd/ctyun/lvdn/api_query_domain_list.go:39 else-after-return`
- `conan-io/conan conan/internal/graph/provides.py:36 else-after-return`
- `openinterpreter/openinterpreter codex-rs/core/tests/common/context_snapshot/normalize.rs:180 redundant-closure`
- `jiangtian616/JHenTai lib/src/pages/details/details_page_logic.dart:952 else-after-return`
- `flutter/flutter dev/manual_tests/lib/drag_and_drop.dart:224 else-after-return`
- `Cloud-Wala-Banda/Labs-Solutions Creating Dynamic Secrets for Google Cloud with Vault/gsp1007.sh:7 legacy-backticks`
- `yt-dlp/yt-dlp test/test_networking.py:112 else-after-return`
- `Cloud-Wala-Banda/Labs-Solutions Developing and Deploying Cloud Functions/shell.sh:8 legacy-backticks`
- `kunchenguid/firstmate bin/fm-backlog-handoff.sh:195 else-after-return`
- `deajan/osync dev/debug_osync.sh:1705 redundant-exit-status-check`
- `vuejs/vue src/core/util/options.ts:110 else-after-return`
- `FyroxEngine/Fyrox fyrox-ui/src/menu.rs:1299 else-after-return`
- `Cloud-Wala-Banda/Labs-Solutions Monitoring Applications in Google Cloud/shell-1.sh:9 legacy-backticks`
- `estuary/flow crates/runtime-next/src/shard/materialize/actor.rs:436 else-after-return`
- `AppFlowy-IO/AppFlowy frontend/appflowy_flutter/packages/flowy_infra_ui/lib/widget/dialog/styled_dialogs.dart:173 else-after-return`
- `BrowserBox/BrowserBox bbx.sh:3993 else-after-return`
- `awslabs/mcp src/aws-healthomics-mcp-server/awslabs/aws_healthomics_mcp_server/tools/ecr_tools.py:2721 else-after-return`
- `juewuy/ShellCrash scripts/menus/6_core_config.sh:349 else-after-return`
- `Cloud-Wala-Banda/Labs-Solutions Implement Cloud Security Fundamentals on Google Cloud Challenge Lab/gsp342.sh:20 legacy-backticks`
- `Kuaishou-OneRec/OpenOneRec verl_rl/verl/models/mcore/qwen2_5_vl/rope_utils.py:207 else-after-return`
- `n8n-io/n8n packages/@n8n/typeorm/src/driver/postgres/PostgresQueryRunner.ts:341 conditional-yields-boolean`
- `kaldi-asr/kaldi egs/spgispeech/s5/local/train_lms_srilm.sh:132 else-after-return`
- `kata-containers/kata-containers src/tools/kata-ctl/src/utils.rs:73 else-after-return`
- `deajan/osync dev/merge.sh:125 else-after-return`
- `AdvancedCustomFields/acf includes/ajax/class-acf-ajax-user-setting.php:55 else-after-return`
- `kaldi-asr/kaldi egs/wsj/s5/steps/train_mmi_fmmi_indirect.sh:152 legacy-backticks`
- `syncthing/syncthing gui/default/vendor/angular/angular-dirPagination.js:476 else-after-return`
- `sinonjs/sinon test/src/create-sinon-api-test.js:50 trailing-bare-return`
- `vercel/next.js turbopack/crates/turbopack-ecmascript/src/chunk/placeable.rs:167 else-after-return`
- `scikit-hep/awkward src/awkward/forms/form.py:201 else-after-return`
- `flutter/flutter packages/flutter_tools/lib/src/commands/daemon.dart:1548 else-after-return`
- `firebase/flutterfire packages/firebase_ai/firebase_ai/example/lib/pages/integration_test_page.dart:305 else-after-return`
- `lavabit/robox scripts/alma9/kernel.sh:11 legacy-backticks`
- `Cloud-Wala-Banda/Labs-Solutions Loading Data into Cloud SQL/gsp196.sh:3 legacy-backticks`
- `bitnami/containers bitnami/valkey/9.1/debian-12/prebuildfs/opt/bitnami/scripts/libvalidations.sh:166 else-after-return`
- `kaldi-asr/kaldi egs/fisher_callhome_spanish/s5/local/subset_data_prep.sh:141 useless-cat`
- `kaldi-asr/kaldi egs/aurora4/s5/local/cstr_wsj_data_prep.sh:21 legacy-backticks`
- `lollipopkit/flutter_server_box crates/sbm_ffi/src/frb_generated.rs:11770 trailing-return`
- `mirarr-app/mirarr lib/moviesPage/movieDetailPageDesktop.dart:495 else-after-return`
- `ytdl-org/youtube-dl youtube_dl/downloader/http.py:184 else-after-return`
- `ansible/ansible lib/ansible/plugins/action/package.py:94 else-after-return`
- `scikit-hep/awkward src/awkward/operations/ak_almost_equal.py:253 else-after-return`
- `nodejs/node deps/v8/tools/eval_gc_time.sh:151 useless-cat`
- `Cloud-Wala-Banda/Labs-Solutions Configuring IAM Permissions with gCloud - Azure/gsp1119-2.sh:12 legacy-backticks`
- `huggingface/accelerate src/accelerate/utils/operations.py:82 else-after-return`
- `xonsh/xonsh xonsh/xoreutils/umask.py:165 else-after-return`
- `centminmod/centminmod installer-minimal-gitlab.sh:1231 else-after-return`
- `crossbeam-rs/crossbeam crossbeam-channel/src/flavors/array.rs:590 else-after-return`
- `estuary/flow crates/runtime-next/src/leader/materialize/fsm.rs:432 else-after-return`
- `breeze-project-org/Breeze lib/network/http/plugin/qjs_download_runtime.dart:391 else-after-return`
- `ansible/ansible lib/ansible/cli/adhoc.py:134 else-after-return`
- `avdept/JellyBoxPlayer packages/flutter_chrome_cast/lib/_discovery_manager/android_discovery_manager.dart:49 trailing-bare-return`
- `centminmod/centminmod centmin.sh:4557 else-after-return`
- `AppFlowy-IO/AppFlowy frontend/appflowy_flutter/lib/user/presentation/anon_user.dart:54 else-after-return`
- `Cloud-Wala-Banda/Labs-Solutions Arcade Hero Enter the Cloud Function Storage/arc236.sh:13 legacy-backticks`
- `deajan/osync dev/debug_osync.sh:5998 redundant-exit-status-check`
- `certimate-go/certimate pkg/core/deployer/providers/tencentcloud-vod/tencentcloud_vod.go:96 else-after-return`
- `awslabs/mcp src/ecs-mcp-server/tests/integ/mcp-inspector/scenarios/02_test_knowledge_proxy_tools/utils/knowledge_validation_helpers.sh:404 else-after-return`
- `kaldi-asr/kaldi egs/lre07/v1/local/split_long_utts.sh:54 useless-cat`
- `scikit-hep/awkward studies/type-parser-1.5/generated_parser.py:1960 else-after-return`
- `spyder-ide/spyder external-deps/qtconsole/qtconsole/frontend_widget.py:58 else-after-return`
- `nektos/act pkg/runner/step_action_remote.go:292 if-else-returns-boolean`
- `awslabs/mcp src/aws-healthomics-mcp-server/awslabs/aws_healthomics_mcp_server/tools/ecr_tools.py:2737 else-after-return`
- `scikit-hep/awkward src/awkward/_nplikes/jax.py:172 else-after-return`
- `nodejs/node deps/cares/config/ltmain.sh:1021 legacy-backticks`
- `n8n-io/n8n packages/nodes-base/nodes/Google/BigQuery/v2/transport/index.ts:60 else-after-return`
- `opencart/opencart upload/extension/opencart/catalog/controller/module/featured.php:88 else-after-return`
- `tower-rs/tower-http tower-http/src/compression_utils.rs:85 trailing-return`
- `lavabit/robox scripts/fedora29/tuning.sh:22 legacy-backticks`
- `Notsfsssf/pixez-flutter plugins/rhttp/rhttp/rust/third_party/reqwest/src/tls.rs:222 redundant-closure`
- `apollographql/router apollo-federation/src/composition/satisfiability/validation_traversal.rs:251 else-after-return`
- `ansible/ansible test/integration/targets/plugin_filtering/runme.sh:71 redundant-exit-status-check`
- `kaldi-asr/kaldi egs/babel/s5d/local/best_scores_kws.sh:161 useless-cat`
- `scikit-hep/awkward src/awkward/highlevel.py:130 else-after-return`
- `kaldi-asr/kaldi egs/mandarin_bn_bc/s5/local/tdt_mandarin_data_prep_filter.sh:23 useless-cat`
- `xiaoyaDev/xiaoya-alist base/base.sh:499 else-after-return`
- `pbakaus/impeccable crates/context/src/concept_seed.rs:329 redundant-closure`
- `scikit-hep/awkward src/awkward/contents/bytemaskedarray.py:643 else-after-return`
- `acmesh-official/acme.sh dnsapi/dns_one.sh:228 else-after-return`
- `kaldi-asr/kaldi egs/mandarin_bn_bc/s5/run.sh:127 legacy-backticks`
- `lollipopkit/flutter_server_box crates/sbm_ffi/src/frb_generated.rs:9782 else-after-return`
- `scikit-hep/awkward studies/rpad.py:919 else-after-return`
- `FuelLabs/sway test/src/snapshot/mod.rs:374 else-after-return`
- `FTXInfotech/laravel-adminpanel public/js/plugins/table/plugin.js:8117 else-after-return`
- `scikit-hep/awkward studies/rpad.py:922 else-after-return`
- `huifer/WellAlly-health scripts/test-family-health.sh:29 else-after-return`
- `bitnami/containers bitnami/deepspeed/0/debian-12/prebuildfs/opt/bitnami/scripts/libvalidations.sh:187 else-after-return`
- `woniuzfb/aios aios.sh:38833 legacy-backticks`
- `sass/dart-sass lib/src/functions/color.dart:943 else-after-return`
- `xonsh/xonsh xonsh/dirstack.py:248 else-after-return`
- `Dicklesworthstone/agentic_coding_flywheel_setup scripts/lib/user.sh:794 else-after-return`
- `ionicfirebaseapp/getwidget lib/components/carousel/gf_carousel.dart:280 else-after-return`
- `rustdesk/rustdesk flutter/lib/common.dart:3135 else-after-return`

### Orphan files

Seed `450845187fcf2345`; 100 of 97613 Findings.

- `grokability/snipe-it resources/lang/af-ZA/tooltips.php unreferenced`
- `grokability/snipe-it resources/lang/fr-FR/admin/accessories/table.php unreferenced`
- `kody-hq/kody packages/worker/start/admin-read-input.ts test-only`
- `codebymitch/TitanBot src/utils/commandPipeline.js unreferenced`
- `n8n-io/n8n packages/nodes-base/nodes/Postgres/v2/methods/credentialTest.ts test-only`
- `nodejs/node benchmark/crypto/randomInt.js unreferenced`
- `mem0ai/mem0 mem0-ts/src/oss/src/vector_stores/pinecone.ts test-only`
- `elie222/rakazo apps/mobile/components/inbox/unread-jump-overlay.tsx test-only`
- `stablyai/orca mobile/src/agent-history/MobileAgentSessionHistoryList.tsx unreferenced`
- `stablyai/orca src/renderer/src/components/pull-request-page/checks/checks-tab-loading.fixture.tsx unreferenced`
- `stablyai/orca src/renderer/src/components/tab-bar/menu-fit/new-tab-menu-fit-plan.ts unreferenced`
- `stablyai/orca src/renderer/src/app-shell/chrome/desktop-window-chrome.ts test-only`
- `nodejs/node deps/v8/tools/clusterfuzz/js_fuzzer/test_data/mjsunit/test_load.js unreferenced`
- `grokability/snipe-it resources/lang/ro-RO/admin/statuslabels/message.php unreferenced`
- `n8n-io/n8n packages/nodes-base/nodes/WhatsApp/GenericFunctions.ts test-only`
- `QuantumNous/new-api web/src/features/system-settings/models/model-ratio-visual-editor.tsx test-only`
- `kody-hq/kody packages/worker/start/mockups/examples/delete-file-dialog.mockup.tsx unreferenced`
- `stablyai/orca src/main/daemon/providers/adapter/daemon-attach-only-retirement.ts test-only`
- `morluto/rea src/domain/internalInvariant.ts test-only`
- `react/react packages/react-server-dom-parcel/npm/static.edge.js unreferenced`
- `SolidInvoice/SolidInvoice src/PaymentBundle/Form/Methods/KlarnaInvoice.php unreferenced`
- `modem-dev/hunk packages/hunk/src/ui/syntax/documentHighlightService.ts test-only`
- `grokability/snipe-it resources/lang/ar-SA/admin/maintenances/message.php unreferenced`
- `stablyai/orca src/main/runtime/rpc/methods/bots-composio-events.ts test-only`
- `aureuserp/aureuserp plugins/webkul/employees/database/migrations/2024_12_16_070029_create_employees_employee_resumes_table.php unreferenced`
- `earendil-works/pi packages/ai/catalog/providers/kimi-coding.ts unreferenced`
- `stablyai/orca src/main/runtime/session-lifecycle/close/close-mobile-row-in-main-layout.ts test-only`
- `stablyai/orca src/main/native-chat/agent-session-wire/options/structured-agent-session-options-read.ts test-only`
- `morluto/rea src/dotnet/ManagedPeMetadata.fixture.ts test-only`
- `awslabs/mcp src/aws-network-mcp-server/awslabs/aws_network_mcp_server/utils/formatters.py test-only`
- `aureuserp/aureuserp plugins/webkul/employees/resources/lang/en/enums/calendar-display-type.php unreferenced`
- `aureuserp/aureuserp plugins/webkul/recruitments/resources/lang/ar/filament/clusters/configurations/resources/utm-source.php unreferenced`
- `stablyai/orca src/main/devin/hook-config-json.ts test-only`
- `krayin/laravel-crm packages/Webkul/Admin/src/Resources/views/dashboard/index/total-leads.blade.php unreferenced`
- `kody-hq/kody packages/worker/client/search-filter.ts test-only`
- `stablyai/orca src/shared/bots/bot-look-around-every-label.ts unreferenced`
- `coollabsio/coolify app/Livewire/Security/CloudInitScriptForm.php test-only`
- `stablyai/orca mobile/src/new-workspace/creation/created-worktree-setup-review.ts unreferenced`
- `stablyai/orca mobile/src/terminal/live-input/use-terminal-text-field-submit-binding.ts unreferenced`
- `stablyai/orca src/main/runtime/terminal/wait/terminal-interactive-wait.ts test-only`
- `stablyai/orca src/renderer/src/store/slices/runtime-status/runtime-environment-status-retention.ts test-only`
- `SolidInvoice/SolidInvoice src/SaasBundle/Resources/config/routing.php unreferenced`
- `stablyai/orca src/main/warp-themes/theme-source-selection.ts unreferenced`
- `matomo-org/matomo plugins/AIProviders/vue/src/ManageAIProviders.vue unreferenced`
- `stablyai/orca src/renderer/src/components/bots/bot-automation-open.ts test-only`
- `n8n-io/n8n packages/nodes-base/nodes/Affinity/PersonDescription.ts unreferenced`
- `Leantime/leantime app/Core/Auth/OAuth/OAuthTokenStore.php test-only`
- `n8n-io/n8n packages/nodes-base/nodes/Switch/V2/SwitchV2.node.ts test-only`
- `nodejs/node lib/net.js test-only`
- `Sylius/Sylius src/Sylius/Bundle/PaymentBundle/Processor/HttpResponseProcessorInterface.php unreferenced`
- `kody-hq/kody packages/worker/start/pages/account-jobs-detail.tsx test-only`
- `stablyai/orca src/main/runtime/orchestration/mailbox/senders/agent-message-relation.ts test-only`
- `aureuserp/aureuserp plugins/webkul/accounts/src/Filament/Resources/CashRoundingResource/Schemas/CashRoundingForm.php unreferenced`
- `stablyai/orca src/shared/ai-vault/scanning/ai-vault-scan-issue-copy.ts test-only`
- `stablyai/orca src/main/rate-limits/providers/claude/claude-oauth-usage-error.ts test-only`
- `grokability/snipe-it resources/lang/cs-CZ/admin/settings/table.php unreferenced`
- `dart-lang/build build_test/lib/src/resolve_source.dart test-only`
- `aureuserp/aureuserp plugins/webkul/time-off/resources/lang/es/filament/pages/dashboard.php unreferenced`
- `stablyai/orca src/renderer/src/components/dashboard/DashboardAgentRow.tsx unreferenced`
- `stablyai/orca src/shared/source-control/ai/generation/commit-message-agent-specs-primary.ts test-only`
- `nodejs/node benchmark/webstreams/creation.js unreferenced`
- `stablyai/orca src/renderer/src/components/plugins/review/PluginUpdateCandidate.tsx unreferenced`
- `stablyai/orca src/shared/workspace-layout/persistence/workspace-layout-load-report.ts test-only`
- `n8n-io/n8n packages/nodes-base/nodes/Airtable/v2/actions/record/create.operation.ts test-only`
- `stablyai/orca src/main/ipc/filesystem/search/filesystem-search-file-paths.ts test-only`
- `loco-rs/loco evals/tasks/nightly-digest/reference/src/tasks/signup_digest.rs unreferenced`
- `nodejs/node benchmark/zlib/inflate.js unreferenced`
- `grokability/snipe-it resources/lang/ga-IE/admin/companies/table.php unreferenced`
- `grokability/snipe-it resources/lang/th-TH/admin/locations/table.php unreferenced`
- `coollabsio/coolify resources/views/livewire/project/service/stack-form.blade.php unreferenced`
- `stablyai/orca src/renderer/src/store/workspaces/host-isolation/host-twin.test-fixture.ts unreferenced`
- `vercel/next.js packages/next/src/server/lib/params-utils.ts test-only`
- `unicity-sphere/sphere src/modules/bridge/assets/evm-usdc/index.ts unreferenced`
- `aureuserp/aureuserp plugins/webkul/inventories/resources/lang/es/system.php unreferenced`
- `pbakaus/impeccable .vibe/skills/impeccable/scripts/live-browser-dom.js unreferenced`
- `langchain-ai/langchain libs/langchain/langchain_classic/tools/edenai/image_explicitcontent.py unreferenced`
- `grokability/snipe-it resources/lang/en-ID/admin/labels/table.php unreferenced`
- `n8n-io/n8n packages/nodes-base/nodes/Aws/S3/V2/AwsS3V2.node.ts test-only`
- `stablyai/orca mobile/src/source-control/review/pull-request/checks/pr-check-observation-contract.ts unreferenced`
- `Kuaishou-OneRec/OpenOneRec verl_rl/verl/interactions/utils/interaction_registry.py unreferenced`
- `octobercms/october modules/system/models/maillayout/_field_preview.php unreferenced`
- `n8n-io/n8n packages/nodes-base/nodes/LoneScale/descriptions/index.ts test-only`
- `n8n-io/n8n packages/nodes-base/nodes/Google/Analytics/v2/actions/report/get.ga4.operation.ts unreferenced`
- `aureuserp/aureuserp plugins/webkul/partners/resources/lang/es/filament/resources/partner/pages/list-partners.php unreferenced`
- `n8n-io/n8n packages/nodes-base/nodes/Kafka/KafkaTrigger.node.ts test-only`
- `stablyai/orca src/main/persistence/restoring-sessions/cross-host-pane-tab-ids.ts test-only`
- `grokability/snipe-it resources/views/partials/forms/edit/fax.blade.php unreferenced`
- `stablyai/orca src/main/claude/agent-view/agent-view-config-lock.ts test-only`
- `mem0ai/mem0 integrations/n8n-nodes-mem0/gulpfile.js unreferenced`
- `louislam/uptime-kuma server/notification-providers/opsimate.js test-only`
- `stablyai/orca src/main/workspaces/cleanup/workspace-cleanup-removal-step.ts unreferenced`
- `grokability/snipe-it resources/lang/tr-TR/admin/statuslabels/table.php unreferenced`
- `Sylius/Sylius src/Sylius/Bundle/ApiBundle/Checker/AppliedCouponEligibilityCheckerInterface.php test-only`
- `morluto/rea src/application/ClientRegistrationStatus.ts test-only`
- `stablyai/orca src/renderer/src/lib/pane-manager/terminal-scroll/pane-split-scroll.ts unreferenced`
- `openai/codex codex-rs/exec-server/testing/wine_exec_server.rs unreferenced`
- `langchain-ai/langchain libs/langchain/langchain_classic/callbacks/streamlit/__init__.py unreferenced`
- `stablyai/orca src/renderer/src/components/bots/items/BotItemsFilter.fixture.tsx unreferenced`
- `morluto/rea src/vsix/VsixPackageProvider.ts test-only`
- `OpenBMB/VisRAG src/rsgrpo/verl/trainer/config.py unreferenced`
