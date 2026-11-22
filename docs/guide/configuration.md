# Configuration (`codeint.json`)

codeint reads `codeint.json` from the root of your Git repository.
Without `codeint.json`, codeint measures every supported language with default settings and no hard Budgets.

## Starter configuration

Run `codeint init` to generate a starter file with a versioned `$schema` URL for editor autocomplete.
When you upgrade via `codeint update`, codeint updates that `$schema` tag automatically.

```json
{
  "$schema": "https://raw.githubusercontent.com/avidianity/codeint/v0.3.0/schema/codeint.schema.json",
  "source_sets": {
    "generated": ["src/**/*.generated.ts"],
    "vendored": ["vendor/**"]
  },
  "budgets": {
    "production": {
      "erosion": 0.35,
      "duplication": 0.08,
      "verbosity": 0.08,
      "import_cycles": 0.05
    }
  },
  "regression_limits": {
    "generated": {
      "duplication": { "ratio_rise": 1, "new_findings": "never" }
    }
  },
  "languages": [
    { "plugin": "typescript-javascript", "include": ["web"], "exclude": ["web/dist"] },
    { "plugin": "rust", "include": ["crates"], "exclude": [] },
    { "plugin": "php", "include": ["app", "src"], "exclude": [] },
    { "plugin": "dart", "include": ["lib"], "exclude": [] },
    { "plugin": "python", "include": ["services"], "exclude": [] }
  ]
}
```

## Key sections

- **`source_sets`**: globs that override built-in classification into `production`, `test`, `generated`, or `vendored`.
- **`budgets`**: hard maximum ratios (`0` to `1`) per Source set and Metric; `codeint check` and `codeint diff` exit `1` when exceeded.
- **`regression_limits`**: per-Metric or per-Source-set tolerances for `codeint diff` (`ratio_rise` defaults to `0.01`; `new_findings` accepts `any_new_finding`, `numerator_rise_with_new_findings`, or `never`).
- **`languages`**: restrict built-in or external Language plugins to specific folders and exclusions.
- **`plugins`** and **`plugin_limits`**: load external WebAssembly Language or Metric plugins with SHA-256 verification and memory/time caps.

See [docs/reference.md#codeintjson](../reference.md#codeintjson) for the complete schema table and tested examples.
