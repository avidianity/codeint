# Feature map: `codeint plugin test`

Verifies the Conformance runner on valid Language plugins and Metric plugins, extra `--source` files, `text`, `json`, and `toon` formats, contract failure reporting with fix hints, invalid Wasm bytes, and unreadable file errors.
See [codeint plugin](../../reference.md#codeint-plugin) for the command specification.

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
| `language_json` | Sample Language plugin `sample_mini_plugin.wasm` with `--format json` | `0` | JSON reports `passed: true`, `kind: "language"`, `summary.failed == 0`, and a passing `end-to-end` check |
| `language_source_text` | Sample Language plugin with `--kind language --source <sample.mini> --format text` | `0` | Text output lists passing Conformance checks without failures |
| `metric_toon` | Sample Metric plugin `sample_long_functions_plugin.wasm` with `--kind metric --format toon` | `0` | TOON output includes `passed: true` and `kind: metric` |
| `contract_fail_json` | Non-conforming `out_of_range_ratio_metric_plugin.wasm` with `--kind metric --format json` | `1` | JSON reports `passed: false` and a failing `measurement-bounds` check with a non-empty `hint`; standard error names the failed check |
| `corrupt_wasm` | Plain non-Wasm bytes in a `.wasm` file with `--format json` | `1` | JSON reports `passed: false` and a failing `component` check with a fix `hint` |
| `missing_file` | Path to a Wasm file that does not exist | `2` | Standard error reports the unreadable file |

## Drive and Assert

```sh
docs/verification/verify.sh drive "$RUN_DIR" plugin-test
docs/verification/verify.sh assert "$RUN_DIR" plugin-test
```

Drive reuses built sample Wasm fixtures under `target/wasm-fixtures/wasm32-wasip2/debug` or builds only the three fixture crates offline when absent.
Assert checks every exit code, JSON and TOON check payloads, fix hints, and `FIXTURE_REPO` cleanliness.

## Evidence and Cleanup

Evidence files are stored under `$EVIDENCE_DIR/plugin-test/` and `$EVIDENCE_DIR/verdict.json`.
Clean up the isolated run workspace while keeping evidence:

```sh
docs/verification/verify.sh cleanup "$RUN_DIR"
```
