# Feature map: `codeint ratchet`

Verifies `.codeint/ratchet.json` creation, monotonic tightening, refusal to loosen without `--reset`, Ratchet enforcement in `codeint check` and `codeint diff`, `--package` skipping, Metric version mismatch handling, and corrupt file recovery.
See [codeint ratchet](../../reference.md#codeint-ratchet) and [Ratchet](../../reference.md#ratchet) for the command specification.

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
| `create_json` | Fresh repo copy with an eroded function, `codeint ratchet update --format json` | `0` | JSON reports `created: true` and `total_entries >= 5`; `.codeint/ratchet.json` stores `schema_version: 1`, `source_sets.production.erosion.ceiling > 0`, and positive `metric_version` |
| `check_pass` | `codeint check --format json` right after `create_json` | `0` | Whole-repository check passes with no `over_ratchet` failures |
| `check_fail` | Working tree adds an eroded file so Erosion rises above the Ratchet ceiling | `1` | JSON includes `over_ratchet` naming `erosion`; standard error names the Ratchet failure |
| `diff_fail` | `codeint diff main --format json` on the same worsened working tree | `1` | Diff JSON includes `over_ratchet` naming `erosion`; standard error names the Ratchet failure |
| `loosen_refused` | `codeint ratchet update` on the worsened tree without `--reset` | `2` | Standard error states `refusing to loosen Ratchet ceilings`; `.codeint/ratchet.json` SHA-256 is unchanged |
| `loosen_reset` | `codeint ratchet update --reset --format toon` on the worsened tree | `0` | TOON reports `reset: true`; `.codeint/ratchet.json` stores the higher ceiling |
| `tighten` | Eroded files simplified, then `codeint ratchet update --format json` | `0` | JSON reports `tightened >= 1`; `.codeint/ratchet.json` ceiling is strictly lower than before |
| `version_mismatch` | `.codeint/ratchet.json` edited with `metric_version: 999`, then `codeint check` | `2` | Standard error names `codeint ratchet update` |
| `corrupt_refused` | `.codeint/ratchet.json` holds invalid JSON, then `codeint ratchet update` | `2` | Standard error reports invalid Ratchet JSON |
| `corrupt_reset` | `codeint ratchet update --reset --format json` on the corrupt file | `0` | JSON reports `replaced_invalid: true`; `.codeint/ratchet.json` is valid JSON again |
| `package_skipped` | Workspace repo copy with `.codeint/ratchet.json`, `codeint check --package packages/a --format json` | `0` | JSON includes non-empty `ratchet_note` explaining why single-package runs skip the Ratchet |

## Drive and Assert

```sh
docs/verification/verify.sh drive "$RUN_DIR" ratchet
docs/verification/verify.sh assert "$RUN_DIR" ratchet
```

Assert checks the stored `.codeint/ratchet.json` contents after create, refusal, reset, tighten, and corrupt-file recovery, alongside `FIXTURE_REPO` cleanliness.

## Evidence and Cleanup

Evidence files are stored under `$EVIDENCE_DIR/ratchet/` and `$EVIDENCE_DIR/verdict.json`.
Clean up the isolated run workspace while keeping evidence:

```sh
docs/verification/verify.sh cleanup "$RUN_DIR"
```
