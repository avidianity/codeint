# Feature map: `codeint update`

Verifies offline release-check behavior (`--check`, `--version`), local `CODEINT_RELEASE_BASE_URL` resolution and error handling, and package-manager installation refusal without making any network requests.
See [codeint update](../../reference.md#codeint-update) for the command specification.

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
| `check_up_to_date` | `codeint update --check --version v<current>` | `0` | Standard output states `is already up to date`; running binary is untouched |
| `check_available` | `codeint update --check --version v99.0.0` | `0` | Standard output states `Update available:` and `v99.0.0`; running binary is untouched |
| `local_latest_available` | `CODEINT_RELEASE_BASE_URL` pointing to a local directory with `LATEST` set to `v99.0.0`, `codeint update --check` | `0` | Standard output states `Update available:` and `v99.0.0` offline without touching the network |
| `local_latest_missing` | `CODEINT_RELEASE_BASE_URL` pointing to a missing local directory, `codeint update --check` | `2` | Standard error states `cannot query the latest release` |
| `node_modules_refused` | Copy of `codeint` placed inside `<RUN_DIR>/work-update-npm/node_modules/.bin/codeint`, run with `update --check` | `2` | Standard error states `managed by a package manager` |

## Drive and Assert

```sh
docs/verification/verify.sh drive "$RUN_DIR" update
docs/verification/verify.sh assert "$RUN_DIR" update
```

Assert checks each offline exit code and message, confirms `BIN_PATH` SHA-256 is unchanged, and confirms `FIXTURE_REPO` stays clean.

## Evidence and Cleanup

Evidence files are stored under `$EVIDENCE_DIR/update/` and `$EVIDENCE_DIR/verdict.json`.
Clean up the isolated run workspace while keeping evidence:

```sh
docs/verification/verify.sh cleanup "$RUN_DIR"
```
