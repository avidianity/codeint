# Feature map: `codeint impact`

Verifies reverse-dependency blast radius across direct and transitive dependents, Production vs Test separation, `--depth`, `--changed`, `--output`, output formats, empty dependent sets, and operational errors.
See [codeint impact](../../reference.md#codeint-impact) for the command specification.

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
| `direct_and_transitive_json` | Import graph repo, `codeint impact src/a.ts --format json` | `0` | JSON reports `summary.direct_dependents == 2`, `summary.transitive_dependents == 1`, `src/b.ts` in `production.direct`, `src/c.ts` in `production.transitive`, and `src/a.test.ts` in `test.direct` |
| `depth_one_text` | `codeint impact src/a.ts --depth 1 --format text` | `0` | Text output lists `src/b.ts` and omits transitive dependent `src/c.ts` |
| `changed_toon` | Branch changing `src/a.ts`, `codeint impact --changed main --format toon` | `0` | TOON output includes `src/a.ts` in `targets` and its dependents |
| `no_dependents_output` | Isolated file with `--format json --output impact.json` | `0` | Standard output is empty; `impact.json` reports `summary.total_dependents == 0` |
| `missing_package` | `--package packages/missing` | `2` | Standard error reports the unknown Workspace package |
| `missing_file` | Target path `src/no-such-file.ts` | `2` | Standard error reports the missing or unmeasured file |
| `non_repo` | Folder outside a Git repository | `2` | Standard error reports the folder is not inside a Git repository |

## Drive and Assert

```sh
docs/verification/verify.sh drive "$RUN_DIR" impact
docs/verification/verify.sh assert "$RUN_DIR" impact
```

Assert checks direct and transitive dependent sets, `--depth` truncation, `--output` file creation with empty stdout, error exits, and `FIXTURE_REPO` cleanliness.

## Evidence and Cleanup

Evidence files are stored under `$EVIDENCE_DIR/impact/` and `$EVIDENCE_DIR/verdict.json`.
Clean up the isolated run workspace while keeping evidence:

```sh
docs/verification/verify.sh cleanup "$RUN_DIR"
```
