# Feature map: `codeint init`

Verifies starter config creation in a fresh repository, refusal to overwrite an existing config, failure outside a Git repository, and `--adopt` onboarding of an existing codebase with its refusals.
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
| `adopt_fresh` | Git repo with a committed eroded function and no config, run with `--adopt` | `0` | Stored `codeint.jsonc` has `regression_limits.per_file: true` and a zero `erosion` Budget under `budgets.production`, and no `test_gap` Budget; `.codeint/ratchet.json` and `.codeint/baseline.json` exist with the legacy `erosion` Finding baselined; standard output prints `Next steps` |
| `adopt_check_clean` | `codeint check` right after `adopt_fresh` | `0` | The baselined legacy Finding does not fail the check |
| `adopt_check_new` | `codeint check` after a new eroded file is added to the adopted repo while the legacy eroded file is tidied | `1` | Standard error says `the production erosion ratio ... is over its Budget 0` and names no Ratchet |
| `adopt_rollback` | Git repo with a regular file named `.codeint` and a committed eroded function, run with `--adopt` | `2` | Standard error says `cannot write Ratchet file` and has no `could not remove`; no `codeint.jsonc` remains and `.codeint` is still the regular file |
| `adopt_existing_config` | Repo where the config already exists, run with `--adopt` | `2` | Standard error states `already exists` and names `codeint ratchet update` and `codeint baseline update`; SHA-256 of the config is unchanged; no file appears under `.codeint/` |
| `adopt_existing_state` | Repo with `.codeint/baseline.json` and no config, run with `--adopt` | `2` | Standard error states `already exists` and names `codeint baseline update` and `codeint baseline update --reset`; SHA-256 of `.codeint/baseline.json` is unchanged; no config and no `.codeint/ratchet.json` appear |

## Drive and Assert

```sh
docs/verification/verify.sh drive "$RUN_DIR" init
docs/verification/verify.sh assert "$RUN_DIR" init
```

Assert checks that `codeint.json` is valid JSON after `fresh` and byte-identical before and after `existing`.
For `--adopt`, Assert checks the stored config with its zero Budgets, the Ratchet, and the Baseline after `adopt_fresh`, the Budget failure message of `adopt_check_new`, the rolled-back config after `adopt_rollback`, and the byte-identical state and absent new files after each refusal.

## Evidence and Cleanup

Evidence files are stored under `$EVIDENCE_DIR/init/` and `$EVIDENCE_DIR/verdict.json`.
Clean up the isolated run workspace while keeping evidence:

```sh
docs/verification/verify.sh cleanup "$RUN_DIR"
```
