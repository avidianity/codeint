# Feature map: `codeint baseline`

Verifies `.codeint/baseline.json` creation, Finding hiding and non-baselined Budget evaluation in `codeint check`, `--ignore-baseline`, pruning fixed Findings without adding new ones, `--reset`, Metric version mismatch handling, and corrupt file recovery.
See [codeint baseline](../../reference.md#codeint-baseline) for the command specification.

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
| `create_json` | Fresh repo copy with an eroded function, `codeint baseline update --format json` | `0` | JSON reports `created: true` and `added >= 1`; `.codeint/baseline.json` stores `schema_version: 2` and the eroded Finding anchor under `source_sets.production.erosion.findings` |
| `check_hides` | `codeint check --format json` with `budgets.production.erosion: 0` | `0` | JSON reports `baseline.path == ".codeint/baseline.json"`, `baseline.hidden_findings >= 1`, `baselined_findings >= 1` on `erosion`, and no `over_budget` |
| `check_ignore_baseline` | `codeint check --ignore-baseline --format json` on the same repo | `1` | JSON omits `baseline` hiding and lists `over_budget` for `erosion`; standard error names the Budget failure |
| `prune_and_skip_new` | Legacy eroded function fixed and a new eroded file added, `codeint baseline update --format json` | `0` | JSON reports `removed >= 1` and `skipped_new >= 1`; `.codeint/baseline.json` drops the fixed Finding and does not add the new Finding |
| `reset_adds_new` | `codeint baseline update --reset --format toon` followed by `codeint check --format json` | `0` | `.codeint/baseline.json` records the new Finding; subsequent `codeint check` passes |
| `version_mismatch` | `.codeint/baseline.json` edited with `metric_version: 999`, then `codeint check` | `2` | Standard error names `codeint baseline update` |
| `corrupt_refused` | `.codeint/baseline.json` holds invalid JSON, then `codeint baseline update` | `2` | Standard error reports invalid Baseline JSON |
| `corrupt_reset` | `codeint baseline update --reset --format json` on the corrupt file | `0` | JSON reports `replaced_invalid: true`; `.codeint/baseline.json` is valid JSON again |

## Drive and Assert

```sh
docs/verification/verify.sh drive "$RUN_DIR" baseline
docs/verification/verify.sh assert "$RUN_DIR" baseline
```

Assert inspects the stored `.codeint/baseline.json` after create, prune, reset, and corrupt-file recovery, verifies `check` vs `check --ignore-baseline`, and confirms `FIXTURE_REPO` stays clean.

## Evidence and Cleanup

Evidence files are stored under `$EVIDENCE_DIR/baseline/` and `$EVIDENCE_DIR/verdict.json`.
Clean up the isolated run workspace while keeping evidence:

```sh
docs/verification/verify.sh cleanup "$RUN_DIR"
```
