# Configuration (`codeint.jsonc` / `codeint.json`)

codeint reads `codeint.jsonc` or `codeint.json` from the root of your Git repository.
Both files accept JSONC syntax: `//` line comments, `/* ... */` block comments, and trailing commas before `}` or `]`.
Keep only one of `codeint.jsonc` or `codeint.json` at the repository root; having both files present stops the run with exit code `2`.
Without either file, codeint measures every supported language with default settings and no hard Budgets.

## Starter configuration

Run `codeint init` to generate a starter `codeint.jsonc` with a versioned `$schema` URL for editor autocomplete.
`codeint init` never replaces an existing `codeint.json` or `codeint.jsonc`.
When you upgrade via `codeint update`, codeint updates that `$schema` tag automatically.
If your config file pins an official `$schema` tag newer than the running binary, codeint stops with exit code `2` and instructs you to upgrade.

```jsonc
{
  "$schema": "https://raw.githubusercontent.com/avidianity/codeint/v0.4.0/schema/codeint.schema.json",
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
    },
    "unused_dependencies": 0,
    "unlisted_dependencies": 0,
    "dev_in_production_dependencies": 0,
  },
  // Ratchet rules for `codeint diff`
  "regression_limits": {
    "generated": {
      "duplication": { "ratio_rise": 1, "new_findings": "never" },
    },
    "production": {
      "boundaries": { "ratio_rise": 0, "new_findings": "any_new_finding" },
    },
  },
  // Optional shingle-based near-clone detection
  "duplication": {
    "near_clones": true,
    "min_similarity": 0.85,
  },
  // Additional Production entry points for Orphan files detection
  "entry_points": ["src/cli.ts", "src/workers/*.ts"],
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
        "reason": "Domain stays independent of infrastructure",
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
  ],
}
```

## Key sections

- **`source_sets`**: globs that override built-in classification into `production`, `test`, `generated`, or `vendored`.
- **`budgets`**: hard maximum ratios (`0` to `1`) per Source set (`erosion`, `cognitive_complexity`, `duplication`, `verbosity`, `import_cycles`, `boundaries`, `orphan_files`, plus external Metric keys) and top-level Dependency hygiene Metrics (`unused_dependencies`, `unlisted_dependencies`, `dev_in_production_dependencies`).
  `codeint check` and `codeint diff` exit `1` when any Budget is exceeded.
- **`regression_limits`**: per-Metric or per-Source-set tolerances for `codeint diff`.
  `ratio_rise` defaults to `0.01`; `new_findings` accepts `any_new_finding`, `numerator_rise_with_new_findings`, or `never`.
  By default, `import_cycles`, `boundaries`, `orphan_files`, `unused_dependencies`, `unlisted_dependencies`, and `dev_in_production_dependencies` fail on `any_new_finding`.
- **`boundaries`**: architectural import zones, optional `preset` (`layered` or `hexagonal`), `public_files` encapsulation, directional `allow` or `deny` rules with optional `reason`, `strict_zones`, and `source_sets` (default `["production"]`).
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
