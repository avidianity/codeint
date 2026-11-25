# How to read the configuration

## Read codeint.json

```rust
use codeint_config::read_configuration;

let repository = Repository::discover(&folder)?;
let configuration = read_configuration(repository.root())?;
```

- `read_configuration` reads `codeint.json`, `codeint.jsonc`, `.codeint/config.json`, or `.codeint/config.jsonc` at the root of the repository, so pass the root of the working tree, never the folder the user named.
  All four files accept JSONC syntax (`//` line comments, `/* ... */` block comments, and trailing commas before `}` or `]`).
  Having more than one of those four files present fails with `ConfigurationFileError::AmbiguousConfigFiles` (exit code 2) naming every conflicting file.
- `read_resolved_configuration` runs the same merge pipeline with `ConfigOverrides` and returns both the merged `Configuration` and per-field `ResolvedSetting` entries with `ConfigProvenance` (`default`, `file`, or `cli`).
- It returns `Configuration::default()` when neither file is present; codeint runs with defaults.
- Every other failure is a `ConfigurationFileError`, which stops the run with exit code 2.
  Its message names the key with its path, such as `source_sets.test[1]`, and the expected JSON type.
- Only this crate reads `codeint.json`; every other crate receives the typed `Configuration`.
- `codeint diff` reads `codeint.json` of the working tree once and measures both the merge-base and the working tree with it, so a comparison shows changes in the code, not in the configuration.
  Never read `codeint.json` from a commit.

## Apply Source set overrides

```rust
use codeint_model::settle_source_set;

let mut model = plugins[0].lower(&path, text)?;
model.source_set = configuration
    .source_set_overrides
    .source_set(&path, settle_source_set(&path, text, model.source_set));
```

- Apply the overrides once per lowered file, last, on the Source set that the Language plugin and the core rules settled.
  An override wins over every rule.
- `source_set` returns the settled Source set unchanged when no override glob matches the path.
- Globs match the repository-relative path with forward slashes: `*` matches within one folder, `**` matches across folders.
- When globs of several Source sets match one file, the first of Vendored, Generated, Test, and Production wins.

## Read Budgets

```rust
use codeint_model::{Metric, SourceSet};

if let Some(budget) = configuration.budgets.budget(SourceSet::Production, Metric::Erosion) {
    let passes = budget.admits(displayed_ratio);
}
```

- `budget` returns `None` when `codeint.json` sets no Budget for that Metric in that Source set; that Metric then never fails.
- `production_test_gap` reads `budgets.production.test_gap` as `Option<f64>`, and `budget_for_key(SourceSet::Production, "test_gap")` returns the corresponding `Option<Budget>`.
- Setting `orphan_files` under `budgets.test`, `budgets.generated`, or `budgets.vendored` fails with `ConfigurationFileError::MetricBudgetOutsideProduction` (exit code 2), and setting `orphan_files` under `regression_limits.test`, `regression_limits.generated`, or `regression_limits.vendored` fails with `ConfigurationFileError::MetricRegressionLimitOutsideProduction` (exit code 2), because Orphan files applies to the Production Source set only.
- Setting `test_gap` under `budgets.test`, `budgets.generated`, or `budgets.vendored` fails with `ConfigurationFileError::TestGapBudgetOutsideProduction` (exit code 2), because Test gap covers only the Production Source set.
- Setting `test_gap` under `regression_limits` (top-level or under any Source set) fails with `ConfigurationFileError::TestGapRegressionLimit` (exit code 2), because Test gap has no Regression check.
- `dependency_budget` reads top-level Dependency hygiene Budgets (`unused_dependencies`, `unlisted_dependencies`, and `dev_in_production_dependencies`).
- A Budget applies to the total of every Language plugin; the report never checks the section of one Language plugin against it.
- A Budget is a ratio from 0 to 1; the shape check rejects any other number with exit code 2.
- `admits` passes a ratio equal to the Budget.
  Pass the ratio rounded as the report shows it, so that a ratio the user sees as equal to its Budget passes.
- `configuration.new_file_budgets` uses the same `Budgets` type for per-Source-set ceilings enforced on newly added files in `codeint diff`.
  It rejects repository-wide Dependency hygiene keys, unknown keys, and Production-only keys (`orphan_files`, `test_gap`) outside `production` with exit code 2, and `is_empty()` returns `true` when unset.

## Read Regression limits

```rust
use codeint_config::NewFindingsRule;
use codeint_model::{Metric, SourceSet};

let limit = configuration
    .regression_limits
    .limit(SourceSet::Production, Metric::Erosion);
let rise_passes = limit.ratio_rise.admits(displayed_head_ratio_minus_displayed_base_ratio);
let any_new_finding_fails = limit.new_findings == NewFindingsRule::AnyNewFinding;
```

- `limit` always returns a `RegressionLimit`; without a setting it uses `RegressionLimit::default_for(metric)` (`NewFindingsRule::NumeratorRise` for Erosion, Cognitive complexity, Duplication, Verbosity, and external Metrics; `NewFindingsRule::AnyNewFinding` for Import cycles, Boundaries, Orphan files, Hazards, and Dependency hygiene).
- `codeint.json` sets a limit under `regression_limits.{metric}` for every Source set, and under `regression_limits.{source_set}.{metric}` for one Source set.
  `limit` settles each of `ratio_rise` and `new_findings` on its own: the Source set setting wins, then the Metric setting, then the default.
- `RatioRise::DEFAULT` is 0.01, and `admits` passes a rise equal to the limit.
  Round the difference of the two displayed ratios to four places before you pass it, because the plain floating-point difference of 0.0142 and 0.0042 is above 0.01.
- `NewFindingsRule::AnyNewFinding` fails on any new Finding or numerator rise; `NewFindingsRule::NumeratorRise` fails whenever the numerator rises (rounded to four decimal places), so a Finding that only moves passes; `NewFindingsRule::Never` never fails on new Findings or numerator rises, so only `ratio_rise` can still make a Regression.
  In `codeint.json` the canonical values are `any_new_finding`, `numerator_rise`, and `never`, and `numerator_rise_with_new_findings` is accepted as a deprecated alias for `numerator_rise`.
- `regression_limits.per_file()` defaults to `false`; when `true`, `codeint diff` also compares file-level Metric numerators and Findings for every modified file and fails if any modified file worsens (respecting `NewFindingsRule::Never`).

## Limit each Language plugin to its folders

```rust
let names: Vec<&str> = plugins.iter().map(|plugin| plugin.name).collect();
let coverage = configuration.languages.coverage_of(&names)?;
if !coverage[index].covers(&path) {
    continue;
}
```

- `coverage_of` takes the name of every Language plugin of the run and returns one `LanguageCoverage` per name, in the same order.
- It fails with `ConfigurationFileError::UnknownLanguagePlugin` when an entry of `languages` names a plugin outside that list; the message names the plugin, its key, and the known plugins.
- Without the `languages` key, every `LanguageCoverage` covers the whole repository.
  With it, a Language plugin covers only what its entries include; a plugin without an entry covers nothing.
- An entry covers a file when an `include` glob matches the file or a folder above it, and no `exclude` glob of that entry does.
  So `packages/web` covers every file below that folder.
- Several entries for one plugin add up: the plugin covers what any of its entries covers.
- Call `covers` before reading a file; a file that its Language plugin does not cover is not measured and not listed.

## Declare external Language plugins

```rust
for declaration in &configuration.plugins {
    let hex = declaration.sha256.to_hex();
}
```

- Each entry of `plugins` in `codeint.json` requires `name`, `sha256`, and either `path` or `url` (mutually exclusive).
- A missing `sha256` fails the shape check with `ConfigurationFileError::MissingKey` (exit code 2).
- A `sha256` that is not 64 hexadecimal characters fails with `ConfigurationFileError::InvalidSha256` (exit code 2).
- A `url` that does not use `https://` fails with `ConfigurationFileError::NonHttpsPluginUrl` (exit code 2).
- `plugins` is empty by default and omitted when serialized as empty, so `codeint init` leaves it out.

## Configure Duplication near-clone detection

- `configuration.duplication.near_clones` is `false` by default, keeping exact Clone groups as the default behavior.
- `configuration.duplication.min_similarity` is a validated `MinSimilarity` in `(0.0, 1.0]`, defaulting to `0.85`.
- Any `min_similarity` outside `(0.0, 1.0]` fails `read_configuration` with `ConfigurationFileError::OutOfExclusiveMinRange` (exit code 2).

## Read Coverage report paths

- `configuration.coverage_reports` holds the repository-relative paths declared under `coverage_reports` in configuration, and defaults to an empty list.
- Empty entries, absolute paths, and `..` parent traversal segments fail `read_configuration` with `ConfigurationFileError` (exit code 2) naming `coverage_reports[i]`.

## Read and update a Ratchet file

- `configuration.ratchet.path` holds the repository-relative Ratchet file path (`".codeint/ratchet.json"` by default) and rejects empty, absolute, and `..` paths with `ConfigurationFileError` (exit code 2).
- `Ratchet::exists` and `Ratchet::load_optional` verify repository containment (including symlinks) before reading, returning `Ok(None)` when the Ratchet file is absent.
- `Ratchet::check` compares each measured Metric's `metric_version` first (returning `Err(Vec<RatchetVersionMismatch>)`, an alias of `MetricVersionMismatch`, on any mismatch) and then checks each rounded 4-decimal ratio against its stored ceiling.
- `Ratchet::update` creates on first run, monotonically lowers ceilings on improvement, auto-refreshes version-changed Metrics, and refuses to loosen unchanged-version Metrics unless `reset` is `true`; `Ratchet::write_atomic` creates parent directories and writes via a sibling temporary file rename.

## Read and update the Baseline

```rust
use codeint_config::Baseline;

let baseline = Baseline::load_optional(repository.root())?;
let (updated, summary) = Baseline::update(baseline.as_ref(), &report.baseline_metrics(), reset);
updated.write_atomic(repository.root())?;
```

- The Baseline always lives at `BASELINE_PATH` (`.codeint/baseline.json`); `Baseline::load_optional` returns `Ok(None)` when it is absent and shares the Ratchet's repository-containment rules.
- `Baseline::is_baselined(source_set, metric, id)` says whether a Finding is hidden; pass `None` as the Source set for Dependency hygiene Metrics.
- `Baseline::version_mismatches` returns a `MetricVersionMismatch` for each measured Metric whose stored Metric version differs.
- `Baseline::update` records every Finding on the first run, later only removes fixed Findings (counting new ones in `skipped_new`), refreshes Metrics whose Metric version changed, replaces every measured Metric's Findings when `reset` is `true`, and keeps entries of unmeasured Metrics.

## Add a setting

- Add a field to `ConfigurationFile` in `src/codeint_json/schema.rs`, with `#[serde(default)]` and a `#[schemars(description = ...)]` that tells the user what it does.
- Convert it into a typed field of `Configuration`; never hand the raw JSON to another crate.
- Run `cargo test -p codeint-config`.
  The schema test rewrites `schema/codeint.schema.json` when it drifts from the types and fails once; review and commit the rewritten file.
- The shape check reads the generated schema to name the bad key and the expected type.
  It also names a key that the schema lists under `required` and the file leaves out.
  When a new type makes schemars write a keyword the shape check does not understand, `the_shape_check_understands_every_keyword_of_the_configuration_schema` fails; teach `src/codeint_json/shape.rs` that keyword first.

## Write a starter codeint.jsonc

```rust
use codeint_config::{DetectedLanguage, StarterBoundaries, write_starter_configuration};

let languages = vec![DetectedLanguage {
    plugin: "typescript-javascript".to_owned(),
    include: vec![RepoPath::new("packages")?],
}];
let written = write_starter_configuration(tree.root(), &languages, None)?;
```

- Pass the root of the working tree; the file lands at `{root}/codeint.jsonc`.
- It fails with `StarterConfigurationError::AlreadyExists` when any of `codeint.json`, `codeint.jsonc`, `.codeint/config.json`, or `.codeint/config.jsonc` already exists, and never replaces it.
- `include` holds repository-relative paths, not globs; the writer escapes glob characters, so `[draft]` covers that folder only.
- An empty `languages` slice leaves the `languages` key out, so every Language plugin covers the whole repository.
- Pass `Some(&StarterBoundaries::WorkspaceZones(...))` (from `StarterBoundaries::from_workspace_packages`) or `Some(&StarterBoundaries::Preset(...))` (from `BoundaryPreset::detect`) to append commented-out starter `boundaries` hints that keep `boundaries` inactive until uncommented.
- The `$schema` value is `published_schema_address()`: the raw GitHub address of `schema/codeint.schema.json` at the tag `v{version}` of the running codeint.

## Find the result cache folder

```rust
if let Command::Diff { result_cache, .. } | Command::Trend { result_cache, .. } =
    read_invocation()?.command
{
    let cache = result_cache.map(ResultCache::in_folder);
}
```

- `codeint diff` and `codeint trend` find the folder by the same rules.
- `CODEINT_CACHE_DIR` names the folder itself; a relative path starts at the current folder.
- Otherwise the folder is `codeint` inside the user cache folder: `$XDG_CACHE_HOME`, else `$HOME/.cache`, on Linux and macOS, and `%LOCALAPPDATA%` on Windows.
  An empty or relative `XDG_CACHE_HOME` is ignored, as the XDG Base Directory Specification says.
- `None` means no variable gives a folder; codeint then runs without a cache.
- Every test that runs `codeint diff` or `codeint trend` sets `CODEINT_CACHE_DIR` to a temporary folder, so no test touches the user cache.

## Read the range of a trend

```rust
use codeint_config::{CalendarDay, TrendRange};

let unix_seconds = match range {
    TrendRange::Latest(count) => None,
    TrendRange::Since(day) => Some(day.start_unix_seconds()),
};
let shown_day = CalendarDay::containing(committed_at_unix_seconds).to_string();
```

- Without `--since`, the range is `TrendRange::Latest(20)`.
- `--since` takes a plain `YYYY-MM-DD` day; codeint parses it itself, with no date-time library, and rejects a day that does not exist, such as `2026-02-30`, with exit code 2.
- A `CalendarDay` is a day in UTC: `start_unix_seconds` is its first second, and `containing` gives the UTC day of a Unix time.
- `TrendFormat` lists the formats `codeint trend` supports: TOON, JSON, and Markdown.

## Key a cached result by the configuration

- `Configuration` serializes as the effective `codeint.json`: every key with its default filled in, without the `$schema` address.
  A file that only differs in `$schema` or in keys set to their defaults serializes the same.
- Put the whole `Configuration` in the settings of a cached result, not chosen keys, so that a new setting can never leave a stale entry.
