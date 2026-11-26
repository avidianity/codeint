# Feature map: `codeint agent`

Verifies local `codeint mcp` registration for Claude Code, Cursor, and Codex using isolated `--config-root` and `CODEINT_AGENT_HOME` folders inside `RUN_DIR` so host user configurations are never touched.
See [codeint agent](../../reference.md#codeint-agent) for the command specification.

## Preconditions

Run Launch and Doctor from [docs/verification/README.md](../README.md):

```sh
export CODEINT_VERIFY_OWNER="$(id -un):$$"
eval "$(docs/verification/verify.sh launch)"
docs/verification/verify.sh doctor "$RUN_DIR"
```

## States covered

| Case | State | Expected exit | Expected visible result and authoritative effect |
| ---- | ----- | ------------- | ------------------------------------------------ |
| `status_empty_json` | Empty isolated `--config-root`, `codeint agent status --format json` | `0` | JSON lists `claude-code`, `cursor`, and `codex`, each with `registered: false` |
| `install_dry_run` | `codeint agent install --client claude-code --dry-run --format json` | `0` | JSON reports `dry_run: true` and `written: false`; `.claude.json` is not created on disk |
| `install_claude_preserve` | Pre-existing `.claude.json` with unrelated keys and another MCP server, `install --client claude-code --format json` | `0` | JSON reports `written: true`; stored `.claude.json` keeps the unrelated keys and existing MCP server while adding `mcpServers.codeint` (`command: "codeint"`, `args: ["mcp"]`) |
| `install_claude_idempotent` | Second `install --client claude-code --format json` | `0` | JSON reports `action: "unchanged"` and `written: false`; `.claude.json` SHA-256 is unchanged |
| `install_cursor_toon` | `CODEINT_AGENT_HOME=<isolated> codeint agent install --client cursor --format toon` | `0` | Writes `.cursor/mcp.json` with `mcpServers.codeint` |
| `install_codex_json` | `codeint agent install --config-root <isolated> --client codex --format json` | `0` | Writes `.codex/config.toml` registering `codeint` with `mcp` |
| `status_registered_json` | `codeint agent status --config-root <isolated> --format json` after all installs | `0` | JSON reports `registered: true` for all three clients |
| `invalid_json_config` | Isolated folder containing invalid JSON in `.claude.json`, `install --client claude-code` | `2` | Standard error reports invalid JSON without overwriting the file |

## Drive and Assert

```sh
docs/verification/verify.sh drive "$RUN_DIR" agent
docs/verification/verify.sh assert "$RUN_DIR" agent
```

Assert inspects the generated `.claude.json`, `.cursor/mcp.json`, and `.codex/config.toml` files inside `RUN_DIR`, checks SHA-256 preservation on idempotent reinstall, and confirms `FIXTURE_REPO` stays clean.

## Evidence and Cleanup

Evidence files are stored under `$EVIDENCE_DIR/agent/` and `$EVIDENCE_DIR/verdict.json`.
Clean up the isolated run workspace while keeping evidence:

```sh
docs/verification/verify.sh cleanup "$RUN_DIR"
```
