---
status: superseded by ADR 0035
---

# PHP Reference ranges come from a panel of 20 public repositories

The Owner approved running the calibrated PHP panel on 2026-11-22 using the pre-2023 human-written selection rule established in [ADR 0008](./0008-typescript-reference-ranges.md).

## Context

A Reference range must describe human-written code, or it cannot show how far agent-written code drifts from it.
This record applies the same selection rule and measurement method as ADR 0008 to 20 public PHP repositories.

### Selection rule and run

- Query on 2026-11-22: `language:PHP created:<2022-01-01 fork:false archived:false`, sorted by stars descending.
- Take the first 20 repositories passing the ADR 0008 exclusions (no forks/archived/lists/tutorials/books/collections, >= 5,000 Production source lines, Generated + Vendored <= Production, and `codeint check` exits 0).
- Pin each repository at the last commit on its default branch dated before `2023-01-01`.
- Measured with `codeint 0.2.1` release build at commit `ac42e2f` (default clone size 75 tokens, filter 14 distinct normalized tokens) on Apple Silicon.
- Run time across the 20 repositories: 0.014s to 58.235s per repository (76.3s total).

### Exclusions

- `danielmiessler/SecLists`: topic/description (collection of).
- `guzzle/guzzle`: 3542 Production lines (< 5,000).
- `PHPMailer/PHPMailer`: 4939 Production lines (< 5,000).
- `DesignPatternsPHP/DesignPatternsPHP`: 2018 Production lines (< 5,000).
- `fruitcake/laravel-debugbar`: 4080 Production lines (< 5,000).
- `BookStackApp/BookStack`: topic/description (book).
- `easychen/howto-make-more-money`: 515 Production lines (< 5,000).

### Panel

| Repository | Stars band | Pinned commit | Production lines |
| ---------- | ---------- | ------------- | ---------------- |
| `coollabsio/coolify` | 50k to 100k | `7d892bb19dd73838648ed9ea2e75402790ae90f0` | 19,730 |
| `nextcloud/server` | 25k to 50k | `e4e20bf40ad8ca139655b36a6efa2b1710ae50f0` | 403,544 |
| `laravel/framework` | 25k to 50k | `7797de80132599971fdaf03f6c0bfd62b5ae15d8` | 86,155 |
| `filamentphp/filament` | 25k to 50k | `40ede43905d9346404bad8867da888b799e4e3a3` | 54,436 |
| `symfony/symfony` | 25k to 50k | `fc502bec69586c4c5db31d126d4bc2506bae40f3` | 1,052,486 |
| `composer/composer` | 25k to 50k | `f07614f1c948a7b961b2da3b91506716a8ba8124` | 39,601 |
| `bagisto/bagisto` | 25k to 50k | `7a5b52e6d4d46a71281c9a7d1a9581617d91b53d` | 107,697 |
| `monicahq/monica` | 25k to 50k | `80934104eb26b98b7fe6ec6f6dce6ee3b90007a6` | 36,084 |
| `firefly-iii/firefly-iii` | 25k to 50k | `15605dd1633d7b1e014e7f44fbb945faf8755f8c` | 238,729 |
| `krayin/laravel-crm` | 10k to 25k | `d2f4a8083867dee2b6c111932dea54356fa054d7` | 21,839 |
| `livewire/livewire` | 10k to 25k | `0ce8661880f96bcc0cc8bd17933dbd3fd47953bb` | 20,523 |
| `matomo-org/matomo` | 10k to 25k | `583a3f403f39f0bd7be6d83335f6a50d4d23a1dd` | 270,684 |
| `WordPress/WordPress` | 10k to 25k | `0908abdfc694c4d9a9b3dc2bd0ad2ebbad5ca51f` | 416,143 |
| `Seldaek/monolog` | 10k to 25k | `1387e02612584ffa1a9e93384d2d63ba0a747e11` | 7,376 |
| `sebastianbergmann/phpunit` | 10k to 25k | `0f420c1babd1dec8162588845687da2df2175d63` | 37,161 |
| `bcit-ci/CodeIgniter` | 10k to 25k | `7eda20fae113e9fe6cb52c218d111223f059c5e7` | 28,643 |
| `nikic/PHP-Parser` | 10k to 25k | `8ad412944259c8028993e7708efc796040e890d5` | 16,326 |
| `koel/koel` | 10k to 25k | `545a303e0a3eca6a10909cdda73dd04264683727` | 14,706 |
| `FreshRSS/FreshRSS` | 10k to 25k | `2850c0baea89a39ee090f207197c2ed5390254a7` | 65,960 |
| `briannesbitt/Carbon` | 10k to 25k | `fea280e2943fee6f6bea93593f4ea5f0d5adc69b` | 26,521 |

## Measurements

### Per Metric, over the 20 repositories

| Metric | Mean | Median | Standard deviation | 10th percentile | 90th percentile | Lowest | Highest |
| ------ | ---- | ------ | ------------------ | --------------- | --------------- | ------ | ------- |
| Erosion | 0.369 | 0.358 | 0.221 | 0.055 | 0.677 | 0.036 | 0.774 |
| Verbosity | 0.140 | 0.096 | 0.097 | 0.042 | 0.292 | 0.025 | 0.362 |
| Duplication | 0.139 | 0.093 | 0.097 | 0.041 | 0.290 | 0.025 | 0.360 |
| Import cycles | 0.032 | 0.011 | 0.046 | 0.000 | 0.107 | 0.000 | 0.175 |

### Per repository, in the Production Source set

| Repository | Erosion | Verbosity | Duplication | Import cycles |
| ---------- | ------- | --------- | ----------- | ------------- |
| `coollabsio/coolify` | 0.675 | 0.362 | 0.360 | 0.048 |
| `nextcloud/server` | 0.353 | 0.087 | 0.083 | 0.010 |
| `laravel/framework` | 0.134 | 0.081 | 0.080 | 0.036 |
| `filamentphp/filament` | 0.132 | 0.126 | 0.126 | 0.006 |
| `symfony/symfony` | 0.504 | 0.025 | 0.025 | 0.016 |
| `composer/composer` | 0.691 | 0.043 | 0.043 | 0.175 |
| `bagisto/bagisto` | 0.216 | 0.081 | 0.079 | 0.042 |
| `monicahq/monica` | 0.057 | 0.337 | 0.337 | 0.011 |
| `firefly-iii/firefly-iii` | 0.312 | 0.197 | 0.197 | 0.012 |
| `krayin/laravel-crm` | 0.358 | 0.190 | 0.182 | 0.003 |
| `livewire/livewire` | 0.043 | 0.078 | 0.077 | 0.034 |
| `matomo-org/matomo` | 0.406 | 0.082 | 0.079 | 0.105 |
| `WordPress/WordPress` | 0.666 | 0.164 | 0.162 | 0.000 |
| `Seldaek/monolog` | 0.226 | 0.058 | 0.057 | 0.000 |
| `sebastianbergmann/phpunit` | 0.504 | 0.235 | 0.235 | 0.000 |
| `bcit-ci/CodeIgniter` | 0.493 | 0.155 | 0.155 | 0.000 |
| `nikic/PHP-Parser` | 0.359 | 0.287 | 0.284 | 0.007 |
| `koel/koel` | 0.036 | 0.026 | 0.025 | 0.119 |
| `FreshRSS/FreshRSS` | 0.774 | 0.103 | 0.097 | 0.000 |
| `briannesbitt/Carbon` | 0.433 | 0.088 | 0.088 | 0.008 |

## Decision

**The PHP Language plugin shows, for each Metric, the 10th to 90th percentile of this panel as its Reference range.**
The source label reads `codeint panel of 20 PHP repositories, ADR 0011, codeint 0.2.1`.

| Metric | Reference range |
| ------ | --------------- |
| Erosion | 0.055 to 0.677 |
| Verbosity | 0.042 to 0.292 |
| Duplication | 0.041 to 0.290 |
| Import cycles | 0.000 to 0.107 |

## Consequences

- PHP reports show a Reference range next to each Metric with the source label above.
- No code, fixture, or text of a panel repository enters codeint; this record holds only names, commits, and numbers.
- Any future change to default Metric rules or Source set rules requires re-measuring the panel and amending this record.
