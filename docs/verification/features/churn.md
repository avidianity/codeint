# Feature map: `codeint churn`

Verifies per-file commit touch counting over main-line Git history, ranking of eroded files, `--commits`, `--since`, `--all`, `--output`, report formats, and operational errors.
See [codeint churn](../../reference.md#codeint-churn) for the command specification.

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
| `commits_json` | Multi-commit repo copy with repeated touches to eroded `src/hot.ts`, `--commits 10 --format json` | `0` | JSON reports `commits_analyzed >= 2` and ranks `src/hot.ts` under `source_sets.production.erosion.findings` with `commits >= 2` |
| `since_toon` | `--since 2026-11-05 --format toon` | `0` | TOON output includes `commits_analyzed` and `source_sets` |
| `text_all` | `--commits 10 --all --format text` | `0` | Text output lists `src/hot.ts` |
| `markdown_output` | `--commits 10 --format markdown --output churn.md` | `0` | Standard output is empty; `churn.md` is non-empty |
| `bad_since` | Invalid calendar date `--since 2026-02-30` | `2` | Standard error rejects the date |
| `unborn_head` | Empty repository with unborn `HEAD` | `2` | Standard error reports no commits on the branch |

## Drive and Assert

```sh
docs/verification/verify.sh drive "$RUN_DIR" churn
docs/verification/verify.sh assert "$RUN_DIR" churn
```

Assert checks commit touch counts, eroded file rankings, `--output` file creation with empty stdout, error exits, and `FIXTURE_REPO` cleanliness.

## Evidence and Cleanup

Evidence files are stored under `$EVIDENCE_DIR/churn/` and `$EVIDENCE_DIR/verdict.json`.
Clean up the isolated run workspace while keeping evidence:

```sh
docs/verification/verify.sh cleanup "$RUN_DIR"
```
