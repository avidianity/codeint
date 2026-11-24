# How to build and render reports

## Summarize one snapshot with `Report`

```rust
use codeint_config::{Budgets, FindingsShown, ReportFormat};
use codeint_report::{MeasuredLanguage, Report};

let report = Report::summarize(
    vec![MeasuredLanguage {
        plugin: "typescript-javascript".to_owned(),
        model,
        reference_ranges,
        has_imports: true,
    }],
    submodules,
    FindingsShown::default(),
    &Budgets::default(),
);
let text = report.render(ReportFormat::Toon)?;
if !report.over_budget().is_empty() {
    for failure in report.over_budget() {
        eprintln!("{failure}");
    }
}
```

- Pass one `MeasuredLanguage` per Language plugin that ran on the snapshot.
- `Report::summarize` splits the merged `CodeModel` by Source set, computes all Metrics per Source set, evaluates Dependency hygiene per Language plugin when manifests are present, and checks every `Budget`.
- Use `Report::summarize_with_options`, `Report::summarize_with_extras`, `CommitMeasurement::of_languages_with_options`, and `CommitMeasurement::of_languages_with_full_options` to pass `DependenciesConfig` (including `dependencies.ignore`), `Boundaries`, external Metric plugins, `OwnershipOptions`, `ReportExtras` (`disabled_hazard_rules`, `rejected_statements`, `sql_schemas`), and optional precomputed `orphan_files`, `dependency_hygiene`, and `dependency_hygiene_by_language` (used when `--package` evaluates cross-package reachability and workspace hoisting before filtering).
- When two or more Language plugins measure files, the report adds a `languages` section per plugin and omits Reference ranges on the combined totals.
- Hazards is measured only over languages whose `LanguageFacts::has_hazard_rules` is `true` (on both numerator and denominator) and is omitted when no measured language has Hazard line rules; a Hazards Budget passes when Hazards is absent.
- When `MeasuredLanguage::has_imports` is `false`, TOON, JSON, and Markdown reports omit `import_cycles`, `orphan_files`, `boundaries`, and `dependencies` for that language and emit a one-line `import_metrics_note`, and SARIF emits no import Findings for files of that language.
- Repository totals and Budgets for `import_cycles`, `orphan_files`, `boundaries`, and `dependencies` (including Workspace package `import_cycles`) count only languages with `has_imports: true` on both sides of each ratio, and stay absent when no measured language has imports.
- `render` formats the report as `Toon`, `Json`, `Sarif`, or `Markdown`.
- SARIF output emits one result per listed Finding and stores each Finding id in `partialFingerprints.codeintFindingId`.
  When overlapping or transitively related Clone groups form a Clone family across three or more locations, `duplication.clone_families` lists the Clone families worst-first by `duplicated_lines` in JSON, TOON, Markdown, and MCP, and SARIF emits one consolidated result per Clone family instead of duplicate results for its member Clone groups.
- When `DuplicationConfig::near_clones` is enabled via `Report::summarize_with_config` or `CommitMeasurement::of_with_config`, near-clone `CloneGroupFinding` entries carry their `similarity` ratio (rendered as a percentage in Markdown and SARIF), `duplication.near_clones` records the active `min_similarity`, Duplication omits exact-clone Reference ranges, and Verbosity continues to count only exact clone lines.
- `over_budget` returns every `BudgetFailure` whose displayed ratio exceeds its configured `Budget`.

## Measure one commit with `CommitMeasurement`

```rust
use codeint_report::CommitMeasurement;

let measurement = CommitMeasurement::of(model);
```

- `CommitMeasurement::of` splits the `CodeModel` by Source set and records the ratios, numerators, and worst-first Findings for all four Metrics.
- `CommitMeasurement` derives `Serialize` and `Deserialize`, so callers can cache it per commit in `codeint_git::ResultCache`.
- Pass `CommitMeasurement` values into `DiffReport::compare` or `TrendReport::of`.

## Compare two snapshots with `DiffReport`

```rust
use codeint_config::{Budgets, FindingsShown, RegressionLimits, ReportFormat};
use codeint_report::{BaseCommit, CommitMeasurement, DiffReport, Head};

let diff = DiffReport::compare(
    BaseCommit {
        git_ref: "main".to_owned(),
        merge_base: merge_base_id.to_string(),
    },
    Head::WorkingTree,
    CommitMeasurement::of(base_model),
    CommitMeasurement::of(head_model),
    FindingsShown::default(),
    &Budgets::default(),
    &RegressionLimits::default(),
);
let text = diff.render(ReportFormat::Markdown)?;
for message in diff.failure_messages() {
    eprintln!("{message}");
}
```

- Use `Head::WorkingTree` when comparing against the working tree, or `Head::Commit(HeadCommit { git_ref, commit })` when comparing two commits.
- `DiffReport::compare` matches Findings by id to classify each one as `new`, `resolved`, or `unchanged`.
- `failure_messages` returns every `BudgetFailure` on the head side followed by every `Regression` that exceeds `RegressionLimits`.
- `render` formats the diff as `Toon`, `Json`, `Sarif` (new Findings only), or `Markdown`.

## Summarize history with `TrendReport`

```rust
use codeint_config::{CalendarDay, TrendFormat};
use codeint_report::{CommitMeasurement, TrendPoint, TrendReport};

let trend = TrendReport::of(vec![TrendPoint {
    commit: commit_id.to_string(),
    committed_on: CalendarDay::from_unix_seconds(timestamp),
    measurement: CommitMeasurement::of(model),
}]);
let text = trend.render(TrendFormat::Toon)?;
```

- Pass `TrendPoint` values oldest first.
- `TrendReport::of` records the four Metric ratios per Source set across commits and omits Source sets that have zero files in every commit.
- `render` formats the trend as `Toon`, `Json`, or `Markdown`.

## Re-render a saved JSON report with `SavedReport`

```rust
use codeint_config::ReportRenderFormat;
use codeint_report::SavedReport;

let saved = SavedReport::from_json(&json_text)?;
let sarif = saved.render(ReportRenderFormat::Sarif)?;
```

- `SavedReport::from_json` accepts JSON produced by `Report::render(ReportFormat::Json)` or `DiffReport::render(ReportFormat::Json)`.
- It rejects malformed JSON, non-object JSON, unsupported schema versions, and unrecognized payloads with `SavedReportError`.
- `render` formats the saved report as `Toon`, `Json`, `Sarif`, `Markdown`, or `Text` without re-running analysis.
