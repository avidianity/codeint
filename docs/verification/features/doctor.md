# Feature map: `codeint doctor`

Verifies strictly read-only, offline diagnostics for Git repository state, configuration validity, Language plugin detection, external Wasm Plugin validation, subfolder inspection, and `text`, `json`, and `toon` formats.
See [codeint doctor](../../reference.md#codeint-doctor) for the command specification.

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
| `healthy_with_config_json` | Repo copy with valid `codeint.json`, `codeint doctor --format json` | `0` | JSON reports `status: "pass"`, `summary.errors == 0`, and `summary.warnings == 0` |
| `no_config_warn_text` | Repo without `codeint.json`, `codeint doctor --format text` | `0` | Text output reports passing checks alongside a missing-configuration warning |
| `subfolder_toon` | `codeint doctor src --format toon` | `0` | TOON output includes `status:` and `checks` |
| `invalid_config_fail` | Repo copy with unknown key `budget` in `codeint.json`, `--format json` | `1` | JSON reports `status: "error"`, `summary.errors >= 1`, and an error `config` check |
| `missing_plugin_fail` | Repo copy with a missing Wasm Plugin path in `codeint.json`, `--format json` | `1` | JSON reports `status: "error"` and an error `plugin:missing` check |
| `non_repo_warn` | Folder outside a Git repository with `--format json` | `0` | JSON reports `status: "warn"`, `summary.errors == 0`, and a warning `git` check |

## Drive and Assert

```sh
docs/verification/verify.sh drive "$RUN_DIR" doctor
docs/verification/verify.sh assert "$RUN_DIR" doctor
```

Assert checks passing, warning, and blocking-error diagnostics across JSON, TOON, and text formats, and confirms `FIXTURE_REPO` stays clean.

## Evidence and Cleanup

Evidence files are stored under `$EVIDENCE_DIR/doctor/` and `$EVIDENCE_DIR/verdict.json`.
Clean up the isolated run workspace while keeping evidence:

```sh
docs/verification/verify.sh cleanup "$RUN_DIR"
```
