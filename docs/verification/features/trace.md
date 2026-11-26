# Feature map: `codeint trace`

Verifies shortest runtime import path discovery, `--all` with `--max-paths`, disconnected modules, `toon`, `json`, and `text` formats, `--output`, and operational errors.
See [codeint trace](../../reference.md#codeint-trace) for the command specification.

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
| `connected_json` | Diamond import graph, `codeint trace src/entry.ts src/leaf.ts --format json` | `0` | JSON reports `connected: true`, `shortest_distance: 2`, and one shortest path of length 3 |
| `all_paths_json` | Same pair with `--all --max-paths 5 --format json` | `0` | JSON reports `connected: true`, two distinct shortest paths, and `truncated: false` |
| `disconnected_toon` | Unconnected pair with `--format toon` | `0` | TOON output reports `connected: false` and `paths: []` |
| `text_output` | Connected pair with `--format text --output trace.txt` | `0` | Standard output is empty; `trace.txt` names `src/entry.ts` and `src/leaf.ts` |
| `missing_package` | `--package packages/missing` | `2` | Standard error reports the unknown Workspace package |
| `unknown_file` | Missing starting file `src/missing.ts` | `2` | Standard error reports the missing or unmeasured file |
| `non_repo` | Folder outside a Git repository | `2` | Standard error reports the folder is not inside a Git repository |

## Drive and Assert

```sh
docs/verification/verify.sh drive "$RUN_DIR" trace
docs/verification/verify.sh assert "$RUN_DIR" trace
```

Assert checks connected, multi-path, and disconnected payloads, `--output` file creation with empty stdout, error exits, and `FIXTURE_REPO` cleanliness.

## Evidence and Cleanup

Evidence files are stored under `$EVIDENCE_DIR/trace/` and `$EVIDENCE_DIR/verdict.json`.
Clean up the isolated run workspace while keeping evidence:

```sh
docs/verification/verify.sh cleanup "$RUN_DIR"
```
