# Import boundaries

**Import boundaries** (`boundaries`) enforce architectural rules across your codebase in every Language plugin that emits runtime imports.
An Import boundary governs both internal dependencies between named architectural zones (with optional `public_files` entry-point encapsulation) and external dependencies on third-party packages (`allow_external` and `deny_external`).

## How Import boundaries work

- **Inactive until configured**: when `boundaries` is omitted from `codeint.jsonc` or `codeint.json`, boundary evaluation does not run and reports omit `boundaries`.
- **Named zones and precedence**: each zone under `boundaries.zones` lists repository-relative path globs under `patterns` and optional `public_files` globs naming the only files other zones may import.
  When a file matches multiple zones, the most specific pattern wins (most literal characters, then longest pattern, then zone name alphabetically).
- **Internal zone rules (`allow` and `deny`)**: each rule under `boundaries.rules` names a `from` zone (or `*` as a fallback for zones without a specific rule) and sets either `allow` or `deny` (mutually exclusive) to govern cross-zone internal imports.
  Same-zone imports are always permitted.
- **External package rules (`allow_external` and `deny_external`)**: each rule can also set `allow_external` or `deny_external` (mutually exclusive) using package-name globs such as `"prisma"`, `"@aws-sdk/*"`, or `"github.com/aws/*"`.
  Internal (`allow`/`deny`) and external (`allow_external`/`deny_external`) rules are completely independent: you can combine them on the same `from` zone, put them in separate rules, or write a rule with only `allow_external` or `deny_external`.
- **Standard-library built-ins always pass**: `allow_external` and `deny_external` inspect third-party external packages only, never standard-library or runtime built-ins (`node:fs`, `std::io`, Python stdlib, Go stdlib, and similar).
  Setting `"allow_external": []` keeps a domain zone free of third-party packages while still permitting pure standard-library code.
- **Denominator stability**: when no rule in `boundaries.rules` sets `allow_external` or `deny_external`, `resolved_imports` counts resolved internal imports only so existing ratios never shift.
  When at least one external-package rule is configured, checked external imports are included alongside resolved internal imports in `resolved_imports`.
- **Source-set scoping**: `boundaries` evaluates `["production"]` by default so test fixtures and test files do not trigger Production architecture violations.
  Set `boundaries.source_sets` (for example `["production", "test"]`) to opt additional Source sets into boundary checks.
- **Strict zone coverage (`strict_zones`)**: set `boundaries.strict_zones` to `true` to report every file in evaluated Source sets that matches no configured zone (`kind: "unzoned_file"`).

## Configuration examples

### Zones, internal rules, and external package rules

```jsonc
{
  "budgets": {
    "production": {
      "boundaries": 0,
    },
  },
  "regression_limits": {
    "production": {
      "boundaries": {
        "ratio_rise": 0,
        "new_findings": "any_new_finding",
      },
    },
  },
  "boundaries": {
    "zones": {
      "domain": {
        "patterns": ["src/domain/**"],
        "public_files": ["src/domain/index.ts"],
      },
      "application": {
        "patterns": ["src/application/**"],
      },
      "infrastructure": {
        "patterns": ["src/infrastructure/**"],
      },
    },
    "rules": [
      {
        "from": "domain",
        "allow": ["domain"],
        "allow_external": [],
        "reason": "Domain stays pure: no outer layers and no third-party packages",
      },
      {
        "from": "application",
        "allow": ["domain"],
        "allow_external": ["zod", "@acme/*"],
        "reason": "Application depends on domain and approved validation helpers only",
      },
      {
        "from": "infrastructure",
        "deny_external": ["lodash"],
        "reason": "Prefer native runtime helpers over lodash",
      },
    ],
    "strict_zones": false,
    "source_sets": ["production"],
  },
}
```

### Built-in presets (`layered` and `hexagonal`)

Set `boundaries.preset` to `"layered"` (`presentation`, `application`, `domain`, `infrastructure`) or `"hexagonal"` (`domain`, `ports`, `application`, `adapters`) to start from built-in zone globs and internal directional rules.
Built-in presets never guess third-party package rules; they define internal zone boundaries only.
Entries in `boundaries.zones` override matching preset zones or add new ones.
Rules in `boundaries.rules` that set `allow` or `deny` replace the preset's internal rule for that `from` zone, while an external-only rule (setting only `allow_external` or `deny_external`) keeps the preset's internal rule intact:

```jsonc
{
  "boundaries": {
    "preset": "hexagonal",
    "rules": [
      {
        "from": "domain",
        "allow_external": [],
        "reason": "Keep hexagonal domain free of third-party packages",
      },
      {
        "from": "application",
        "deny_external": ["prisma", "@aws-sdk/*"],
        "reason": "Keep persistence and cloud SDKs inside adapters",
      },
    ],
  },
}
```

### Validation errors (exit code `2`)

codeint validates `boundaries` before scanning and stops with exit code `2` and an actionable remedy when:

- a rule references an unknown zone name in `from`, `allow`, or `deny`;
- a rule sets neither internal (`allow`/`deny`) nor external (`allow_external`/`deny_external`) directives;
- a rule mixes `allow` with `deny`, or mixes `allow_external` with `deny_external`, on the same rule entry or across multiple rule entries for the same `from` zone;
- a zone `patterns`, `public_files`, `allow_external`, or `deny_external` entry contains invalid glob syntax or an empty pattern.

## Pre-edit checks with `codeint guard`

Before creating or editing a file, run `codeint guard <path>` (or call the `guard` tool over `codeint mcp`) to inspect the file's matched zone, `public_files` visibility, allowed and denied target zones, allowed importers, and any active `allow_external` or `deny_external` policy:

```sh
codeint guard src/domain/order.ts
codeint guard src/domain/new_policy.ts --format json
codeint guard src/application/checkout.ts --format toon
```

`codeint guard` works even when the file does not exist on disk yet, so humans and coding agents can verify architectural rules before writing imports.

## Reports, `codeint diff`, and MCP

- **`codeint check`**: reports `boundaries` (`ratio`, `resolved_imports`, and `findings`) per evaluated Source set across TOON, JSON, Markdown, and SARIF, and re-renders saved JSON reports cleanly into `text`, `toon`, `json`, `sarif`, and `markdown` with `codeint report`.
- **Finding kinds**:
  - `zone_rule`: disallowed cross-zone internal import (`importer`, `target`, `specifier`, `source_zone`, `target_zone`, optional `reason`, `message`).
  - `external_package`: disallowed external package import (`importer`, `external_package`, `specifier`, `source_zone`, optional `reason`, `message`).
  - `public_files`: cross-zone import of a file outside the target zone's `public_files`.
  - `unzoned_file`: file matching no zone when `strict_zones` is `true`.
- **`codeint diff <base>`**: assigns every boundary Finding a deterministic content-based `id` (for `external_package`, derived from the importing file, kind, and external package name) so pre-existing violations stay `unchanged` while newly introduced violations are classified as `new` and fail by default (`new_findings: "any_new_finding"`).
- **MCP (`codeint mcp`)**: coding agents can call `guard` before editing and `check` or `diff` after editing to prevent architectural drift.

See [docs/reference.md#boundaries](../reference.md#boundaries) and [docs/reference.md#codeint-guard](../reference.md#codeint-guard) for exact schema keys, flags, and tested examples.
