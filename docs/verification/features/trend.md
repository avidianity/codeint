# Feature map: `codeint trend`

Verifies multi-commit main-line history measurement, `--commits` commit-count windows, `--since` date filtering, report formats, result cache population, and operational errors.
See [codeint trend](../../reference.md#codeint-trend) for the command specification.

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
| `history_json` | Multi-commit branch with `--format json` | `0` | Valid JSON with `commits == 3` and three `production` rows oldest first; populates `CODEINT_CACHE_DIR` |
| `history_toon` | Multi-commit branch with `--format toon` | `0` | Valid TOON table of commits |
| `history_text` | Multi-commit branch with `--format text` | `0` | Monochrome aligned commit table starts with `codeint trend`, lists 3 dates oldest first, and has no ANSI escapes |
| `config_unsupported_format` | `codeint.json` sets `"format": "sarif"` | `2` | Standard error names `trend`, `sarif`, and supported formats (`toon`, `text`, `json`, `markdown`) |
| `since_json` | `--since 2026-11-05 --format json` | `0` | Valid JSON with `commits == 2` |
| `commits_json` | `--commits 2 --format json` | `0` | Valid JSON with `commits == 2` and two `production` rows oldest first |
| `history_md` | `--format markdown --output` | `0` | Writes non-empty `trend.md` |
| `bad_since` | Invalid calendar date `2026-02-30` | `2` | Standard error rejects the date |
| `bad_commits_zero` | `--commits 0` | `2` | Standard error rejects zero commit count |
| `commits_since_conflict` | `--since 2026-11-05 --commits 2` | `2` | Standard error rejects combining `--since` and `--commits` |
| `unborn_head` | Empty repository with unborn `HEAD` | `2` | Standard error reports no commits on the branch |

## Drive and Assert

```sh
docs/verification/verify.sh drive "$RUN_DIR" trend
docs/verification/verify.sh assert "$RUN_DIR" trend
```

Assert checks that `CODEINT_CACHE_DIR` holds at least one cache file and that `FIXTURE_REPO` has empty `git status --porcelain` and unchanged `HEAD`.

## Evidence and Cleanup

Evidence files are stored under `$EVIDENCE_DIR/trend/` and `$EVIDENCE_DIR/verdict.json`.
Clean up the isolated run workspace while keeping evidence:

```sh
docs/verification/verify.sh cleanup "$RUN_DIR"
```
