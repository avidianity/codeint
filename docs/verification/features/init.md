# Feature map: `codeint init`

Verifies starter `codeint.json` creation in a fresh repository, refusal to overwrite an existing `codeint.json`, and failure outside a Git repository.
See [codeint init](../../reference.md#codeint-init) for the command specification.

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
| `fresh` | Fresh Git repo without `codeint.json` | `0` | Writes valid `codeint.json` with `$schema` and `languages` |
| `existing` | Repo where `codeint.json` already exists | `2` | Standard error states `already exists`; SHA-256 of `codeint.json` is unchanged |
| `non_repo` | Folder outside a Git repository | `2` | Standard error reports the folder is not inside a Git repository |

## Drive and Assert

```sh
docs/verification/verify.sh drive "$RUN_DIR" init
docs/verification/verify.sh assert "$RUN_DIR" init
```

Assert checks that `codeint.json` is valid JSON after `fresh` and byte-identical before and after `existing`.

## Evidence and Cleanup

Evidence files are stored under `$EVIDENCE_DIR/init/` and `$EVIDENCE_DIR/verdict.json`.
Clean up the isolated run workspace while keeping evidence:

```sh
docs/verification/verify.sh cleanup "$RUN_DIR"
```
