# Feature map: `codeint check`

Verifies working-tree measurement, report formats, `--output`, `--all`, Budget failures, and operational errors.
See [codeint check](../../reference.md#codeint-check) for the command specification.

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
| `golden_toon` | Clean fixture repo, default format | `0` | TOON report lists `source_sets`, `skipped` (`src/broken.ts`), and `declaration_only` (`src/types.d.ts`) |
| `golden_json` | `--format json` | `0` | Valid JSON with `files == 3`, `source_sets.production`, one skipped file, and `src/types.d.ts` in `declaration_only` |
| `golden_sarif` | `--format sarif` | `0` | Valid SARIF `2.1.0` with driver name `codeint` |
| `golden_md_output` | `--format markdown --output` | `0` | Standard output is empty; `report.md` contains `codeint report` and the Metric table |
| `golden_all` | `--all --format json` | `0` | Valid JSON report listing every Finding |
| `over_budget` | Verbosity Budget set to `0` | `1` | Full JSON report includes `over_budget`; standard error names the Budget failure |
| `invalid_manifest` | Empty `fixtures/go.mod` added to the fixture repo copy | `0` | JSON report has one `manifest_warnings` entry naming `fixtures/go.mod` and `missing module directive`; the rest is measured |
| `invalid_config` | Unknown key `budget` in `codeint.json` | `2` | Standard error names the unknown key |
| `missing_path` | Path `missing` does not exist | `2` | Standard error reports the missing path |
| `non_repo` | Folder outside a Git repository | `2` | Standard error reports the folder is not inside a Git repository |

## Drive and Assert

```sh
docs/verification/verify.sh drive "$RUN_DIR" check
docs/verification/verify.sh assert "$RUN_DIR" check
```

Assert also checks that `FIXTURE_REPO` has empty `git status --porcelain` and unchanged `HEAD`.

## Evidence and Cleanup

Evidence files are stored under `$EVIDENCE_DIR/check/` and `$EVIDENCE_DIR/verdict.json`.
Clean up the isolated run workspace while keeping evidence:

```sh
docs/verification/verify.sh cleanup "$RUN_DIR"
```
