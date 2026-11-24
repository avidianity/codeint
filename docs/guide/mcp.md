# Coding-Agent Integration (MCP Server)

`codeint mcp` runs a local Model Context Protocol (MCP) server over standard input and standard output with no open network port.
Coding agents can check architectural boundaries before writing a file, inspect blast radius and import paths, spot churn and hidden coupling, and catch Software entropy Regressions before opening a pull request.

## Automatic setup with `codeint agent`

Inspect detected local coding agents (Claude Code, Cursor, and Codex) and register `codeint mcp` automatically:

```sh
codeint agent status
codeint agent install --dry-run
codeint agent install
```

- Pass `--client <claude-code|cursor|codex>` to target one client.
- Pass `--config-root <DIR>` (or set `CODEINT_AGENT_HOME`) to override the home folder containing agent config files.
- `codeint agent install` preserves unrelated keys and existing MCP servers, writes atomically, and is safe to re-run.

## Manual client configuration

For other MCP-compatible clients (such as Antigravity), add `codeint mcp` as a stdio MCP server:

```json
{
  "mcpServers": {
    "codeint": {
      "command": "codeint",
      "args": ["mcp"]
    }
  }
}
```

If you run codeint through npm without a global binary install, set `"command": "npx"` and `"args": ["-y", "codeint", "mcp"]`.

## Exposed MCP tools

`codeint mcp` exposes eight read-only tools:

| Tool | Required arguments | Optional arguments | Purpose |
| ---- | ------------------ | ------------------ | ------- |
| `check` | none | `path`, `package`, `coverage_reports`, `all` | Measure Software entropy and Hazards of the working tree (Erosion, Cognitive complexity, Test gap when Coverage reports are configured, Duplication, Verbosity, Import cycles, Boundaries, Orphan files, Dependency hygiene, and Hazards when a measured language defines Hazard line rules) and report Budget failures, `sql_schemas`, and `rejected_statements`. |
| `diff` | `base_ref` | `head_ref`, `path`, `package`, `all` | Compare against the merge-base of `base_ref` (including Hazards and new Hazard line Findings) and classify Findings as new, resolved, or unchanged. |
| `trend` | none | `path`, `since` (`YYYY-MM-DD`) | Measure each main-line commit over time (latest 20 commits or since a UTC date), including `hazards` when measured. |
| `guard` | `file` | `path` | Query the architectural boundary zone, allowed and denied target zones, `public_files` constraints, and allowed importers for an existing or planned file path. |
| `impact` | none (`paths` or `changed`) | `path`, `paths`, `changed`, `depth`, `package` | Compute the transitive reverse-dependency blast radius, separating Production and Test dependents and listing affected Workspace packages. |
| `trace` | `from`, `to` | `path`, `package`, `all`, `max_paths` | Find the shortest runtime import path(s) connecting two repository files. |
| `churn` | none | `path`, `since`, `commits`, `all` | Rank eroded and duplicated files alongside their Git commit churn independently per Metric. |
| `coupling` | none | `path`, `since`, `commits`, `max_commit_files`, `min_co_changes`, `min_ratio`, `include_tests`, `all` | Detect pairs of files that repeatedly change together in the same commits and flag direct runtime imports. |

## Machine-readable capability discovery

Run `codeint schema` (or `codeint schema --format toon`) to print a deterministic offline manifest of the running binary's CLI commands, flags, built-in Metrics, Source sets, built-in Language plugins, output formats, MCP tools, and exit codes.
