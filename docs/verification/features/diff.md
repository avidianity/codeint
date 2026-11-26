# Feature map: `codeint diff`

Verifies comparison against a base Git ref, `--head`, Regression detection, per-file Regression limits (`regression_limits.per_file`), new-file Budgets (`new_file_budgets`), result cache reuse, and operational errors.
See [codeint diff](../../reference.md#codeint-diff), [Regression limits](../../reference.md#regression-limits), and [New-file Budgets](../../reference.md#new-file-budgets) for the command specification.

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
| `per_file_regression` | `regression_limits.per_file: true` while `src/parse.ts` improves and `src/sign.ts` adds an eroded function | `1` | Full JSON report lists a per-file Regression for `src/sign.ts` in `erosion`; standard error names `file src/sign.ts regressed` |
| `per_file_clean` | `regression_limits.per_file: true` comparing `main --head feature` where no modified file regresses | `0` | Valid JSON comparison with no `regressions` |
| `new_file_budget_fail` | `new_file_budgets.production.erosion: 0` with a new eroded file `src/added.ts` | `1` | JSON report lists `over_new_file_budget` for `production` `erosion`; SARIF includes `newFileBudgetFailures`; standard error names the new-file Budget failure |
| `new_file_budget_clean` | `new_file_budgets.production.erosion: 0` modifying only an existing file with no new files added | `0` | Valid JSON comparison with no `over_new_file_budget` |
| `new_file_budget_invalid` | `new_file_budgets.test.orphan_files: 0` (allowed under `production` only) | `2` | Standard error rejects `orphan_files` under `new_file_budgets.test` |
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
