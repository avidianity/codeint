# Feature map: `codeint mcp`

Verifies the local stdio Model Context Protocol server (`initialize`, `ping`, `tools/list`, `tools/call`), `check` with `ignore_baseline`, JSON-RPC error responses, and clean exit when standard input closes.
See [codeint mcp](../../reference.md#codeint-mcp) for the command specification.

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
| `session_golden` | Pipe `initialize`, `ping`, `tools/list`, `tools/call` (`check` default TOON), `tools/call` (`guard` default TOON), and `tools/call` (`guard` with `format: "json"`) over stdio, then close stdin | `0` | Returns six JSON-RPC 2.0 responses: `serverInfo.name == "codeint"`, empty `ping`, all 8 read-only tools advertising optional `format` (`"toon"` default, `"json"` option), `check` and `guard` default TOON content, and `guard` valid JSON content when `format: "json"` is passed |
| `check_ignore_baseline` | Baselined repo copy with `budgets.production.erosion: 0`; call `check` with `ignore_baseline: false` and `ignore_baseline: true` | `0` | First call hides the baselined Finding with no Budget failure; second call ignores the Baseline and reports `over_budget` |
| `protocol_errors` | Pipe unknown method, unknown tool, `diff` missing `base_ref`, `check` with invalid `format`, `trace` with `max_paths: 0`, `impact` missing targets, and malformed JSON, then close stdin | `0` | Returns JSON-RPC errors `-32601` (unknown method), `-32602` (unknown tool), `isError: true` (missing `base_ref`, invalid `format`, `max_paths: 0`, and `impact` with MCP-tailored remedy naming `changed` rather than `--changed`), and `-32700` (parse error) |

## Drive and Assert

```sh
docs/verification/verify.sh drive "$RUN_DIR" mcp
docs/verification/verify.sh assert "$RUN_DIR" mcp
```

Assert parses every NDJSON response line, verifies tool names, tool payloads, Baseline handling, JSON-RPC error codes, and `FIXTURE_REPO` cleanliness.

## Evidence and Cleanup

Evidence files are stored under `$EVIDENCE_DIR/mcp/` and `$EVIDENCE_DIR/verdict.json`.
Clean up the isolated run workspace while keeping evidence:

```sh
docs/verification/verify.sh cleanup "$RUN_DIR"
```
