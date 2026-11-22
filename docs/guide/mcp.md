# Coding-Agent Integration (MCP Server)

`codeint mcp` runs a local Model Context Protocol (MCP) server over standard input and standard output with no open network port.
Coding agents can call `check`, `diff`, and `trend` directly as structured tools to catch Software entropy Regressions before opening a pull request.

## Exposed tools

| Tool | Required arguments | Optional arguments | Purpose |
| ---- | ------------------ | ------------------ | ------- |
| `check` | none | `path`, `all` | Measure the working tree and report any Budget failures. |
| `diff` | `base_ref` | `head_ref`, `path`, `all` | Compare against the merge-base of `base_ref` and classify Findings as new, resolved, or unchanged. |
| `trend` | none | `path`, `since` (`YYYY-MM-DD`) | Measure each main-line commit over time. |

## Configure in your coding agent

Add `codeint mcp` as a stdio MCP server in your client configuration (Claude Code, Cursor, Codex, or Antigravity):

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
