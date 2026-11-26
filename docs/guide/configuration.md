# Configuration (`codeint.jsonc`, `codeint.json`, or `.codeint/config.*`)

codeint reads `codeint.json`, `codeint.jsonc`, `.codeint/config.json`, or `.codeint/config.jsonc` from the root of your Git repository.
All four files accept JSONC syntax: `//` line comments, `/* ... */` block comments, and trailing commas before `}` or `]`.
Keep only one configuration file across those four paths; having more than one present stops the run with exit code `2` and lists every conflicting file.
Without any configuration file, codeint measures every supported language with default settings and no hard Budgets.

## Starter configuration

Run `codeint init` to generate a starter `codeint.jsonc` with a versioned `$schema` URL for editor autocomplete.
`codeint init` never replaces an existing configuration file at `codeint.json`, `codeint.jsonc`, `.codeint/config.json`, or `.codeint/config.jsonc`.
When built-in Language plugins discover two or more Workspace packages, `codeint init` appends commented-out starter `boundaries.zones` entries for each package.
In a single-package repository whose Production source folders match at least two zones of the `layered` or `hexagonal` preset, `codeint init` appends a commented-out `boundaries.preset` suggestion.
In flat or custom repositories without those signals, `codeint init` emits no guessed boundary zones.
When you upgrade via `codeint update`, codeint updates that `$schema` tag automatically.
If your config file pins an official `$schema` tag newer than the running binary, codeint stops with exit code `2` and instructs you to upgrade.

```jsonc
{
  "$schema": "https://raw.githubusercontent.com/avidianity/codeint/main/schema/codeint.schema.json",
  // Default output format for check, diff, trend, report, impact, trace, churn, and coupling
  "format": "text",
  // Override default Source-set classification
  "source_sets": {
    "generated": ["src/**/*.generated.ts"],
    "vendored": ["vendor/**"],
  },
  // Hard ceilings enforced by `codeint check` and `codeint diff`
  "budgets": {
    "production": {
      "erosion": 0.35,
      "cognitive_complexity": 0.30,
      "duplication": 0.08,
      "verbosity": 0.08,
      "import_cycles": 0.05,
      "boundaries": 0,
      "orphan_files": 0,
      "hazards": 0.02,
      "test_gap": 0.25,
    },
    "unused_dependencies": 0,
    "unlisted_dependencies": 0,
    "dev_in_production_dependencies": 0,
  },
  // Stricter ceilings enforced on newly added files in `codeint diff`
  "new_file_budgets": {
    "production": {
      "erosion": 0,
      "cognitive_complexity": 0,
      "duplication": 0,
      "verbosity": 0.05,
      "import_cycles": 0,
      "boundaries": 0,
      "orphan_files": 0,
      "hazards": 0,
    },
  },
  // Ratchet rules for `codeint diff`
  "regression_limits": {
    "per_file": false,
    "generated": {
      "duplication": { "ratio_rise": 1, "new_findings": "never" },
    },
    "production": {
      "boundaries": { "ratio_rise": 0, "new_findings": "any_new_finding" },
      "hazards": { "ratio_rise": 0, "new_findings": "any_new_finding" },
    },
  },
  // SQL Dialect mappings (files covered by no entry use generic SQL)
  "sql": {
    "dialects": [
      { "dialect": "postgresql", "include": ["db/postgres"] },
      { "dialect": "duckdb", "include": ["analytics"] },
    ],
  },
  // Turn off specific Hazard line rules across the repository
  "hazards": {
    "disabled_rules": ["select-star"],
  },
  // Global and per-Workspace-package Framework profile overrides
  "framework_profiles": {
    "exclude": ["svelte"],
  },
  "packages": {
    "packages/legacy": { "framework_profiles": false },
  },

  // Optional custom path for the Ratchet file (defaults to .codeint/ratchet.json)
  "ratchet": {
    "path": ".codeint/ratchet.json",
  },
  // Optional shingle-based near-clone detection
  "duplication": {
    "near_clones": true,
    "min_similarity": 0.85,
  },
  // Additional Production entry points for Orphan files detection
  "entry_points": ["src/cli.ts", "src/workers/*.ts"],
  // LCOV or Cobertura XML Coverage reports for Test gap
  "coverage_reports": ["coverage/lcov.info"],
  // External package names excluded from Dependency hygiene Findings
  "dependencies": {
    "ignore": ["tailwindcss"],
  },
  // Architectural import boundaries
  "boundaries": {
    "preset": "layered",
    "zones": {
      "domain": {
        "patterns": ["src/domain/**"],
        "public_files": ["src/domain/index.ts"],
      },
      "infra": {
        "patterns": ["src/infra/**"],
      },
    },
    "rules": [
      {
        "from": "domain",
        "allow": ["domain"],
        "allow_external": [],
        "reason": "Domain stays independent of infrastructure and third-party packages",
      },
    ],
    "strict_zones": false,
    "source_sets": ["production"],
  },
  "languages": [
    { "plugin": "typescript-javascript", "include": ["web"], "exclude": ["web/dist"] },
    { "plugin": "rust", "include": ["crates"], "exclude": [] },
    { "plugin": "php", "include": ["app", "src"], "exclude": [] },
    { "plugin": "dart", "include": ["lib"], "exclude": [] },
    { "plugin": "python", "include": ["services"], "exclude": [] },
    { "plugin": "go", "include": ["cmd", "internal"], "exclude": [] },
    { "plugin": "sql", "include": ["db", "analytics"], "exclude": [] },
    { "plugin": "shell", "include": ["scripts"], "exclude": [] },
  ],
}
```

## Key sections

- **`format`**: default output format (`toon`, `text`, `json`, `sarif`, or `markdown`; defaults to `toon`) for `check`, `diff`, `trend`, `report`, `impact`, `trace`, `churn`, and `coupling`.
  CLI `--format` always overrides configured `format`, utility commands (`doctor`, `guard`, `config`, `ratchet`, `baseline`, `agent`, `plugin test`), `schema`, and MCP tools ignore it, and running a command that does not support the configured format exits with code `2` naming the supported formats.
- **`source_sets`**: globs that override built-in classification into `production`, `test`, `generated`, or `vendored`.
- **`budgets`**: hard maximum ratios (`0` to `1`) per Source set (`erosion`, `cognitive_complexity`, `duplication`, `verbosity`, `import_cycles`, `boundaries`, `hazards`, plus `orphan_files` and `test_gap` under `production` only and external Metric keys) and top-level Dependency hygiene Metrics (`unused_dependencies`, `unlisted_dependencies`, `dev_in_production_dependencies`).
  `codeint check` and `codeint diff` exit `1` when any Budget is exceeded.
  A `hazards` Budget passes when Hazards is absent because no measured language defines Hazard line rules.
  Setting `budgets.production.test_gap` without a matching Coverage report stops the run with exit code `2`; setting `orphan_files` or `test_gap` under another Source set also stops with exit code `2`.
- **`new_file_budgets`**: per-Source-set maximum ratios (`0` to `1`) enforced by `codeint diff` solely across newly added files (present on the head side and absent at the merge-base), so teams with legacy debt can keep new code clean.
  Stays silent when no new files were added.
  Accepts the same per-Source-set keys as `budgets` (`orphan_files` and `test_gap` under `production` only, plus external Metric keys) and rejects repository-wide Dependency hygiene keys or Production-only keys outside `production` with exit code `2`.
- **`regression_limits`**: per-Metric or per-Source-set tolerances for `codeint diff`, plus an opt-in `per_file` check (`false` by default).
  Set `per_file: true` to compare file-level Metric numerators and Findings for every modified file so `codeint diff` fails whenever any individual modified file worsens, even if another file deleted a larger Finding.
  Keep `per_file: false` (the default) when you want moving an unchanged function between files to pass.
  `ratio_rise` defaults to `0.01`; `new_findings` accepts `any_new_finding`, `numerator_rise` (with `numerator_rise_with_new_findings` retained as a deprecated 0.x alias), or `never`.
  By default, `erosion`, `cognitive_complexity`, `duplication`, `verbosity`, and external Metrics use `numerator_rise` (failing whenever the numerator rises, rounded to 4 decimal places, while keeping the `0.01` `ratio_rise` buffer when the numerator is flat), whereas `import_cycles`, `boundaries`, `orphan_files`, `hazards`, `unused_dependencies`, `unlisted_dependencies`, and `dev_in_production_dependencies` fail on `any_new_finding` (plus numerator rise).
  Setting `orphan_files` under `regression_limits.test`, `regression_limits.generated`, or `regression_limits.vendored` stops the run with exit code `2`, because Orphan files applies to the Production Source set only.
  Setting `test_gap` under `regression_limits` stops the run with exit code `2`, because `codeint diff` leaves Test gap out.
- **`sql`**: map folders or file globs under `sql.dialects` to `postgresql`, `mysql` (covering MariaDB), `sqlite`, or `duckdb`; unmatched `.sql` files stay generic SQL.
  See [SQL](./sql.md).
- **`hazards`**: list Hazard line rule names under `hazards.disabled_rules` to turn them off across the repository.
  See [Hazards](./hazards.md).
- **`framework_profiles`** and **`packages`**: control Framework profile detection globally (`false`, `true`, `["react"]`, `{"only": [...]}`, or `{"exclude": [...]}`) and per Workspace package under `packages.<path>.framework_profiles`.
  Unknown profile names fail fast with exit code `2`.
- **`ratchet`**: configure `ratchet.path` (repository-relative, no `..`, defaults to `.codeint/ratchet.json`) for the Ratchet file managed by `codeint ratchet update` and enforced automatically in whole-repository `codeint check` and `codeint diff` runs ([ADR 0026](../adr/0026-regression-and-ratchet.md)).
  `--package` runs skip Ratchet enforcement with a `ratchet_note`, and Metric version mismatches fail with exit code `2` naming `codeint ratchet update`.
- **`coverage_reports`**: repository-relative paths to LCOV or Cobertura XML Coverage reports used to compute **Test gap**; overridden when `--coverage-report` is passed on `codeint check`.
  See [Test gap](./test-gap.md).
- **`boundaries`**: architectural import zones, optional `preset` (`layered` or `hexagonal`), `public_files` encapsulation, directional internal `allow` or `deny` rules, external package `allow_external` or `deny_external` globs with optional `reason`, `strict_zones`, and `source_sets` (default `["production"]`).
  See [Import boundaries](./import-boundaries.md).
- **`duplication`**: set `near_clones: true` to enable shingle-based near-duplicate detection alongside exact Clone groups, and tune `min_similarity` in `(0, 1]` (default `0.85`).
- **`entry_points`**: repository-relative globs treated as additional Production entry points for Orphan files detection, combined with manifest-discovered entry points.
- **`dependencies.ignore`**: canonical external package names excluded from `unused_dependencies`, `unlisted_dependencies`, and `dev_in_production_dependencies` Findings.
- **`languages`**: restrict built-in or external Language plugins to specific folders and exclusions.
- **`plugins`** and **`plugin_limits`**: load external WebAssembly Language or Metric plugins with required SHA-256 verification and memory/time caps (`64` MiB and `5000` ms by default).

## Inspect effective configuration

Run `codeint config --resolved` to print every effective setting alongside its provenance (`default`, `file`, or `cli`):

```sh
codeint config --resolved
codeint config --resolved --format json --budget production.erosion=0.25 --source-set test=spec/** --plugin-memory-mb 128 --plugin-timeout-ms 10000
```

See [docs/reference.md#codeintjson](../reference.md#codeintjson) for the complete schema table and tested examples.
