# Feature map: `codeint schema`

Verifies deterministic machine-readable manifest output in `json` and `toon` formats, `--output`, offline execution outside a Git repository, and operational errors.
See [codeint schema](../../reference.md#codeint-schema) for the command specification.

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
| `json_default` | `codeint schema` in fixture repo | `0` | Valid JSON with `name: "codeint"`, version matching `codeint --version`, all 19 CLI commands, all 8 MCP tools, built-in Languages, Metrics, Source sets, and exit codes |
| `toon_format` | `codeint schema --format toon` | `0` | TOON output includes `name: codeint` and `commands` |
| `output_file` | `codeint schema --format json --output schema.json` | `0` | Standard output is empty; `schema.json` is byte-identical to `json_default` |
| `non_repo` | `codeint schema --format json` outside any Git repository | `0` | Succeeds offline and produces output byte-identical to `json_default` |
| `unknown_option` | Unknown flag `--no-such-flag` | `2` | Standard error rejects the unknown option |

## Drive and Assert

```sh
docs/verification/verify.sh drive "$RUN_DIR" schema
docs/verification/verify.sh assert "$RUN_DIR" schema
```

Assert checks manifest completeness, byte-for-byte determinism across stdout, `--output`, and `NON_REPO`, error exits, and `FIXTURE_REPO` cleanliness.

## Evidence and Cleanup

Evidence files are stored under `$EVIDENCE_DIR/schema/` and `$EVIDENCE_DIR/verdict.json`.
Clean up the isolated run workspace while keeping evidence:

```sh
docs/verification/verify.sh cleanup "$RUN_DIR"
```
