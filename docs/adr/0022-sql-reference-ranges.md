---
status: superseded by ADR 0035
---

# SQL Reference ranges come from a panel of 20 public repositories

This record establishes calibrated SQL Reference ranges for Spec #224 (Tickets #390 to #393) using the pre-2023 human-written selection rule from [ADR 0008](./0008-typescript-reference-ranges.md), adapted for SQL repositories.

## Context

A Reference range must describe human-written code, or it cannot show how far agent-written code drifts from it.
This record applies the selection rule and measurement method of [ADR 0008](./0008-typescript-reference-ranges.md) to 20 public SQL repositories.

### SQL adaptations to ADR 0008

Standalone SQL repositories differ from application repositories in TypeScript, Rust, or Go in four ways that require pre-committed adaptations (committed before any `codeint` measurement and approved by the Owner with this record):

1. **Combined GitHub language query (`language:SQL language:PLpgSQL`)**: GitHub Linguist splits `.sql` repositories across `SQL`, `PLpgSQL`, `PLSQL`, `TSQL`, and `SQLPL`.
   `PLSQL`, `TSQL`, and `SQLPL` target proprietary Oracle, SQL Server, and Db2 dialects outside Spec #224's Dialect scope.
   Meanwhile, `language:SQL` alone returns only 875 pre-2022 public repositories because GitHub Linguist classifies most procedural `.sql` repositories (including both PostgreSQL and MySQL repositories) under `PLpgSQL` (19,542 repositories).
   Combining `language:SQL language:PLpgSQL` walks all 20,417 matching repositories in one strict descending star order.
   DuckDB adds nothing to a pre-2023 panel, and a verification query confirmed zero standalone pre-2023 SQLite-primary SQL repositories above 100 stars because SQLite `.sql` files live inside host-language applications.
2. **Minimum Production SQL source-line threshold (`>= 1,000` SQL Production source lines)**: ADR 0008 and [ADR 0021](./0021-go-reference-ranges.md) used a 5,000-line floor for full application codebases.
   Standalone hand-written SQL extensions, DBA toolkits, and schemas are much more compact (typically 1,300 to 4,500 Production source lines).
   A 1,000-line SQL Production floor excludes single-file scripts and tiny helper extensions while retaining real SQL libraries and schemas.
3. **SQL-specific content exclusions**: in addition to ADR 0008's exclusions (forks, mirrors, archived repositories, lists, books, courses, tutorials, challenges, snippets, Generated plus Vendored exceeding Production, and non-zero exit), we exclude pure database dumps (`pg_dump`, `mysqldump`, `Navicat Premium Data Transfer`), pure sample databases and static reference datasets (`Pagila`, `Sakila`, `AdventureWorks`, regional lookup tables, word dictionaries), test-fixture-only repositories (`pg_regress` and grammar fixture suites), templated-only dbt packages, and proprietary out-of-scope dialect or embedded-language wrappers (Redshift UDFs, PLV8 JavaScript wrappers).
4. **Sub-5k star bands**: all qualifying SQL repositories sit below 5,000 stars, so this record lists finer bands (`1k to 5k`, `500 to 1k`, `250 to 500`, and `100 to 250`) for transparency.

### Selection rule and run

- Query on 2026-11-24: `language:SQL language:PLpgSQL created:<2022-01-01 fork:false archived:false`, sorted by stars descending.
- Walk the result list in strict descending star order and take the first 20 repositories passing every rule above: at least 1,000 SQL Production source lines, Generated plus Vendored SQL source lines no larger than Production source lines, and `codeint check` exits with code 0.
- Pin each repository at the last commit on its default branch dated before `2023-01-01` by committer date.
- Shallow-clone each pinned commit into a temporary scratch folder outside codeint, write a temporary `codeint.json` mapping `**/*.sql` to `postgresql` or `mysql` only when the repository targets that Dialect (leaving generic SQL repositories unconfigured), measure in place with `/usr/bin/time -l codeint check --format json --all`, never build or run any panel repository, and delete the clone immediately afterward.
- On multi-language repositories, read the `languages.sql` section so every count and Metric ratio measures SQL code only.
- Measured with `codeint 0.6.0` release build (commit `07ffb95`, default minimum clone size 75 tokens, clone filter 14 distinct normalized tokens) on Apple Silicon.
- Every percentile interpolates linearly between the two nearest ranked values (`idx = p * (n - 1)`), and standard deviation is the sample standard deviation (`n - 1`).
- Run time across the 20 panel repositories: `0.00s` to `1.29s` per repository (`3.25s` total across 1,978 Production SQL files, 2,213 total SQL files, and 427,252 SQL Production source lines).
  Across the 20 repositories, codeint skipped 1 templated SQL file (`basedosdados/sdk`) and 4 unparsable or non-UTF-8 SQL files (`aquametalabs/aquameta` 1, `OHDSI/Vocabulary-v5.0` 2, `omniti-labs/pg_jobmon` 1), and listed 3,647 rejected statements (which still counted in every token-layer Metric).
- Largest panel repository by SQL Production source lines: `OHDSI/Vocabulary-v5.0` (480 Production SQL files, 127,897 SQL Production source lines), which completed in `0.31s` wall time with `603,848,704` bytes (`575.9 MiB`) maximum resident set size (`568.6 MiB` peak memory footprint).
  The second largest repository by SQL Production source lines, `pgpartman/pg_partman` (123 Production SQL files, 126,807 SQL Production source lines), recorded the highest peak memory across the panel at `799,571,968` bytes (`762.5 MiB`) maximum resident set size (`672.4 MiB` peak memory footprint) in `0.91s` wall time.

### Exclusions

Walking search ranks 1 through 92 in strict descending star order excluded 72 repositories before completing the 20-repository panel: 70 during pre-screening and 2 during `codeint check` measurement.

Pre-screening excluded 70 repositories across ranks 1 to 92:

- 35 repositories with fewer than 1,000 SQL Production source lines (or 0 non-test `.sql` files).
- 14 sample databases, static reference datasets, or benchmark data dumps (such as `datacharmer/test_db`, `devrimgunduz/pagila`, `jOOQ/sakila`, `lorint/AdventureWorks-for-Postgres`, and regional `INSERT` datasets).
- 8 generated dumps, build outputs, or vendored SQL repositories where Generated plus Vendored SQL exceeded Production SQL.
- 8 books, courses, coding challenges, interview question banks, guides, or snippet collections.
- 4 test-fixture-only repositories (`darold/pgFormatter`, `pgpointcloud/pointcloud`, `pgspider/sqlite_fdw`, and `pgspider/parquet_s3_fdw`).
- 1 mirror (`postgis/postgis`).

During `codeint check` measurement, 2 borderline pre-screened candidates failed the 1,000-line SQL Production floor:

| Rank | Repository | Stars band | Exclusion reason |
| ---- | ---------- | ---------- | ---------------- |
| 8 | `NixOS/hydra` | 1k to 5k | 986 SQL Production source lines (< 1,000) |
| 57 | `mapbox/postgis-vt-util` | 250 to 500 | 634 SQL Production source lines (< 1,000) |

After fixing the bare-CR defect described below, `codeint check` exited with code 0 on every measured candidate.

### Panel

| Repository | Stars band | Pinned commit | Committer date (UTC) | Dialect | SQL files | Production lines |
| ---------- | ---------- | ------------- | -------------------- | ------- | --------- | ---------------- |
| `openmaptiles/openmaptiles` | 1k to 5k | `c4d86d44a661146c273b97ac294db2381a570b2c` | `2022-11-28T18:19:29Z` | `postgresql` | 54 | 8,257 |
| `pgpartman/pg_partman` | 1k to 5k | `7a159d5d60bb8a0737741bd63efd19f84d952048` | `2022-12-16T20:18:31Z` | `postgresql` | 123 | 126,807 |
| `NikolayS/postgres_dba` | 1k to 5k | `1fc49179383a4fb1568e6cfedb6a99c77cb094c6` | `2022-09-04T04:03:50Z` | `postgresql` | 26 | 2,041 |
| `theory/pgtap` | 1k to 5k | `ff4a30b59d602ab7a3ea48ce91402fa9bc83abdf` | `2022-05-21T23:19:54Z` | `postgresql` | 9 | 4,271 |
| `aquametalabs/aquameta` | 1k to 5k | `66f674c2e01ea2a79774b1492b86c1dcdcfa4e16` | `2022-05-10T04:17:57Z` | `postgresql` | 37 | 14,216 |
| `CrunchyData/pgmonitor` | 500 to 1k | `da0824c7486c2f5c1b5fd4065ccfea5e65ce9f2f` | `2022-11-17T15:03:56Z` | `postgresql` | 7 | 2,152 |
| `trickerer/Trinity-Bots` | 500 to 1k | `512308b343cba5620d5a50717f8324a51c4605a8` | `2022-12-31T01:50:18Z` | `mysql` | 122 | 10,657 |
| `3dcitydb/3dcitydb` | 250 to 500 | `5297efcd8d96f525569b9c715209988c3a4124eb` | `2022-12-22T13:59:40Z` | `postgresql` | 25 | 2,272 |
| `geocompass/pg-coordtransform` | 250 to 500 | `e5cf4ab0aea0b9803698d9e394360f8ee56c744e` | `2021-02-02T03:13:00Z` | `postgresql` | 53 | 2,117 |
| `Abstrct/Schemaverse` | 250 to 500 | `1ccd9284def20804a784c059d4cbb14840097f2b` | `2017-10-13T16:50:02Z` | `postgresql` | 297 | 3,815 |
| `basedosdados/sdk` | 250 to 500 | `91a942f56bd94d0f101c4e94b54f92bde0bc48e8` | `2022-12-30T16:59:52Z` | `generic` | 440 | 16,175 |
| `pgq/pgq` | 250 to 500 | `ecedef02bde77aa9b09deb1f22386b048e24905c` | `2022-08-09T12:38:50Z` | `postgresql` | 73 | 3,223 |
| `pgMemento/pgMemento` | 250 to 500 | `33c98951f04b3a03d8fd97ba7311d68e23ae3e66` | `2022-10-30T21:50:43Z` | `postgresql` | 19 | 6,725 |
| `postgis/h3-pg` | 250 to 500 | `686349f5506c2dd5d5f0bccbd18960af443b5da1` | `2022-12-01T13:01:48Z` | `postgresql` | 57 | 1,966 |
| `xocolatl/periods` | 250 to 500 | `971941730312d0ac855afcce4c2be2b1c94fe744` | `2022-10-24T14:05:53Z` | `postgresql` | 20 | 9,677 |
| `OHDSI/Vocabulary-v5.0` | 250 to 500 | `6b510f66070f82302ab20e415031eff58b3106cb` | `2022-12-29T13:30:41Z` | `postgresql` | 480 | 127,897 |
| `stac-utils/pgstac` | 100 to 250 | `da8d6935a6b50b1eeaf226f38d02ca126cc62034` | `2022-12-06T15:08:24Z` | `postgresql` | 82 | 77,472 |
| `denishpatel/pg-clone-schema` | 100 to 250 | `0fba663ea3ec7736a1bcad5d415a541e83e405b5` | `2022-12-23T11:49:17Z` | `postgresql` | 2 | 2,594 |
| `omniti-labs/pg_jobmon` | 100 to 250 | `b9d49e6d4603f2670b3a2d512c31fc7cd5e9a334` | `2022-05-26T18:44:20Z` | `postgresql` | 46 | 3,600 |
| `cybertec-postgresql/pg_permissions` | 100 to 250 | `5b3e79e9514d93f7e4d173ae6543b4cca389a811` | `2021-09-08T16:09:28Z` | `postgresql` | 6 | 1,318 |

The SQL files column counts Production SQL files in `languages.sql` (or top-level `source_sets.production` on SQL-only repositories).

## Measurements

### Per Metric, over the 20 repositories

| Metric | Mean | Median | Standard deviation | 10th percentile | 90th percentile | Lowest | Highest |
| ------ | ---- | ------ | ------------------ | --------------- | --------------- | ------ | ------- |
| Erosion | 0.601 | 0.689 | 0.347 | 0.094 | 0.976 | 0.000 | 0.997 |
| Verbosity | 0.521 | 0.466 | 0.346 | 0.042 | 0.921 | 0.013 | 0.950 |
| Duplication | 0.521 | 0.466 | 0.346 | 0.042 | 0.921 | 0.013 | 0.950 |

Import cycles is absent for SQL because SQL has no runtime imports, so Import cycles has no SQL measurement and no Reference range.

### Per repository, in the Production Source set

| Repository | Erosion | Verbosity | Duplication |
| ---------- | ------- | --------- | ----------- |
| `openmaptiles/openmaptiles` | 0.728 | 0.467 | 0.467 |
| `pgpartman/pg_partman` | 0.990 | 0.920 | 0.920 |
| `NikolayS/postgres_dba` | 0.937 | 0.208 | 0.208 |
| `theory/pgtap` | 0.339 | 0.465 | 0.465 |
| `aquametalabs/aquameta` | 0.449 | 0.931 | 0.931 |
| `CrunchyData/pgmonitor` | 0.338 | 0.887 | 0.887 |
| `trickerer/Trinity-Bots` | 0.997 | 0.293 | 0.293 |
| `3dcitydb/3dcitydb` | 0.114 | 0.013 | 0.013 |
| `geocompass/pg-coordtransform` | 0.000 | 0.917 | 0.917 |
| `Abstrct/Schemaverse` | 0.504 | 0.043 | 0.043 |
| `basedosdados/sdk` | 0.000 | 0.040 | 0.040 |
| `pgq/pgq` | 0.771 | 0.263 | 0.263 |
| `pgMemento/pgMemento` | 0.818 | 0.548 | 0.548 |
| `postgis/h3-pg` | 0.104 | 0.447 | 0.447 |
| `xocolatl/periods` | 0.945 | 0.878 | 0.878 |
| `OHDSI/Vocabulary-v5.0` | 0.858 | 0.276 | 0.276 |
| `stac-utils/pgstac` | 0.649 | 0.950 | 0.950 |
| `denishpatel/pg-clone-schema` | 0.975 | 0.175 | 0.175 |
| `omniti-labs/pg_jobmon` | 0.876 | 0.809 | 0.809 |
| `cybertec-postgresql/pg_permissions` | 0.640 | 0.892 | 0.892 |

### Distribution and outliers

No repository lies more than 1.5 interquartile ranges outside the middle half of the panel on any Metric.
The wide 10th to 90th percentile bands reflect real structural diversity in standalone SQL repositories:

- Simple declarative view suites and short math helpers (`basedosdados/sdk`, `geocompass/pg-coordtransform`) record `0.000` Erosion, while long monolithic PL/pgSQL and MySQL stored procedures (`trickerer/Trinity-Bots`, `pgpartman/pg_partman`, `denishpatel/pg-clone-schema`) concentrate almost all Function mass in high-complexity routines.
- PostgreSQL extensions that ship version-to-version upgrade scripts alongside base install scripts (`stac-utils/pgstac`, `aquametalabs/aquameta`, `pgpartman/pg_partman`, `cybertec-postgresql/pg_permissions`) naturally repeat full `CREATE OR REPLACE FUNCTION` and view definitions across upgrade files, driving Duplication and Verbosity above `0.85`.
- The two SQL Flagged line rules (`or-chain-collapses-to-in` and `having-without-aggregate`) matched 3 lines across the 427,252 Production SQL source lines (all in `OHDSI/Vocabulary-v5.0`), leaving Verbosity and Duplication equal at 3 decimal places on every repository.

### Defect fixed during calibration

During the initial measurement pass, candidate #18 (`OHDSI/Vocabulary-v5.0`) failed with exit code 2 (`HazardLineOutOfRange`) on `DPD/create_source_tables.sql`:

- **Bare CR line-ending drift (`plugins/languages/sql/src/tokenize.rs`)**: `DPD/create_source_tables.sql` mixes `\r\n` and bare `\r` bytes (80 `\n` lines under Rust `str::lines()`, plus 122 bare `\r` bytes).
  `normalize_source` converted every bare `\r` into `\n`, inflating the tokenizer's line numbers to 202 and causing a Hazard line at line 93 to exceed the file's 80 lines checked by `validate_hazard_line_bounds`.
  We updated `normalize_source` to replace `\r\n` with `\n` and bare `\r` with a space (matching `str::lines()` and the Go and Python lexers), added the synthetic regression test `bare_cr_bytes_preserve_line_bounds_matching_str_lines`, rebuilt the release binary (`07ffb95`), and re-measured all 20 panel repositories from scratch with that binary.

## Decision

**The SQL Language plugin shows, for Erosion, Verbosity, and Duplication, the 10th to 90th percentile of this panel as its Reference range.**
The source label reads `codeint panel of 20 SQL repositories, ADR 0022, codeint 0.6.0`.

| Metric | Reference range |
| ------ | --------------- |
| Erosion | 0.094 to 0.976 |
| Verbosity | 0.042 to 0.921 |
| Duplication | 0.042 to 0.921 |

- SQL publishes no Import cycles Reference range because SQL has no runtime imports and Import cycles is absent for SQL.
- The 10th to 90th percentile holds the middle 80% of the panel without removing any qualifying repository.

## Considered options

- **Keep ADR 0008's 5,000-line Production floor for SQL.**
  Rejected: most standalone hand-written SQL libraries, DBA toolkits, and extension schemas sit between 1,300 and 4,500 Production lines, so a 5,000-line floor would exclude them and bias the panel toward versioned migration archives.
- **Publish separate Reference ranges per SQL Dialect.**
  Rejected: Spec #224 specifies one calibrated Reference range for SQL, and pre-2023 public SQL repositories on GitHub do not provide 20 qualifying standalone repositories per Dialect for MySQL, SQLite, or DuckDB.
- **Publish `0.000 to 0.000` for SQL Import cycles.**
  Rejected: SQL has no imports and leaves Import cycles absent in every report, so publishing an Import cycles Reference range would contradict the absent-Metric rule.

## Consequences

- SQL reports show a Reference range next to Erosion, Verbosity, and Duplication with the source label above, and leave Import cycles absent.
- No code, fixture, or text of a panel repository enters codeint; this record holds only names, commits, and numbers.
- Any future change to default Metric rules, SQL statement or routine complexity rules, SQL Flagged line rules, or SQL Source set rules requires re-measuring the panel and amending this record.
- The Owner accepted this ADR and its SQL adaptations to [ADR 0008](./0008-typescript-reference-ranges.md) alongside PR #434.
