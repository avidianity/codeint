# How to read the configuration

## Read codeint.json

```rust
use codeint_config::read_configuration;

let repository = Repository::discover(&folder)?;
let configuration = read_configuration(repository.root())?;
```

- `read_configuration` reads `codeint.json` at the root of the repository, so pass the root of the working tree, never the folder the user named.
- It returns `Configuration::default()` when the file is absent; codeint runs with defaults.
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
- A Budget applies to the total of every Language plugin; the report never checks the section of one Language plugin against it.
- A Budget is a ratio from 0 to 1; the shape check rejects any other number with exit code 2.
- `admits` passes a ratio equal to the Budget.
  Pass the ratio rounded as the report shows it, so that a ratio the user sees as equal to its Budget passes.

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

- `limit` always returns a `RegressionLimit`; without a setting it is `RegressionLimit::DEFAULT`, or `RegressionLimit::DEFAULT_FOR_IMPORT_CYCLES` for Import cycles.
- `codeint.json` sets a limit under `regression_limits.{metric}` for every Source set, and under `regression_limits.{source_set}.{metric}` for one Source set.
  `limit` settles each of `ratio_rise` and `new_findings` on its own: the Source set setting wins, then the Metric setting, then the default.
- `RatioRise::DEFAULT` is 0.01, and `admits` passes a rise equal to the limit.
  Round the difference of the two displayed ratios to four places before you pass it, because the plain floating-point difference of 0.0142 and 0.0042 is above 0.01.
- `NewFindingsRule::AnyNewFinding` fails on any new Finding; `NewFindingsRule::NumeratorRiseWithNewFindings` fails only when the numerator also rises.

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

## Add a setting

- Add a field to `ConfigurationFile` in `src/codeint_json.rs`, with `#[serde(default)]` and a `#[schemars(description = ...)]` that tells the user what it does.
- Convert it into a typed field of `Configuration`; never hand the raw JSON to another crate.
- Run `cargo test -p codeint-config`.
  The schema test rewrites `schema/codeint.schema.json` when it drifts from the types and fails once; review and commit the rewritten file.
- The shape check reads the generated schema to name the bad key and the expected type.
  It also names a key that the schema lists under `required` and the file leaves out.
  When a new type makes schemars write a keyword the shape check does not understand, `the_shape_check_understands_every_keyword_of_the_configuration_schema` fails; teach `src/codeint_json/shape.rs` that keyword first.

## Write a starter codeint.json

```rust
use codeint_config::{DetectedLanguage, write_starter_configuration};

let languages = vec![DetectedLanguage {
    plugin: "typescript-javascript".to_owned(),
    include: vec![RepoPath::new("packages")?],
}];
let written = write_starter_configuration(tree.root(), &languages)?;
```

- Pass the root of the working tree; the file lands at `{root}/codeint.json`.
- It fails with `StarterConfigurationError::AlreadyExists` when the file exists, and never replaces it.
- `include` holds repository-relative paths, not globs; the writer escapes glob characters, so `[draft]` covers that folder only.
- An empty `languages` slice leaves the `languages` key out, so every Language plugin covers the whole repository.
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
