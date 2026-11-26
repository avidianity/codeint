# Feature map: `codeint config`

Verifies `codeint config --resolved` provenance across built-in defaults, `codeint.json`, and CLI overrides (`--budget`, `--source-set`, `--plugin-memory-mb`, `--plugin-timeout-ms`), subfolder paths, `text`, `toon`, and `json` formats, and operational errors.
See [codeint config](../../reference.md#codeint-config) for the command specification.

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
| `resolved_default_json` | Repo with no `codeint.json`, `codeint config --resolved --format json` | `0` | JSON reports `config_file_present: false` and every setting has `source: "default"` |
| `resolved_file_and_cli_json` | Repo with `regression_limits.per_file: true` plus `--budget`, `--source-set`, `--plugin-memory-mb`, and `--plugin-timeout-ms` | `0` | JSON reports `config_file_present: true`, `regression_limits.per_file` with `source: "file"`, and CLI-overridden `budgets.production.erosion`, `plugin_limits.memory_mb`, and `plugin_limits.timeout_ms` with `source: "cli"` |
| `resolved_subfolder_text` | `codeint config --resolved src --format text` | `0` | Non-empty text listing resolved settings |
| `resolved_toon` | `codeint config --resolved --format toon` | `0` | TOON output lists `settings` |
| `invalid_config` | Unknown key in `codeint.json` | `2` | Standard error names the unknown key |
| `invalid_override` | `--budget production.erosion=2.0` outside `0..=1` | `2` | Standard error rejects the Budget ratio |
| `non_repo` | Folder outside a Git repository | `2` | Standard error reports the folder is not inside a Git repository |

## Drive and Assert

```sh
docs/verification/verify.sh drive "$RUN_DIR" config
docs/verification/verify.sh assert "$RUN_DIR" config
```

Assert checks default, file, and CLI provenance entries, format shapes, error exits, and `FIXTURE_REPO` cleanliness.

## Evidence and Cleanup

Evidence files are stored under `$EVIDENCE_DIR/config/` and `$EVIDENCE_DIR/verdict.json`.
Clean up the isolated run workspace while keeping evidence:

```sh
docs/verification/verify.sh cleanup "$RUN_DIR"
```
