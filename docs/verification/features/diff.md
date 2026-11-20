# Feature map: `codeint diff`

Verifies comparison against a base Git ref, `--head`, Regression detection, result cache reuse, and operational errors.
See [codeint diff](../../reference.md#codeint-diff) for the command specification.

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
| `no_regression_1` | `main --head feature --format json` | `0` | Valid JSON comparison with no `regressions`; populates `CODEINT_CACHE_DIR` |
| `no_regression_2` | Repeat `main --head feature --format json` | `0` | Identical JSON output served with cache populated |
| `no_regression_md` | `--format markdown --output` | `0` | Writes non-empty `diff.md` |
| `regression_json` | Working tree adds eroded `src/route.ts` | `1` | Full JSON report lists `regressions`; standard error names the Regression |
| `regression_sarif` | Regression with `--format sarif --output` | `1` | Writes SARIF `2.1.0` `diff.sarif` containing the new Finding |
| `bad_ref` | Unknown base ref `no-such-branch` | `2` | Standard error reports the unknown ref |
| `unborn_head` | Empty repository with unborn `HEAD` | `2` | Standard error reports the missing ref |

## Drive and Assert

```sh
docs/verification/verify.sh drive "$RUN_DIR" diff
docs/verification/verify.sh assert "$RUN_DIR" diff
```

Assert checks that `CODEINT_CACHE_DIR` holds at least one cache file and that `FIXTURE_REPO` has empty `git status --porcelain` and unchanged `HEAD`.

## Evidence and Cleanup

Evidence files are stored under `$EVIDENCE_DIR/diff/` and `$EVIDENCE_DIR/verdict.json`.
Clean up the isolated run workspace while keeping evidence:

```sh
docs/verification/verify.sh cleanup "$RUN_DIR"
```
