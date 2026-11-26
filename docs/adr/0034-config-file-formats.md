---
status: accepted
---

# Support TOML and YAML configuration files alongside JSON and JSONC

## Context

`codeint` previously discovered only JSON and JSONC configuration files (`codeint.json`, `codeint.jsonc`, `.codeint/config.json`, and `.codeint/config.jsonc`).
Teams working in Rust, Python, Go, or DevOps-heavy repositories often prefer TOML or YAML over JSONC for repository configuration because those formats match the rest of their toolchain.
At the same time, configuration parsing is part of `codeint`'s trust boundary and determinism guarantee: configuration files must be static, unambiguous, schema-checked against the same canonical contract, and parsed by battle-tested libraries rather than custom parsers.

## Decision

1. Support TOML and YAML alongside JSON and JSONC as the only configuration file formats.
   Executable configuration formats (such as `.js`, `.ts`, or `.mjs`) and niche formats (such as JSON5, KDL, or HCL) are excluded.
2. Parse TOML with the workspace-pinned `toml` crate and YAML with the workspace-pinned `serde-saphyr` crate.
   Do not hand-write a TOML or YAML parser.
3. Discover dedicated configuration files only; never read `codeint` configuration from host manifests such as `Cargo.toml`, `pyproject.toml`, or `package.json`.
4. Keep `.codeint/ratchet.json` and `.codeint/baseline.json` as JSON-only machine-managed state files.
5. Expand configuration discovery to exactly ten candidate paths at the repository root: `codeint.json`, `codeint.jsonc`, `codeint.toml`, `codeint.yaml`, `codeint.yml`, `.codeint/config.json`, `.codeint/config.jsonc`, `.codeint/config.toml`, `.codeint/config.yaml`, and `.codeint/config.yml`.
   If more than one candidate exists, stop with exit code 2 (`AmbiguousConfigFiles`).
6. Keep `codeint init` (including `--adopt`) writing `codeint.jsonc` by default, and add `--format <jsonc|json|toml|yaml>` writing `codeint.jsonc`, `codeint.json`, `codeint.toml`, or `codeint.yaml`.
   Refuse to write if any of the ten configuration files already exists.
   `codeint init --format json` writes strict comment-free JSON and omits commented starter hints; `jsonc`, `toml`, and `yaml` include the commented starter hints.
7. Emit editor schema comment directives in TOML (`#:schema <url>`) and YAML (`# yaml-language-server: $schema=<url>`), and `"$schema"` in JSON and JSONC.
   Recognize both a leading schema comment directive and a top-level `$schema` key during configuration loading, `codeint doctor`, and `codeint update`.
   If both appear with different URLs, stop with exit code 2 (`ConflictingSchemaDirectives`).
8. Enforce strict static-configuration safety rules for TOML and YAML before converting parsed documents to `serde_json::Value` and passing them through the shared schema shape check and `Configuration` merge pipeline:
   - An empty or comment-only TOML or YAML file resolves to default settings (`{}`).
   - Stop with exit code 2 on non-mapping top-level values, duplicate keys, multi-document YAML streams, non-string YAML mapping keys, YAML anchors or aliases, YAML merge keys, non-finite floats, and TOML native date-time values.

## Consequences

- Teams can author `codeint` configuration in JSON, JSONC, TOML, or YAML while sharing one canonical JSON Schema, one validation and error-path pipeline, one `codeint config --resolved` representation, and identical `ResultCache` keys across equivalent configurations.
- Zero new third-party dependencies are added because `toml` and `serde-saphyr` are already pinned in the workspace.
- `codeint doctor` and `codeint update` work uniformly across all ten configuration file locations and both editor schema hint styles.
- Ambiguous multi-file layouts fail fast instead of silently preferring one format over another.
