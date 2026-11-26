# Feature map: `codeint coupling`

Verifies co-change coupling detection over main-line Git history, `--min-co-changes`, `--min-ratio`, `--max-commit-files` bulk commit exclusion, `--include-tests`, direct import flagging, `--output`, report formats, and operational errors.
See [codeint coupling](../../reference.md#codeint-coupling) for the command specification.

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
| `pairs_json` | Multi-commit repo with `src/alpha.ts` and `src/beta.ts` co-changing 3+ times without importing each other | `0` | JSON lists the `(src/alpha.ts, src/beta.ts)` pair with `co_change_count >= 3`, `has_direct_import: false`, and `source_sets == ["production"]` |
| `bulk_excluded_json` | Same repo with `--max-commit-files 3` | `0` | JSON reports `commits_excluded_by_bulk_limit >= 1` |
| `include_tests_toon` | Same repo with `--include-tests --format toon` | `0` | TOON output includes `test` in `source_sets` and lists `src/alpha.test.ts` |
| `markdown_output` | `--format markdown --output coupling.md` | `0` | Standard output is empty; `coupling.md` is non-empty |
| `text_format` | `--format text` | `0` | Text output lists `src/alpha.ts` and `src/beta.ts` |
| `invalid_ratio` | `--min-ratio 1.5` outside `0..=1` | `2` | Standard error rejects the ratio |
| `unborn_head` | Empty repository with unborn `HEAD` | `2` | Standard error reports no commits on the branch |

## Drive and Assert

```sh
docs/verification/verify.sh drive "$RUN_DIR" coupling
docs/verification/verify.sh assert "$RUN_DIR" coupling
```

Assert checks co-change pairs, bulk commit exclusion, `--include-tests`, `--output` file creation with empty stdout, error exits, and `FIXTURE_REPO` cleanliness.

## Evidence and Cleanup

Evidence files are stored under `$EVIDENCE_DIR/coupling/` and `$EVIDENCE_DIR/verdict.json`.
Clean up the isolated run workspace while keeping evidence:

```sh
docs/verification/verify.sh cleanup "$RUN_DIR"
```
